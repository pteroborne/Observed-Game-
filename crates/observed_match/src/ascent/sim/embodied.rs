//! The rules over a first-person facility, with Observers who have bodies.
//!
//! A lab board moves an Observer one cell a beat. A first-person Observer is a body the
//! physical match moves, and the rules take its cell and facing from that body every
//! tick: they never step it, never fall it and never hand it to a behaviour tree. The
//! facility is the one the bodies walk in, so the rules' writes to it must be ones the
//! authored corpus can build, and they are kept for the host to commit physically.

use std::collections::BTreeMap;

use observed_facility::hex_wfc::{HexPlacement, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass};

use super::{
    ArchitectLab, ArchitectMode, Deck, EconomyState, Observer, ObserverId, ObserverState, Parts,
    TeamId, TileShape,
};

/// One Observer's body, as the rules first see it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Embodiment {
    pub id: ObserverId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub facing: HexFace,
}

impl ArchitectLab {
    /// The rules over `world`, a facility bodies walk in, with every Observer embodied.
    ///
    /// No Guardian is placed: the physical match's Guardian hunts, and its catch reaching
    /// the rules is the prison's work. The mode is a lab scenario label and means nothing
    /// here; nothing in the rules reads it.
    #[must_use]
    pub fn over_facility(world: HexWfcWorld, seed: u64, bodies: &[Embodiment]) -> Self {
        let prison = crate::ascent::prison::PrisonState::new(world.config, &world);
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
        let economy = EconomyState::new(&world, &observers, &prison_core, seed);
        let levels = world.config.levels;
        let mut lab = Self::assemble(Parts {
            mode: ArchitectMode::FullAscent,
            seed,
            world,
            deck: Deck::with_shapes(seed, levels, &TileShape::AUTHORED),
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

    /// A fresh deck of the shapes this match can build.
    #[must_use]
    pub fn new_deck(&self, seed: u64) -> Deck {
        let levels = self.world.config.levels;
        if self.authored {
            Deck::with_shapes(seed, levels, &TileShape::AUTHORED)
        } else {
            Deck::for_levels(seed, levels)
        }
    }

    /// The single path by which the rules change the facility.
    pub(crate) fn rewrite(&mut self, placement: HexPlacement) {
        let cell = placement.coord;
        self.world.placements.insert(cell, placement);
        *self.world.cell_revisions.entry(cell).or_default() += 1;
        if self.authored {
            self.rewrites.insert(cell, placement);
            // The physical commit re-derives open air from the new shape; so do the
            // rules, or the two facilities would disagree about which rock is sky.
            let _ = self.world.mark_open_air();
        }
    }

    /// Everything rewritten since the last call, for the host to build.
    pub(crate) fn take_rewrites(&mut self) -> BTreeMap<HexCoord, HexPlacement> {
        std::mem::take(&mut self.rewrites)
    }

    /// A stamped room, or a cell a stair or ramp links vertically. A first-person
    /// facility builds these whole; a lab board has no such thing.
    #[must_use]
    pub fn fixed_structure(&self, cell: HexCoord) -> bool {
        if !self.authored {
            return false;
        }
        let vertical = |at: Option<HexCoord>, face: HexFace| {
            at.and_then(|at| self.world.placements.get(&at))
                .is_some_and(|p| p.ports().port(face) != PortClass::Sealed)
        };
        let grid = self.world.config.grid();
        self.world
            .blueprints
            .iter()
            .any(|blueprint| blueprint.cells.contains(&cell))
            || vertical(Some(cell), HexFace::Up)
            || vertical(Some(cell), HexFace::Down)
            || vertical(grid.neighbor(cell, HexFace::Down), HexFace::Up)
            || vertical(grid.neighbor(cell, HexFace::Up), HexFace::Down)
    }

    /// Put an Observer where its body is. A corrupted Observer has left play and no
    /// longer has a body the rules follow.
    pub(crate) fn embody(&mut self, id: ObserverId, cell: HexCoord, facing: HexFace) {
        if let Some(observer) = self.observers.get_mut(&id)
            && observer.state != ObserverState::Corrupted
        {
            observer.cell = cell;
            observer.facing = facing;
        }
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
