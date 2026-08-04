//! Instances resident portions of the authoritative hex geometry snapshot: one render
//! mesh per collider piece, district-tinted by role and source-cell architecture. The
//! simulation owns the complete snapshot; this module keeps only lightweight indices
//! into it and projects requested cell parents on demand. The outer boundary shell is
//! small and remains resident for the whole match.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexCoord, HexWfcWorld};
use observed_hex::hex_origin;
use observed_match::hex_wfc::{
    HexLightSource, HexStructurePiece, HexStructureRole, HexTrimPiece, derive_trim_for,
};

use super::assets::HexWfcVisualAssets;
use super::{HexPractical, HexWfcGeometry};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

/// Per-tile fixture tuning (lighting-lab "per-place staging"). A ceiling-seated omni
/// fill lights the whole tile — floor, walls, ceiling — so every tile reads in first
/// person, not just a floor disc. Every non-boundary cell gets one; the shadow budget
/// ([`super::sync_practical_shadow_budget`]) turns a few of them into cast-shadow sources
/// near the runner, which is where the contrast comes from.
const PRACTICAL_BASE_INTENSITY: f32 = 720_000.0;
const PRACTICAL_RANGE: f32 = 14.0;
const PRACTICAL_HEIGHT: f32 = 5.6;
/// Connective halls on `pools_rhythm` registers still read a touch dimmer than the lit
/// places, preserving the register identity — but every tile stays clearly lit.
const HALL_RHYTHM_DIM: f32 = 0.7;

/// Lightweight, presentation-only lookup into the authoritative geometry vectors.
///
/// Keeping indices rather than cloning pieces makes the resident renderer cheap to
/// construct even for the production-sized facility. A relayout can reorder the
/// snapshot's packed vectors, so [`HexGeometryCatalog::rebuild`] is called after every
/// accepted geometry generation before any new resident cell is projected.
pub(super) struct HexGeometryCatalog {
    pub(super) generation: u32,
    pub(super) cells: BTreeMap<HexCoord, CellGeometryIndex>,
    pub(super) boundary_piece_indices: Vec<usize>,
}

pub(super) struct CellGeometryIndex {
    pub(super) footprint: Vec<HexCoord>,
    pub(super) piece_indices: Vec<usize>,
    pub(super) light_indices: Vec<usize>,
}

impl HexGeometryCatalog {
    pub(super) fn build(runtime: &HexWfcRuntime) -> Self {
        let world = &runtime.match_state.facility;
        let geometry = &runtime.match_state.geometry;
        let mut cells = BTreeMap::<HexCoord, CellGeometryIndex>::new();
        let mut boundary_piece_indices = Vec::new();
        for (index, piece) in geometry.pieces.iter().enumerate() {
            if piece.role == HexStructureRole::Boundary {
                boundary_piece_indices.push(index);
                continue;
            }
            cells
                .entry(piece.source_cell)
                .or_insert_with(|| CellGeometryIndex {
                    footprint: cell_footprint(world, piece.source_cell),
                    piece_indices: Vec::new(),
                    light_indices: Vec::new(),
                })
                .piece_indices
                .push(index);
        }
        for (index, light) in geometry.lights.iter().enumerate() {
            if let Some(cell) = cells.get_mut(&light.source_cell) {
                cell.light_indices.push(index);
            }
        }
        Self {
            generation: geometry.generation,
            cells,
            boundary_piece_indices,
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &HexWfcRuntime) {
        *self = Self::build(runtime);
    }

    pub(super) fn contains(&self, coord: HexCoord) -> bool {
        self.cells.contains_key(&coord)
    }
}

/// Result returned to the residency owner after one cell parent is projected. The
/// entity is a transient presentation handle; the stable key remains [`HexCoord`].
pub(super) struct SpawnedCell {
    pub(super) coord: HexCoord,
    pub(super) entity: Entity,
    pub(super) child_pieces: usize,
}

pub(super) fn spawn_boundary(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
    catalog: &HexGeometryCatalog,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    for (hull_index, &piece_index) in catalog.boundary_piece_indices.iter().enumerate() {
        if let Some(piece) = runtime.match_state.geometry.pieces.get(piece_index) {
            spawn_piece(
                commands,
                assets,
                meshes,
                piece,
                fallback_arch,
                None,
                hull_index,
            );
        }
    }
}

/// Group derived seam-trim pieces by their owning cell so each cell's parent
/// entity can own its trim (and a changed-cell rebuild regenerates only the
/// affected cells' trim).
fn group_trim_by_cell(trim: &[HexTrimPiece]) -> BTreeMap<HexCoord, Vec<&HexTrimPiece>> {
    let mut by_cell: BTreeMap<HexCoord, Vec<&HexTrimPiece>> = BTreeMap::new();
    for piece in trim {
        by_cell.entry(piece.cell).or_default().push(piece);
    }
    by_cell
}

/// The grid cells a spawned cell's fixtures actually occupy. An ordinary
/// tile's footprint is just its own coordinate — `push_tile` in
/// `observed_match::hex_wfc::geometry` keys it that way. A whole-room
/// module is different: `push_room` stamps `source_cell = anchor` on every
/// hull of the room, so all of a room's pieces are grouped here under one
/// coordinate, its anchor. Looking that anchor up against the solved
/// world's stamped blueprints recovers the room's true footprint, so
/// streaming and the defensive light fallback can treat every cell the room
/// actually occupies rather than only its anchor. For every coordinate that
/// is *not* a room anchor (all of today's catalog, and every ordinary tile
/// once rooms exist) this returns exactly `[coord]`, matching prior
/// behavior precisely.
fn cell_footprint(world: &HexWfcWorld, coord: HexCoord) -> Vec<HexCoord> {
    world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor == coord)
        .map(|blueprint| blueprint.cells.clone())
        .unwrap_or_else(|| vec![coord])
}

pub(super) fn spawn_cells(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
    catalog: &HexGeometryCatalog,
    requested: &BTreeSet<HexCoord>,
) -> Vec<SpawnedCell> {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    // Trim is derived only for the cells entering residency this frame. In particular,
    // off-screen relayout cells never pay a presentation rebuild cost.
    let trim = derive_trim_for(&runtime.match_state.geometry, requested);
    let mut trim_by_cell = group_trim_by_cell(&trim);
    let mut spawned = Vec::with_capacity(requested.len());
    for &coord in requested {
        let Some(index) = catalog.cells.get(&coord) else {
            continue;
        };
        let pieces = index
            .piece_indices
            .iter()
            .filter_map(|&piece_index| runtime.match_state.geometry.pieces.get(piece_index))
            .collect();
        let lights = index
            .light_indices
            .iter()
            .filter_map(|&light_index| runtime.match_state.geometry.lights.get(light_index))
            .collect();
        spawned.push(spawn_cell(
            commands,
            assets,
            meshes,
            CellProjection {
                coord,
                pieces,
                lights,
                trim: trim_by_cell.remove(&coord).unwrap_or_default(),
            },
            world,
            fallback_arch,
        ));
    }
    spawned
}

struct CellProjection<'a> {
    coord: HexCoord,
    pieces: Vec<&'a HexStructurePiece>,
    lights: Vec<&'a HexLightSource>,
    trim: Vec<&'a HexTrimPiece>,
}

fn spawn_cell(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    projection: CellProjection<'_>,
    world: &observed_facility::hex_wfc::HexWfcWorld,
    fallback_arch: ArchitectureRegister,
) -> SpawnedCell {
    let CellProjection {
        coord,
        pieces,
        lights,
        trim,
    } = projection;
    let architecture = *world.architecture.get(&coord).unwrap_or(&fallback_arch);
    let cell_role = pieces
        .first()
        .map_or(HexStructureRole::Hall, |piece| piece.role);
    let composition = super::lighting::composition_at(world, coord);
    let footprint = cell_footprint(world, coord);
    let cell = commands
        .spawn((
            HexWfcGeometry,
            DespawnOnExit(GameState::HexWfc),
            Transform::IDENTITY,
            Visibility::default(),
            Name::new(format!(
                "Hex cell q{} r{} L{}",
                coord.q, coord.r, coord.level
            )),
        ))
        .id();
    let mut child_pieces = spawn_cell_practicals(
        commands,
        assets,
        meshes,
        PracticalProjection {
            parent: cell,
            coord,
            footprint: &footprint,
            architecture,
            role: cell_role,
            composition,
            authored_lights: &lights,
        },
    );
    for (index, piece) in pieces.into_iter().enumerate() {
        child_pieces += usize::from(spawn_piece(
            commands,
            assets,
            meshes,
            piece,
            architecture,
            Some(cell),
            index,
        ));
    }
    for piece in trim {
        spawn_trim(commands, assets, meshes, piece, architecture, cell);
        child_pieces += 1;
    }
    SpawnedCell {
        coord,
        entity: cell,
        child_pieces,
    }
}

/// Spawn one derived seam-trim descriptor as a dim structural mesh, parented to
/// its owning cell so it streams and relayout-rebuilds with that cell. Trim is
/// non-authoritative decoration (no collider) and non-signal (Legibility
/// Contract) — it only hides tile seams and makes the facility read as one
/// megastructure.
fn spawn_trim(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexTrimPiece,
    architecture: ArchitectureRegister,
    parent: Entity,
) {
    let mesh = assets.trim_mesh(meshes, piece.kind);
    let material = assets.trim_material(architecture);
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(piece.position).with_rotation(Quat::from_array(piece.rotation)),
        Name::new(format!("Hex trim {:?} {:?}", piece.kind, piece.face)),
        ChildOf(parent),
    ));
}

/// Stage the per-tile downlight fixture for a cell — tier 2 of the lighting-lab rig.
///
/// A `light_color`-tinted omni fill seated near the ceiling, parented to the cell so it
/// streams (and relayout-rebuilds) with its geometry. Every non-boundary cell gets one or
/// more, with a defensive centered fallback, so no tile is unlit;
/// `pools_rhythm` halls read a touch dimmer for register identity.
/// Shadows start off — [`super::sync_practical_shadow_budget`] turns them on for the
/// handful of fixtures nearest the runner each time the cell changes, so wherever you
/// stand there is real cast-shadow contrast.
struct PracticalProjection<'a> {
    parent: Entity,
    coord: HexCoord,
    /// The full set of cells this fixture group is responsible for lighting
    /// — `[coord]` for an ordinary tile, or a room's complete footprint when
    /// `coord` is a whole-room module's anchor (see [`cell_footprint`]).
    footprint: &'a [HexCoord],
    architecture: ArchitectureRegister,
    role: HexStructureRole,
    composition: observed_style::HexComposition,
    authored_lights: &'a [&'a HexLightSource],
}

fn spawn_cell_practicals(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    projection: PracticalProjection<'_>,
) -> usize {
    let PracticalProjection {
        parent,
        coord,
        footprint,
        architecture,
        role,
        composition,
        authored_lights,
    } = projection;
    if role == HexStructureRole::Boundary {
        return 0;
    }
    let palette = observed_style::architecture_for_composition(architecture, composition);
    let is_place = composition != observed_style::HexComposition::Hall;
    let role_scale = match composition {
        observed_style::HexComposition::Room => 1.0,
        observed_style::HexComposition::Vertical => 1.05,
        observed_style::HexComposition::Hall => 0.85,
    };
    let rhythm_dim = if palette.pools_rhythm && !is_place {
        HALL_RHYTHM_DIM
    } else {
        1.0
    };
    let has_authored_lights = !authored_lights.is_empty();
    let positions: Vec<Vec3> = if !has_authored_lights {
        // Defensive fallback: one fixture per footprint cell, not just the
        // anchor, so a whole-room module with no authored lights still has
        // every part of its floor lit (Legibility Contract). For an
        // ordinary tile `footprint` is exactly `[coord]`, so this produces
        // the same single fixture as before.
        footprint
            .iter()
            .map(|&cell| Vec3::from_array(hex_origin(cell)) + Vec3::Y * PRACTICAL_HEIGHT)
            .collect()
    } else {
        authored_lights
            .iter()
            .map(|source| source.position)
            .collect()
    };
    let per_source_scale = (positions.len() as f32).sqrt().recip().clamp(0.55, 1.0);
    let mut child_pieces = 0;
    for position in positions {
        if has_authored_lights && matches!(role, HexStructureRole::Room | HexStructureRole::Hall) {
            // A diffuser is geometry, so it gets the same two treatments the
            // rest of the geometry gets: the storey filter and the cutaway.
            //
            // It used to get neither. The light beside it carried
            // `HexPractical` and the mesh carried nothing, so it drew on every
            // storey at once whatever the cutaway said - and a ceiling-mounted
            // diffuser whose ceiling had been cut away is a thin bright stub
            // hanging in the air. That is what the "floating fixtures" over the
            // overview's floor plan were.
            let origin = Vec3::from_array(hex_origin(coord));
            let at = position + Vec3::Y * 0.18;
            commands.spawn((
                Mesh3d(assets.fixture_mesh(meshes)),
                MeshMaterial3d(assets.register(architecture).fixture()),
                Transform::from_translation(at),
                HexPractical(coord),
                // Measured as a point: a diffuser is small next to the tests
                // being applied to it, and its height is what decides them.
                super::spectate::Cutaway {
                    local: at - origin,
                    min_y: at.y - origin.y,
                    max_y: at.y - origin.y,
                    origin_y: origin.y,
                    cell_level: coord.level,
                },
                ChildOf(parent),
                Name::new("Authored fluorescent diffuser"),
            ));
            child_pieces += 1;
        }
        commands.spawn((
            HexPractical(coord),
            PointLight {
                color: palette.light_color,
                intensity: PRACTICAL_BASE_INTENSITY * role_scale * rhythm_dim * per_source_scale,
                range: PRACTICAL_RANGE,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(position),
            ChildOf(parent),
            Name::new(if authored_lights.is_empty() {
                "Legacy tile fill"
            } else {
                "Authored tile practical"
            }),
        ));
        child_pieces += 1;
    }
    child_pieces
}

fn spawn_piece(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexStructurePiece,
    architecture: ArchitectureRegister,
    parent: Option<Entity>,
    hull_index: usize,
) -> bool {
    let Some(mesh) = assets.mesh_for(meshes, piece, hull_index) else {
        return false;
    };
    let material = assets.material_for_piece(architecture, piece);
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(piece.center).with_rotation(Quat::from_array(piece.rotation)),
        Name::new(format!("Hex {:?} collider {}", piece.role, piece.id.0)),
    ));
    if let Some(parent) = parent {
        entity.insert(ChildOf(parent));
    } else {
        entity.insert((HexWfcGeometry, DespawnOnExit(GameState::HexWfc)));
    }
    // Tagged where the role is already known, so the spectator overview can
    // drop it: in play the shell is the far wall you never reach, but from
    // outside looking in it is a lid over the whole building.
    if piece.role == HexStructureRole::Boundary {
        entity.insert(super::spectate::BoundaryShell);
    } else {
        // Measured once, here: the overview's cutaway needs a hull's height
        // range and its offset within its cell on every rebuild, and computing
        // it per frame means walking the point cloud again.
        entity.insert(cutaway_measure(piece));
    }
    true
}

/// A hull's height range and its centroid within its own cell.
fn cutaway_measure(piece: &HexStructurePiece) -> super::spectate::Cutaway {
    let rotation = Quat::from_array(piece.rotation);
    let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
    // The *hull's own* centroid, not the collider's origin.
    //
    // `piece.center` is where the collider is placed, which for a convex hull
    // is the tile origin - so using it made every hull look like it sat at the
    // cell centre, `INTERIOR_RADIUS` matched them all, and no near wall was
    // ever cut. Only ceilings were, which is exactly why the cutaway read as
    // half-working here and correct in the studio: `detail::measure` averages
    // the points.
    let mut centroid = Vec3::ZERO;
    match &piece.shape {
        observed_traversal::ColliderShape::Cuboid { half } => {
            // A rotated box's vertical reach is each axis's contribution to Y,
            // which is exactly the AABB half-height. Its centroid is its centre.
            let reach = (rotation * Vec3::new(half.x, 0.0, 0.0)).y.abs()
                + (rotation * Vec3::new(0.0, half.y, 0.0)).y.abs()
                + (rotation * Vec3::new(0.0, 0.0, half.z)).y.abs();
            min_y = piece.center.y - reach;
            max_y = piece.center.y + reach;
            centroid = piece.center;
        }
        observed_traversal::ColliderShape::ConvexHull { points } => {
            let mut sum = Vec3::ZERO;
            for point in points {
                let placed = rotation * *point + piece.center;
                min_y = min_y.min(placed.y);
                max_y = max_y.max(placed.y);
                sum += placed;
            }
            if !points.is_empty() {
                #[allow(clippy::cast_precision_loss)]
                {
                    centroid = sum / points.len() as f32;
                }
            }
        }
    }
    let origin = Vec3::from_array(hex_origin(piece.source_cell));
    super::spectate::Cutaway {
        local: centroid - origin,
        min_y: min_y - origin.y,
        max_y: max_y - origin.y,
        origin_y: origin.y,
        cell_level: piece.source_cell.level,
    }
}
