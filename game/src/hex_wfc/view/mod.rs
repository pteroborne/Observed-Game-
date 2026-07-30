//! Streamed presentation for the canonical hex facility.
//!
//! The authoritative geometry snapshot is the single collision/render source; this
//! module adds style-owned materials, a bounded lighting/post-process rig, and
//! presentation residency streaming by proximity to the runner.

use std::collections::BTreeMap;
use std::time::Instant;

mod assets;
mod lighting;
/// The full-screen isometric survivor map, wired by `hex_wfc::mod`.
pub(crate) mod map;
mod residency;
mod shell;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexCoord;

use self::assets::HexWfcVisualAssets;
use super::sim::HexWfcRuntime;
use crate::settings::Settings;
use crate::view::components::{GameCam, GameSun, MENU_SUN_ILLUMINANCE};

/// Cells enter presentation residency inside this window. The logical facility and its
/// pure collision snapshot remain complete regardless of presentation residency.
const STREAM_ENTER_RADIUS: f32 = 30.0;
const STREAM_ENTER_LEVELS: u8 = 2;
/// A larger removal window prevents churn when the runner hovers near the enter edge.
const STREAM_EXIT_RADIUS: f32 = 42.0;
const STREAM_EXIT_LEVELS: u8 = 3;
/// Entry always projects the current cell and its same-level one-ring neighborhood.
const ENTRY_SAFE_RADIUS: f32 = 15.0;
const ENTRY_CELL_SPAWN_BUDGET: usize = 24;
/// Normal play may instantiate at most this many cell parents in one update frame.
const CELL_SPAWN_BUDGET: usize = 8;
/// Retiring parents is cheap, but bounding it avoids a hierarchy-despawn cliff after a
/// teleport or spectator jump.
const CELL_DESPAWN_BUDGET: usize = 24;

/// Marks shell entities for presentation diagnostics and state-scoped cleanup.
#[derive(Component)]
pub(super) struct HexWfcGeometry;

#[derive(Clone, Copy, Debug)]
struct ResidentCell {
    entity: Entity,
    child_pieces: usize,
}

/// Presentation-only residency ownership. Stable [`HexCoord`] keys remain the identity;
/// Bevy entities are transient handles that never enter simulation state.
#[derive(Resource)]
pub(super) struct HexPresentationResidency {
    catalog: shell::HexGeometryCatalog,
    resident: BTreeMap<HexCoord, ResidentCell>,
    /// `OnEnter` already consumes the entry-frame budget. The first `Update` therefore
    /// reports readiness but does not enqueue a second batch in the same rendered frame.
    defer_incremental_once: bool,
    capture_unbounded: bool,
}

/// Honest counters for debug overlays, profiling, and loading/readiness inspection.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HexPresentationReadiness {
    pub catalog_cells: usize,
    pub desired_cells: usize,
    pub resident_cells: usize,
    pub pending_cells: usize,
    pub resident_child_pieces: usize,
    pub spawned_this_frame: usize,
    pub despawned_this_frame: usize,
    pub entry_neighborhood_ready: bool,
    pub stream_window_ready: bool,
}

#[derive(Component)]
pub(super) struct HexWfcKeyLight;

/// A per-tile downlight fixture (tier 2 of the rig). Carries its cell so the shadow
/// budget can pick the fixtures nearest the runner to cast; the rest stay shadowless
/// fill. Every non-boundary cell gets one, so no tile is left without a light source.
#[derive(Component)]
pub(super) struct HexPractical(pub HexCoord);

#[allow(clippy::too_many_arguments)]
pub(super) fn setup_view(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<Settings>,
    mut camera: Query<(Entity, &mut Transform), With<GameCam>>,
    mut projection: Query<&mut Projection, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
    mut perf: Option<ResMut<super::perf::HexPerfMetrics>>,
) {
    let started = Instant::now();
    let architecture = *runtime
        .match_state
        .facility
        .architecture
        .get(&runtime.local().cell)
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    let current = runtime.local().cell;
    let composition = lighting::composition_at(&runtime.match_state.facility, current);
    let palette = observed_style::architecture_for_composition(architecture, composition);
    if let Ok((camera, mut transform)) = camera.single_mut() {
        lighting::prime_camera(&mut transform, runtime.local());
        commands.entity(camera).insert((
            Hdr,
            Bloom {
                intensity: 0.08,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: palette.fog_color,
                falloff: FogFalloff::Linear {
                    start: palette.fog_start,
                    end: palette.fog_end,
                },
                ..default()
            },
            Msaa::Off,
            Fxaa::default(),
            DepthPrepass,
            NormalPrepass,
            ScreenSpaceAmbientOcclusion {
                // The authored shell already carries strong semantic edge light and
                // fog separation. Low SSAO preserves local contact depth without
                // consuming the GPU margin needed by a 1440x900 mutation frame.
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
                ..default()
            },
        ));
    }
    if let Ok(mut projection) = projection.single_mut()
        && let Projection::Perspective(perspective) = &mut *projection
    {
        perspective.fov = settings.fov_degrees.clamp(50.0, 80.0).to_radians();
    }
    for mut light in &mut sun {
        light.illuminance = 0.0;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });
    commands.insert_resource(ClearColor(palette.fog_color));
    lighting::spawn_rig(
        &mut commands,
        architecture,
        composition,
        current,
        runtime.local(),
    );

    // Geometry is deliberately enqueued only after the camera, atmosphere, menu sun,
    // and both semantic lights have their exact initial values. Entry projects only a
    // safe local neighborhood; the production-sized logical snapshot remains resident
    // in simulation without synchronously creating its ~100k presentation pieces.
    let mut assets = HexWfcVisualAssets::load(&asset_server, &mut materials);
    let catalog = shell::HexGeometryCatalog::build(&runtime);
    shell::spawn_boundary(&mut commands, &mut assets, &mut meshes, &runtime, &catalog);
    let capture_unbounded = capture_requests_deterministic_residency();
    let initial_budget = if capture_unbounded {
        usize::MAX
    } else {
        ENTRY_CELL_SPAWN_BUDGET
    };
    let initial = initial_spawn_batch(&catalog, runtime.local(), initial_budget);
    let spawned = shell::spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        &runtime,
        &catalog,
        &initial,
    );
    let resident = spawned
        .into_iter()
        .map(|spawned| {
            (
                spawned.coord,
                ResidentCell {
                    entity: spawned.entity,
                    child_pieces: spawned.child_pieces,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let readiness = presentation_readiness(&catalog, &resident, runtime.local(), 0, 0);
    debug_assert!(
        readiness.entry_neighborhood_ready,
        "entry cell and its same-level one-ring must fit the entry budget"
    );
    commands.insert_resource(HexPresentationResidency {
        catalog,
        resident,
        defer_incremental_once: true,
        capture_unbounded,
    });
    commands.insert_resource(readiness);
    commands.insert_resource(assets);
    super::perf::record_view(
        &mut perf,
        super::perf::ViewTimingKind::Startup,
        &runtime,
        started.elapsed(),
    );
}

pub(super) use lighting::{
    sync_camera, sync_lighting_and_atmosphere, sync_practical_shadow_budget,
};
use residency::{initial_spawn_batch, presentation_readiness};
pub(super) use residency::{sync_changed_geometry, sync_streamed_cells};

pub(super) fn clear_view(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    mut projection: Query<&mut Projection, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    if let Ok(camera) = camera.single() {
        commands
            .entity(camera)
            .remove::<(
                Hdr,
                Bloom,
                DistanceFog,
                Fxaa,
                ScreenSpaceAmbientOcclusion,
                DepthPrepass,
                NormalPrepass,
            )>()
            .insert(Msaa::Sample4);
    }
    if let Ok(mut projection) = projection.single_mut()
        && let Projection::Perspective(perspective) = &mut *projection
    {
        perspective.fov = PerspectiveProjection::default().fov;
    }
    for mut light in &mut sun {
        light.illuminance = MENU_SUN_ILLUMINANCE;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
    commands.insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.065)));
    commands.remove_resource::<HexWfcVisualAssets>();
    commands.remove_resource::<HexPresentationResidency>();
    commands.remove_resource::<HexPresentationReadiness>();
}

fn capture_requests_deterministic_residency() -> bool {
    [
        "OBSERVED2_CAPTURE_HEX_WFC_STYLE",
        "OBSERVED2_CAPTURE_HEX_WFC_RELAYOUT",
        "OBSERVED2_CAPTURE_HEX_WFC_TRAVERSAL",
        "OBSERVED2_CAPTURE_HEX_WFC",
        "OBSERVED2_CAPTURE_HEX_WFC_MAP",
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
