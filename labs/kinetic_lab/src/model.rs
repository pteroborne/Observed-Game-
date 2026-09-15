//! Pure fixed-tick model for the Observer's kinetic tool.
//!
//! The lab's one technical question: **is a shove that commits a minor Guardian
//! to void deterministic, readable, and fair?**
//!
//! Three canon rules shape everything here (see `docs/architect_ascent_design.md`):
//!
//! 1. The tool deals no damage. A shove only *moves* things; what kills is
//!    always the architecture the target lands in.
//! 2. Minor Guardians are not frozen by observation; major Guardians are. This
//!    module keeps one of each so the asymmetry is testable rather than
//!    asserted.
//! 3. Shove resolution is fixed-tick simulation, never authored physics. Travel
//!    is a discrete walk along hex faces, so an identical snapshot and intent
//!    reproduce an identical impulse, destination, and destroyed actor.
//!
//! Charge is finite and restored only at a station on a powered floor, which
//! makes the generator -> station -> tool dependency observable in one lab.

// `Resource` is the only Bevy item this module touches: it lets the app hold the
// world without a wrapper type. No camera, sprite, asset, or system appears
// here, and the model is fully exercisable without an `App`.
use bevy::prelude::Resource;
use observed_core::{PlayerId, SplitMix};
use observed_hex::{
    coords::{HexCoord, HexGridSize, lateral_distance},
    faces::HexFace,
};

/// Simulation rate. Every duration in this module is a fixed-tick count so that
/// a replay of the same intents reproduces the same match.
pub const TICKS_PER_SECOND: u32 = 60;

/// Charge capacity of one Observer's tool.
pub const MAX_CHARGE: u8 = 6;
/// A push costs more than a pull: committing something to void is the strong
/// verb, and dragging it one cell closer is the cheap setup.
pub const PUSH_COST: u8 = 2;
pub const PULL_COST: u8 = 1;
/// Ticks a powered station needs to restore a single charge.
pub const RECHARGE_TICKS: u32 = 30;
/// Cells a push drives its target along the Observer's facing.
pub const PUSH_IMPULSE: u32 = 3;
/// Cells a pull drags its target back toward the Observer.
pub const PULL_IMPULSE: u32 = 1;
/// How far down its facing lane the tool finds a target.
pub const TOOL_RANGE: u32 = 3;
/// Ticks a minor Guardian spends recovering after surviving a shove.
pub const STAGGER_TICKS: u32 = 45;
/// Ticks between minor Guardian steps.
///
/// Scale matters here and the first value did not respect it. A cell is 14
/// metres across, so a 24-tick step is 35 m/s — against an Observer who walks
/// at 4.6 and sprints at 7.0. On the schematic board that read as "brisk"; in
/// first person it meant an adjacent Guardian jailed you before you finished
/// turning, and no chase existed at all. At 150 ticks a minor makes about
/// 5.6 m/s: it gains on a walking Observer and loses to a sprinting one, which
/// is the pressure the horde is supposed to apply.
pub const MINOR_STEP_TICKS: u32 = 150;
/// Ticks between major Guardian steps. Majors are slower and far more dangerous:
/// roughly 3.5 m/s, so one can be outrun but not ignored.
pub const MAJOR_STEP_TICKS: u32 = 240;
/// How far an Observer sees down their facing lane. Observation freezes a major
/// Guardian and does nothing whatsoever to a minor one.
pub const OBSERVATION_RANGE: u32 = 4;
/// Upper bound on cells one shove may traverse. Ledges do not consume impulse,
/// so a closed ring of ledge cells would otherwise loop forever. No authored
/// board approaches this; it exists so the invariant is enforced, not assumed.
const MAX_TRAVEL_CELLS: u32 = 64;
/// Every lateral face open — the authored board's "no walls anywhere".
const ALL_LATERAL_DOORS: u8 = 0b0011_1111;

/// Stable identity for a minor Guardian. Bevy entities reference this; they
/// never replace it.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MinorGuardianId(pub u32);

/// Stable identity for a recharge station.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StationId(pub u32);

/// What one cell of the lab floor is made of.
///
/// This is the whole "architecture kills, the tool does not" rule expressed as
/// data: every lethal outcome below is a property of the destination cell.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CellKind {
    /// Ordinary floor. Stops a shoved actor and holds it.
    #[default]
    Solid,
    /// Unrailed geometry. An actor crossing it keeps its momentum, so a ledge
    /// does not consume impulse and a ledge run leads somewhere.
    Ledge,
    /// Open air. Anything that enters is committed to void.
    Void,
    /// A tile mid-retraction. It still carries an actor, and becomes void when
    /// its countdown commits, so shoving something here kills on a delay.
    Retracting { ticks_remaining: u32 },
    /// Structure the tool cannot move anything through.
    Wall,
}

impl CellKind {
    /// Whether an actor may occupy this cell at rest.
    #[must_use]
    pub const fn is_standable(self) -> bool {
        matches!(
            self,
            CellKind::Solid | CellKind::Ledge | CellKind::Retracting { .. }
        )
    }

    /// Whether the tool may move an actor through this cell at all.
    #[must_use]
    pub const fn blocks_travel(self) -> bool {
        matches!(self, CellKind::Wall)
    }
}

/// An Observer: the first-person seat holding the tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observer {
    pub id: PlayerId,
    pub cell: HexCoord,
    pub facing: HexFace,
    pub charge: u8,
    /// Ticks accumulated toward the next charge while standing on a live
    /// station. Reset the moment the station stops supplying.
    pub recharge_progress: u32,
    pub jailed: bool,
}

/// A minor Guardian: released by disturbance, immune to observation, removed
/// only by the architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinorGuardian {
    pub id: MinorGuardianId,
    pub cell: HexCoord,
    /// Ticks left recovering from a shove. A staggered minor does not pursue.
    pub stagger: u32,
    /// Ticks accumulated toward its next step.
    pub step_progress: u32,
    pub alive: bool,
}

/// A major Guardian, present purely so the observation asymmetry is provable in
/// the same board: this one *does* freeze when an Observer looks at it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MajorGuardian {
    pub cell: HexCoord,
    pub step_progress: u32,
    /// Recomputed every tick from current observation; presentation reads it
    /// rather than deriving its own.
    pub frozen: bool,
    /// Whether this Guardian is in play at all. A disabled major never moves,
    /// never captures, and is not drawn.
    pub enabled: bool,
}

/// Which parts of the opposition are switched on.
///
/// Feel is tuned by taking things away, not by reading numbers: the only honest
/// way to answer "is the shove satisfying" or "is this board too big" is to walk
/// it with the pressure removed and then put it back. These are authoritative —
/// they live in the world, ride in the digest, and are obeyed identically by the
/// schematic view, the first-person view, and the headless runner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KineticRules {
    /// Release the minor Guardians — the horde that observation does not stop.
    pub minors: bool,
    /// Release the major Guardian, the one that freezes when looked at.
    pub major: bool,
    /// Whether a Guardian reaching an Observer ends the run.
    ///
    /// With this off a capture simply does not resolve, so the board keeps
    /// running and a Guardian becomes a thing to be shoved rather than a fail
    /// state. This is the flag for studying the tool without a clock on you.
    pub jail: bool,
    /// Play the timed siege in a solved facility instead of the authored board.
    pub siege: bool,
    /// How long that siege lasts.
    pub siege_minutes: f32,
}

impl Default for KineticRules {
    fn default() -> Self {
        Self {
            minors: true,
            major: true,
            jail: true,
            siege: false,
            siege_minutes: 3.0,
        }
    }
}

impl KineticRules {
    /// Parse launch flags, ignoring anything that is not ours.
    ///
    /// Bevy and cargo both put their own arguments on this command line, so an
    /// unknown argument is never an error.
    #[must_use]
    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut rules = Self::default();
        for arg in args {
            match arg.as_ref() {
                "--no-minors" => rules.minors = false,
                "--no-major" => rules.major = false,
                "--no-guardians" => {
                    rules.minors = false;
                    rules.major = false;
                }
                "--no-jail" => rules.jail = false,
                "--siege" => rules.siege = true,
                other => {
                    if let Some(value) = other.strip_prefix("--minutes=")
                        && let Ok(minutes) = value.parse::<f32>()
                        && minutes > 0.0
                    {
                        rules.siege_minutes = minutes;
                    }
                }
            }
        }
        rules
    }

    /// One line for the HUD, naming only what has been switched off.
    #[must_use]
    pub fn summary(self) -> String {
        let mut off = Vec::new();
        if !self.minors {
            off.push("minors");
        }
        if !self.major {
            off.push("major");
        }
        if !self.jail {
            off.push("jail");
        }
        let base = if off.is_empty() {
            "all on".to_string()
        } else {
            format!("off: {}", off.join(", "))
        };
        if self.siege {
            format!("siege {:.0}m, {base}", self.siege_minutes)
        } else {
            base
        }
    }

    /// The siege schedule these rules ask for.
    #[must_use]
    pub fn siege_rules(self) -> SiegeRules {
        SiegeRules {
            enabled: self.siege,
            duration_ticks: (self.siege_minutes * 60.0 * TICKS_PER_SECOND as f32) as u32,
            ..SiegeRules::default()
        }
    }
}

/// Most minor Guardians that may be alive at once.
///
/// A hard ceiling rather than a soft one: presentation keeps a fixed pool of
/// shells, and a siege that outgrew the pool would start dropping Guardians from
/// the screen while they still hunted you in the simulation.
pub const MAX_LIVE_MINORS: usize = 40;

/// A timed siege: waves of minor Guardians, and a clock to outlast.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SiegeRules {
    pub enabled: bool,
    /// How long the Observer has to survive.
    pub duration_ticks: u32,
    /// When the first wave arrives, so there is time to get your bearings.
    pub first_wave_tick: u32,
    /// Ticks between waves.
    pub wave_interval_ticks: u32,
    /// Waves per size increment. Every `n` waves, one more Guardian arrives.
    pub waves_per_increment: u32,
    /// Largest single wave.
    pub max_wave_size: u32,
    /// Nothing spawns nearer than this, in plates.
    pub min_spawn_distance: u32,
}

impl Default for SiegeRules {
    fn default() -> Self {
        Self {
            enabled: false,
            duration_ticks: 180 * TICKS_PER_SECOND,
            first_wave_tick: 6 * TICKS_PER_SECOND,
            wave_interval_ticks: 12 * TICKS_PER_SECOND,
            waves_per_increment: 2,
            max_wave_size: 6,
            min_spawn_distance: 4,
        }
    }
}

impl SiegeRules {
    /// How many Guardians wave `index` brings.
    #[must_use]
    pub fn wave_size(self, index: u32) -> u32 {
        (1 + index / self.waves_per_increment.max(1)).min(self.max_wave_size)
    }
}

/// How a siege ended.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MatchOutcome {
    #[default]
    Running,
    /// The clock ran out with the Observer still free.
    Survived,
    /// A Guardian reached the Observer.
    Lost,
}

/// The flags this lab understands, for `--help` and for the READMEs.
pub const RULES_HELP: &str = concat!(
    "  --no-minors      leave the minor Guardians out of play\n",
    "  --no-major       leave the major Guardian out of play\n",
    "  --no-guardians   both of the above\n",
    "  --no-jail        a Guardian reaching you no longer ends the run\n",
    "  --siege          a solved WFC facility, and waves of Guardians to outlast\n",
    "  --minutes=N      how long the siege lasts (default 3)",
);

/// A recharge station: Architect-placed equipment that is inert without floor
/// power.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Station {
    pub id: StationId,
    pub cell: HexCoord,
}

/// One Observer's abstract intent for a tick. Input systems produce these;
/// nothing in this module reads a key or a mouse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KineticIntent {
    #[default]
    Idle,
    /// Turn to look down a different face without moving.
    Face(HexFace),
    /// Walk one cell along a face, if the destination is standable.
    Step(HexFace),
    /// Drive the first target in the facing lane away from the Observer.
    Push,
    /// Drag the first target in the facing lane toward the Observer.
    Pull,
    /// Operate the floor generator. Only legal while standing on it.
    ToggleGenerator,
}

/// Why a shove ended where it did. Presentation and tests both read this rather
/// than re-deriving the outcome from positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShoveFate {
    /// Came to rest on standable floor.
    Rest,
    /// Entered void and was destroyed immediately.
    Void,
    /// Came to rest on a retracting tile: destroyed when that tile commits.
    Doomed,
    /// Ran into structure, the lattice boundary, or another actor.
    Blocked,
}

/// Everything a shove resolved to, in one value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShoveResolution {
    pub guardian: MinorGuardianId,
    pub from: HexCoord,
    pub to: HexCoord,
    pub face: HexFace,
    pub cells_travelled: u32,
    pub fate: ShoveFate,
}

/// Why the tool refused to fire. A refusal is never silent: the lab shows it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolRefusal {
    NoTargetInLane,
    NotEnoughCharge,
    NotOnGenerator,
}

/// Observable transitions for one tick, in resolution order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KineticEvent {
    Shoved(ShoveResolution),
    /// A minor Guardian left play. Always caused by a cell, never by the tool.
    GuardianDestroyed {
        id: MinorGuardianId,
        cell: HexCoord,
        by_retraction: bool,
    },
    ToolRefused {
        observer: PlayerId,
        refusal: ToolRefusal,
    },
    ChargeRestored {
        observer: PlayerId,
        station: StationId,
        charge: u8,
    },
    GeneratorToggled {
        observer: PlayerId,
        powered: bool,
    },
    TileRetracted {
        cell: HexCoord,
    },
    ObserverCaptured {
        observer: PlayerId,
        by_major: bool,
    },
    /// A siege wave arrived.
    WaveReleased {
        index: u32,
        size: u32,
    },
    MinorReleased {
        id: MinorGuardianId,
        cell: HexCoord,
    },
    SiegeEnded {
        outcome: MatchOutcome,
    },
}

/// The whole lab world. Pure data: no Bevy entity, camera, or asset appears
/// anywhere in this module.
#[derive(Clone, Debug, PartialEq, Resource)]
pub struct KineticWorld {
    pub grid: HexGridSize,
    cells: Vec<CellKind>,
    /// Lateral door mask per cell, bit `face.index()`.
    ///
    /// A face without its bit is a **wall**: nothing walks, shoves, sees or
    /// pursues through it. The authored rectangle leaves every face open, which
    /// is why it plays as an open plain; a facility solved by the real WFC has
    /// rooms and corridors, and this is what makes them mean something.
    doors: Vec<u8>,
    pub observers: Vec<Observer>,
    pub minors: Vec<MinorGuardian>,
    pub major: MajorGuardian,
    pub stations: Vec<Station>,
    /// Floor power. False kills recharge and shortens observation to the cell
    /// the Observer occupies.
    pub powered: bool,
    pub generator: HexCoord,
    /// Which parts of the opposition are switched on for this run.
    pub rules: KineticRules,
    /// The siege schedule, if this is a timed run.
    pub siege: SiegeRules,
    pub outcome: MatchOutcome,
    /// Waves released so far.
    pub waves_released: u32,
    /// Minor Guardians destroyed. The only score this lab keeps.
    pub kills: u32,
    /// Seed for wave placement, advanced per wave so spawns are reproducible.
    pub spawn_seed: u64,
    pub tick: u32,
    pub events: Vec<KineticEvent>,
}

impl KineticWorld {
    /// The authored proving board with every rule switched on.
    #[must_use]
    pub fn authored() -> Self {
        Self::authored_with(KineticRules::default())
    }

    /// Build a board from a solved facility.
    ///
    /// The authored rectangle exists to make one rule visible at a time; it is
    /// deliberately an open plain, and it plays like one. This takes the real
    /// solver's output instead: `Void` cells are holes, `Room` and `Hall` cells
    /// are floor, and the per-cell door mask becomes walls, so a shove stops at
    /// a wall, sight stops at a wall, and a Guardian has to come through a
    /// doorway like everything else.
    ///
    /// What it does *not* take is the authored tile geometry. Plates stay flat
    /// rectangles and walls are face slabs, so this is the solver's **layout**,
    /// not its hulls. Projecting real hulls needs a mesh collider and is a
    /// separate piece of work; saying so here is cheaper than a reader
    /// discovering it from the screenshots.
    #[must_use]
    pub fn from_placements(
        grid: HexGridSize,
        level: u8,
        placement_at: &dyn Fn(HexCoord) -> Option<(bool, u8)>,
        rules: KineticRules,
    ) -> Self {
        let mut world = Self::authored_with(rules);
        world.grid = grid;
        world.cells = vec![CellKind::Void; grid.cell_count()];
        world.doors = vec![0; grid.cell_count()];
        world.stations.clear();

        for index in 0..grid.cell_count() {
            let coord = grid.coord(index);
            if coord.level != level {
                continue;
            }
            let Some((solid, doors)) = placement_at(coord) else {
                continue;
            };
            world.cells[index] = if solid {
                CellKind::Solid
            } else {
                CellKind::Void
            };
            world.doors[index] = doors & ALL_LATERAL_DOORS;
        }

        // A door is only a door if both sides agree. The solver already
        // guarantees that, but a lab that trusted it silently would produce a
        // one-way wall the first time a placement was edited by hand.
        for index in 0..grid.cell_count() {
            let coord = grid.coord(index);
            for face in HexFace::LATERAL {
                let open = world.doors[index] & (1 << face.index()) != 0;
                let mutual = grid.neighbor(coord, face).is_some_and(|next| {
                    world.doors[grid.index(next)] & (1 << face.opposite().index()) != 0
                });
                if open && !mutual {
                    world.doors[index] &= !(1 << face.index());
                }
            }
        }

        world
    }

    /// Every cell an actor could stand on.
    #[must_use]
    pub fn standable_cells(&self) -> Vec<HexCoord> {
        (0..self.grid.cell_count())
            .map(|index| self.grid.coord(index))
            .filter(|coord| self.cell(*coord).is_standable())
            .collect()
    }

    /// The authored proving board: a solid approach, a ledge run ending over
    /// void, one retracting tile, a station, and the generator that powers it.
    #[must_use]
    pub fn authored_with(rules: KineticRules) -> Self {
        let grid = HexGridSize {
            cols: 9,
            rows: 7,
            levels: 1,
        };
        let mut cells = vec![CellKind::Solid; grid.cell_count()];
        let mut set = |q: u16, r: u16, kind: CellKind| {
            let coord = HexCoord { q, r, level: 0 };
            cells[grid.index(coord)] = kind;
        };

        // A rim of void around the floor: the board has real edges to shove
        // things over, and no shove can leave the lattice silently.
        for q in 0..grid.cols {
            set(q, 0, CellKind::Void);
            set(q, grid.rows - 1, CellKind::Void);
        }
        for r in 0..grid.rows {
            set(0, r, CellKind::Void);
            set(grid.cols - 1, r, CellKind::Void);
        }

        // An unrailed run reaching east toward the rim. Momentum carries across
        // it, so a push landing on the first ledge continues over the edge.
        set(5, 3, CellKind::Ledge);
        set(6, 3, CellKind::Ledge);
        set(7, 3, CellKind::Ledge);

        // One tile already retracting, to prove the delayed kill.
        set(
            3,
            5,
            CellKind::Retracting {
                ticks_remaining: 180,
            },
        );

        // Structure that stops a shove dead, to prove Blocked.
        set(2, 2, CellKind::Wall);

        Self {
            grid,
            cells,
            // The authored board is an open plain: every lateral face is a
            // doorway, so this behaves exactly as it did before walls existed.
            doors: vec![ALL_LATERAL_DOORS; grid.cell_count()],
            observers: vec![Observer {
                id: PlayerId(0),
                cell: HexCoord {
                    q: 3,
                    r: 3,
                    level: 0,
                },
                facing: HexFace::East,
                charge: MAX_CHARGE,
                recharge_progress: 0,
                jailed: false,
            }],
            // Both of these used to start on plates *adjacent* to the Observer,
            // so one stepped onto the spawn 150 ticks — two and a half seconds —
            // after launch and jailed the player before they had finished
            // reading the controls. A Guardian should have to cross the room.
            minors: if rules.minors {
                vec![
                    MinorGuardian {
                        id: MinorGuardianId(0),
                        cell: HexCoord {
                            q: 7,
                            r: 1,
                            level: 0,
                        },
                        stagger: 0,
                        step_progress: 0,
                        alive: true,
                    },
                    MinorGuardian {
                        id: MinorGuardianId(1),
                        cell: HexCoord {
                            q: 1,
                            r: 5,
                            level: 0,
                        },
                        stagger: 0,
                        step_progress: 0,
                        alive: true,
                    },
                ]
            } else {
                Vec::new()
            },
            major: MajorGuardian {
                cell: HexCoord {
                    q: 6,
                    r: 5,
                    level: 0,
                },
                step_progress: 0,
                frozen: false,
                enabled: rules.major,
            },
            stations: vec![Station {
                id: StationId(0),
                cell: HexCoord {
                    q: 2,
                    r: 4,
                    level: 0,
                },
            }],
            powered: true,
            generator: HexCoord {
                q: 1,
                r: 1,
                level: 0,
            },
            rules,
            siege: SiegeRules::default(),
            outcome: MatchOutcome::Running,
            waves_released: 0,
            kills: 0,
            spawn_seed: 0x51E6_E5EE_D000,
            tick: 0,
            events: Vec::new(),
        }
    }

    #[must_use]
    pub fn cell(&self, coord: HexCoord) -> CellKind {
        if !self.grid.contains(coord) {
            return CellKind::Wall;
        }
        self.cells[self.grid.index(coord)]
    }

    /// Whether `face` of `coord` is a doorway rather than a wall.
    #[must_use]
    pub fn connected(&self, coord: HexCoord, face: HexFace) -> bool {
        if !face.is_lateral() || !self.grid.contains(coord) {
            return false;
        }
        self.doors[self.grid.index(coord)] & (1 << face.index()) != 0
    }

    /// The neighbour behind `face`, but only if the face is open.
    ///
    /// Every rule that used to walk the raw lattice goes through this now:
    /// sight, the targeting lane, shove travel, pursuit and the schematic
    /// view's step. That single change is what turns a solved facility from a
    /// coloured floor plan into somewhere with rooms.
    #[must_use]
    pub fn passable_neighbor(&self, coord: HexCoord, face: HexFace) -> Option<HexCoord> {
        self.connected(coord, face)
            .then(|| self.grid.neighbor(coord, face))
            .flatten()
    }

    #[must_use]
    pub fn observer(&self, id: PlayerId) -> Option<&Observer> {
        self.observers.iter().find(|observer| observer.id == id)
    }

    #[must_use]
    pub fn minor(&self, id: MinorGuardianId) -> Option<&MinorGuardian> {
        self.minors.iter().find(|minor| minor.id == id)
    }

    #[must_use]
    pub fn living_minors(&self) -> usize {
        self.minors.iter().filter(|minor| minor.alive).count()
    }

    /// Whether any Observer currently sees `coord`.
    ///
    /// Sight runs down the facing lane and stops at a wall. Unpowered, it
    /// collapses to the Observer's own cell: what cannot be seen cannot be
    /// frozen. This costs *range*, never legibility — the presentation layer
    /// still draws every critical signal at its documented minimum.
    #[must_use]
    pub fn is_observed(&self, coord: HexCoord) -> bool {
        self.observers
            .iter()
            .filter(|observer| !observer.jailed)
            .any(|observer| {
                if observer.cell == coord {
                    return true;
                }
                if !self.powered {
                    return false;
                }
                let mut cursor = observer.cell;
                for _ in 0..OBSERVATION_RANGE {
                    let Some(next) = self.passable_neighbor(cursor, observer.facing) else {
                        return false;
                    };
                    if self.cell(next).blocks_travel() {
                        return false;
                    }
                    if next == coord {
                        return true;
                    }
                    cursor = next;
                }
                false
            })
    }

    /// Whether a Guardian stands here. Guardians do not walk through each other.
    fn guardian_occupies(&self, coord: HexCoord) -> bool {
        self.minors
            .iter()
            .any(|minor| minor.alive && minor.cell == coord)
            // A disabled major is out of play entirely: it must not block a
            // shove or a pursuit from somewhere it is not really standing.
            || (self.major.enabled && self.major.cell == coord)
    }

    /// Whether any actor stands here.
    ///
    /// Used where a body is genuinely in the way — a shove cannot drive its
    /// target through someone, and an Observer cannot walk into an occupied
    /// cell. Pursuit deliberately does *not* use this: a Guardian reaching an
    /// Observer's cell is the capture, so treating an Observer as an obstacle
    /// would make capture unreachable.
    fn actor_occupies(&self, coord: HexCoord) -> bool {
        self.guardian_occupies(coord)
            || self
                .observers
                .iter()
                .any(|observer| !observer.jailed && observer.cell == coord)
    }

    /// The first living minor Guardian down an Observer's facing lane.
    #[must_use]
    pub fn target_in_lane(&self, observer: &Observer) -> Option<MinorGuardianId> {
        let mut cursor = observer.cell;
        for _ in 0..TOOL_RANGE {
            let next = self.passable_neighbor(cursor, observer.facing)?;
            if self.cell(next).blocks_travel() {
                return None;
            }
            if let Some(minor) = self
                .minors
                .iter()
                .find(|minor| minor.alive && minor.cell == next)
            {
                return Some(minor.id);
            }
            cursor = next;
        }
        None
    }

    /// Resolve a shove without applying it.
    ///
    /// Pure and total: the same world, target, face and impulse always produce
    /// the same resolution. Ledge cells do not consume impulse, which is how
    /// "shoved off unrailed geometry" becomes a rule rather than a physics
    /// accident.
    #[must_use]
    pub fn resolve_shove(
        &self,
        id: MinorGuardianId,
        face: HexFace,
        impulse: u32,
    ) -> Option<ShoveResolution> {
        let minor = self.minor(id).filter(|minor| minor.alive)?;
        let from = minor.cell;
        let mut cell = from;
        let mut remaining = impulse;
        let mut travelled = 0;
        let mut fate = ShoveFate::Rest;

        while travelled < MAX_TRAVEL_CELLS {
            // Momentum is spent only once the target is on railed floor.
            if remaining == 0 && self.cell(cell) != CellKind::Ledge {
                fate = ShoveFate::Rest;
                break;
            }
            let Some(next) = self.passable_neighbor(cell, face) else {
                fate = ShoveFate::Blocked;
                break;
            };
            let kind = self.cell(next);
            if kind.blocks_travel() || (self.actor_occupies(next) && next != from) {
                fate = ShoveFate::Blocked;
                break;
            }
            cell = next;
            travelled += 1;
            remaining = remaining.saturating_sub(1);

            match kind {
                CellKind::Void => {
                    fate = ShoveFate::Void;
                    break;
                }
                CellKind::Retracting { .. } => {
                    fate = ShoveFate::Doomed;
                    break;
                }
                // A ledge neither stops the target nor spends its momentum.
                CellKind::Ledge => continue,
                CellKind::Solid => {
                    if remaining == 0 {
                        fate = ShoveFate::Rest;
                        break;
                    }
                }
                CellKind::Wall => unreachable!("walls are rejected above"),
            }
        }

        Some(ShoveResolution {
            guardian: id,
            from,
            to: cell,
            face,
            cells_travelled: travelled,
            fate,
        })
    }

    /// Advance one fixed tick.
    ///
    /// Resolution order is part of the determinism contract: Observer intents,
    /// recharge, retraction, Guardian movement, capture. Events are emitted in
    /// that same order.
    pub fn step(&mut self, intents: &[(PlayerId, KineticIntent)]) {
        self.events.clear();
        self.apply_intents(intents);
        self.apply_recharge();
        self.apply_retraction();
        self.release_waves();
        self.move_guardians();
        self.resolve_captures();
        self.resolve_siege_clock();
        self.tick += 1;
    }

    /// Release this tick's wave, if the siege schedule calls for one.
    fn release_waves(&mut self) {
        if !self.siege.enabled || self.outcome != MatchOutcome::Running {
            return;
        }
        let due = self.tick >= self.siege.first_wave_tick
            && (self.tick - self.siege.first_wave_tick)
                .is_multiple_of(self.siege.wave_interval_ticks.max(1));
        if !due {
            return;
        }

        let index = self.waves_released;
        self.waves_released += 1;
        let size = self.siege.wave_size(index);

        let Some(observer) = self
            .observers
            .iter()
            .find(|observer| !observer.jailed)
            .copied()
        else {
            return;
        };

        // Spawn far enough away to be seen coming, never on top of anyone, and
        // never inside a sealed pocket the Observer could not be reached from.
        let candidates: Vec<HexCoord> = self
            .standable_cells()
            .into_iter()
            .filter(|coord| {
                lateral_distance(*coord, observer.cell) >= self.siege.min_spawn_distance
                    && !self.actor_occupies(*coord)
            })
            .collect();
        if candidates.is_empty() {
            return;
        }

        let mut rng = SplitMix::new(self.spawn_seed ^ u64::from(index).wrapping_mul(0x9E37_79B9));
        let mut released = 0;
        for _ in 0..size {
            if self.living_minors() >= MAX_LIVE_MINORS {
                break;
            }
            let cell = candidates[rng.below(candidates.len())];
            if self.actor_occupies(cell) {
                continue;
            }
            let id = self.spawn_minor(cell);
            self.events.push(KineticEvent::MinorReleased { id, cell });
            released += 1;
        }

        if released > 0 {
            self.events.push(KineticEvent::WaveReleased {
                index,
                size: released,
            });
        }
    }

    /// Put one minor Guardian into play, reusing a dead slot when there is one.
    ///
    /// Reuse keeps `minors` bounded, which is what lets presentation hold a
    /// fixed pool of shells instead of spawning entities mid-match.
    fn spawn_minor(&mut self, cell: HexCoord) -> MinorGuardianId {
        let fresh = MinorGuardian {
            id: MinorGuardianId(0),
            cell,
            stagger: 0,
            step_progress: 0,
            alive: true,
        };
        if let Some(slot) = self.minors.iter().position(|minor| !minor.alive) {
            let id = self.minors[slot].id;
            self.minors[slot] = MinorGuardian { id, ..fresh };
            return id;
        }
        let id = MinorGuardianId(self.minors.len() as u32);
        self.minors.push(MinorGuardian { id, ..fresh });
        id
    }

    fn resolve_siege_clock(&mut self) {
        if !self.siege.enabled || self.outcome != MatchOutcome::Running {
            return;
        }
        if self.observers.iter().all(|observer| observer.jailed) {
            self.outcome = MatchOutcome::Lost;
            self.events.push(KineticEvent::SiegeEnded {
                outcome: MatchOutcome::Lost,
            });
        } else if self.tick + 1 >= self.siege.duration_ticks {
            self.outcome = MatchOutcome::Survived;
            self.events.push(KineticEvent::SiegeEnded {
                outcome: MatchOutcome::Survived,
            });
        }
    }

    /// Ticks left on the siege clock.
    #[must_use]
    pub fn siege_remaining(&self) -> u32 {
        self.siege.duration_ticks.saturating_sub(self.tick)
    }

    fn apply_intents(&mut self, intents: &[(PlayerId, KineticIntent)]) {
        for &(id, intent) in intents {
            let Some(index) = self
                .observers
                .iter()
                .position(|observer| observer.id == id && !observer.jailed)
            else {
                continue;
            };
            match intent {
                KineticIntent::Idle => {}
                KineticIntent::Face(face) => self.observers[index].facing = face,
                KineticIntent::Step(face) => {
                    let observer = self.observers[index];
                    if let Some(next) = self.passable_neighbor(observer.cell, face)
                        && self.cell(next).is_standable()
                        && !self.actor_occupies(next)
                    {
                        self.observers[index].cell = next;
                        self.observers[index].recharge_progress = 0;
                    }
                    self.observers[index].facing = face;
                }
                KineticIntent::Push => self.fire(index, PUSH_COST, PUSH_IMPULSE, true),
                KineticIntent::Pull => self.fire(index, PULL_COST, PULL_IMPULSE, false),
                KineticIntent::ToggleGenerator => {
                    let observer = self.observers[index];
                    if observer.cell == self.generator {
                        self.powered = !self.powered;
                        self.events.push(KineticEvent::GeneratorToggled {
                            observer: observer.id,
                            powered: self.powered,
                        });
                    } else {
                        self.events.push(KineticEvent::ToolRefused {
                            observer: observer.id,
                            refusal: ToolRefusal::NotOnGenerator,
                        });
                    }
                }
            }
        }
    }

    /// Fire the tool. `away` pushes down the facing lane; otherwise the target
    /// is dragged back along it.
    fn fire(&mut self, index: usize, cost: u8, impulse: u32, away: bool) {
        let observer = self.observers[index];
        let Some(target) = self.target_in_lane(&observer) else {
            self.events.push(KineticEvent::ToolRefused {
                observer: observer.id,
                refusal: ToolRefusal::NoTargetInLane,
            });
            return;
        };
        if observer.charge < cost {
            self.events.push(KineticEvent::ToolRefused {
                observer: observer.id,
                refusal: ToolRefusal::NotEnoughCharge,
            });
            return;
        }
        let face = if away {
            observer.facing
        } else {
            observer.facing.opposite()
        };
        let Some(resolution) = self.resolve_shove(target, face, impulse) else {
            return;
        };

        self.observers[index].charge -= cost;
        if let Some(minor) = self.minors.iter_mut().find(|minor| minor.id == target) {
            minor.cell = resolution.to;
            minor.stagger = STAGGER_TICKS;
            minor.step_progress = 0;
        }
        self.events.push(KineticEvent::Shoved(resolution));

        // The tool did not kill this: the destination did.
        if resolution.fate == ShoveFate::Void {
            self.destroy_minor(target, resolution.to, false);
        }
    }

    fn destroy_minor(&mut self, id: MinorGuardianId, cell: HexCoord, by_retraction: bool) {
        if let Some(minor) = self
            .minors
            .iter_mut()
            .find(|minor| minor.id == id && minor.alive)
        {
            minor.alive = false;
            self.kills += 1;
            self.events.push(KineticEvent::GuardianDestroyed {
                id,
                cell,
                by_retraction,
            });
        }
    }

    fn apply_recharge(&mut self) {
        for index in 0..self.observers.len() {
            let observer = self.observers[index];
            let station = self
                .stations
                .iter()
                .find(|station| station.cell == observer.cell)
                .copied();
            // A station without floor power is inert, and progress toward the
            // next charge does not survive the outage.
            let Some(station) = station.filter(|_| self.powered && !observer.jailed) else {
                self.observers[index].recharge_progress = 0;
                continue;
            };
            if observer.charge >= MAX_CHARGE {
                self.observers[index].recharge_progress = 0;
                continue;
            }
            let progress = observer.recharge_progress + 1;
            if progress >= RECHARGE_TICKS {
                self.observers[index].recharge_progress = 0;
                self.observers[index].charge += 1;
                let charge = self.observers[index].charge;
                self.events.push(KineticEvent::ChargeRestored {
                    observer: observer.id,
                    station: station.id,
                    charge,
                });
            } else {
                self.observers[index].recharge_progress = progress;
            }
        }
    }

    fn apply_retraction(&mut self) {
        let mut committed = Vec::new();
        for index in 0..self.cells.len() {
            if let CellKind::Retracting { ticks_remaining } = self.cells[index] {
                let remaining = ticks_remaining.saturating_sub(1);
                if remaining == 0 {
                    self.cells[index] = CellKind::Void;
                    committed.push(self.grid.coord(index));
                } else {
                    self.cells[index] = CellKind::Retracting {
                        ticks_remaining: remaining,
                    };
                }
            }
        }
        for cell in committed {
            self.events.push(KineticEvent::TileRetracted { cell });
            let doomed: Vec<MinorGuardianId> = self
                .minors
                .iter()
                .filter(|minor| minor.alive && minor.cell == cell)
                .map(|minor| minor.id)
                .collect();
            for id in doomed {
                self.destroy_minor(id, cell, true);
            }
        }
    }

    /// Step toward the nearest Observer along the lateral face that reduces
    /// distance most, breaking ties by `HexFace::LATERAL` order.
    fn pursuit_step(&self, from: HexCoord) -> Option<HexCoord> {
        let target = self
            .observers
            .iter()
            .filter(|observer| !observer.jailed)
            .min_by_key(|observer| (lateral_distance(from, observer.cell), observer.id.0))?
            .cell;
        let mut best: Option<(u32, HexCoord)> = None;
        for face in HexFace::LATERAL {
            let Some(next) = self.passable_neighbor(from, face) else {
                continue;
            };
            // A Guardian will not walk itself into void or structure, and will
            // not displace another Guardian — but an Observer's cell is a legal
            // destination, because arriving there is the capture.
            if !self.cell(next).is_standable() || self.guardian_occupies(next) {
                continue;
            }
            let distance = lateral_distance(next, target);
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, next));
            }
        }
        let (distance, next) = best?;
        (distance < lateral_distance(from, target)).then_some(next)
    }

    fn move_guardians(&mut self) {
        // Minors first, in id order, so the tick's movement is reproducible.
        for index in 0..self.minors.len() {
            let minor = self.minors[index];
            if !minor.alive {
                continue;
            }
            if minor.stagger > 0 {
                self.minors[index].stagger = minor.stagger - 1;
                continue;
            }
            // Deliberately unconditional: observation does not reach minors.
            let progress = minor.step_progress + 1;
            if progress < MINOR_STEP_TICKS {
                self.minors[index].step_progress = progress;
                continue;
            }
            self.minors[index].step_progress = 0;
            if let Some(next) = self.pursuit_step(minor.cell) {
                self.minors[index].cell = next;
            }
        }

        if !self.major.enabled {
            return;
        }
        // The major freezes under observation. This is the contrast the lab
        // exists to make visible.
        self.major.frozen = self.is_observed(self.major.cell);
        if self.major.frozen {
            return;
        }
        let progress = self.major.step_progress + 1;
        if progress < MAJOR_STEP_TICKS {
            self.major.step_progress = progress;
            return;
        }
        self.major.step_progress = 0;
        if let Some(next) = self.pursuit_step(self.major.cell) {
            self.major.cell = next;
        }
    }

    fn resolve_captures(&mut self) {
        // With jail switched off a Guardian may share your plate and nothing
        // happens: it becomes something to shove rather than a fail state.
        if !self.rules.jail {
            return;
        }
        for index in 0..self.observers.len() {
            let observer = self.observers[index];
            if observer.jailed {
                continue;
            }
            let by_major = self.major.enabled && self.major.cell == observer.cell;
            let touched = by_major
                || self
                    .minors
                    .iter()
                    .any(|minor| minor.alive && minor.cell == observer.cell);
            if touched {
                self.observers[index].jailed = true;
                self.events.push(KineticEvent::ObserverCaptured {
                    observer: observer.id,
                    by_major,
                });
            }
        }
    }

    /// Order-sensitive digest of authoritative state.
    ///
    /// Two runs fed the same intents must agree here on every tick. The lab's
    /// determinism test compares digests rather than fields so that a new piece
    /// of state cannot quietly escape the contract.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash = SplitMix::new(0x0B5E_2FED ^ u64::from(self.tick));
        let mut mix = |value: u64| {
            hash.0 ^= value;
            hash.next_u64()
        };
        let mut acc = mix(u64::from(self.powered));
        // The rules change what the same inputs produce, so they are part of
        // the state a replay has to agree on.
        acc ^= mix(u64::from(self.rules.minors));
        acc ^= mix(u64::from(self.rules.major));
        acc ^= mix(u64::from(self.rules.jail));
        for cell in &self.cells {
            acc ^= mix(match cell {
                CellKind::Solid => 1,
                CellKind::Ledge => 2,
                CellKind::Void => 3,
                CellKind::Wall => 4,
                CellKind::Retracting { ticks_remaining } => 5 + u64::from(*ticks_remaining) * 8,
            });
        }
        for observer in &self.observers {
            acc ^= mix(coord_key(observer.cell));
            acc ^= mix(observer.facing.index() as u64);
            acc ^= mix(u64::from(observer.charge));
            acc ^= mix(u64::from(observer.recharge_progress));
            acc ^= mix(u64::from(observer.jailed));
        }
        for minor in &self.minors {
            acc ^= mix(coord_key(minor.cell));
            acc ^= mix(u64::from(minor.stagger));
            acc ^= mix(u64::from(minor.step_progress));
            acc ^= mix(u64::from(minor.alive));
        }
        acc ^= mix(coord_key(self.major.cell));
        acc ^= mix(u64::from(self.major.step_progress));
        acc ^= mix(u64::from(self.major.frozen));
        acc ^= mix(u64::from(self.major.enabled));
        acc
    }
}

fn coord_key(coord: HexCoord) -> u64 {
    u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(q: u16, r: u16) -> HexCoord {
        HexCoord { q, r, level: 0 }
    }

    /// Place one minor at `cell` and nothing else in the way.
    fn board_with_minor(cell: HexCoord) -> KineticWorld {
        let mut world = KineticWorld::authored();
        world.minors.truncate(1);
        world.minors[0].cell = cell;
        world.major.cell = coord(7, 5);
        world
    }

    #[test]
    fn a_push_into_the_rim_commits_the_target_to_void() {
        let mut world = board_with_minor(coord(4, 1));
        world.observers[0].cell = coord(4, 2);
        world.observers[0].facing = HexFace::NorthWest;

        world.step(&[(PlayerId(0), KineticIntent::Push)]);

        let resolution = world
            .events
            .iter()
            .find_map(|event| match event {
                KineticEvent::Shoved(resolution) => Some(*resolution),
                _ => None,
            })
            .expect("the push resolved");
        assert_eq!(resolution.fate, ShoveFate::Void);
        assert_eq!(world.living_minors(), 0);
        assert!(world.events.iter().any(|event| matches!(
            event,
            KineticEvent::GuardianDestroyed {
                by_retraction: false,
                ..
            }
        )));
    }

    #[test]
    fn momentum_carries_across_a_ledge_run_and_over_the_edge() {
        // The minor starts west of the ledge run at (5..7, 3); the rim is at
        // q == 8. A three-cell impulse alone would stop on (7, 3), but ledges
        // do not spend impulse, so the target keeps going into the rim.
        let mut world = board_with_minor(coord(4, 3));
        world.observers[0].cell = coord(3, 3);
        world.observers[0].facing = HexFace::East;

        let resolution = world
            .resolve_shove(MinorGuardianId(0), HexFace::East, PUSH_IMPULSE)
            .expect("the shove resolved");
        assert_eq!(resolution.fate, ShoveFate::Void);
        assert_eq!(resolution.to, coord(8, 3));
        assert!(resolution.cells_travelled > PUSH_IMPULSE);
    }

    #[test]
    fn a_push_onto_solid_floor_leaves_the_target_alive_and_staggered() {
        let mut world = board_with_minor(coord(4, 4));
        world.observers[0].cell = coord(3, 4);
        world.observers[0].facing = HexFace::East;

        world.step(&[(PlayerId(0), KineticIntent::Push)]);

        let minor = world.minor(MinorGuardianId(0)).expect("minor still exists");
        assert!(minor.alive, "solid floor is not lethal");
        assert_eq!(minor.cell, coord(7, 4));
        // Stagger is set during the intent phase and starts running down in the
        // same tick's movement phase, so one tick has already been spent.
        assert_eq!(minor.stagger, STAGGER_TICKS - 1);
    }

    #[test]
    fn a_shove_onto_a_retracting_tile_kills_only_when_the_tile_commits() {
        let mut world = board_with_minor(coord(3, 5));
        let resolution = world
            .resolve_shove(MinorGuardianId(0), HexFace::East, 0)
            .expect("resolved");
        assert_eq!(resolution.fate, ShoveFate::Rest);

        // Standing on the retracting tile, the minor survives until the
        // countdown commits, then dies to the tile rather than to the tool.
        // The countdown is shortened below one pursuit step so the minor cannot
        // simply walk off the tile before the point under test.
        let index = world.grid.index(coord(3, 5));
        world.cells[index] = CellKind::Retracting {
            ticks_remaining: 10,
        };
        world.minors[0].cell = coord(3, 5);
        for _ in 0..9 {
            world.step(&[]);
        }
        assert_eq!(world.living_minors(), 1);
        world.step(&[]);
        assert_eq!(world.living_minors(), 0);
        assert!(world.events.iter().any(|event| matches!(
            event,
            KineticEvent::GuardianDestroyed {
                by_retraction: true,
                ..
            }
        )));
    }

    #[test]
    fn structure_blocks_a_shove_without_destroying_anything() {
        let world = board_with_minor(coord(3, 2));
        let resolution = world
            .resolve_shove(MinorGuardianId(0), HexFace::West, PUSH_IMPULSE)
            .expect("resolved");
        assert_eq!(resolution.fate, ShoveFate::Blocked);
        assert_eq!(resolution.to, coord(3, 2), "a blocked target does not move");
        assert_eq!(world.living_minors(), 1);
    }

    #[test]
    fn observation_freezes_a_major_guardian() {
        let mut world = KineticWorld::authored();
        world.minors.clear();
        world.observers[0].cell = coord(4, 4);
        world.observers[0].facing = HexFace::East;
        world.major.cell = coord(5, 4);

        for _ in 0..MAJOR_STEP_TICKS + 4 {
            world.step(&[]);
        }

        assert!(world.major.frozen, "a looked-at major holds still");
        assert_eq!(world.major.cell, coord(5, 4));
    }

    #[test]
    fn observation_does_nothing_at_all_to_a_minor_guardian() {
        let mut world = KineticWorld::authored();
        world.minors.truncate(1);
        world.observers[0].cell = coord(4, 4);
        world.observers[0].facing = HexFace::East;
        world.minors[0].cell = coord(6, 4);
        // Parked well clear: a major standing in the minor's path would block
        // the step under test and prove nothing about observation.
        world.major.cell = coord(1, 2);

        assert!(
            world.is_observed(coord(6, 4)),
            "the minor is squarely in the observation lane"
        );
        for _ in 0..MINOR_STEP_TICKS {
            world.step(&[]);
        }

        assert_eq!(
            world.minors[0].cell,
            coord(5, 4),
            "a looked-at minor keeps coming"
        );
    }

    #[test]
    fn cutting_power_costs_observation_range_and_wakes_the_major() {
        let mut world = KineticWorld::authored();
        world.observers[0].cell = coord(4, 4);
        world.observers[0].facing = HexFace::East;
        world.major.cell = coord(5, 4);

        world.step(&[]);
        assert!(world.major.frozen);

        world.powered = false;
        world.step(&[]);
        assert!(!world.major.frozen, "darkness releases what cannot be seen");
        assert!(
            world.is_observed(world.observers[0].cell),
            "an Observer still sees their own cell"
        );
    }

    #[test]
    fn charge_is_finite_and_restored_only_by_a_powered_station() {
        let mut world = KineticWorld::authored();
        // Isolate recharge: live Guardians would jail the Observer standing
        // still on the station long before the countdown finished.
        world.minors.clear();
        world.major.cell = coord(7, 1);
        let station = world.stations[0].cell;
        world.observers[0].cell = station;
        world.observers[0].charge = 0;

        // Unpowered, the station is inert no matter how long you stand on it.
        world.powered = false;
        for _ in 0..RECHARGE_TICKS * 2 {
            world.step(&[]);
        }
        assert_eq!(world.observers[0].charge, 0);
        assert_eq!(world.observers[0].recharge_progress, 0);

        world.powered = true;
        for _ in 0..RECHARGE_TICKS {
            world.step(&[]);
        }
        assert_eq!(world.observers[0].charge, 1);
        assert!(
            world
                .events
                .iter()
                .any(|event| matches!(event, KineticEvent::ChargeRestored { charge: 1, .. }))
        );
    }

    #[test]
    fn the_tool_refuses_rather_than_firing_on_an_empty_lane_or_empty_charge() {
        let mut world = board_with_minor(coord(4, 4));
        world.observers[0].cell = coord(3, 4);
        world.observers[0].facing = HexFace::West;
        world.step(&[(PlayerId(0), KineticIntent::Push)]);
        assert!(world.events.iter().any(|event| matches!(
            event,
            KineticEvent::ToolRefused {
                refusal: ToolRefusal::NoTargetInLane,
                ..
            }
        )));

        world.observers[0].facing = HexFace::East;
        world.observers[0].charge = PUSH_COST - 1;
        world.step(&[(PlayerId(0), KineticIntent::Push)]);
        assert!(world.events.iter().any(|event| matches!(
            event,
            KineticEvent::ToolRefused {
                refusal: ToolRefusal::NotEnoughCharge,
                ..
            }
        )));
        assert_eq!(world.living_minors(), 1, "a refusal moves nothing");
    }

    #[test]
    fn a_pull_drags_the_target_one_cell_back_toward_the_observer() {
        let mut world = board_with_minor(coord(5, 4));
        world.observers[0].cell = coord(3, 4);
        world.observers[0].facing = HexFace::East;

        world.step(&[(PlayerId(0), KineticIntent::Pull)]);

        assert_eq!(world.minors[0].cell, coord(4, 4));
        assert_eq!(world.observers[0].charge, MAX_CHARGE - PULL_COST);
    }

    #[test]
    fn the_generator_only_answers_an_observer_standing_on_it() {
        let mut world = KineticWorld::authored();
        world.step(&[(PlayerId(0), KineticIntent::ToggleGenerator)]);
        assert!(world.powered, "a remote toggle changes nothing");
        assert!(world.events.iter().any(|event| matches!(
            event,
            KineticEvent::ToolRefused {
                refusal: ToolRefusal::NotOnGenerator,
                ..
            }
        )));

        world.observers[0].cell = world.generator;
        world.step(&[(PlayerId(0), KineticIntent::ToggleGenerator)]);
        assert!(!world.powered);
    }

    #[test]
    fn a_minor_guardian_can_actually_reach_and_jail_an_observer() {
        // Regression: pursuit once treated an Observer as an obstacle, so a
        // Guardian could close to an adjacent cell and never arrive. Capture was
        // unreachable and `resolve_captures` was dead code.
        let mut world = KineticWorld::authored();
        world.minors.truncate(1);
        world.observers[0].cell = coord(4, 4);
        world.observers[0].facing = HexFace::West;
        world.minors[0].cell = coord(6, 4);
        world.major.cell = coord(1, 2);

        // Events are per-tick, so the capture must be caught as it happens.
        let mut captures = Vec::new();
        for _ in 0..MINOR_STEP_TICKS * 3 {
            world.step(&[]);
            captures.extend(world.events.iter().copied().filter(|event| {
                matches!(
                    event,
                    KineticEvent::ObserverCaptured {
                        by_major: false,
                        ..
                    }
                )
            }));
        }

        assert_eq!(
            world.minors[0].cell,
            coord(4, 4),
            "it arrives on the target"
        );
        assert!(world.observers[0].jailed);
        assert_eq!(captures.len(), 1, "captured once, not once per tick");
    }

    #[test]
    fn guardians_still_refuse_to_walk_through_each_other() {
        let mut world = KineticWorld::authored();
        world.observers[0].cell = coord(4, 4);
        world.minors[0].cell = coord(6, 4);
        world.minors[1].cell = coord(5, 4);
        world.major.cell = coord(1, 2);

        for _ in 0..MINOR_STEP_TICKS {
            world.step(&[]);
        }

        assert_ne!(
            world.minors[0].cell, world.minors[1].cell,
            "two Guardians never share a cell"
        );
    }

    #[test]
    fn launch_flags_parse_and_ignore_everything_else() {
        assert_eq!(
            KineticRules::from_args::<[&str; 0], _>([]),
            KineticRules::default()
        );

        let rules = KineticRules::from_args(["--no-jail"]);
        assert!(rules.minors && rules.major && !rules.jail);

        let rules = KineticRules::from_args(["--no-guardians"]);
        assert!(!rules.minors && !rules.major && rules.jail);

        let rules = KineticRules::from_args(["--no-minors", "--no-jail"]);
        assert!(!rules.minors && rules.major && !rules.jail);

        // Bevy, cargo and the capture harness all put their own arguments on
        // this command line, so an unknown one is never an error.
        let rules = KineticRules::from_args(["target/debug/kinetic_fps", "--verbose", "-Zfoo"]);
        assert_eq!(rules, KineticRules::default());
    }

    #[test]
    fn no_minors_leaves_none_in_play() {
        let world = KineticWorld::authored_with(KineticRules {
            minors: false,
            ..Default::default()
        });
        assert_eq!(world.living_minors(), 0);
        assert!(world.minors.is_empty());
    }

    /// A disabled major must be *out of play*, not merely invisible: it cannot
    /// move, cannot capture, and cannot block a shove from a plate it is not
    /// really standing on.
    #[test]
    fn no_major_takes_it_out_of_play_entirely() {
        let mut world = KineticWorld::authored_with(KineticRules {
            major: false,
            ..Default::default()
        });
        world.minors.clear();
        world.observers[0].cell = coord(4, 4);
        let parked = coord(5, 4);
        world.major.cell = parked;

        for _ in 0..MAJOR_STEP_TICKS * 4 {
            world.step(&[]);
        }
        assert_eq!(world.major.cell, parked, "a disabled major moved");
        assert!(!world.observers[0].jailed, "a disabled major captured");

        // And it does not stand in the way of a shove passing through its cell.
        world.minors.push(MinorGuardian {
            id: MinorGuardianId(9),
            cell: coord(5, 4),
            stagger: 0,
            step_progress: 0,
            alive: true,
        });
        world.major.cell = coord(6, 4);
        let resolution = world
            .resolve_shove(MinorGuardianId(9), HexFace::East, PUSH_IMPULSE)
            .expect("resolved");
        assert_ne!(
            resolution.fate,
            ShoveFate::Blocked,
            "a disabled major blocked a shove"
        );
    }

    /// The point of `--no-jail`: a Guardian may reach you and the run continues.
    #[test]
    fn no_jail_lets_a_guardian_reach_you_without_ending_the_run() {
        let mut world = KineticWorld::authored_with(KineticRules {
            jail: false,
            ..Default::default()
        });
        world.minors.truncate(1);
        world.observers[0].cell = coord(4, 4);
        world.minors[0].cell = coord(6, 4);
        world.major.cell = coord(1, 2);

        for _ in 0..MINOR_STEP_TICKS * 4 {
            world.step(&[]);
            assert!(
                !world.observers[0].jailed,
                "jail was switched off and the run ended anyway"
            );
            assert!(
                !world
                    .events
                    .iter()
                    .any(|event| matches!(event, KineticEvent::ObserverCaptured { .. })),
                "jail was switched off and a capture still resolved"
            );
        }
        // It really did arrive — otherwise this proves nothing.
        assert_eq!(world.minors[0].cell, coord(4, 4));
    }

    #[test]
    fn the_rules_summary_names_only_what_is_off() {
        assert_eq!(KineticRules::default().summary(), "all on");
        assert_eq!(
            KineticRules {
                jail: false,
                ..Default::default()
            }
            .summary(),
            "off: jail"
        );
        assert_eq!(
            KineticRules {
                minors: false,
                major: false,
                jail: false,
                ..Default::default()
            }
            .summary(),
            "off: minors, major, jail"
        );
        assert_eq!(
            KineticRules {
                siege: true,
                siege_minutes: 2.0,
                ..Default::default()
            }
            .summary(),
            "siege 2m, all on"
        );
    }

    #[test]
    fn identical_intents_reproduce_identical_state_every_tick() {
        let script = |tick: u32| -> KineticIntent {
            match tick % 7 {
                0 => KineticIntent::Face(HexFace::East),
                1 => KineticIntent::Step(HexFace::East),
                2 => KineticIntent::Push,
                3 => KineticIntent::Pull,
                4 => KineticIntent::Step(HexFace::SouthWest),
                5 => KineticIntent::ToggleGenerator,
                _ => KineticIntent::Idle,
            }
        };

        let mut left = KineticWorld::authored();
        let mut right = KineticWorld::authored();
        for tick in 0..600 {
            let intents = [(PlayerId(0), script(tick))];
            left.step(&intents);
            right.step(&intents);
            assert_eq!(
                left.digest(),
                right.digest(),
                "divergence at tick {tick}: {:?} vs {:?}",
                left.events,
                right.events
            );
        }
        assert_eq!(left, right);
    }

    #[test]
    fn resolving_a_shove_never_mutates_the_world() {
        let world = KineticWorld::authored();
        let before = world.clone();
        for face in HexFace::LATERAL {
            for impulse in 0..=PUSH_IMPULSE {
                let _ = world.resolve_shove(MinorGuardianId(0), face, impulse);
            }
        }
        assert_eq!(world, before);
    }

    #[test]
    fn a_ledge_ring_cannot_spin_a_shove_forever() {
        // Ring the whole interior with ledge so momentum never settles, then
        // confirm the travel cap ends the walk instead of hanging the tick.
        let mut world = KineticWorld::authored();
        for q in 1..world.grid.cols - 1 {
            for r in 1..world.grid.rows - 1 {
                let index = world.grid.index(coord(q, r));
                world.cells[index] = CellKind::Ledge;
            }
        }
        world.minors.truncate(1);
        world.minors[0].cell = coord(4, 3);
        world.major.cell = coord(1, 1);
        world.observers[0].cell = coord(1, 2);

        let resolution = world
            .resolve_shove(MinorGuardianId(0), HexFace::East, PUSH_IMPULSE)
            .expect("resolved");
        // It runs east across ledge until the rim, which is void.
        assert_eq!(resolution.fate, ShoveFate::Void);
        assert!(resolution.cells_travelled <= MAX_TRAVEL_CELLS);
    }
}
