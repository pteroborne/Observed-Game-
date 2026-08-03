//! Real authored geometry, cut away so you can see inside it.
//!
//! The schematic answers *topology* — which faces are open, where the rooms
//! are. It cannot answer *craft*: do these two tiles actually meet, is the
//! doorway where the solver thinks it is, does this seam line up. Those are
//! questions about the authored hulls, so this module draws them.
//!
//! # Why a cutaway is not optional
//!
//! The camera is a fixed isometric looking down at roughly 35 degrees. A tile
//! rendered whole shows you its ceiling and the three walls between you and its
//! interior, which is to say: nothing you wanted. So three tests decide what
//! survives, in this order:
//!
//! 1. **Floor always stays.** Anything whose top sits at or below the floor
//!    slab. Cull it and cells appear to float.
//! 2. **Ceiling goes** when a hull's *lowest* point is above head clearance.
//!    Deliberately min-Y and not the centroid: a pillar spanning floor to
//!    ceiling has a high centroid, and dropping it would silently delete
//!    structure rather than roofing.
//! 3. **The near walls go** — a hull outside the interior radius whose plan
//!    azimuth lies within 90 degrees of the camera bearing. At any bearing that
//!    is three of the six walls, which falls out of hex geometry rather than
//!    being a tuned number.
//!
//! # Overlay, never replace
//!
//! The schematic keeps drawing on top of this. Its walls come from
//! `placement.is_open(face)` — the *solver's* truth — while these hulls are the
//! *authored* truth. Where the two disagree there is a real bug: a face the
//! solver routes traffic through that the tile has walled off. That is exactly
//! what `tilec audit-seams` exists to catch, and showing both makes it visible
//! for free.

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexFace, HexWfcWorld, PortClass};
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexStructurePiece, HexWfcGeometrySnapshot};
use observed_traversal::{ColliderShape, ConvexRenderMesh};

use observed_content::ArchitectureRegister;
use std::collections::{BTreeMap, BTreeSet};

// The cutaway rule and its thresholds live in `observed_style::iso`, shared
// with the game's spectator overview: two surfaces cutting away differently
// would show the same facility as two buildings.

/// Which cells draw their real geometry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DetailMode {
    /// Schematic only.
    #[default]
    Off,
    /// The selected cell and everything it actually connects to. Bounded at
    /// eight cells, so this costs the same on a 5,600-cell facility as on a
    /// hundred-cell one.
    Focus,
    /// Every cell on the **focus** floor. Floors above and below stay dim
    /// wireframe, so vertical context is visible without ten layers of solid
    /// geometry hiding each other — which is an occlusion problem before it is
    /// a performance one.
    ///
    /// Refused on "all layers": one production floor is roughly fourteen
    /// thousand hulls, heavy but drawable; ten floors is over a hundred
    /// thousand, and the top one would hide the rest anyway.
    Layer,
}

impl DetailMode {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "schematic",
            Self::Focus => "detail: focus",
            Self::Layer => "detail: layer",
        }
    }
}

/// What the last detail pass drew, for the status line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DetailReport {
    pub cells: usize,
    pub hulls_drawn: usize,
    pub hulls_cut: usize,
    /// Distinct hulls triangulated so far this session. Grows to the size of
    /// the tile vocabulary in use and then stops, however many cells are drawn.
    pub distinct_hulls_cached: usize,
}

/// The cells to draw in detail.
///
/// In [`DetailMode::Focus`] this is the selected cell plus every neighbour it
/// shares an **open port** with — the connection set, which is precisely what
/// "which tiles are connecting" is asking about. A sealed neighbour is not part
/// of the question and only gets in the way.
#[must_use]
pub fn focus_set(world: &HexWfcWorld, selected: Option<HexCoord>) -> BTreeSet<HexCoord> {
    let mut set = BTreeSet::new();
    let Some(centre) = selected else {
        return set;
    };
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
        if !open {
            continue;
        }
        if let Some(neighbour) = grid.neighbor(centre, face)
            && world.placements.contains_key(&neighbour)
        {
            set.insert(neighbour);
        }
    }
    set
}

/// Whether a hull survives the cutaway. See `observed_style::iso::survives`.
#[must_use]
pub fn survives(min_y: f32, max_y: f32, local: Vec3, bearing: Vec2, cutaway: bool) -> bool {
    observed_style::iso::survives(min_y, max_y, local, bearing, cutaway)
}

/// One hull, triangulated once and measured once.
///
/// The extents ride along with the mesh because the cutaway needs them on every
/// rebuild and recomputing them means walking the point cloud again.
pub struct CachedHull {
    mesh: ConvexRenderMesh,
    min_y: f32,
    max_y: f32,
    centroid: Vec3,
}

/// Triangulated tile geometry, computed once per distinct tile and instanced by
/// offset at every cell that resolves to it.
///
/// Without this, a rebuild re-triangulates the same handful of prototypes once
/// per placement — a hundred cells of one hall shape pay for that shape a
/// hundred times. `iso_observer_lab`'s `EdgeCache` makes exactly this trade for
/// line work and states the reason in the same terms.
///
/// **Keying by [`TileKey`] is safe because rotation is part of the key.**
/// `runtime_catalog` assigns `variant = module.variant * 6 + turn`, so each of a
/// module's six turns is a distinct variant, and the projector bakes the turn
/// into the hull points (it leaves `HexStructurePiece::rotation` at identity).
/// Two cells sharing a key therefore share geometry exactly.
#[derive(Default, Resource)]
pub struct TileMeshCache {
    by_key: BTreeMap<String, Option<CachedHull>>,
}

impl TileMeshCache {
    /// The cached hull for `key`, triangulating and measuring on first sight.
    ///
    /// `None` means the hull is degenerate — a coplanar sliver, say. That is
    /// cached too, so a bad hull is not re-triangulated every rebuild just to
    /// fail again.
    pub fn entry(&mut self, key: String, points: &[Vec3]) -> Option<&CachedHull> {
        self.by_key
            .entry(key)
            .or_insert_with(|| Self::build_owned(points))
            .as_ref()
    }

    /// Build without caching, for hulls that have no stable key.
    #[must_use]
    pub fn build_hull(points: &[Vec3]) -> Option<CachedHull> {
        Self::build_owned(points)
    }

    fn build_owned(points: &[Vec3]) -> Option<CachedHull> {
        let mesh = ConvexRenderMesh::from_convex_hull(points)?;
        let (min_y, max_y, centroid) = measure(points);
        Some(CachedHull {
            mesh,
            min_y,
            max_y,
            centroid,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// Drop everything. Needed if the corpus changes under the tool.
    pub fn clear(&mut self) {
        self.by_key.clear();
    }
}

/// A hull's vertical extent and centroid.
///
/// Public because the module viewer runs the same cutaway over raw authored
/// hulls: one classifier, so a module and the facility it lands in cut away
/// identically.
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

/// Accumulated triangles for one colour of detail geometry.
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
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::RENDER_WORLD
                | bevy::asset::RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_indices(bevy::mesh::Indices::U32(self.indices));
        Some(mesh)
    }
}

/// A Bevy mesh from one convex hull.
///
/// `ConvexRenderMesh` carries positions, normals, and indices but no Bevy
/// dependency - it lives in `observed_traversal`, which the headless simulation
/// also uses. This is the one place that bridge is crossed, so a renderer never
/// has to know the attribute names.
#[must_use]
pub fn mesh_from(render: &ConvexRenderMesh) -> Option<Mesh> {
    if render.indices.is_empty() {
        return None;
    }
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, render.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, render.normals.clone());
    mesh.insert_indices(bevy::mesh::Indices::U32(render.indices.clone()));
    Some(mesh)
}

/// Build the cut-away solid geometry for `cells`.
///
/// Returns one batch for the selected cell and one for everything else, so the
/// cell under inspection can be told apart from its neighbours without a second
/// pass over the hulls.
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
    let mut focus = HullBatch::default();
    // Context hulls batch by district, because solid massing is drawn in the
    // *surface* language rather than the schematic one: this view shows the
    // geometry that will really be built, so it should look like it.
    let mut context: BTreeMap<ArchitectureRegister, HullBatch> = BTreeMap::new();
    let mut report = DetailReport {
        cells: cells.len(),
        ..DetailReport::default()
    };

    // Group by cell first. The projector emits one piece per hull, so a piece's
    // position within its cell is a stable ordinal, and (tile key, ordinal)
    // identifies one hull of one tile exactly. Iterating pieces flat and
    // caching per tile would either re-emit a tile's whole hull set once per
    // hull, or need a guard that breaks on cells mixing tiled and untiled
    // pieces. Grouping avoids both.
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

            // Boundary shells carry no tile key. There are few of them, and
            // filing them under one shared key would hand back another shell's
            // geometry, so they are built fresh and held here for the borrow.
            let fresh: Option<CachedHull>;
            let cached = match piece.tile.as_ref() {
                Some(tile) => {
                    let key = format!(
                        "{}/{}/{}#{ordinal}",
                        tile.archetype, tile.register, tile.variant
                    );
                    cache.entry(key, points.as_slice())
                }
                None => {
                    fresh = TileMeshCache::build_hull(points.as_slice());
                    fresh.as_ref()
                }
            };
            let Some(cached) = cached else { continue };

            if survives(
                cached.min_y,
                cached.max_y,
                cached.centroid,
                bearing,
                cutaway,
            ) {
                batch.push(&cached.mesh, origin);
                report.hulls_drawn += 1;
            } else {
                report.hulls_cut += 1;
            }
        }
    }

    report.distinct_hulls_cached = cache.len();
    (focus, context, report)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Looking from +X: the bearing points from the scene toward the camera.
    fn bearing() -> Vec2 {
        Vec2::new(1.0, 0.0)
    }

    #[test]
    fn the_floor_is_never_cut_away() {
        // Low, wide, and on the near side: every other rule would drop it.
        assert!(survives(
            0.0,
            0.5,
            Vec3::new(7.0, 0.2, 0.0),
            bearing(),
            true
        ));
    }

    /// Min-Y, not the centroid. A pillar from floor to ceiling has a high
    /// centroid; culling it would delete structure and call it roofing.
    #[test]
    fn a_full_height_pillar_survives_but_a_ceiling_slab_does_not() {
        let pillar_centroid = Vec3::new(0.0, 4.0, 0.0);
        assert!(
            survives(0.4, 8.0, pillar_centroid, bearing(), true),
            "a pillar spanning the cell is structure"
        );
        assert!(
            !survives(7.5, 8.0, Vec3::new(0.0, 7.8, 0.0), bearing(), true),
            "a slab starting above head clearance is roofing"
        );
    }

    #[test]
    fn interior_fittings_are_never_treated_as_near_walls() {
        // Inside the interior radius, on the near side, at wall height.
        assert!(survives(
            1.0,
            2.0,
            Vec3::new(1.0, 1.5, 0.0),
            bearing(),
            true
        ));
    }

    /// The whole point: walls between the camera and the interior go, the far
    /// ones stay, and it is three of six either way.
    #[test]
    fn the_near_walls_are_cut_and_the_far_walls_are_kept() {
        let mut kept = 0;
        let mut cut = 0;
        for index in 0..6 {
            #[allow(clippy::cast_precision_loss)]
            let angle = std::f32::consts::TAU * index as f32 / 6.0;
            let local = Vec3::new(angle.cos() * 7.0, 1.5, angle.sin() * 7.0);
            if survives(1.0, 2.5, local, bearing(), true) {
                kept += 1;
            } else {
                cut += 1;
            }
        }
        assert_eq!(kept + cut, 6);
        assert_eq!(cut, 3, "exactly half the perimeter faces the camera");
    }

    /// Rotating the view must move which walls are cut, or the detents do
    /// nothing and the far side is never inspectable.
    #[test]
    fn rotating_the_bearing_moves_which_walls_are_cut() {
        let wall = Vec3::new(7.0, 1.5, 0.0);
        assert!(
            !survives(1.0, 2.5, wall, Vec2::new(1.0, 0.0), true),
            "cut when the camera is on that side"
        );
        assert!(
            survives(1.0, 2.5, wall, Vec2::new(-1.0, 0.0), true),
            "kept when the camera is opposite"
        );
    }

    #[test]
    fn cutaway_off_keeps_everything() {
        assert!(survives(
            7.5,
            8.0,
            Vec3::new(7.0, 7.8, 0.0),
            bearing(),
            false
        ));
    }
}
