//! The rules over a first-person facility, with Observers who have bodies.
//!
//! A lab board moves an Observer one cell a beat. A first-person Observer is a body the
//! physical match moves, and the rules take its cell and facing from that body every
//! tick: they never step it, never fall it and never hand it to a behaviour tree. The
//! facility is the one the bodies walk in, so the rules' writes to it must be ones the
//! authored corpus can build, and they are kept for the host to commit physically.

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::{HexPlacement, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass};

use super::{
    ArchitectLab, ArchitectMode, Deck, EconomyState, GuardianId, GuardianKind, LabEventKind,
    Observer, ObserverId, ObserverState, Parts, TeamId, TileShape,
};

/// Where an embodied Observer's body is: the first-person match's own answer.
pub type Place = crate::hex_wfc::HexBodyPlace;

/// One Observer's body, as the rules first see it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Embodiment {
    pub id: ObserverId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub facing: HexFace,
}

/// Whether a stair or ramp in `world` links `cell` to the floor above or below it.
///
/// Every cell of a climb composition does: its foot and mid carry the flight to the
/// high cell as surely as the high cell carries it through the floor, though only the
/// high cell and its landing have a vertical port to show for it.
#[must_use]
pub fn linked_vertically(world: &HexWfcWorld, cell: HexCoord) -> bool {
    let vertical = |at: Option<HexCoord>, face: HexFace| {
        at.and_then(|at| world.placements.get(&at))
            .is_some_and(|p| p.ports().port(face) != PortClass::Sealed)
    };
    let grid = world.config.grid();
    let climbing = world.placements.get(&cell).is_some_and(|placement| {
        matches!(
            placement.archetype,
            observed_facility::hex_wfc::HexArchetype::Climb { .. }
        )
    });
    climbing
        || vertical(Some(cell), HexFace::Up)
        || vertical(Some(cell), HexFace::Down)
        || vertical(grid.neighbor(cell, HexFace::Down), HexFace::Up)
        || vertical(grid.neighbor(cell, HexFace::Up), HexFace::Down)
}

/// Stair cards in each district of an Architect's deck on the real facility.
pub const STAIRS_PER_DISTRICT: u8 = 3;
/// Copies of each authored wonder in its district: Cistern and Chargeworks.
pub const WONDERS_PER_DISTRICT: u8 = 1;

impl ArchitectLab {
    /// The rules over `world`, a facility bodies walk in, with every Observer embodied.
    ///
    /// No Guardian is placed: the physical match's Guardian hunts, and its catch reaching
    /// the rules is the prison's work. The mode is a lab scenario label and means nothing
    /// here; nothing in the rules reads it.
    /// `lobby` is the prison lobby's cells and the cell a released body appears in: the
    /// rules' prison core, which no card rewrites. `site` places each floor's generator
    /// and station, which on a first-person facility must stand where a body can
    /// (`ascent::facility::power`), given the facility, its Observers and the prison core.
    #[must_use]
    pub fn over_facility(
        world: HexWfcWorld,
        seed: u64,
        bodies: &[Embodiment],
        lobby: (BTreeSet<HexCoord>, HexCoord),
        site: impl FnOnce(
            &HexWfcWorld,
            &BTreeMap<ObserverId, Observer>,
            &BTreeSet<HexCoord>,
        ) -> EconomyState,
    ) -> Self {
        let prison = crate::ascent::prison::PrisonState::lobby(lobby.0, lobby.1);
        let prison_core = prison.cells.clone();
        let observers: BTreeMap<_, _> = bodies
            .iter()
            .map(|body| {
                (
                    body.id,
                    Observer {
                        id: body.id,
                        team: body.team,
                        cell: body.cell,
                        facing: body.facing,
                        state: ObserverState::Active,
                        hold_beats: 0,
                    },
                )
            })
            .collect();
        let mut per_team: BTreeMap<TeamId, usize> = BTreeMap::new();
        for body in bodies {
            *per_team.entry(body.team).or_default() += 1;
        }
        let known = world
            .placements
            .keys()
            .copied()
            .chain(prison_core.iter().copied())
            .collect();
        let economy = site(&world, &observers, &prison_core);
        let levels = world.config.levels;
        let mut lab = Self::assemble(Parts {
            mode: ArchitectMode::FullAscent,
            seed,
            world,
            deck: Deck::rogue(seed, levels, &TileShape::AUTHORED),
            known,
            prison_core,
            prison,
            loyal_team_size: per_team.values().copied().max().unwrap_or(1).clamp(1, 3),
            observers,
            guardians: BTreeMap::new(),
            economy,
        });
        lab.authored = true;
        lab.embodied = bodies.iter().map(|body| body.id).collect();
        lab.refresh_observation();
        lab
    }

    /// Whether these rules run over a first-person facility rather than a lab board.
    #[must_use]
    pub const fn is_authored(&self) -> bool {
        self.authored
    }

    /// A fresh Rogue deck of the shapes this match can build: a player who joins the Rogue
    /// is dealt one. On a lab board, the lab's deck.
    #[must_use]
    pub fn new_rogue_deck(&self, seed: u64) -> Deck {
        if self.authored {
            Deck::rogue(seed, self.world.config.levels, &TileShape::AUTHORED)
        } else {
            Deck::for_levels(seed, self.world.config.levels)
        }
    }

    /// A fresh deck of the shapes this match can build.
    #[must_use]
    pub fn new_deck(&self, seed: u64) -> Deck {
        let levels = self.world.config.levels;
        if self.authored {
            // The real facility's climb is gated on stairs, so an Architect who cannot lay
            // one cannot build the way up: three to a district, and one wonder in each district that has one.
            Deck::for_team_with_wonders(
                seed,
                levels,
                &TileShape::AUTHORED,
                STAIRS_PER_DISTRICT,
                WONDERS_PER_DISTRICT,
            )
        } else {
            Deck::for_levels(seed, levels)
        }
    }

    /// The single path by which the rules change the facility.
    pub(crate) fn rewrite(&mut self, placement: HexPlacement) {
        let cell = placement.coord;
        // A station is deployable equipment on the old tile, not permanent
        // structure inherited by the replacement or surviving a retraction.
        self.economy.stations.remove(&cell);
        self.station_sites.remove(&cell);
        self.world.placements.insert(cell, placement);
        *self.world.cell_revisions.entry(cell).or_default() += 1;
        if self.authored {
            self.rewrites.insert(cell, placement);
            // The physical commit re-derives open air from the new shape; so do the
            // rules, or the two facilities would disagree about which rock is sky.
            let _ = self.world.mark_open_air();
        }
    }

    /// Put a released Guardian into the facility: on a lab board, straight into the
    /// rules; on a first-person facility, into the host's hands to give a body
    /// (`take_releases`), after which the rules follow that body.
    pub(crate) fn release(&mut self, guardian: super::Guardian) {
        if self.authored {
            self.releases.push(guardian);
        } else {
            self.guardians.insert(guardian.id, guardian);
        }
    }

    /// Every Guardian released since the last call, for the host to give a body.
    pub(crate) fn take_releases(&mut self) -> Vec<super::Guardian> {
        std::mem::take(&mut self.releases)
    }

    /// Forget every Guardian the host moved whose body `has_body` no longer finds: lost
    /// to the void, taken with a collapsed floor, or a Guardian that stopped hunting.
    pub(crate) fn retire_embodied_guardians(&mut self, has_body: impl Fn(GuardianId) -> bool) {
        let gone: Vec<GuardianId> = self
            .embodied_guardians
            .iter()
            .copied()
            .filter(|&id| !has_body(id))
            .collect();
        for id in gone {
            self.embodied_guardians.remove(&id);
            self.guardians.remove(&id);
        }
    }

    /// Everything rewritten since the last call, for the host to build.
    pub(crate) fn take_rewrites(&mut self) -> BTreeMap<HexCoord, HexPlacement> {
        std::mem::take(&mut self.rewrites)
    }

    /// A stamped room, a cell a stair or ramp links vertically, or a floor's
    /// generator. A placed recharge station remains mutable with its tile.
    #[must_use]
    pub fn fixed_structure(&self, cell: HexCoord) -> bool {
        if !self.authored {
            return false;
        }
        self.world
            .blueprints
            .iter()
            .any(|blueprint| blueprint.cells.contains(&cell))
            || self.linked_vertically(cell)
            || self.economy.is_at_generator(cell)
    }

    /// Whether a stair or ramp links `cell` to the floor above or below it.
    #[must_use]
    pub fn linked_vertically(&self, cell: HexCoord) -> bool {
        linked_vertically(&self.world, cell)
    }

    /// Put a Guardian the host moves and catches with where its body is: one of `kind` on
    /// `cell` while it `hunts`, and gone from the rules while it does not. The rules never
    /// move it or catch with it themselves (`embodied_guardians`); they see it, route
    /// against it, and show it to whoever can.
    pub(crate) fn embody_guardian(
        &mut self,
        id: GuardianId,
        kind: GuardianKind,
        cell: HexCoord,
        hunts: bool,
    ) {
        if !hunts {
            self.guardians.remove(&id);
            self.embodied_guardians.remove(&id);
            return;
        }
        self.embodied_guardians.insert(id);
        let last_detection = self.guardians.get(&id).and_then(|g| g.last_detection);
        self.guardians.insert(
            id,
            super::Guardian {
                id,
                cell,
                last_detection,
                kind,
            },
        );
    }

    /// What an embodied Observer's body sees (`hex_wfc::sight`), or `None` for a body that
    /// sees nothing of the facility. It wards and knows by this rather than by lines along
    /// its facing.
    pub(crate) fn see(&mut self, id: ObserverId, sight: Option<BTreeMap<HexCoord, f32>>) {
        match sight {
            Some(sight) => {
                self.sight.insert(id, sight);
            }
            None => {
                self.sight.remove(&id);
            }
        }
    }

    /// Put an Observer where its body is, in the state its body's place implies.
    ///
    /// A jailed body is in the prison, which the rules see as the lobby it will come out
    /// of; its maze cell means nothing here. A lost body fell into true void and has
    /// corrupted, which is permanent: a corrupted Observer has left play and no longer
    /// has a body the rules follow.
    pub(crate) fn embody(&mut self, id: ObserverId, cell: HexCoord, facing: HexFace, place: Place) {
        let lobby = self.prison.lowest_cell;
        let Some(observer) = self.observers.get_mut(&id) else {
            return;
        };
        let before = observer.state;
        if before == ObserverState::Corrupted {
            return;
        }
        observer.facing = facing;
        (observer.state, observer.cell) = match place {
            Place::Facility => (ObserverState::Active, cell),
            Place::Prison => (ObserverState::Jailed, lobby),
            Place::Void => (ObserverState::Corrupted, cell),
        };
        let after = observer.state;
        let at = observer.cell;
        let message = match (before, after) {
            (ObserverState::Active, ObserverState::Jailed) => {
                format!("Observer {} was caught and wakes in the prison.", id.0)
            }
            (ObserverState::Jailed, ObserverState::Active) => {
                format!("Observer {} is out of the prison.", id.0)
            }
            (_, ObserverState::Corrupted) => format!(
                "Observer {} fell into true void and corrupted into Rogue AI.",
                id.0
            ),
            _ => return,
        };
        let kind = match after {
            ObserverState::Jailed => LabEventKind::Captured,
            ObserverState::Corrupted => LabEventKind::Corrupted,
            ObserverState::Active => LabEventKind::Released,
        };
        self.record_event(kind, Some(at), &message);
    }

    /// In a built facility a hall's open edges and a room's windows are drawn from its
    /// neighbours (`exposure::follows_neighbours`), so rewriting the cell beside a warded
    /// one would redraw the warded one in front of whoever is watching it. Those
    /// neighbours are warded too. A lab board draws nothing, and gains nothing.
    pub(crate) fn ward_what_would_redraw(&mut self) {
        if !self.authored {
            return;
        }
        let grid = self.world.config.grid();
        let redraw: Vec<HexCoord> = self
            .observed
            .iter()
            .filter(|cell| {
                self.world
                    .placements
                    .get(cell)
                    .is_some_and(observed_facility::hex_wfc::exposure::follows_neighbours)
            })
            .flat_map(|&cell| {
                HexFace::LATERAL
                    .into_iter()
                    .filter_map(move |face| grid.neighbor(cell, face))
            })
            .collect();
        self.observed.extend(redraw);
    }
}
