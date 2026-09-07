//! A mode is **data**, not code.
//!
//! Swapping a mechanic at runtime means rebuilding [`Rules`] from an edited
//! [`ModeSpec`] — never mutating a trait object in place. Three things fall out
//! of that for free: a mode is printable and shareable as text, a determinism
//! test is `ModeSpec + intent log -> digest`, and the runtime panel is an
//! editor over data rather than a wiring diagram.

use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;

use crate::sim::board::Board;
use crate::sim::mutation::{NoMutation, TelegraphedRewire};
use crate::sim::objective::{PlantFlags, PlantRule, PlantWin, ReachExit};
use crate::sim::prng::Prng;
use crate::sim::resolution::{Sequential, Simultaneous};
use crate::sim::rival::RivalPawns;
use crate::sim::rules::{Mutation, Objective, Resolution, Setback, Threat, Vision};
use crate::sim::setback::{Prison, RespawnAtStart};
use crate::sim::state::{Flag, Guardian, MatchState, Pawn, PawnId, TeamId, TurnReport};
use crate::sim::threat::{ConeInteraction, GuardianTarget, Guardians, NoThreat};
use crate::sim::vision::{Cone, Radius};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResolutionKind {
    #[default]
    Simultaneous,
    Sequential,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisionKind {
    Cone {
        width: u8,
        range: u16,
        lock_own_hex: bool,
    },
    Radius {
        range: u16,
    },
}

impl Default for VisionKind {
    fn default() -> Self {
        Self::Cone {
            width: 1,
            range: 1,
            lock_own_hex: true,
        }
    }
}

/// How much of the coming change a player may see.
///
/// **Presentation only.** The simulation stores the full pending change either
/// way, and a test digests the same match under all three settings to prove
/// they agree — for the same reason `tactics_lab` pins its whole-map view: a
/// display setting that quietly changed the rules would be measuring a
/// different game than the one being judged.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MutationPreview {
    /// Nothing. The control: is the telegraph doing any work at all?
    Hidden,
    /// Which boundaries will change, but not into what.
    Location,
    /// The exact geometry the turn will produce, drawn as ghost walls.
    #[default]
    Outcome,
}

/// Whether teammates may share a cell.
///
/// A seam rather than a decision, because the two play differently enough to be
/// worth measuring: forbidding it makes corridors genuinely scarce and turns a
/// narrow doorway into a real bottleneck for your own squad, while allowing it
/// makes a squad able to move as one body and makes the overwatch rule much
/// easier to satisfy. Rivals are never affected — cross-team contact is what
/// recency adjudicates, and that has always been allowed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stacking {
    /// One pawn per cell. Teammates block each other.
    #[default]
    Forbidden,
    /// Teammates may pile up.
    Allowed,
}

/// When the cone is sampled.
///
/// This is not a branch inside a strategy — it relocates the `vision.locks`
/// call within the pipeline. That single move is the whole difference between
/// "locks are a blind commitment about where I will be looking" and "locks are
/// what I set up last turn", which is why it earns a setting rather than a
/// guess.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConeTiming {
    /// Sampled after movement, from where pawns ended up.
    #[default]
    PostMove,
    /// Sampled before movement, from where pawns began the turn.
    PreMove,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThreatKind {
    #[default]
    Guardians,
    /// Stink base: rival pawns take each other by recency.
    RivalPawns,
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SetbackKind {
    #[default]
    Prison,
    RespawnAtStart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationKind {
    TelegraphedRewire { base: u16, cap: u16 },
    None,
}

impl Default for MutationKind {
    fn default() -> Self {
        Self::TelegraphedRewire { base: 2, cap: 6 }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveKind {
    PlantFlags { rule: PlantRule, win: PlantWin },
    ReachExit,
}

impl Default for ObjectiveKind {
    fn default() -> Self {
        Self::PlantFlags {
            rule: PlantRule::Overwatch,
            win: PlantWin::All,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoardSpec {
    pub radius: u16,
    /// One per team; `prisons[t]` is team `t`'s and holds the other team's
    /// pawns. A single-team board lists one, which is the neutral centre cell.
    pub prisons: Vec<HexCoord>,
    pub flags: Vec<HexCoord>,
    pub spawns: Vec<HexCoord>,
    pub guardian_posts: Vec<HexCoord>,
}

impl BoardSpec {
    /// The mode-1 board: a radius-3 hexagon, 37 cells.
    ///
    /// The prison sits at the centre — visible, central and contested — and the
    /// three flags occupy alternating corners of the ring so no two are close
    /// together. The squad starts on the remaining corner, which leaves one
    /// flag six steps away and two flags three.
    #[must_use]
    pub fn radius_three() -> Self {
        Self {
            radius: 3,
            prisons: vec![HexCoord {
                q: 3,
                r: 3,
                level: 0,
            }],
            flags: vec![
                HexCoord {
                    q: 6,
                    r: 3,
                    level: 0,
                },
                HexCoord {
                    q: 3,
                    r: 0,
                    level: 0,
                },
                HexCoord {
                    q: 0,
                    r: 6,
                    level: 0,
                },
            ],
            spawns: vec![HexCoord {
                q: 0,
                r: 3,
                level: 0,
            }],
            guardian_posts: vec![
                HexCoord {
                    q: 4,
                    r: 1,
                    level: 0,
                },
                HexCoord {
                    q: 2,
                    r: 5,
                    level: 0,
                },
                HexCoord {
                    q: 5,
                    r: 4,
                    level: 0,
                },
                HexCoord {
                    q: 1,
                    r: 2,
                    level: 0,
                },
            ],
        }
    }

    /// The mode-2 board: the same radius-3 hexagon, laid out for two teams.
    ///
    /// Everything is symmetric under the 180-degree rotation `(q, r) ->
    /// (6-q, 6-r)`, which swaps the two bases — so neither side has a shorter
    /// route to anything. Each team's near flag is two steps from its base and
    /// four from the rival's; the third sits dead centre, three from both. With
    /// `PlantWin::Majority` that middle flag decides the match, which is where
    /// the two squads are forced to meet.
    #[must_use]
    pub fn radius_three_contested() -> Self {
        Self {
            radius: 3,
            prisons: vec![
                HexCoord {
                    q: 1,
                    r: 2,
                    level: 0,
                },
                HexCoord {
                    q: 5,
                    r: 4,
                    level: 0,
                },
            ],
            flags: vec![
                HexCoord {
                    q: 2,
                    r: 1,
                    level: 0,
                },
                HexCoord {
                    q: 3,
                    r: 3,
                    level: 0,
                },
                HexCoord {
                    q: 4,
                    r: 5,
                    level: 0,
                },
            ],
            spawns: vec![
                HexCoord {
                    q: 0,
                    r: 3,
                    level: 0,
                },
                HexCoord {
                    q: 6,
                    r: 3,
                    level: 0,
                },
            ],
            guardian_posts: vec![
                HexCoord {
                    q: 3,
                    r: 1,
                    level: 0,
                },
                HexCoord {
                    q: 3,
                    r: 5,
                    level: 0,
                },
                HexCoord {
                    q: 5,
                    r: 1,
                    level: 0,
                },
                HexCoord {
                    q: 1,
                    r: 5,
                    level: 0,
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModeSpec {
    pub name: String,
    pub seed: u64,
    pub board: BoardSpec,
    /// Supported from commit one so a rival team drops in without rework.
    /// Mode 1 ships with one.
    pub teams: u8,
    pub pawns_per_team: u8,
    pub turn_limit: u16,
    /// Interior boundaries walled off at deal time. The board is never dealt
    /// disconnected, so a wall that would strand part of it is simply not put
    /// up and the real count may come in under this.
    pub walls: u16,
    /// Presentation only — how much of the telegraph a player is shown.
    pub preview: MutationPreview,
    pub resolution: ResolutionKind,
    pub stacking: Stacking,
    pub vision: VisionKind,
    pub cone_timing: ConeTiming,
    /// Threats run in listed order each turn. Mode 2 lists both.
    pub threats: Vec<ThreatKind>,
    pub guardian_count: u8,
    pub guardian_target: GuardianTarget,
    pub cone_interaction: ConeInteraction,
    /// Turns between guardian moves; see [`Guardians::cadence`].
    pub guardian_cadence: u16,
    pub setback: SetbackKind,
    pub mutation: MutationKind,
    pub objective: ObjectiveKind,
}

impl ModeSpec {
    /// **Plant** — the first mode: a radius-3 hexagon, simultaneous resolution,
    /// cone vision, guardians with a jailbreakable prison, and three flags.
    #[must_use]
    pub fn plant() -> Self {
        Self {
            name: "Plant".to_string(),
            seed: 0x0B5E_2ED0_0000_0001,
            board: BoardSpec::radius_three(),
            teams: 1,
            pawns_per_team: 3,
            turn_limit: 16,
            walls: 22,
            preview: MutationPreview::Outcome,
            resolution: ResolutionKind::Simultaneous,
            stacking: Stacking::Forbidden,
            vision: VisionKind::default(),
            cone_timing: ConeTiming::PostMove,
            threats: vec![ThreatKind::Guardians],
            guardian_count: 2,
            guardian_target: GuardianTarget::NearestPawn,
            cone_interaction: ConeInteraction::Ignores,
            guardian_cadence: 2,
            setback: SetbackKind::Prison,
            mutation: MutationKind::default(),
            objective: ObjectiveKind::default(),
        }
    }

    /// **Base** — mode 2: two squads, stink base recency, and neutral
    /// guardians hunting both.
    ///
    /// Threats run in the listed order: rival pawns settle their contact first,
    /// then the guardians sweep up whoever is left standing. Running guardians
    /// first would let the facility take a pawn that a rival had already
    /// claimed, and the tag would silently never happen.
    #[must_use]
    pub fn base() -> Self {
        Self {
            name: "Base".to_string(),
            board: BoardSpec::radius_three_contested(),
            teams: 2,
            pawns_per_team: 3,
            turn_limit: 24,
            threats: vec![ThreatKind::RivalPawns, ThreatKind::Guardians],
            guardian_count: 2,
            guardian_target: GuardianTarget::NearestPawn,
            objective: ObjectiveKind::PlantFlags {
                rule: PlantRule::Overwatch,
                win: PlantWin::Majority,
            },
            ..Self::plant()
        }
    }

    /// The modes the runtime picker cycles.
    ///
    /// This list *is* the experiment. Each entry is one complete rule set with
    /// a name, not a knob position — you compare modes by playing them, which
    /// is the discipline `tactics_lab` lost when it grew forty sliders.
    #[must_use]
    pub fn presets() -> Vec<Self> {
        vec![
            Self::plant(),
            Self::base(),
            Self {
                name: "Base: no guardians".to_string(),
                threats: vec![ThreatKind::RivalPawns],
                guardian_count: 0,
                ..Self::base()
            },
            Self {
                name: "Base: stand-only plant".to_string(),
                objective: ObjectiveKind::PlantFlags {
                    rule: PlantRule::StandOnly,
                    win: PlantWin::Majority,
                },
                ..Self::base()
            },
            Self {
                name: "Base: cone blocks guardians".to_string(),
                cone_interaction: ConeInteraction::Blocked,
                ..Self::base()
            },
            Self {
                name: "Base: sequential turns".to_string(),
                resolution: ResolutionKind::Sequential,
                ..Self::base()
            },
            Self {
                name: "Base: squad may stack".to_string(),
                stacking: Stacking::Allowed,
                ..Self::base()
            },
            Self {
                name: "Plant: squad may stack".to_string(),
                stacking: Stacking::Allowed,
                ..Self::plant()
            },
            Self {
                name: "Plant: solo, no rivals".to_string(),
                objective: ObjectiveKind::PlantFlags {
                    rule: PlantRule::StandOnly,
                    win: PlantWin::Majority,
                },
                turn_limit: 24,
                ..Self::plant()
            },
        ]
    }
}

/// The strategies a [`ModeSpec`] selects, built once per match.
pub struct Rules {
    pub resolution: Box<dyn Resolution>,
    pub vision: Box<dyn Vision>,
    pub threats: Vec<Box<dyn Threat>>,
    pub setback: Box<dyn Setback>,
    pub mutation: Box<dyn Mutation>,
    pub objective: Box<dyn Objective>,
    pub cone_timing: ConeTiming,
}

impl Rules {
    #[must_use]
    pub fn from_spec(spec: &ModeSpec) -> Self {
        let resolution: Box<dyn Resolution> = match spec.resolution {
            ResolutionKind::Simultaneous => Box::new(Simultaneous {
                stacking: spec.stacking,
            }),
            ResolutionKind::Sequential => Box::new(Sequential {
                stacking: spec.stacking,
            }),
        };
        let vision: Box<dyn Vision> = match spec.vision {
            VisionKind::Cone {
                width,
                range,
                lock_own_hex,
            } => Box::new(Cone {
                width,
                range,
                lock_own_hex,
            }),
            VisionKind::Radius { range } => Box::new(Radius { range }),
        };
        let threats: Vec<Box<dyn Threat>> = spec
            .threats
            .iter()
            .map(|kind| -> Box<dyn Threat> {
                match kind {
                    ThreatKind::Guardians => Box::new(Guardians {
                        target: spec.guardian_target,
                        cone: spec.cone_interaction,
                        cadence: spec.guardian_cadence,
                    }),
                    ThreatKind::RivalPawns => Box::new(RivalPawns),
                    ThreatKind::None => Box::new(NoThreat),
                }
            })
            .collect();
        let setback: Box<dyn Setback> = match spec.setback {
            SetbackKind::Prison => Box::new(Prison),
            SetbackKind::RespawnAtStart => Box::new(RespawnAtStart),
        };
        let mutation: Box<dyn Mutation> = match spec.mutation {
            MutationKind::TelegraphedRewire { base, cap } => {
                Box::new(TelegraphedRewire { base, cap })
            }
            MutationKind::None => Box::new(NoMutation),
        };
        let objective: Box<dyn Objective> = match spec.objective {
            ObjectiveKind::PlantFlags { rule, win } => Box::new(PlantFlags {
                rule,
                win,
                turn_limit: spec.turn_limit,
            }),
            ObjectiveKind::ReachExit => Box::new(ReachExit {
                turn_limit: spec.turn_limit,
            }),
        };
        Self {
            resolution,
            vision,
            threats,
            setback,
            mutation,
            objective,
            cone_timing: spec.cone_timing,
        }
    }

    fn threat_names(&self) -> String {
        if self.threats.is_empty() {
            return "None".to_string();
        }
        self.threats
            .iter()
            .map(|threat| threat.name())
            .collect::<Vec<_>>()
            .join("+")
    }

    /// The one-line summary the HUD shows, so what is running is never a guess.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} / {} / {} / {} / {} / {}",
            self.resolution.name(),
            self.vision.name(),
            self.threat_names(),
            self.setback.name(),
            self.mutation.name(),
            self.objective.name(),
        )
    }
}

/// Lay a match out from its spec, including the first telegraph. Pawns spread from their team spawn outwards
/// in face order; guardians take posts from the board spec, cycling if the
/// count exceeds the posts.
#[must_use]
pub fn deal(spec: &ModeSpec) -> MatchState {
    let mut state = deal_board(spec);
    // The opening telegraph, so turn one is a decision rather than a surprise.
    Rules::from_spec(spec).mutation.telegraph(&mut state);
    state
}

fn deal_board(spec: &ModeSpec) -> MatchState {
    let mut rng = Prng::new(spec.seed);
    let board = Board::hexagon_with_walls(spec.board.radius, spec.walls as usize, &mut rng);
    let mut pawns = Vec::new();
    let mut next_id = 0_u8;
    for team in 0..spec.teams {
        let spawn = spec.board.spawns[team as usize % spec.board.spawns.len()];
        let mut placed = vec![spawn];
        let mut cursor = 0;
        while placed.len() < spec.pawns_per_team as usize {
            let Some(&from) = placed.get(cursor) else {
                break;
            };
            for (_, next) in board.open_neighbours(from) {
                if placed.len() >= spec.pawns_per_team as usize {
                    break;
                }
                if !placed.contains(&next) {
                    placed.push(next);
                }
            }
            cursor += 1;
        }
        for at in placed {
            pawns.push(Pawn {
                id: PawnId(next_id),
                team: TeamId(team),
                at,
                prev_at: at,
                facing: HexFace::East,
                jailed: false,
                left_base_at: 0,
                immune: false,
            });
            next_id += 1;
        }
    }

    let guardians = (0..spec.guardian_count)
        .map(|i| Guardian {
            at: spec.board.guardian_posts[i as usize % spec.board.guardian_posts.len()],
            prev_at: spec.board.guardian_posts[i as usize % spec.board.guardian_posts.len()],
            stalled: false,
        })
        .collect();

    let flags = spec
        .board
        .flags
        .iter()
        .map(|&at| Flag {
            at,
            planted_by: None,
        })
        .collect();

    MatchState {
        board,
        pawns,
        guardians,
        flags,
        prisons: spec.board.prisons.clone(),
        spawns: spec.board.spawns.clone(),
        turn: 0,
        telegraph: Vec::new(),
        outcome: None,
        report: TurnReport::default(),
        rng,
    }
}
