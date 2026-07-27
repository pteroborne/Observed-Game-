//! The full-screen isometric survivor map (Tab).
//!
//! Promoted from `iso_observer_lab`, which is where the projection was proven.
//! The lab renders ground truth because its job is to audit what the WFC
//! composed; this one renders **only what the team has discovered**, because its
//! job is to be a survivor's sketch. The whole module reads
//! [`HexPlayerMapKnowledge`] and indexes the facility solely at cells that
//! knowledge already contains — see `discovered_cells`. Nothing here may consult
//! rival players, other teams' knowledge, or an undiscovered placement.
//!
//! Two channels, as in the lab: colour is the district, height is the
//! archetype. On top of those, every survivor state the corner sketch carried is
//! preserved — traversed reads bright, glimpsed dim, stale grey, and the focus
//! level is lifted out of the stack — while the signal-tier cells (you, the
//! exit, a deployed anchor) are recoloured outright so they punch through the
//! district palette the way the Legibility Contract requires.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexWfcWorld};
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT, hex_origin, prism_hull};
use observed_match::hex_wfc::{HexMapCellKnowledge, HexMapDiscovery, HexPlayerMapKnowledge};
use observed_style::{MarkerRole, ObservedState, SurfaceRole};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

use super::assets::hull_mesh;

/// The map draws on its own layer so the facility never bleeds into it and it
/// never bleeds into the facility. Layer 1 belongs to the portal previews.
const MAP_RENDER_LAYER: usize = 2;
/// Drawn after the world camera, with an opaque clear, so it covers the frame.
const MAP_CAMERA_ORDER: isize = 1;
const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 900.0;
/// True isometric pitch: `atan(1 / sqrt(2))`.
const ISO_PITCH: f32 = -0.615_479_7;
/// Cells read as countable tiles without open regions losing continuity.
const CELL_INSET: f32 = 0.94;
/// How far a level off the focus floor is pushed down in brightness.
const UNFOCUSED_DIM: f32 = 0.42;
/// Glimpsed cells are dimmer than traversed ones but must stay readable.
const GLIMPSED_DIM: f32 = 0.5;

#[derive(Component)]
pub(crate) struct HexMapVisual;

#[derive(Component)]
pub(crate) struct HexMapCamera;

#[derive(Component)]
pub(crate) struct HexMapLegend;

/// Change-detection cache, so the map rebuilds on a real change rather than
/// every frame — the same contract the corner sketch held.
#[derive(Resource, Default)]
pub(crate) struct HexMapProjection {
    signature: u64,
    built: bool,
}

pub(in crate::hex_wfc) fn setup(mut commands: Commands) {
    commands.insert_resource(HexMapProjection::default());
    commands.spawn((
        HexMapCamera,
        DespawnOnExit(GameState::HexWfc),
        Camera3d::default(),
        Camera {
            order: MAP_CAMERA_ORDER,
            is_active: false,
            clear_color: Color::srgb(0.008, 0.010, 0.020).into(),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0,
            ..OrthographicProjection::default_3d()
        }),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::default(),
        Name::new("Hex survivor map camera"),
    ));
    // The map carries its own key. Borrowing the world's rig would make the
    // survivor's sketch change brightness with whatever district the runner
    // happens to be standing in, and would leave it unlit wherever the world's
    // lights are not on this render layer.
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        DirectionalLight {
            illuminance: 3_200.0,
            shadows_enabled: false,
            ..default()
        },
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.95, 0.0)),
        Name::new("Hex survivor map key"),
    ));
    commands.spawn((
        HexMapLegend,
        DespawnOnExit(GameState::HexWfc),
        Visibility::Hidden,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(observed_style::marker(MarkerRole::You).base_color),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(18),
            ..default()
        },
        GlobalZIndex(60),
    ));
}

pub(in crate::hex_wfc) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HexMapProjection>();
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    mut projection: ResMut<HexMapProjection>,
    mut camera: Query<(&mut Camera, &mut Projection, &mut Transform), With<HexMapCamera>>,
    mut legend: Query<(&mut Visibility, &mut Text), With<HexMapLegend>>,
    existing: Query<Entity, With<HexMapVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let open = runtime.map_open;
    if let Ok((mut camera, _, _)) = camera.single_mut() {
        camera.is_active = open;
    }
    if let Ok((mut visibility, _)) = legend.single_mut() {
        *visibility = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if !open {
        // Keep the built geometry: reopening the map is a frequent, cheap action
        // and a survivor should not pay a full rebuild for a glance.
        return;
    }
    let signature = signature(&runtime);
    if projection.built && projection.signature == signature {
        return;
    }
    projection.signature = signature;
    projection.built = true;

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let Some(knowledge) = runtime.match_state.player_map(runtime.local_player) else {
        return;
    };
    let world = &runtime.match_state.facility;
    let focus = runtime.map_level;
    let you_cell = runtime.local().cell;
    let exit_cell = world.config.exit();

    let mut bounds: Option<(Vec3, Vec3)> = None;
    let mut mesh_cache: std::collections::BTreeMap<u32, Handle<Mesh>> =
        std::collections::BTreeMap::new();
    let mut material_cache: std::collections::BTreeMap<(u8, u8, u8, u8), Handle<StandardMaterial>> =
        std::collections::BTreeMap::new();

    for (&cell, known) in &knowledge.cells {
        // Indexing the facility is gated on membership in `knowledge.cells`, so
        // an undiscovered placement is never read.
        let Some(placement) = world.placements.get(&cell) else {
            continue;
        };
        let Some(height) = archetype_height(placement.archetype) else {
            continue;
        };
        let register = world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let state = CellState::of(known, world, cell);
        let focused = cell.level == focus;
        // Signal tier overrides the district. The Legibility Contract requires
        // gameplay-critical cues to punch through atmosphere, and "where am I"
        // and "where is the way out" are exactly that. Recolouring the cell
        // rather than floating a token above it is deliberate: a token is
        // occluded by whatever sits on the level above, a recoloured cell never
        // is. This is the 3D equivalent of the corner sketch's `@` and `X`.
        let signal = if cell == you_cell {
            Some(MarkerRole::You)
        } else if cell == exit_cell {
            Some(MarkerRole::Exit)
        } else if known.anchored {
            Some(MarkerRole::Control)
        } else {
            None
        };

        let mesh = mesh_cache
            .entry(height.to_bits())
            .or_insert_with(|| {
                let hull = prism_hull(height, CELL_INSET).map(Vec3::from_array);
                meshes.add(hull_mesh(&hull).expect("a hex prism is a non-degenerate hull"))
            })
            .clone();
        let material = material_cache
            .entry((
                register as u8,
                state.key(),
                u8::from(focused),
                signal.map_or(0, marker_key),
            ))
            .or_insert_with(|| {
                let (base_color, emissive) = match signal {
                    Some(role) => {
                        let treatment = observed_style::marker(role);
                        (treatment.base_color, treatment.emissive)
                    }
                    None => state.treatment(register, focused),
                };
                materials.add(StandardMaterial {
                    base_color,
                    emissive,
                    perceptual_roughness: 0.92,
                    ..default()
                })
            })
            .clone();

        let origin = Vec3::from_array(hex_origin(cell));
        bounds = Some(match bounds {
            None => (
                origin - Vec3::new(8.0, 0.0, 8.0),
                origin + Vec3::new(8.0, height, 8.0),
            ),
            Some((min, max)) => (
                min.min(origin - Vec3::new(8.0, 0.0, 8.0)),
                max.max(origin + Vec3::new(8.0, height, 8.0)),
            ),
        });
        commands.spawn((
            HexMapVisual,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(origin),
            Name::new("Hex map cell"),
        ));
    }

    if let (Some((min, max)), Ok((_, mut projection, mut transform))) =
        (bounds, camera.single_mut())
    {
        let (framed, scale, far) = frame_map(min, max);
        *transform = framed;
        *projection = Projection::Orthographic(OrthographicProjection {
            scale,
            near: 0.1,
            // The 3D default far plane is 1000 m, which a production facility's
            // diagonal exceeds on its own — leave it and the map is clipped.
            far,
            ..OrthographicProjection::default_3d()
        });
    }

    if let Ok((_, mut text)) = legend.single_mut() {
        **text = legend_text(knowledge, world, focus);
    }
}

/// Distinct cache key per signal role, so two roles never share a material.
fn marker_key(role: MarkerRole) -> u8 {
    match role {
        MarkerRole::You => 1,
        MarkerRole::Exit => 2,
        MarkerRole::Control => 3,
        _ => 4,
    }
}

/// The survivor states the corner sketch carried, preserved verbatim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CellState {
    Traversed,
    Glimpsed,
    Stale,
}

impl CellState {
    /// Stale wins over discovery: a cell the facility has rewired past is no
    /// longer trustworthy no matter how well it was once known. An anchored
    /// cell is never stale, which is what makes a deployed lantern worth having.
    pub(super) fn of(known: &HexMapCellKnowledge, world: &HexWfcWorld, cell: HexCoord) -> Self {
        if known.is_stale(world, cell) {
            Self::Stale
        } else if known.discovery == HexMapDiscovery::Traversed {
            Self::Traversed
        } else {
            Self::Glimpsed
        }
    }

    fn key(self) -> u8 {
        match self {
            Self::Traversed => 0,
            Self::Glimpsed => 1,
            Self::Stale => 2,
        }
    }

    /// Colour is the district accent; the survivor state modulates it. Stale
    /// deliberately leaves the district palette entirely and reads as inert
    /// structure, because the district is exactly what may have changed.
    fn treatment(self, register: ArchitectureRegister, focused: bool) -> (Color, LinearRgba) {
        let gain = if focused { 1.0 } else { UNFOCUSED_DIM };
        let (base, emissive) = match self {
            Self::Stale => {
                let treatment = observed_style::observed_modulate(
                    observed_style::surface(SurfaceRole::Wall),
                    ObservedState::Unobserved,
                );
                (treatment.base_color, treatment.emissive)
            }
            Self::Traversed => {
                let accent = observed_style::architecture(register).accent;
                (Color::LinearRgba(accent), accent * 0.22)
            }
            Self::Glimpsed => {
                let accent = observed_style::architecture(register).accent * GLIMPSED_DIM;
                (Color::LinearRgba(accent), accent * 0.10)
            }
        };
        (Color::LinearRgba(base.to_linear() * gain), emissive * gain)
    }
}

/// Height in metres for an archetype's slab; the ordering is the legend.
#[must_use]
pub(super) fn archetype_height(archetype: HexArchetype) -> Option<f32> {
    match archetype {
        HexArchetype::Void => None,
        HexArchetype::Straight | HexArchetype::Corner => Some(0.9),
        HexArchetype::Junction => Some(1.5),
        HexArchetype::RampHead => Some(0.6),
        HexArchetype::Room => Some(2.6),
        HexArchetype::RampUp => Some(4.5),
        HexArchetype::Shaft => Some(TILE_LEVEL_HEIGHT * 0.85),
    }
}

/// Frame the discovered facility in an orthographic isometric view, returning
/// (transform, world-units-per-pixel, far plane).
#[must_use]
pub(super) fn frame_map(min: Vec3, max: Vec3) -> (Transform, f32, f32) {
    let rotation = Quat::from_euler(EulerRot::YXZ, std::f32::consts::FRAC_PI_4, ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        extent = extent.max((inverse * (corner - centre)).truncate().abs());
    }
    let scale = (extent.x * 2.0 / WINDOW_WIDTH)
        .max(extent.y * 2.0 / WINDOW_HEIGHT)
        .max(f32::MIN_POSITIVE)
        * 1.12;
    let diagonal = (max - min).length().max(1.0);
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * diagonal).with_rotation(rotation);
    (transform, scale, diagonal * 2.0)
}

fn legend_text(knowledge: &HexPlayerMapKnowledge, world: &HexWfcWorld, focus: u8) -> String {
    let mut traversed = 0usize;
    let mut glimpsed = 0usize;
    let mut stale = 0usize;
    let mut anchored = 0usize;
    for (&cell, known) in &knowledge.cells {
        match CellState::of(known, world, cell) {
            CellState::Traversed => traversed += 1,
            CellState::Glimpsed => glimpsed += 1,
            CellState::Stale => stale += 1,
        }
        if known.anchored {
            anchored += 1;
        }
    }
    let levels = knowledge
        .cells
        .keys()
        .map(|cell| cell.level)
        .collect::<std::collections::BTreeSet<_>>();
    let floors = levels
        .iter()
        .map(|level| {
            if *level == focus {
                format!("[{level}]")
            } else {
                format!("{level}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "SURVIVOR MAP   floor {focus}   {} cells known\n\
         bright traversed {traversed}   dim glimpsed {glimpsed}   grey stale {stale}   capped anchored {anchored}\n\
         colour is the district, height is the archetype\n\
         floors discovered: {floors}\n\
         PageUp/PageDown change floor    Tab close",
        knowledge.cells.len(),
    )
}

/// Rebuild only when something a survivor would see has actually changed.
fn signature(runtime: &HexWfcRuntime) -> u64 {
    let mut hasher = DefaultHasher::new();
    runtime.map_level.hash(&mut hasher);
    let local = runtime.local().cell;
    (local.q, local.r, local.level).hash(&mut hasher);
    if let Some(knowledge) = runtime.match_state.player_map(runtime.local_player) {
        knowledge.cells.len().hash(&mut hasher);
        let world = &runtime.match_state.facility;
        for (&cell, known) in &knowledge.cells {
            (cell.q, cell.r, cell.level).hash(&mut hasher);
            known.anchored.hash(&mut hasher);
            (known.discovery == HexMapDiscovery::Traversed).hash(&mut hasher);
            known.is_stale(world, cell).hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void_is_the_only_archetype_the_map_does_not_draw() {
        assert!(archetype_height(HexArchetype::Void).is_none());
        for archetype in [
            HexArchetype::Room,
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
        ] {
            assert!(archetype_height(archetype).is_some_and(|h| h > 0.0));
        }
    }

    #[test]
    fn vertical_circulation_reads_taller_than_flat_circulation() {
        let straight = archetype_height(HexArchetype::Straight).expect("draws");
        let junction = archetype_height(HexArchetype::Junction).expect("draws");
        let room = archetype_height(HexArchetype::Room).expect("draws");
        let shaft = archetype_height(HexArchetype::Shaft).expect("draws");
        assert!(straight < junction && junction < room && room < shaft);
    }

    #[test]
    fn the_framing_fits_the_discovered_bounds_inside_the_viewport() {
        let min = Vec3::new(-140.0, 0.0, -120.0);
        let max = Vec3::new(260.0, 80.0, 240.0);
        let (transform, scale, far) = frame_map(min, max);
        let inverse = transform.rotation.inverse();
        let centre = (min + max) * 0.5;
        for i in 0..8u8 {
            let corner = Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            );
            let projected = (inverse * (corner - centre)).truncate();
            assert!(projected.x.abs() <= scale * WINDOW_WIDTH * 0.5 + 1e-3);
            assert!(projected.y.abs() <= scale * WINDOW_HEIGHT * 0.5 + 1e-3);
        }
        assert!(
            far > (max - min).length(),
            "the far plane must clear the facility diagonal"
        );
    }

    #[test]
    fn stale_leaves_the_district_palette_and_traversed_does_not() {
        let register = ArchitectureRegister::Megastructure;
        let accent = observed_style::architecture(register).accent;
        let (traversed, _) = CellState::Traversed.treatment(register, true);
        let (stale, _) = CellState::Stale.treatment(register, true);
        assert_eq!(traversed, Color::LinearRgba(accent));
        assert_ne!(
            stale,
            Color::LinearRgba(accent),
            "a stale cell must not keep claiming its district"
        );
    }

    #[test]
    fn glimpsed_is_dimmer_than_traversed_in_the_same_district() {
        let register = ArchitectureRegister::Megastructure;
        let (traversed, _) = CellState::Traversed.treatment(register, true);
        let (glimpsed, _) = CellState::Glimpsed.treatment(register, true);
        let luminance = |color: Color| observed_style::luminance(color.to_linear());
        assert!(
            luminance(glimpsed) < luminance(traversed),
            "glimpsed must read below traversed"
        );
    }

    #[test]
    fn the_three_survivor_states_are_distinct_material_keys() {
        let keys = [
            CellState::Traversed.key(),
            CellState::Glimpsed.key(),
            CellState::Stale.key(),
        ];
        let unique = keys.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 3, "states must not share a cached material");
    }
}
