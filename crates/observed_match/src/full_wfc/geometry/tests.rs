use observed_facility::full_wfc::{FullWfcConfig, FullWfcWorld, ModuleFace, ModuleSpace};

use super::*;

#[test]
fn canonical_snapshot_is_valid_and_leaves_open_boundaries_unblocked() {
    let world = FullWfcWorld::new(31, FullWfcConfig::default()).expect("world");
    let snapshot = FullWfcGeometrySnapshot::project(&world);
    snapshot.arena.validate().expect("valid arena");
    assert!(
        snapshot
            .pieces
            .iter()
            .any(|piece| piece.role == StructureRole::Floor)
    );
    for placement in world.placements.values() {
        if placement.is_open(ModuleFace::East) {
            let boundary = cell_origin(placement.coord)
                + Vec3::new(
                    CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5,
                    WALL_HEIGHT * 0.5,
                    0.0,
                );
            assert!(!snapshot.pieces.iter().any(|piece| {
                piece.role == StructureRole::Wall
                    && piece.cell == placement.coord
                    && piece.center == boundary
            }));
        }
    }
}

#[test]
fn room_thresholds_have_a_canonical_clear_aperture() {
    let world = FullWfcWorld::new(31, FullWfcConfig::default()).expect("world");
    let snapshot = FullWfcGeometrySnapshot::project(&world);
    for placement in world
        .placements
        .values()
        .filter(|placement| placement.space == ModuleSpace::Room)
    {
        for face in [
            ModuleFace::East,
            ModuleFace::West,
            ModuleFace::South,
            ModuleFace::North,
        ] {
            if !placement.is_open(face) {
                continue;
            }
            let base = cell_origin(placement.coord);
            let sample = match face {
                ModuleFace::East => {
                    base + Vec3::new(CELL_SIZE * 0.5, THRESHOLD_CLEAR_HEIGHT * 0.5, 0.0)
                }
                ModuleFace::West => {
                    base + Vec3::new(-CELL_SIZE * 0.5, THRESHOLD_CLEAR_HEIGHT * 0.5, 0.0)
                }
                ModuleFace::South => {
                    base + Vec3::new(0.0, THRESHOLD_CLEAR_HEIGHT * 0.5, CELL_SIZE * 0.5)
                }
                ModuleFace::North => {
                    base + Vec3::new(0.0, THRESHOLD_CLEAR_HEIGHT * 0.5, -CELL_SIZE * 0.5)
                }
                ModuleFace::Up | ModuleFace::Down => unreachable!(),
            };
            assert!(snapshot.pieces.iter().all(|piece| {
                piece.role != StructureRole::Wall
                    || piece.cell != placement.coord
                    || !point_inside_piece(sample, piece)
            }));
        }
    }
}

#[test]
fn hall_to_hall_openings_remain_full_width() {
    let world = FullWfcWorld::new(31, FullWfcConfig::default()).expect("world");
    let snapshot = FullWfcGeometrySnapshot::project(&world);
    for placement in world
        .placements
        .values()
        .filter(|placement| placement.space == ModuleSpace::Hall)
    {
        for face in [ModuleFace::East, ModuleFace::South] {
            let Some(neighbor) = world.config.neighbor(placement.coord, face) else {
                continue;
            };
            if !placement.is_open(face) || world.placements[&neighbor].space != ModuleSpace::Hall {
                continue;
            }
            let boundary = cell_origin(placement.coord)
                + match face {
                    ModuleFace::East => Vec3::new(CELL_SIZE * 0.5, 2.0, 0.0),
                    ModuleFace::South => Vec3::new(0.0, 2.0, CELL_SIZE * 0.5),
                    _ => unreachable!(),
                };
            assert!(snapshot.pieces.iter().all(|piece| {
                piece.role != StructureRole::Wall || !point_inside_piece(boundary, piece)
            }));
        }
    }
}

fn point_inside_piece(point: Vec3, piece: &StructurePiece) -> bool {
    let delta = (point - piece.center).abs();
    delta.x <= piece.half.x && delta.y <= piece.half.y && delta.z <= piece.half.z
}

#[test]
fn complete_catalog_fixture_uses_one_render_and_rapier_piece_list() {
    let world = FullWfcWorld::catalog_fixture(0xC011_1DE3).expect("fixture");
    let snapshot = FullWfcGeometrySnapshot::project(&world);
    let scene = snapshot.rapier_scene();
    assert_eq!(scene.collider_count(), snapshot.pieces.len());
    assert!(
        snapshot
            .pieces
            .iter()
            .any(|piece| piece.role == StructureRole::Feature)
    );
    for room in world.rooms.values() {
        assert!(snapshot.pieces.iter().any(|piece| piece.cell == room.coord));
    }
    for corridor in world.corridors.values() {
        assert!(
            corridor
                .cells
                .iter()
                .all(|cell| snapshot.pieces.iter().any(|piece| piece.cell == *cell))
        );
    }
}
