//! Batched authored-hull rendering for isometric cutaway views.
//!
//! This is the colour-blind bridge between the canonical WFC geometry snapshot
//! and Bevy meshes. The Composition Studio and tactical lab choose their own
//! materials; this crate owns only hull triangulation, caching, cutaway
//! classification, and batching.

use std::collections::{BTreeMap, BTreeSet};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexFace, HexWfcWorld, PortClass};
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexStructurePiece, HexWfcGeometrySnapshot};
use observed_traversal::{ColliderShape, ConvexRenderMesh};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DetailMode {
    #[default]
    Off,
    Focus,
    Layer,
    Neighborhood,
}

impl DetailMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "schematic",
            Self::Focus => "detail: focus",
            Self::Layer => "detail: layer",
            Self::Neighborhood => "detail: neighbours",
        }
    }
}

/// How much of a cell's authored walls to draw.
///
/// The public face of the private [`Projection`] enum. Both projections already
/// existed and were reached through separate entry points; naming the choice is
/// what lets a viewer put it on a key and a legend, and lets a capture script
/// name the view it photographed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WallMode {
    /// Ceiling and the walls between the camera and the interior are removed,
    /// so an interior can be looked into from a fixed isometric.
    Cutaway,
    /// Ceilings removed and every perimeter hull capped to a third of a level.
    /// Floors, ramps, stairs and columns keep their authored shape, so a whole
    /// deck reads at once from any bearing without anything being cut away.
    #[default]
    Partial,
    /// Everything the catalog authored, ceilings included.
    Full,
}

impl WallMode {
    pub const ALL: [Self; 3] = [Self::Cutaway, Self::Partial, Self::Full];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cutaway => "cutaway",
            Self::Partial => "partial walls",
            Self::Full => "full walls",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Cutaway => Self::Partial,
            Self::Partial => Self::Full,
            Self::Full => Self::Cutaway,
        }
    }

    /// Parse a script's spelling. `None` rather than a default, because a
    /// capture that quietly photographed a different projection than the one it
    /// named is the failure the script runner exists to refuse.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| {
            mode.label().eq_ignore_ascii_case(name)
                || name.eq_ignore_ascii_case(match mode {
                    Self::Cutaway => "cutaway",
                    Self::Partial => "partial",
                    Self::Full => "full",
                })
        })
    }
}

/// The height a [`WallMode::Partial`] perimeter is capped to.
///
/// One third of a level. Low enough to see over from a fixed isometric, tall
/// enough that a doorway still reads as a gap in something.
#[must_use]
pub fn partial_wall_height() -> f32 {
    observed_hex::TILE_LEVEL_HEIGHT / 3.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DetailReport {
    pub cells: usize,
    pub hulls_drawn: usize,
    pub hulls_cut: usize,
    pub distinct_hulls_cached: usize,
}

#[must_use]
pub fn focus_set(world: &HexWfcWorld, selected: Option<HexCoord>) -> BTreeSet<HexCoord> {
    let mut set = BTreeSet::new();
    let Some(centre) = selected else { return set };
    let Some(placement) = world.placements.get(&centre) else {
        return set;
    };
    set.insert(centre);
    let grid = world.config.grid();
    for face in HexFace::ALL {
        let open = if face.is_lateral() {
            placement.is_open(face)
        } else if face == HexFace::Up {
            placement.up != PortClass::Sealed
        } else {
            placement.down != PortClass::Sealed
        };
        if open
            && let Some(neighbour) = grid.neighbor(centre, face)
            && world.placements.contains_key(&neighbour)
        {
            set.insert(neighbour);
        }
    }
    set
}

#[must_use]
pub fn survives(min_y: f32, max_y: f32, local: Vec3, bearing: Vec2, cutaway: bool) -> bool {
    observed_style::iso::survives(min_y, max_y, local, bearing, cutaway)
}

pub struct CachedHull {
    mesh: ConvexRenderMesh,
}

#[derive(Default, Resource)]
pub struct TileMeshCache {
    by_key: BTreeMap<String, Option<CachedHull>>,
}

impl TileMeshCache {
    pub fn entry(&mut self, key: String, points: &[Vec3]) -> Option<&CachedHull> {
        self.by_key
            .entry(key)
            .or_insert_with(|| Self::build_owned(points))
            .as_ref()
    }

    #[must_use]
    pub fn build_hull(points: &[Vec3]) -> Option<CachedHull> {
        Self::build_owned(points)
    }

    fn build_owned(points: &[Vec3]) -> Option<CachedHull> {
        let mesh = ConvexRenderMesh::from_convex_hull(points)?;
        Some(CachedHull { mesh })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    pub fn clear(&mut self) {
        self.by_key.clear();
    }
}

#[must_use]
pub fn measure(points: &[Vec3]) -> (f32, f32, Vec3) {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut sum = Vec3::ZERO;
    for point in points {
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
        sum += *point;
    }
    #[allow(clippy::cast_precision_loss)]
    let centroid = sum / points.len() as f32;
    (min_y, max_y, centroid)
}

#[derive(Default)]
pub struct HullBatch {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl HullBatch {
    fn push(&mut self, mesh: &ConvexRenderMesh, offset: Vec3) {
        #[allow(clippy::cast_possible_truncation)]
        let base = self.positions.len() as u32;
        for (position, normal) in mesh.positions.iter().zip(mesh.normals.iter()) {
            self.positions.push([
                position[0] + offset.x,
                position[1] + offset.y,
                position[2] + offset.z,
            ]);
            self.normals.push(*normal);
        }
        self.indices
            .extend(mesh.indices.iter().map(|index| index + base));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    #[must_use]
    pub fn into_mesh(self) -> Option<Mesh> {
        if self.indices.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

#[must_use]
pub fn mesh_from(render: &ConvexRenderMesh) -> Option<Mesh> {
    if render.indices.is_empty() {
        return None;
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, render.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, render.normals.clone());
    mesh.insert_indices(Indices::U32(render.indices.clone()));
    Some(mesh)
}

#[must_use]
pub fn build(
    world: &HexWfcWorld,
    snapshot: &HexWfcGeometrySnapshot,
    cells: &BTreeSet<HexCoord>,
    selected: Option<HexCoord>,
    bearing: Vec2,
    cutaway: bool,
    cache: &mut TileMeshCache,
) -> (
    HullBatch,
    BTreeMap<ArchitectureRegister, HullBatch>,
    DetailReport,
) {
    build_with_projection(
        world,
        snapshot,
        cells,
        selected,
        Projection::Cutaway {
            bearing,
            enabled: cutaway,
        },
        cache,
    )
}

/// Draw an authored deck with its ceilings removed and every perimeter hull
/// capped to `wall_height`. Floors, ramps, stairs, columns, and other interior
/// structure retain their authored shape. Collision data is never touched.
#[must_use]
pub fn build_low_walls(
    world: &HexWfcWorld,
    snapshot: &HexWfcGeometrySnapshot,
    cells: &BTreeSet<HexCoord>,
    selected: Option<HexCoord>,
    wall_height: f32,
    cache: &mut TileMeshCache,
) -> (
    HullBatch,
    BTreeMap<ArchitectureRegister, HullBatch>,
    DetailReport,
) {
    build_with_projection(
        world,
        snapshot,
        cells,
        selected,
        Projection::LowWalls {
            height: wall_height.max(0.1),
        },
        cache,
    )
}

/// Draw a deck under a named [`WallMode`].
///
/// The single entry point a viewer with a wall control wants; `build` and
/// `build_low_walls` remain for callers that only ever want one projection.
#[must_use]
pub fn build_walls(
    world: &HexWfcWorld,
    snapshot: &HexWfcGeometrySnapshot,
    cells: &BTreeSet<HexCoord>,
    selected: Option<HexCoord>,
    mode: WallMode,
    bearing: Vec2,
    cache: &mut TileMeshCache,
) -> (
    HullBatch,
    BTreeMap<ArchitectureRegister, HullBatch>,
    DetailReport,
) {
    let projection = match mode {
        WallMode::Cutaway => Projection::Cutaway {
            bearing,
            enabled: true,
        },
        WallMode::Partial => Projection::LowWalls {
            height: partial_wall_height(),
        },
        WallMode::Full => Projection::Cutaway {
            bearing,
            enabled: false,
        },
    };
    build_with_projection(world, snapshot, cells, selected, projection, cache)
}

#[derive(Clone, Copy)]
enum Projection {
    Cutaway { bearing: Vec2, enabled: bool },
    LowWalls { height: f32 },
}

fn build_with_projection(
    world: &HexWfcWorld,
    snapshot: &HexWfcGeometrySnapshot,
    cells: &BTreeSet<HexCoord>,
    selected: Option<HexCoord>,
    projection: Projection,
    cache: &mut TileMeshCache,
) -> (
    HullBatch,
    BTreeMap<ArchitectureRegister, HullBatch>,
    DetailReport,
) {
    let mut focus = HullBatch::default();
    let mut context: BTreeMap<ArchitectureRegister, HullBatch> = BTreeMap::new();
    let mut report = DetailReport {
        cells: cells.len(),
        ..DetailReport::default()
    };
    let mut by_cell: BTreeMap<HexCoord, Vec<&HexStructurePiece>> = BTreeMap::new();
    for piece in &snapshot.pieces {
        if cells.contains(&piece.source_cell) {
            by_cell.entry(piece.source_cell).or_default().push(piece);
        }
    }

    for (coord, pieces) in by_cell {
        let origin = Vec3::from_array(observed_hex::hex_origin(coord));
        let register = world
            .architecture
            .get(&coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let batch = if selected == Some(coord) {
            &mut focus
        } else {
            context.entry(register).or_default()
        };
        for (ordinal, piece) in pieces.iter().enumerate() {
            let ColliderShape::ConvexHull { points } = &piece.shape else {
                continue;
            };
            if points.is_empty() {
                continue;
            }
            let (min_y, max_y, centroid) = measure(points);
            let projected_points;
            let source = match projection {
                Projection::Cutaway { bearing, enabled } => {
                    if !survives(min_y, max_y, centroid, bearing, enabled) {
                        report.hulls_cut += 1;
                        continue;
                    }
                    points.as_slice()
                }
                Projection::LowWalls { height } => {
                    use observed_style::iso::HullRegion;
                    match observed_style::iso::hull_region(min_y, max_y, centroid) {
                        HullRegion::Ceiling => {
                            report.hulls_cut += 1;
                            continue;
                        }
                        HullRegion::Perimeter => {
                            projected_points = low_wall_points(points, min_y + height);
                            projected_points.as_slice()
                        }
                        HullRegion::Floor | HullRegion::Interior => points.as_slice(),
                    }
                }
            };
            let fresh: Option<CachedHull>;
            let cached = match piece.tile.as_ref() {
                Some(tile) => {
                    let prefix = match projection {
                        Projection::Cutaway { .. } => "full".to_string(),
                        Projection::LowWalls { height } => {
                            format!("low-{height:.3}")
                        }
                    };
                    let key = format!(
                        "{prefix}/{}/{}/{}#{ordinal}",
                        tile.archetype, tile.register, tile.variant
                    );
                    cache.entry(key, source)
                }
                None => {
                    fresh = TileMeshCache::build_hull(source);
                    fresh.as_ref()
                }
            };
            let Some(cached) = cached else { continue };
            {
                batch.push(&cached.mesh, origin);
                report.hulls_drawn += 1;
            }
        }
    }
    report.distinct_hulls_cached = cache.len();
    (focus, context, report)
}

fn low_wall_points(points: &[Vec3], top: f32) -> Vec<Vec3> {
    points
        .iter()
        .map(|point| Vec3::new(point.x, point.y.min(top), point.z))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutaway_preserves_floors_and_drops_near_walls() {
        let bearing = Vec2::X;
        assert!(survives(0.0, 0.5, Vec3::new(7.0, 0.2, 0.0), bearing, true));
        assert!(!survives(0.8, 3.0, Vec3::new(9.0, 1.5, 0.0), bearing, true));
        assert!(survives(0.8, 3.0, Vec3::new(-9.0, 1.5, 0.0), bearing, true));
    }

    #[test]
    fn detail_mode_labels_are_player_readable() {
        for mode in [
            DetailMode::Off,
            DetailMode::Focus,
            DetailMode::Layer,
            DetailMode::Neighborhood,
        ] {
            assert!(!mode.label().is_empty());
        }
    }

    #[test]
    fn a_low_wall_keeps_its_footprint_and_gains_a_flat_cap() {
        let points = vec![
            Vec3::new(-1.0, 0.0, -0.2),
            Vec3::new(1.0, 0.0, -0.2),
            Vec3::new(-1.0, 8.0, 0.2),
            Vec3::new(1.0, 8.0, 0.2),
            Vec3::new(-1.0, 0.0, 0.2),
            Vec3::new(1.0, 0.0, 0.2),
        ];
        let projected = low_wall_points(&points, 2.0);
        assert!(projected.iter().all(|point| point.y <= 2.0));
        assert!(projected.iter().any(|point| point.y == 2.0));
        assert!(TileMeshCache::build_hull(&projected).is_some());
    }
}
