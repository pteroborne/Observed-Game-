//! Instances the authoritative hex geometry snapshot: one render mesh per collider
//! piece, district-tinted by role and source-cell architecture. Tile pieces are grouped
//! under a per-cell parent so visibility streaming can hide distant cells wholesale; the
//! outer boundary shell is always visible.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexCoord;
use observed_hex::hex_origin;
use observed_match::hex_wfc::{
    HexLightSource, HexStructurePiece, HexStructureRole, HexTrimPiece, derive_trim,
};

use super::assets::HexWfcVisualAssets;
use super::{HexPractical, HexWfcCell, HexWfcGeometry};
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

pub(super) fn spawn_geometry(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);

    let mut by_cell: BTreeMap<HexCoord, Vec<&HexStructurePiece>> = BTreeMap::new();
    let mut lights_by_cell: BTreeMap<HexCoord, Vec<&HexLightSource>> = BTreeMap::new();
    for light in &runtime.match_state.geometry.lights {
        lights_by_cell
            .entry(light.source_cell)
            .or_default()
            .push(light);
    }
    for piece in &runtime.match_state.geometry.pieces {
        if piece.role == HexStructureRole::Boundary {
            spawn_piece(commands, assets, meshes, piece, fallback_arch, None, 0);
        } else {
            by_cell.entry(piece.source_cell).or_default().push(piece);
        }
    }

    let trim = derive_trim(&runtime.match_state.geometry);
    let mut trim_by_cell = group_trim_by_cell(&trim);

    for (coord, pieces) in by_cell {
        let lights = lights_by_cell.remove(&coord).unwrap_or_default();
        spawn_cell(
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
        );
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

pub(super) fn spawn_cells(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
    changed: &BTreeSet<HexCoord>,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    let mut by_cell: BTreeMap<HexCoord, Vec<&HexStructurePiece>> = BTreeMap::new();
    let mut lights_by_cell: BTreeMap<HexCoord, Vec<&HexLightSource>> = BTreeMap::new();
    for light in runtime
        .match_state
        .geometry
        .lights
        .iter()
        .filter(|light| changed.contains(&light.source_cell))
    {
        lights_by_cell
            .entry(light.source_cell)
            .or_default()
            .push(light);
    }
    for piece in runtime.match_state.geometry.pieces.iter().filter(|piece| {
        piece.role != HexStructureRole::Boundary && changed.contains(&piece.source_cell)
    }) {
        by_cell.entry(piece.source_cell).or_default().push(piece);
    }
    let trim = derive_trim(&runtime.match_state.geometry);
    let mut trim_by_cell = group_trim_by_cell(&trim);
    for (coord, pieces) in by_cell {
        let lights = lights_by_cell.remove(&coord).unwrap_or_default();
        spawn_cell(
            commands,
            assets,
            meshes,
            CellProjection {
                coord,
                pieces,
                lights,
                // Only the changed cells are rebuilt here, so their trim is
                // regenerated with them; unchanged cells keep their existing
                // trim entities.
                trim: trim_by_cell.remove(&coord).unwrap_or_default(),
            },
            world,
            fallback_arch,
        );
    }
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
) {
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
    let cell = commands
        .spawn((
            HexWfcCell(coord),
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
    spawn_cell_practicals(
        commands,
        assets,
        meshes,
        PracticalProjection {
            parent: cell,
            coord,
            architecture,
            role: cell_role,
            authored_lights: &lights,
        },
    );
    for (index, piece) in pieces.into_iter().enumerate() {
        spawn_piece(
            commands,
            assets,
            meshes,
            piece,
            architecture,
            Some(cell),
            index,
        );
    }
    for piece in trim {
        spawn_trim(commands, assets, meshes, piece, architecture, cell);
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
    architecture: ArchitectureRegister,
    role: HexStructureRole,
    authored_lights: &'a [&'a HexLightSource],
}

fn spawn_cell_practicals(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    projection: PracticalProjection<'_>,
) {
    let PracticalProjection {
        parent,
        coord,
        architecture,
        role,
        authored_lights,
    } = projection;
    if role == HexStructureRole::Boundary {
        return;
    }
    let palette = observed_style::architecture(architecture);
    let is_place = matches!(
        role,
        HexStructureRole::Room | HexStructureRole::Shaft | HexStructureRole::Ramp
    );
    let role_scale = match role {
        HexStructureRole::Shaft => 1.15,
        HexStructureRole::Room => 1.0,
        HexStructureRole::Ramp => 0.95,
        HexStructureRole::Hall => 0.85,
        HexStructureRole::Boundary => return,
    };
    let rhythm_dim = if palette.pools_rhythm && !is_place {
        HALL_RHYTHM_DIM
    } else {
        1.0
    };
    let has_authored_lights = !authored_lights.is_empty();
    let positions = if !has_authored_lights {
        vec![Vec3::from_array(hex_origin(coord)) + Vec3::Y * PRACTICAL_HEIGHT]
    } else {
        authored_lights
            .iter()
            .map(|source| source.position)
            .collect()
    };
    let per_source_scale = (positions.len() as f32).sqrt().recip().clamp(0.55, 1.0);
    for position in positions {
        if has_authored_lights && matches!(role, HexStructureRole::Room | HexStructureRole::Hall) {
            commands.spawn((
                Mesh3d(assets.fixture_mesh(meshes)),
                MeshMaterial3d(assets.register(architecture).fixture()),
                Transform::from_translation(position + Vec3::Y * 0.18),
                ChildOf(parent),
                Name::new("Authored fluorescent diffuser"),
            ));
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
    }
}

fn spawn_piece(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexStructurePiece,
    architecture: ArchitectureRegister,
    parent: Option<Entity>,
    hull_index: usize,
) {
    let Some(mesh) = assets.mesh_for(meshes, piece, hull_index) else {
        return;
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
}
