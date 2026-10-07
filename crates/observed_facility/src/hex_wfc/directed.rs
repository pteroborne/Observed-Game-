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

use observed_hex::{HexCoord, HexFace, PortClass};

use super::variants::hall_archetype;
use super::{
    ClimbPart, ClimbTurn, HexArchetype, HexMutationRegion, HexPlacement, HexRelayoutDelta,
    HexSpace, HexWfcError, HexWfcWorld,
};

/// The flat hall cell with exactly these lateral doors, if the authored corpus has one.
///
/// Two to four doors: a straight, a turn or a junction. The corpus has no one-door flat
/// hall, and past four an opening is an expanse rather than a corridor.
#[must_use]
pub fn authored_hall(coord: HexCoord, doors: u8) -> Option<HexPlacement> {
    let doors = doors & 0b11_1111;
    (2..=4).contains(&doors.count_ones()).then(|| HexPlacement {
        low_doors: if coord.level == 0 { doors } else { 0 },
        coord,
        space: HexSpace::Hall,
        archetype: hall_archetype(doors),
        doors,
        up: PortClass::Sealed,
        down: PortClass::Sealed,
    })
}

/// The straight climb composition with its foot at `foot`, climbing toward `heading`
/// (`docs/climb_compositions_plan.md`): foot, mid and high along the heading on the
/// foot's storey, entered straight on, and the landing above the high cell, left
/// straight on. `None` where it would leave the lattice, or for a vertical heading.
#[must_use]
pub fn authored_climb(
    config: super::HexWfcConfig,
    foot: HexCoord,
    heading: HexFace,
) -> Option<[HexPlacement; 4]> {
    authored_climb_shaped(config, foot, heading, ClimbTurn::Ahead, ClimbTurn::Ahead)
}

/// Decompose a stair rotation (0..120) into its lateral heading, flight turn at mid,
/// and exit at landing (`docs/climb_compositions_plan.md`).
///
/// Rotations 0..6 encode the six lateral headings with straight flight (Ahead, Ahead),
/// ensuring full backwards compatibility with legacy callers.
#[must_use]
pub fn stair_shape(rotation: u8) -> (HexFace, ClimbTurn, ClimbTurn) {
    let heading = HexFace::LATERAL[(rotation % 6) as usize];
    let shape_index = ((rotation / 6) % 20) as usize;
    let turn = ClimbTurn::BENDS[shape_index % 5];
    let exit = ClimbTurn::EXITS[shape_index / 5];
    (heading, turn, exit)
}

/// Compose a lateral heading, mid turn and landing exit into a stair rotation (0..120).
#[must_use]
pub fn stair_rotation(heading: HexFace, turn: ClimbTurn, exit: ClimbTurn) -> u8 {
    let heading_index = HexFace::LATERAL
        .iter()
        .position(|&f| f == heading)
        .unwrap_or(0);
    let bend_index = ClimbTurn::BENDS
        .iter()
        .position(|&b| b == turn)
        .unwrap_or(0);
    let exit_index = ClimbTurn::EXITS
        .iter()
        .position(|&e| e == exit)
        .unwrap_or(0);
    let shape_index = exit_index * 5 + bend_index;
    (shape_index * 6 + heading_index) as u8
}

/// The climb composition with its foot at `foot`, entered climbing toward `heading`,
/// its flight turned by `turn` in the mid cell and its landing left by `exit`, both
/// from the heading the flight has there. `None` where it would leave the lattice,
/// for a vertical heading, or for a turn or exit the catalogue does not build.
#[must_use]
pub fn authored_climb_shaped(
    config: super::HexWfcConfig,
    foot: HexCoord,
    heading: HexFace,
    turn: ClimbTurn,
    exit: ClimbTurn,
) -> Option<[HexPlacement; 4]> {
    if !heading.is_lateral()
        || !ClimbTurn::BENDS.contains(&turn)
        || !ClimbTurn::EXITS.contains(&exit)
    {
        return None;
    }
    let grid = config.grid();
    let mid = grid.neighbor(foot, heading)?;
    let rising = turn.apply(heading);
    let high = grid.neighbor(mid, rising)?;
    let landing = grid.neighbor(high, HexFace::Up)?;
    let cell = |coord: HexCoord, part, heading, doors, up, down| HexPlacement {
        low_doors: if coord.level == 0 {
            doors & !HexArchetype::Climb { part, heading }.span_mask()
        } else {
            0
        },
        coord,
        space: HexSpace::Hall,
        archetype: HexArchetype::Climb { part, heading },
        doors,
        up,
        down,
    };
    let bit = |face: HexFace| 1u8 << face.index();
    let sealed = PortClass::Sealed;
    let back = bit(heading.opposite());
    Some([
        cell(
            foot,
            ClimbPart::Foot,
            heading,
            back | bit(heading),
            sealed,
            sealed,
        ),
        cell(
            mid,
            ClimbPart::Mid { turn },
            heading,
            back | bit(rising),
            sealed,
            sealed,
        ),
        cell(
            high,
            ClimbPart::High,
            rising,
            bit(rising.opposite()),
            PortClass::RampOpen,
            sealed,
        ),
        cell(
            landing,
            ClimbPart::Landing { exit },
            rising,
            bit(exit.apply(rising)),
            sealed,
            PortClass::RampOpen,
        ),
    ])
}

/// Three sectors of one reservoir. Internal seams are full-height spans, while
/// the three external thresholds face out at 120-degree intervals. The geometry
/// selects the exact authored orientation for each sector's span signature.
#[must_use]
pub fn authored_cistern_room(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::Cistern { heading }
    })
}

/// One card places a fabrication bay, transfer floor and receiving vault.
#[must_use]
pub fn authored_chargeworks(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    use super::ChargeworksPart::{Fabricator, Receiver, Transfer};
    authored_triad(config, anchor, rotation, |offset, heading| {
        HexArchetype::Chargeworks {
            part: [Fabricator, Transfer, Receiver][offset / 2],
            heading,
        }
    })
}

/// One Library card places the three sectors of a continuous reading well.
#[must_use]
pub fn authored_archive_well(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::ArchiveWell { heading }
    })
}

/// One Zen card places three sheltered sectors around a continuous rain garden.
#[must_use]
pub fn authored_rain_court(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::RainCourt { heading }
    })
}

/// One Lumen card places a continuous three-bay transit hall.
#[must_use]
pub fn authored_switching_concourse(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::SwitchingConcourse { heading }
    })
}

/// One Monument card places three joined bays with a raised crossing circuit.
#[must_use]
pub fn authored_jade_nave(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::JadeNave { heading }
    })
}

/// One Sky card builds a thin bridge circuit with three sheltered arrivals.
#[must_use]
pub fn authored_last_promenade(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
) -> Option<[HexPlacement; 3]> {
    authored_triad(config, anchor, rotation, |_, heading| {
        HexArchetype::LastPromenade { heading }
    })
}

fn authored_triad(
    config: super::HexWfcConfig,
    anchor: HexCoord,
    rotation: u8,
    archetype: impl Fn(usize, HexFace) -> HexArchetype,
) -> Option<[HexPlacement; 3]> {
    let turn = usize::from(rotation % 6);
    let face = |offset| HexFace::LATERAL[(turn + offset) % 6];
    let grid = config.grid();
    if !grid.contains(anchor) {
        return None;
    }
    let cell_b = grid.neighbor(anchor, face(0))?;
    let cell_c = grid.neighbor(anchor, face(1))?;
    let cell = |coord: HexCoord, offset| {
        let heading = face(offset);
        let archetype = archetype(offset, heading);
        HexPlacement {
            coord,
            space: HexSpace::Hall,
            archetype,
            doors: archetype.span_mask() | (1 << face(offset + 4).index()),
            low_doors: if coord.level == 0 && matches!(archetype, HexArchetype::Cistern { .. }) {
                1 << face(offset + 4).index()
            } else {
                0
            },
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        }
    };
    Some([cell(anchor, 0), cell(cell_b, 2), cell(cell_c, 4)])
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
        let placements: BTreeMap<_, _> = placements
            .into_iter()
            .map(|(coord, mut placement)| {
                placement.low_doors = if self.architecture.get(&coord)
                    == Some(&observed_content::ArchitectureRegister::LiminalGrid)
                    && coord.level == 0
                {
                    placement.doors & !placement.archetype.span_mask()
                } else {
                    0
                };
                (coord, placement)
            })
            .collect();
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
        && a.low_doors == b.low_doors
        && a.up == b.up
        && a.down == b.down
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::{HexWfcConfig, geometry_demands, placement_tile_archetype};
    use crate::hex_wfc::{climb_bond, lateral_bit, spans_join};

    /// A card's climb is a composition the solver itself could have built: every span
    /// joins its partner in order, the high cell bonds to its landing, and it is
    /// entered and left straight on.
    #[test]
    fn an_authored_climb_is_a_whole_composition() {
        let config = HexWfcConfig {
            cols: 8,
            rows: 8,
            levels: 3,
            ..HexWfcConfig::default()
        };
        let foot = HexCoord {
            q: 4,
            r: 4,
            level: 0,
        };
        for heading in HexFace::LATERAL {
            let [low, mid, high, landing] =
                authored_climb(config, foot, heading).expect("inside the lattice");
            assert!(spans_join(low.archetype, heading, mid.archetype));
            assert!(spans_join(mid.archetype, heading, high.archetype));
            assert!(spans_join(mid.archetype, heading.opposite(), low.archetype));
            assert!(!spans_join(low.archetype, heading, high.archetype));
            assert!(climb_bond(high.archetype, landing.archetype));
            assert_eq!(landing.coord.level, 1);
            assert_eq!(
                low.doors & !lateral_bit(heading),
                lateral_bit(heading.opposite())
            );
            assert_eq!(landing.doors, lateral_bit(heading));
        }
        // Off the edge of the lattice, or off its top, there is no climb.
        let edge = HexCoord {
            q: 7,
            r: 4,
            level: 0,
        };
        assert!(authored_climb(config, edge, HexFace::East).is_none());
        let top = HexCoord { level: 2, ..foot };
        assert!(authored_climb(config, top, HexFace::East).is_none());
    }

    #[test]
    fn every_shape_of_climb_composes_and_is_found_whole_from_each_cell() {
        let config = HexWfcConfig {
            cols: 9,
            rows: 9,
            levels: 2,
            ..HexWfcConfig::default()
        };
        let foot = HexCoord {
            q: 4,
            r: 4,
            level: 0,
        };
        let catalogue = super::super::variants::catalogue();
        let mut shapes = 0;
        for heading in HexFace::LATERAL {
            for turn in ClimbTurn::BENDS {
                for exit in ClimbTurn::EXITS {
                    let cells = authored_climb_shaped(config, foot, heading, turn, exit)
                        .expect("inside the lattice");
                    let [low, mid, high, landing] = cells;
                    let rising = turn.apply(heading);
                    assert!(spans_join(low.archetype, heading, mid.archetype));
                    assert!(spans_join(mid.archetype, heading.opposite(), low.archetype));
                    assert!(spans_join(mid.archetype, rising, high.archetype));
                    assert!(spans_join(high.archetype, rising.opposite(), mid.archetype));
                    assert!(climb_bond(high.archetype, landing.archetype));
                    assert_eq!(landing.doors, lateral_bit(exit.apply(rising)));
                    for placement in cells {
                        assert!(
                            catalogue.iter().any(|variant| variant.archetype
                                == placement.archetype
                                && variant.doors == placement.doors
                                && variant.up == placement.up
                                && variant.down == placement.down),
                            "{placement:?} is a catalogue variant"
                        );
                        let at = |coord: HexCoord| {
                            cells
                                .iter()
                                .find(|cell| cell.coord == coord)
                                .map(|cell| cell.archetype)
                        };
                        assert_eq!(
                            super::super::composition_cells(config.grid(), placement.coord, at),
                            Some(cells.map(|cell| cell.coord))
                        );
                    }
                    shapes += 1;
                }
            }
            // The wrong mid's turn is a mismatch, not a different composition.
            let [low, ..] = authored_climb(config, foot, heading).expect("inside");
            let other = HexArchetype::Climb {
                part: ClimbPart::Mid {
                    turn: ClimbTurn::Left,
                },
                heading: heading.opposite(),
            };
            assert!(!spans_join(low.archetype, heading, other));
        }
        assert_eq!(shapes, 6 * 5 * 4);
        assert!(
            authored_climb_shaped(
                config,
                foot,
                HexFace::East,
                ClimbTurn::Back,
                ClimbTurn::Ahead
            )
            .is_none()
        );
    }

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
                demands.iter().any(|d| d.archetype == archetype
                    && (if placement.coord.level == 0 {
                        d.signature.lowered()
                    } else {
                        d.signature
                    }) == placement.ports()),
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
            low_doors: 0,
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

    #[test]
    fn stair_rotations_roundtrip_all_headings_bends_and_exits() {
        for rotation in 0..120 {
            let (heading, turn, exit) = stair_shape(rotation);
            assert!(heading.is_lateral());
            assert!(ClimbTurn::BENDS.contains(&turn));
            assert!(ClimbTurn::EXITS.contains(&exit));
            let encoded = stair_rotation(heading, turn, exit);
            assert_eq!(encoded, rotation);
        }
        for (idx, &heading) in HexFace::LATERAL.iter().enumerate() {
            let (h, turn, exit) = stair_shape(idx as u8);
            assert_eq!(h, heading);
            assert_eq!(turn, ClimbTurn::Ahead);
            assert_eq!(exit, ClimbTurn::Ahead);
        }
    }

    #[test]
    fn authored_cistern_room_forms_open_triad_with_perimeter_doors() {
        let config = HexWfcConfig {
            cols: 8,
            rows: 8,
            levels: 2,
            ..HexWfcConfig::default()
        };
        let anchor = HexCoord {
            q: 3,
            r: 3,
            level: 0,
        };
        for rotation in 0..6 {
            let placements = authored_cistern_room(config, anchor, rotation)
                .expect("within central grid bounds");
            assert_eq!(placements.len(), 3);
            let [p0, p1, p2] = placements;

            // Bespoke sectors retain the ordinary hall topology.
            for p in &placements {
                assert_eq!(p.space, HexSpace::Hall);
                assert!(matches!(p.archetype, HexArchetype::Cistern { .. }));
                assert_eq!(p.archetype.span_mask().count_ones(), 2);
                assert_eq!(p.doors.count_ones(), 3); // 2 internal + 1 external
            }

            // Internal faces are open symmetrically
            let grid = config.grid();
            let face_01 = HexFace::LATERAL
                .into_iter()
                .find(|&f| grid.neighbor(p0.coord, f) == Some(p1.coord))
                .unwrap();
            let face_02 = HexFace::LATERAL
                .into_iter()
                .find(|&f| grid.neighbor(p0.coord, f) == Some(p2.coord))
                .unwrap();
            let face_12 = HexFace::LATERAL
                .into_iter()
                .find(|&f| grid.neighbor(p1.coord, f) == Some(p2.coord))
                .unwrap();

            assert_eq!(p0.ports().port(face_01), PortClass::Span);
            assert_eq!(p0.ports().port(face_02), PortClass::Span);
            assert_eq!(p1.ports().port(face_12), PortClass::Span);
            let external: Vec<_> = placements
                .iter()
                .flat_map(|p| {
                    HexFace::LATERAL
                        .into_iter()
                        .filter(|&f| p.ports().port(f).is_doorway())
                })
                .map(HexFace::index)
                .collect();
            assert_eq!(external.len(), 3);
            assert!(external.iter().all(|i| i % 2 == external[0] % 2));
            assert!(p0.is_open(face_01));
            assert!(p1.is_open(face_01.opposite()));

            assert!(p0.is_open(face_02));
            assert!(p2.is_open(face_02.opposite()));

            assert!(p1.is_open(face_12));
            assert!(p2.is_open(face_12.opposite()));
        }
    }
}
