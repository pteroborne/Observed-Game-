//! Canonical physical projection of a collapsed full-WFC generation.
//!
//! Simulation collision and assembled-game rendering both consume this snapshot, so
//! a visible opening cannot disagree with the deterministic Rapier scene.

use glam::Vec3;
use observed_facility::full_wfc::{CellCoord, FullWfcWorld, ModuleFace, ModuleSpace};
use observed_facility::map_spec::TraversalArchetype;
use observed_facility::room_def::RoomTemplate;
use observed_traversal::{
    ArenaSpec, ColliderSpec, StableColliderId, rapier_controller::RapierTraversalScene,
};

pub const CELL_SIZE: f32 = 12.0;
pub const LEVEL_HEIGHT: f32 = 5.0;
pub const WALL_HEIGHT: f32 = 5.0;
pub const WALL_THICKNESS: f32 = 0.36;
pub const FLOOR_THICKNESS: f32 = 0.28;
pub const SHAFT_OPENING: f32 = 3.2;
/// Clear width of every horizontal room-to-hall threshold.
pub const THRESHOLD_WIDTH: f32 = 4.5;
/// Clear height below the structural threshold header.
pub const THRESHOLD_CLEAR_HEIGHT: f32 = WALL_HEIGHT - 0.34;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureRole {
    Floor,
    Ceiling,
    Wall,
    Feature,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructurePiece {
    pub id: StableColliderId,
    pub cell: CellCoord,
    pub role: StructureRole,
    pub center: Vec3,
    pub half: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FullWfcGeometrySnapshot {
    pub generation: u32,
    pub pieces: Vec<StructurePiece>,
    pub arena: ArenaSpec,
}

impl FullWfcGeometrySnapshot {
    pub fn project(world: &FullWfcWorld) -> Self {
        let mut pieces = Vec::new();
        let mut next_id = 1u32;
        for placement in world
            .placements
            .values()
            .filter(|placement| placement.space != ModuleSpace::Void)
        {
            let base = cell_origin(placement.coord);
            if placement.is_open(ModuleFace::Down) {
                push_aperture_deck(
                    &mut pieces,
                    &mut next_id,
                    placement.coord,
                    StructureRole::Floor,
                    base.y - FLOOR_THICKNESS * 0.5,
                );
            } else {
                push_piece(
                    &mut pieces,
                    &mut next_id,
                    placement.coord,
                    StructureRole::Floor,
                    Vec3::new(base.x, base.y - FLOOR_THICKNESS * 0.5, base.z),
                    Vec3::new(CELL_SIZE * 0.5, FLOOR_THICKNESS * 0.5, CELL_SIZE * 0.5),
                );
            }
            if !placement.is_open(ModuleFace::Up) {
                push_piece(
                    &mut pieces,
                    &mut next_id,
                    placement.coord,
                    StructureRole::Ceiling,
                    Vec3::new(base.x, base.y + WALL_HEIGHT + FLOOR_THICKNESS * 0.5, base.z),
                    Vec3::new(CELL_SIZE * 0.5, FLOOR_THICKNESS * 0.5, CELL_SIZE * 0.5),
                );
            }
            for face in [
                ModuleFace::East,
                ModuleFace::West,
                ModuleFace::South,
                ModuleFace::North,
            ] {
                if placement.is_open(face) && placement.space == ModuleSpace::Room {
                    push_aperture_wall(&mut pieces, &mut next_id, placement.coord, face);
                } else if !placement.is_open(face) && owns_boundary(world, placement.coord, face) {
                    let (offset, half) = wall_shape(face);
                    push_piece(
                        &mut pieces,
                        &mut next_id,
                        placement.coord,
                        StructureRole::Wall,
                        base + offset,
                        half,
                    );
                }
            }
            push_catalog_features(world, placement.coord, &mut pieces, &mut next_id);
        }
        let colliders = pieces
            .iter()
            .map(|piece| ColliderSpec::cuboid(piece.id, piece.center, piece.half))
            .collect();
        let width = f32::from(world.config.cols) * CELL_SIZE;
        let depth = f32::from(world.config.rows) * CELL_SIZE;
        let height = f32::from(world.config.levels) * LEVEL_HEIGHT;
        let arena = ArenaSpec {
            colliders,
            floor_y: 0.0,
            safety_center: Vec3::new(
                (width - CELL_SIZE) * 0.5,
                height * 0.5,
                (depth - CELL_SIZE) * 0.5,
            ),
            safety_half: Vec3::new(width * 0.55, height, depth * 0.55),
        };
        debug_assert!(arena.validate().is_ok());
        Self {
            generation: world.generation,
            pieces,
            arena,
        }
    }

    pub fn rapier_scene(&self) -> RapierTraversalScene {
        RapierTraversalScene::from_arena_spec(&self.arena)
    }
}

fn push_catalog_features(
    world: &FullWfcWorld,
    cell: CellCoord,
    pieces: &mut Vec<StructurePiece>,
    next_id: &mut u32,
) {
    let origin = cell_origin(cell);
    if let Some(room) = world.room_at(cell) {
        let template = world.rooms[&room].template;
        match template {
            RoomTemplate::StraightCorridor => {
                let placement = &world.placements[&cell];
                let offsets =
                    if placement.is_open(ModuleFace::East) || placement.is_open(ModuleFace::West) {
                        &[(0.0, -4.2), (0.0, 4.2)]
                    } else {
                        &[(-4.2, 0.0), (4.2, 0.0)]
                    };
                posts(
                    pieces,
                    next_id,
                    cell,
                    origin,
                    offsets,
                    Vec3::new(0.22, 2.1, 0.22),
                )
            }
            RoomTemplate::Corner => {
                let placement = &world.placements[&cell];
                let north_closed = !placement.is_open(ModuleFace::North);
                let south_closed = !placement.is_open(ModuleFace::South);
                let east_closed = !placement.is_open(ModuleFace::East);
                let west_closed = !placement.is_open(ModuleFace::West);
                let x = if east_closed && !west_closed {
                    4.0
                } else if west_closed && !east_closed {
                    -4.0
                } else {
                    4.0
                };
                let z = if north_closed && !south_closed {
                    4.0
                } else if south_closed && !north_closed {
                    -4.0
                } else {
                    4.0
                };
                posts(
                    pieces,
                    next_id,
                    cell,
                    origin,
                    &[(x, z)],
                    Vec3::new(0.7, 1.4, 0.7),
                )
            }
            RoomTemplate::Junction => posts(
                pieces,
                next_id,
                cell,
                origin,
                &[(-4.1, -4.1), (4.1, -4.1), (-4.1, 4.1), (4.1, 4.1)],
                Vec3::new(0.3, 1.7, 0.3),
            ),
            RoomTemplate::ControlRoom => push_piece(
                pieces,
                next_id,
                cell,
                StructureRole::Feature,
                origin + Vec3::new(0.0, 4.15, 0.0),
                Vec3::new(2.4, 0.24, 0.35),
            ),
            RoomTemplate::MachineChamber => posts(
                pieces,
                next_id,
                cell,
                origin,
                &[(-3.8, -3.8), (3.8, -3.8), (-3.8, 3.8), (3.8, 3.8)],
                Vec3::new(0.8, 1.0, 0.8),
            ),
            RoomTemplate::FreightRoom => posts(
                pieces,
                next_id,
                cell,
                origin,
                &[(-4.0, -3.7), (4.0, 3.7)],
                Vec3::new(1.0, 0.75, 1.0),
            ),
            RoomTemplate::Shaft => posts(
                pieces,
                next_id,
                cell,
                origin,
                &[(-2.2, -2.2), (2.2, -2.2), (-2.2, 2.2), (2.2, 2.2)],
                Vec3::new(0.12, 2.2, 0.12),
            ),
            RoomTemplate::PlatformRoom => posts(
                pieces,
                next_id,
                cell,
                origin,
                &[(-4.3, 0.0), (4.3, 0.0)],
                Vec3::new(1.3, 0.24, 3.4),
            ),
        }
        return;
    }
    let Some(corridor) = world.corridor_at(cell) else {
        return;
    };
    match world.corridors[&corridor].traversal {
        TraversalArchetype::Straight => {}
        TraversalArchetype::Long => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-4.7, -3.2), (4.7, -3.2), (-4.7, 3.2), (4.7, 3.2)],
            Vec3::new(0.18, 2.2, 0.4),
        ),
        TraversalArchetype::Pressure => push_piece(
            pieces,
            next_id,
            cell,
            StructureRole::Feature,
            origin + Vec3::new(0.0, 3.8, 0.0),
            Vec3::new(4.6, 0.22, 0.32),
        ),
        TraversalArchetype::Climb => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-4.0, -3.0), (-4.0, 0.0), (-4.0, 3.0)],
            Vec3::new(1.2, 0.18, 0.65),
        ),
        TraversalArchetype::Maze => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-3.8, -2.0), (3.8, 2.0)],
            Vec3::new(0.18, 1.35, 1.8),
        ),
        TraversalArchetype::Chicane => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-2.8, -2.2), (2.8, 2.2)],
            Vec3::new(0.5, 1.25, 0.5),
        ),
        TraversalArchetype::GantryExpanse => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-4.4, 0.0), (4.4, 0.0)],
            Vec3::new(0.12, 0.55, 4.1),
        ),
        TraversalArchetype::Wellshaft => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-2.1, -2.1), (2.1, -2.1), (-2.1, 2.1), (2.1, 2.1)],
            Vec3::new(0.1, 2.25, 0.1),
        ),
        TraversalArchetype::Colonnade => posts(
            pieces,
            next_id,
            cell,
            origin,
            &[(-3.8, -3.0), (3.8, -3.0), (-3.8, 3.0), (3.8, 3.0)],
            Vec3::new(0.42, 2.1, 0.42),
        ),
        TraversalArchetype::Orthogonal => {
            push_piece(
                pieces,
                next_id,
                cell,
                StructureRole::Feature,
                origin + Vec3::new(0.0, 4.0, 0.0),
                Vec3::new(4.5, 0.16, 0.16),
            );
            push_piece(
                pieces,
                next_id,
                cell,
                StructureRole::Feature,
                origin + Vec3::new(0.0, 4.0, 0.0),
                Vec3::new(0.16, 0.16, 4.5),
            );
        }
    }
}

fn posts(
    pieces: &mut Vec<StructurePiece>,
    next_id: &mut u32,
    cell: CellCoord,
    origin: Vec3,
    positions: &[(f32, f32)],
    half: Vec3,
) {
    for &(x, z) in positions {
        push_piece(
            pieces,
            next_id,
            cell,
            StructureRole::Feature,
            origin + Vec3::new(x, half.y, z),
            half,
        );
    }
}

pub fn cell_origin(cell: CellCoord) -> Vec3 {
    Vec3::new(
        f32::from(cell.x) * CELL_SIZE,
        f32::from(cell.level) * LEVEL_HEIGHT,
        f32::from(cell.z) * CELL_SIZE,
    )
}

fn owns_boundary(world: &FullWfcWorld, cell: CellCoord, face: ModuleFace) -> bool {
    if matches!(face, ModuleFace::East | ModuleFace::South) {
        return true;
    }
    world
        .config
        .neighbor(cell, face)
        .and_then(|neighbor| world.placement(neighbor))
        .is_none_or(|neighbor| neighbor.space == ModuleSpace::Void)
}

fn wall_shape(face: ModuleFace) -> (Vec3, Vec3) {
    let horizontal = CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5;
    match face {
        ModuleFace::East => (
            Vec3::new(horizontal, WALL_HEIGHT * 0.5, 0.0),
            Vec3::new(WALL_THICKNESS * 0.5, WALL_HEIGHT * 0.5, CELL_SIZE * 0.5),
        ),
        ModuleFace::West => (
            Vec3::new(-horizontal, WALL_HEIGHT * 0.5, 0.0),
            Vec3::new(WALL_THICKNESS * 0.5, WALL_HEIGHT * 0.5, CELL_SIZE * 0.5),
        ),
        ModuleFace::South => (
            Vec3::new(0.0, WALL_HEIGHT * 0.5, horizontal),
            Vec3::new(CELL_SIZE * 0.5, WALL_HEIGHT * 0.5, WALL_THICKNESS * 0.5),
        ),
        ModuleFace::North => (
            Vec3::new(0.0, WALL_HEIGHT * 0.5, -horizontal),
            Vec3::new(CELL_SIZE * 0.5, WALL_HEIGHT * 0.5, WALL_THICKNESS * 0.5),
        ),
        ModuleFace::Up | ModuleFace::Down => unreachable!("horizontal walls only"),
    }
}

fn push_aperture_wall(
    pieces: &mut Vec<StructurePiece>,
    next_id: &mut u32,
    cell: CellCoord,
    face: ModuleFace,
) {
    let base = cell_origin(cell);
    let horizontal = CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5;
    let side_width = (CELL_SIZE - THRESHOLD_WIDTH) * 0.5;
    let side_offset = THRESHOLD_WIDTH * 0.5 + side_width * 0.5;
    let side_half = side_width * 0.5;
    let header_height = WALL_HEIGHT - THRESHOLD_CLEAR_HEIGHT;

    match face {
        ModuleFace::East | ModuleFace::West => {
            let x = if face == ModuleFace::East {
                horizontal
            } else {
                -horizontal
            };
            for z in [-side_offset, side_offset] {
                push_piece(
                    pieces,
                    next_id,
                    cell,
                    StructureRole::Wall,
                    base + Vec3::new(x, THRESHOLD_CLEAR_HEIGHT * 0.5, z),
                    Vec3::new(
                        WALL_THICKNESS * 0.5,
                        THRESHOLD_CLEAR_HEIGHT * 0.5,
                        side_half,
                    ),
                );
            }
            push_piece(
                pieces,
                next_id,
                cell,
                StructureRole::Wall,
                base + Vec3::new(x, THRESHOLD_CLEAR_HEIGHT + header_height * 0.5, 0.0),
                Vec3::new(WALL_THICKNESS * 0.5, header_height * 0.5, CELL_SIZE * 0.5),
            );
        }
        ModuleFace::South | ModuleFace::North => {
            let z = if face == ModuleFace::South {
                horizontal
            } else {
                -horizontal
            };
            for x in [-side_offset, side_offset] {
                push_piece(
                    pieces,
                    next_id,
                    cell,
                    StructureRole::Wall,
                    base + Vec3::new(x, THRESHOLD_CLEAR_HEIGHT * 0.5, z),
                    Vec3::new(
                        side_half,
                        THRESHOLD_CLEAR_HEIGHT * 0.5,
                        WALL_THICKNESS * 0.5,
                    ),
                );
            }
            push_piece(
                pieces,
                next_id,
                cell,
                StructureRole::Wall,
                base + Vec3::new(0.0, THRESHOLD_CLEAR_HEIGHT + header_height * 0.5, z),
                Vec3::new(CELL_SIZE * 0.5, header_height * 0.5, WALL_THICKNESS * 0.5),
            );
        }
        ModuleFace::Up | ModuleFace::Down => unreachable!("horizontal thresholds only"),
    }
}

fn push_aperture_deck(
    pieces: &mut Vec<StructurePiece>,
    next_id: &mut u32,
    cell: CellCoord,
    role: StructureRole,
    y: f32,
) {
    let base = cell_origin(cell);
    let side = (CELL_SIZE - SHAFT_OPENING) * 0.25;
    let edge = (CELL_SIZE + SHAFT_OPENING) * 0.25;
    for x in [-edge, edge] {
        push_piece(
            pieces,
            next_id,
            cell,
            role,
            Vec3::new(base.x + x, y, base.z),
            Vec3::new(side, FLOOR_THICKNESS * 0.5, CELL_SIZE * 0.5),
        );
    }
    for z in [-edge, edge] {
        push_piece(
            pieces,
            next_id,
            cell,
            role,
            Vec3::new(base.x, y, base.z + z),
            Vec3::new(SHAFT_OPENING * 0.5, FLOOR_THICKNESS * 0.5, side),
        );
    }
}

fn push_piece(
    pieces: &mut Vec<StructurePiece>,
    next_id: &mut u32,
    cell: CellCoord,
    role: StructureRole,
    center: Vec3,
    half: Vec3,
) {
    pieces.push(StructurePiece {
        id: StableColliderId(*next_id),
        cell,
        role,
        center,
        half,
    });
    *next_id = next_id.wrapping_add(1);
}

#[cfg(test)]
mod tests;
