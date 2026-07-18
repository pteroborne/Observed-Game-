//! Authored hex-tile prefabs projected into one continuous collision arena.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec2, Vec3};
use observed_authoring::{TileKey, TilePrototype};
use observed_facility::hex_wfc::{
    HexArchetype, HexPlacement, HexSpace, HexWfcWorld, PortSignature, StampedBlueprint,
    blueprint_cell_archetype, blueprint_for_role, placement_tile_archetype,
};
use observed_facility::map_spec::RoomRole;
use observed_hex::{CORNERS, HexCoord, TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::{
    ArenaSpec, ColliderShape, ColliderSpec, StableColliderId,
    rapier_controller::RapierTraversalScene,
};

const COLLIDER_STRIDE: usize = 128;
const SHELL_ID_BASE: u32 = 0xF000_0000;
const SHELL_THICKNESS: f32 = 0.5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexStructureRole {
    Hall,
    Room,
    Ramp,
    Shaft,
    Boundary,
}

/// One render/collision primitive from an authored prefab or the arena shell.
#[derive(Clone, Debug, PartialEq)]
pub struct HexStructurePiece {
    pub id: StableColliderId,
    /// Stable low-cell/blueprint anchor used by debug inspection.
    pub anchor: HexCoord,
    /// The concrete lattice cell whose ID range owns this primitive.
    pub source_cell: HexCoord,
    pub role: HexStructureRole,
    pub tile: Option<TileKey>,
    pub center: Vec3,
    pub rotation: [f32; 4],
    pub shape: ColliderShape,
}

impl HexStructurePiece {
    fn collider(&self) -> ColliderSpec {
        ColliderSpec {
            id: self.id,
            center: self.center,
            rotation: self.rotation,
            shape: self.shape.clone(),
            friction: 0.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexWfcGeometrySnapshot {
    pub generation: u32,
    pub pieces: Vec<HexStructurePiece>,
    pub arena: ArenaSpec,
    /// Ramp heads intentionally emit nothing: their low-cell prefab spans both levels.
    pub ramp_heads: usize,
    /// Each room blueprint is visited once, even when its footprint has many cells.
    pub blueprint_instances: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexGeometryError {
    MissingArchitecture(HexCoord),
    MissingTile {
        coord: HexCoord,
        archetype: &'static str,
        register: String,
        signature: PortSignature,
    },
    BlueprintCellMissing(HexCoord),
    BlueprintArchetypeMissing {
        role: RoomRole,
        cell_index: usize,
    },
    TooManyHulls {
        coord: HexCoord,
        hulls: usize,
    },
    ColliderIdCapacity {
        cells: u64,
        required_max: u64,
    },
    InvalidArena,
}

impl HexWfcGeometrySnapshot {
    /// Project a solved world using its pure per-cell architecture register map.
    /// Manifest ordering does not affect selection: candidates are sorted by key.
    pub fn project(
        world: &HexWfcWorld,
        prototypes: &[TilePrototype],
    ) -> Result<Self, HexGeometryError> {
        validate_id_capacity(world)?;
        let catalogue = Catalogue::new(prototypes);
        let mut pieces = Vec::new();
        let mut consumed_rooms = BTreeSet::new();

        // A room footprint is one semantic instance. Its authored component
        // cells are expanded here, once from the blueprint anchor, and skipped
        // by the ordinary per-placement pass below.
        for blueprint in &world.blueprints {
            project_blueprint(
                world,
                blueprint,
                &catalogue,
                &mut consumed_rooms,
                &mut pieces,
            )?;
        }

        let mut ramp_heads = 0;
        for placement in world.placements.values() {
            if placement.space == HexSpace::Void || consumed_rooms.contains(&placement.coord) {
                continue;
            }
            if placement.archetype == HexArchetype::RampHead {
                ramp_heads += 1;
                continue;
            }
            let role = role_for(placement);
            let archetype = placement_tile_archetype(placement)
                .expect("non-void, non-room, non-RampHead placement emits a prefab");
            let tile = tile_for(
                world,
                &catalogue,
                placement.coord,
                archetype,
                placement.ports(),
            )?;
            push_tile(
                world,
                placement.coord,
                placement.coord,
                role,
                tile,
                &mut pieces,
            )?;
        }

        push_boundary_shell(world, &mut pieces);
        let arena = arena_for(world, &pieces);
        arena
            .validate()
            .map_err(|_| HexGeometryError::InvalidArena)?;
        Ok(Self {
            generation: world.generation,
            pieces,
            arena,
            ramp_heads,
            blueprint_instances: world.blueprints.len(),
        })
    }

    #[must_use]
    pub fn rapier_scene(&self) -> RapierTraversalScene {
        RapierTraversalScene::from_arena_spec(&self.arena)
    }
}

fn validate_id_capacity(world: &HexWfcWorld) -> Result<(), HexGeometryError> {
    let cells = u64::from(world.config.cols)
        * u64::from(world.config.rows)
        * u64::from(world.config.levels);
    let required_max = cells.saturating_mul(COLLIDER_STRIDE as u64);
    if required_max >= u64::from(SHELL_ID_BASE) {
        return Err(HexGeometryError::ColliderIdCapacity {
            cells,
            required_max,
        });
    }
    Ok(())
}

struct Catalogue<'a> {
    by_key: BTreeMap<(&'a str, &'a str, PortSignature), Vec<&'a TilePrototype>>,
}

impl<'a> Catalogue<'a> {
    fn new(prototypes: &'a [TilePrototype]) -> Self {
        let mut by_key: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for prototype in prototypes {
            by_key
                .entry((
                    prototype.key.archetype.as_str(),
                    prototype.key.register.as_str(),
                    prototype.signature,
                ))
                .or_default()
                .push(prototype);
        }
        for candidates in by_key.values_mut() {
            candidates.sort_by(|a, b| a.key.cmp(&b.key));
        }
        Self { by_key }
    }

    fn select(
        &self,
        archetype: &'static str,
        register: &str,
        signature: PortSignature,
        variation: u64,
    ) -> Option<&'a TilePrototype> {
        let candidates = self.by_key.get(&(archetype, register, signature))?;
        Some(candidates[variation_index(variation, candidates.len())])
    }
}

fn variation_index(key: u64, candidate_count: usize) -> usize {
    (key % candidate_count as u64) as usize
}

fn tile_for<'a>(
    world: &HexWfcWorld,
    catalogue: &Catalogue<'a>,
    coord: HexCoord,
    archetype: &'static str,
    signature: PortSignature,
) -> Result<&'a TilePrototype, HexGeometryError> {
    let register = world
        .architecture
        .get(&coord)
        .ok_or(HexGeometryError::MissingArchitecture(coord))?
        .slug();
    catalogue
        .select(
            archetype,
            register,
            signature,
            world.tile_variation_key(coord),
        )
        .ok_or_else(|| HexGeometryError::MissingTile {
            coord,
            archetype,
            register: register.to_string(),
            signature,
        })
}

fn project_blueprint(
    world: &HexWfcWorld,
    stamped: &StampedBlueprint,
    catalogue: &Catalogue<'_>,
    consumed: &mut BTreeSet<HexCoord>,
    pieces: &mut Vec<HexStructurePiece>,
) -> Result<(), HexGeometryError> {
    let blueprint = blueprint_for_role(stamped.role);
    for (index, &coord) in stamped.cells.iter().enumerate() {
        world
            .placements
            .get(&coord)
            .ok_or(HexGeometryError::BlueprintCellMissing(coord))?;
        let archetype = blueprint_cell_archetype(stamped.role, index).ok_or(
            HexGeometryError::BlueprintArchetypeMissing {
                role: stamped.role,
                cell_index: index,
            },
        )?;
        // Boundary stamping seals ports that leave the lattice in simulation.
        // Authored room prefabs retain the blueprint's full exterior signature;
        // the arena shell closes those out-of-grid apertures physically.
        let signature = blueprint.cell_signature(blueprint.cells[index]);
        let tile = tile_for(world, catalogue, coord, archetype, signature)?;
        push_tile(
            world,
            stamped.anchor,
            coord,
            HexStructureRole::Room,
            tile,
            pieces,
        )?;
        consumed.insert(coord);
    }
    debug_assert_eq!(blueprint.cells.len(), stamped.cells.len());
    Ok(())
}

fn role_for(placement: &HexPlacement) -> HexStructureRole {
    match placement.archetype {
        HexArchetype::RampUp => HexStructureRole::Ramp,
        HexArchetype::Shaft => HexStructureRole::Shaft,
        _ => HexStructureRole::Hall,
    }
}

fn push_tile(
    world: &HexWfcWorld,
    anchor: HexCoord,
    source_cell: HexCoord,
    role: HexStructureRole,
    tile: &TilePrototype,
    pieces: &mut Vec<HexStructurePiece>,
) -> Result<(), HexGeometryError> {
    if tile.hulls.len() >= COLLIDER_STRIDE {
        return Err(HexGeometryError::TooManyHulls {
            coord: source_cell,
            hulls: tile.hulls.len(),
        });
    }
    let base = u64::try_from(world.config.grid().index(source_cell))
        .expect("validated grid index fits u64")
        * COLLIDER_STRIDE as u64
        + 1;
    let center = Vec3::from_array(hex_origin(source_cell));
    for (index, hull) in tile.hulls.iter().enumerate() {
        pieces.push(HexStructurePiece {
            id: StableColliderId(
                u32::try_from(base + index as u64).expect("validated collider ID capacity"),
            ),
            anchor,
            source_cell,
            role,
            tile: Some(tile.key.clone()),
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::ConvexHull {
                points: hull.clone(),
            },
        });
    }
    Ok(())
}

fn push_boundary_shell(world: &HexWfcWorld, pieces: &mut Vec<HexStructurePiece>) {
    let outline = rhombus_outline(world);
    let height = f32::from(world.config.levels) * TILE_LEVEL_HEIGHT;
    let anchor = world.config.spawn();
    for (index, pair) in outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .take(outline.len())
        .enumerate()
    {
        let (a, b) = pair;
        let edge = *b - *a;
        let midpoint = (*a + *b) * 0.5;
        let yaw = (-edge.y).atan2(edge.x);
        let rotation = Quat::from_rotation_y(yaw).to_array();
        pieces.push(HexStructurePiece {
            id: StableColliderId(SHELL_ID_BASE + index as u32),
            anchor,
            source_cell: anchor,
            role: HexStructureRole::Boundary,
            tile: None,
            center: Vec3::new(midpoint.x, height * 0.5, midpoint.y),
            rotation,
            shape: ColliderShape::Cuboid {
                half: Vec3::new(edge.length() * 0.5, height * 0.5, SHELL_THICKNESS * 0.5),
            },
        });
    }
}

/// Convex outline of the rhombic axial domain including the canonical hex footprint.
fn rhombus_outline(world: &HexWfcWorld) -> Vec<Vec2> {
    let q = world.config.cols - 1;
    let r = world.config.rows - 1;
    let corners = [
        HexCoord {
            q: 0,
            r: 0,
            level: 0,
        },
        HexCoord { q, r: 0, level: 0 },
        HexCoord { q: 0, r, level: 0 },
        HexCoord { q, r, level: 0 },
    ];
    let mut points = Vec::with_capacity(24);
    for cell in corners {
        let origin = Vec3::from_array(hex_origin(cell));
        points.extend(CORNERS.map(|(x, z)| Vec2::new(origin.x + x as f32, origin.z + z as f32)));
    }
    convex_hull(points)
}

fn convex_hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut lower = Vec::new();
    for &point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 0.0
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper = Vec::new();
    for &point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 0.0
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn arena_for(world: &HexWfcWorld, pieces: &[HexStructurePiece]) -> ArenaSpec {
    let outline = rhombus_outline(world);
    let min = outline
        .iter()
        .copied()
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO);
    let max = outline
        .iter()
        .copied()
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO);
    let height = f32::from(world.config.levels) * TILE_LEVEL_HEIGHT;
    ArenaSpec {
        colliders: pieces.iter().map(HexStructurePiece::collider).collect(),
        floor_y: 0.0,
        safety_center: Vec3::new((min.x + max.x) * 0.5, height * 0.5, (min.y + max.y) * 0.5),
        safety_half: Vec3::new((max.x - min.x) * 0.55, height, (max.y - min.y) * 0.55),
    }
}

#[cfg(test)]
mod tests;
