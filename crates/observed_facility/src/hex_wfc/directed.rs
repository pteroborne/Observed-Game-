//! Directed rewrites: cells a player chose, committed as they were chosen.
//!
//! A relayout is the facility solving a pocket for itself, and its commit refuses
//! anything that does not fit: every boundary port must match and every route must
//! survive. An Architect's card play is the opposite kind of change. The rules have
//! already decided it is legal, and a play that fits locally while contradicting the
//! wider facility is accepted on purpose, because the contradiction is play rather than
//! an error (`docs/architect_ascent_design.md`, section 3).
//!
//! So a directed rewrite skips those checks and keeps everything else a relayout
//! commit does: the generation advances, each changed cell's revision is bumped, open
//! air is re-derived from the new shape, and the result is the same
//! [`HexRelayoutDelta`] geometry, physics and presentation already consume.

use std::collections::{BTreeMap, BTreeSet};

use observed_hex::{HexCoord, PortClass};

use super::variants::hall_archetype;
use super::{
    HexMutationRegion, HexPlacement, HexRelayoutDelta, HexSpace, HexWfcError, HexWfcWorld,
};

/// The flat hall cell with exactly these lateral doors, if the authored corpus has one.
///
/// Two to four doors: a straight, a turn or a junction. The corpus has no one-door flat
/// hall, and past four an opening is an expanse rather than a corridor.
#[must_use]
pub fn authored_hall(coord: HexCoord, doors: u8) -> Option<HexPlacement> {
    let doors = doors & 0b11_1111;
    (2..=4).contains(&doors.count_ones()).then(|| HexPlacement {
        coord,
        space: HexSpace::Hall,
        archetype: hall_archetype(doors),
        doors,
        up: PortClass::Sealed,
        down: PortClass::Sealed,
    })
}

impl HexWfcWorld {
    /// Commit `placements` exactly as given, as one generation.
    ///
    /// Refuses a cell outside the lattice and a cell of a stamped room: a room is
    /// projected whole from its blueprint, and one cell of it cannot be rewritten alone.
    /// Nothing is changed when it refuses. Cells whose placement is already the one
    /// given are not reported as changed.
    pub fn commit_directed_delta(
        &mut self,
        placements: BTreeMap<HexCoord, HexPlacement>,
    ) -> Result<HexRelayoutDelta, HexWfcError> {
        if placements.is_empty() {
            return Err(HexWfcError::NoMutationRegion);
        }
        for (&coord, placement) in &placements {
            if placement.coord != coord
                || !self.placements.contains_key(&coord)
                || self
                    .blueprints
                    .iter()
                    .any(|blueprint| blueprint.cells.contains(&coord))
            {
                return Err(HexWfcError::UnsafeChange(coord));
            }
        }
        let cells: BTreeSet<HexCoord> = placements.keys().copied().collect();
        let previous_placements: BTreeMap<_, _> = cells
            .iter()
            .map(|&coord| (coord, self.placements[&coord]))
            .collect();
        let architecture: BTreeMap<_, _> = cells
            .iter()
            .map(|&coord| (coord, self.architecture[&coord]))
            .collect();
        let changed_cells: BTreeSet<HexCoord> = placements
            .iter()
            .filter(|&(coord, placement)| !same_shape(&previous_placements[coord], placement))
            .map(|(&coord, _)| coord)
            .collect();
        for (coord, placement) in placements {
            self.placements.insert(coord, placement);
        }
        if self.open_air {
            let _ = self.mark_open_air();
        }
        let previous_generation = self.generation;
        let previous_attempts = self.last_attempts;
        self.generation = self.generation.wrapping_add(1);
        self.last_attempts = 0;
        let mut cell_revisions = BTreeMap::new();
        let mut previous_cell_revisions = BTreeMap::new();
        for &coord in &changed_cells {
            let revision = self.cell_revisions.entry(coord).or_default();
            previous_cell_revisions.insert(coord, *revision);
            *revision = revision.wrapping_add(1);
            cell_revisions.insert(coord, *revision);
        }
        Ok(HexRelayoutDelta {
            previous_generation,
            generation: self.generation,
            previous_attempts,
            region: HexMutationRegion {
                cells,
                boundary_cells: BTreeSet::new(),
                protected_cells: BTreeSet::new(),
            },
            placements: changed_cells
                .iter()
                .map(|&coord| (coord, self.placements[&coord]))
                .collect(),
            architecture: architecture
                .iter()
                .filter(|(coord, _)| changed_cells.contains(coord))
                .map(|(&coord, &register)| (coord, register))
                .collect(),
            changed_cells,
            cell_revisions,
            previous_placements,
            previous_architecture: architecture,
            previous_cell_revisions,
            previous_blueprints: self.blueprints.clone(),
            removed_blueprints: Vec::new(),
            upserted_blueprints: Vec::new(),
        })
    }
}

/// Whether two placements build the same thing. Rock and open air are both nothing
/// built, and the air classification is re-derived after the write rather than given.
fn same_shape(a: &HexPlacement, b: &HexPlacement) -> bool {
    let unbuilt = |p: &HexPlacement| matches!(p.space, HexSpace::Void | HexSpace::Air);
    if unbuilt(a) && unbuilt(b) {
        return true;
    }
    a.space == b.space
        && a.archetype == b.archetype
        && a.doors == b.doors
        && a.up == b.up
        && a.down == b.down
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::{HexWfcConfig, geometry_demands, placement_tile_archetype};

    fn world() -> HexWfcWorld {
        let mut world = HexWfcWorld::generate(
            7,
            HexWfcConfig {
                cols: 10,
                rows: 8,
                levels: 2,
                min_rooms: 5,
                max_rooms: 7,
                retry_budget: 100,
                min_room_distance: 2,
            },
        )
        .expect("the lab's full-ascent facility solves");
        let _ = world.mark_open_air();
        world
    }

    fn open_hall(world: &HexWfcWorld) -> HexCoord {
        *world
            .placements
            .iter()
            .find(|(coord, p)| {
                p.space == HexSpace::Hall
                    && p.up == PortClass::Sealed
                    && p.down == PortClass::Sealed
                    && !world.blueprints.iter().any(|b| b.cells.contains(coord))
            })
            .expect("a solved facility has a flat hall")
            .0
    }

    #[test]
    fn every_authored_hall_is_one_the_corpus_is_required_to_build() {
        let demands = geometry_demands();
        let mut built = 0;
        for doors in 0u8..64 {
            let Some(placement) = authored_hall(HexCoord::default(), doors) else {
                assert!(!(2..=4).contains(&doors.count_ones()), "mask {doors:06b}");
                continue;
            };
            let archetype = placement_tile_archetype(&placement).expect("a flat hall draws");
            assert!(
                demands
                    .iter()
                    .any(|d| d.archetype == archetype && d.signature == placement.ports()),
                "{archetype} {doors:06b} is not a geometry demand"
            );
            built += 1;
        }
        assert_eq!(built, 15 + 20 + 15, "every two-, three- and four-door mask");
    }

    #[test]
    fn a_directed_rewrite_commits_a_contradiction_a_relayout_would_refuse() {
        let mut world = world();
        let cell = open_hall(&world);
        let before = world.clone();
        // One door, rotated off whatever the neighbours offer: a relayout would refuse
        // the mismatched boundary, and the rules accept it as a contradiction.
        let doors = world.placements[&cell].doors;
        let rotated = ((doors << 1) | (doors >> 5)) & 0b11_1111;
        let placement = authored_hall(cell, rotated).expect("same degree, still authored");
        let delta = world
            .commit_directed_delta(BTreeMap::from([(cell, placement)]))
            .expect("a directed rewrite is not validated against its boundary");

        assert_eq!(world.placements[&cell].doors, rotated);
        assert_eq!(delta.generation, before.generation + 1);
        assert_eq!(delta.changed_cells, BTreeSet::from([cell]));
        assert_eq!(
            world.cell_revisions.get(&cell).copied().unwrap_or_default(),
            before
                .cell_revisions
                .get(&cell)
                .copied()
                .unwrap_or_default()
                + 1
        );

        world.revert_relayout_delta(delta).expect("revertible");
        assert_eq!(world.placements, before.placements);
        assert_eq!(
            world.cell_revisions.get(&cell).copied().unwrap_or_default(),
            before
                .cell_revisions
                .get(&cell)
                .copied()
                .unwrap_or_default()
        );
        assert_eq!(world.generation, before.generation);
    }

    #[test]
    fn retracting_a_cell_to_rock_re_derives_the_air_around_it() {
        let mut world = world();
        // A built cell on the lattice edge: once it is gone, the outside reaches in.
        let (&cell, _) = world
            .placements
            .iter()
            .find(|(coord, p)| {
                p.space.built()
                    && coord.level == 0
                    && (coord.q == 0 || coord.r == 0)
                    && !world.blueprints.iter().any(|b| b.cells.contains(coord))
            })
            .expect("something is built on the edge");
        let rock = HexPlacement {
            coord: cell,
            space: HexSpace::Void,
            archetype: crate::hex_wfc::HexArchetype::Void,
            doors: 0,
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        };
        let delta = world
            .commit_directed_delta(BTreeMap::from([(cell, rock)]))
            .expect("commits");
        assert_eq!(world.placements[&cell].space, HexSpace::Air);
        assert_eq!(delta.placements[&cell].space, HexSpace::Air);
    }

    #[test]
    fn a_stamped_room_cell_is_refused_and_nothing_changes() {
        let mut world = world();
        let room = world.blueprints[0].cells[0];
        let before = world.clone();
        let placement = authored_hall(room, 0b00_1001).expect("a straight");
        assert_eq!(
            world.commit_directed_delta(BTreeMap::from([(room, placement)])),
            Err(HexWfcError::UnsafeChange(room))
        );
        assert_eq!(world.placements, before.placements);
        assert_eq!(world.generation, before.generation);
    }
}
