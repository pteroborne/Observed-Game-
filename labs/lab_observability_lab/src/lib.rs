//! lab_observability_lab -- Phase A7 of the Bevy asset-integration roadmap.
//!
//! It answers one question: **can labs expose useful knobs and event traces
//! without making debug state part of the game simulation?**
//!
//! Two assets were on the table. `bevy_mod_config 0.6.2` is **adopted** (egui
//! editor feature off) as a typed, JSON-persisted config root: every knob lives
//! in [`config::ObservabilityConfig`], reads route through `ReadConfig`, and
//! persistence goes through its Serde JSON manager. `bevy_log_events 0.7.0` is
//! **not** adopted: its only logging-capable feature (`enabled`) force-pulls
//! `bevy_egui` and its plugin build-asserts `EguiPlugin`, which would drag a
//! whole egui UI stack into the workspace against this project's dependency
//! rule. The event-trace capability it would have provided is implemented
//! lab-locally over Bevy's own `tracing` log (see [`lab::trace_events`]).
//!
//! The architectural spine of the lab is the boundary between the deterministic
//! [`model::LaunchManifest`] (the recorded launch inputs) and the live debug
//! knobs: editing a knob is provably incapable of changing the simulation, and
//! a config value only enters deterministic state when explicitly committed.

mod config;
mod lab;
mod model;

pub use config::{ConfigManager, ObservabilityConfig};
pub use lab::{
    ActiveManifest, ActiveStream, CommitRequested, ConfigPersistPath, LabSpawned, ObsCamera,
    ObsSet, ObsUiRoot, ObservedEvent, PersistRequest, ResetRequested, RoomTile, RuntimeState,
    SimClock, StepGate, TileHighlights, TraceLog,
};
pub use model::{EventKind, LaunchManifest, TickEvent, TracedEvent};

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowFocused, WindowResolution},
};
use bevy_mod_config::AppExt;
use bevy_mod_config::manager::serde::Json;
use observed_style::{SurfaceRole, surface};

/// The lab plugin: config, simulation, lab-local trace, and presentation.
pub struct ObservabilityLabPlugin;

impl Plugin for ObservabilityLabPlugin {
    fn build(&self, app: &mut App) {
        app
            // Ensure the focus message exists even under MinimalPlugins (tests).
            .add_message::<WindowFocused>()
            .add_message::<ObservedEvent>()
            // bevy_mod_config: one JSON-persisted config root, egui editor off.
            .init_config_with::<ConfigManager, ObservabilityConfig>("observability", Json::new)
            .init_resource::<ActiveManifest>()
            .init_resource::<ActiveStream>()
            .init_resource::<SimClock>()
            .init_resource::<StepGate>()
            .init_resource::<TraceLog>()
            .init_resource::<TileHighlights>()
            .init_resource::<RuntimeState>()
            .init_resource::<ResetRequested>()
            .init_resource::<CommitRequested>()
            .init_resource::<PersistRequest>()
            .init_resource::<ConfigPersistPath>()
            .configure_sets(
                Update,
                (
                    ObsSet::Input,
                    ObsSet::Apply,
                    ObsSet::Simulate,
                    ObsSet::Trace,
                    ObsSet::Present,
                )
                    .chain(),
            )
            .add_systems(Startup, (lab::startup_launch, lab::setup_scene).chain())
            .add_systems(
                Update,
                (lab::handle_input, lab::update_focus).in_set(ObsSet::Input),
            )
            .add_systems(
                Update,
                (lab::apply_commit, lab::apply_reset, ApplyDeferred)
                    .chain()
                    .in_set(ObsSet::Apply),
            )
            .add_systems(
                Update,
                lab::apply_persist
                    .in_set(ObsSet::Apply)
                    .after(lab::apply_reset),
            )
            .add_systems(Update, lab::step_simulation.in_set(ObsSet::Simulate))
            .add_systems(Update, lab::trace_events.in_set(ObsSet::Trace))
            .add_systems(
                Update,
                (lab::present_scene, lab::update_overlay).in_set(ObsSet::Present),
            );
    }
}

/// Builds and runs the windowed lab.
pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(surface(SurfaceRole::Ceiling).base_color))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Observability Lab (Phase A7)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ObservabilityLabPlugin);

    // Live runs pace the simulation so the event flow is watchable.
    app.world_mut().resource_mut::<StepGate>().interval = lab::LIVE_STEP_SECONDS;

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        // For capture, step every frame so the trace and tiles fill quickly and
        // deterministically before the screenshot.
        app.world_mut().resource_mut::<StepGate>().interval = 0.0;
        app.insert_resource(CaptureRequest {
            path,
            frame: 0,
            shot: false,
        })
        .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    frame: u32,
    shot: bool,
}

fn capture_progress(
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frame += 1;
    if request.frame == 20 && !request.shot {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.shot = true;
    } else if request.frame >= 40 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use bevy_mod_config::{ConfigNode, ScalarData};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A unique, non-existent temp config path so tests never collide with each
    /// other or with a developer's saved config.
    fn unique_config_path() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "observed2_obslab_test_{}_{}.json",
            std::process::id(),
            id
        ))
    }

    fn test_app_with_path(path: std::path::PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(ObservabilityLabPlugin)
            // Override the default persist path BEFORE the first update/startup.
            .insert_resource(ConfigPersistPath(path));
        app.update();
        app
    }

    fn test_app() -> App {
        test_app_with_path(unique_config_path())
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn write_u32(app: &mut App, field: &str, value: u32) {
        let world = app.world_mut();
        let mut query = world.query::<(&mut ConfigNode, &mut ScalarData<u32>)>();
        for (mut node, mut data) in query.iter_mut(world) {
            if config::is_field(&node, field) {
                data.0 = value;
                config::bump(&mut node);
            }
        }
    }

    fn write_f32(app: &mut App, field: &str, value: f32) {
        let world = app.world_mut();
        let mut query = world.query::<(&mut ConfigNode, &mut ScalarData<f32>)>();
        for (mut node, mut data) in query.iter_mut(world) {
            if config::is_field(&node, field) {
                data.0 = value;
                config::bump(&mut node);
            }
        }
    }

    fn read_f32(app: &mut App, field: &str) -> f32 {
        let world = app.world_mut();
        let mut query = world.query::<(&ConfigNode, &ScalarData<f32>)>();
        query
            .iter(world)
            .find(|(node, _)| config::is_field(node, field))
            .map(|(_, data)| data.0)
            .expect("field exists")
    }

    #[test]
    fn boots_with_camera_tiles_and_overlay_at_the_default_manifest() {
        let mut app = test_app();
        assert_eq!(count::<ObsCamera>(&mut app), 1);
        assert_eq!(count::<ObsUiRoot>(&mut app), 1);
        assert_eq!(count::<RoomTile>(&mut app), model::ROOM_COUNT as usize);

        // With no saved config, the launch manifest uses the default seed and the
        // live stream reproduces the pure model exactly.
        let manifest = app.world().resource::<ActiveManifest>().0;
        assert_eq!(manifest.seed, model::DEFAULT_SEED);
        let stream = app.world().resource::<ActiveStream>();
        assert_eq!(stream.checksum, model::checksum(&model::simulate(manifest)));
    }

    #[test]
    fn reset_rebuilds_the_scene_without_leaking_entities() {
        let mut app = test_app();
        let baseline = count::<LabSpawned>(&mut app);
        assert!(baseline > 0);

        for expected in 1..=3 {
            app.world_mut().resource_mut::<ResetRequested>().0 = true;
            app.update();
            assert_eq!(count::<LabSpawned>(&mut app), baseline);
            assert_eq!(count::<RoomTile>(&mut app), model::ROOM_COUNT as usize);
            assert_eq!(count::<ObsCamera>(&mut app), 1);
            assert_eq!(app.world().resource::<RuntimeState>().reset_count, expected);
        }
    }

    #[test]
    fn debug_knobs_and_trace_verbosity_never_change_the_simulation() {
        // Run A: full verbosity.
        let mut app_a = test_app();
        for _ in 0..20 {
            app_a.update();
        }
        let logged_loud = app_a.world().resource::<RuntimeState>().logged;
        let checksum_a = app_a.world().resource::<ActiveStream>().checksum;
        let steps_a = app_a.world().resource::<SimClock>().steps;
        assert!(logged_loud > 0, "loud run logged events");

        // Run B: same seed, but tracing off and every other debug knob changed.
        let mut app_b = test_app();
        write_u32(&mut app_b, config::field::TRACE_VERBOSITY, 0);
        write_u32(&mut app_b, config::field::COLOR_VISION, 3);
        write_f32(&mut app_b, config::field::FOG, 0.9);
        write_f32(&mut app_b, config::field::BLOOM, 0.1);
        // Measure only the verbosity-0 window (the startup frame ran at the
        // default verbosity before we disabled tracing).
        app_b.world_mut().resource_mut::<RuntimeState>().logged = 0;
        for _ in 0..20 {
            app_b.update();
        }
        let logged_silent = app_b.world().resource::<RuntimeState>().logged;
        let checksum_b = app_b.world().resource::<ActiveStream>().checksum;
        let steps_b = app_b.world().resource::<SimClock>().steps;

        // The simulation advanced identically; only the log output differs.
        assert_eq!(checksum_a, checksum_b, "stream is seed-determined only");
        assert_eq!(steps_a, steps_b, "sim stepped identically");
        assert_eq!(logged_silent, 0, "verbosity 0 disabled logging");
    }

    #[test]
    fn editing_the_seed_is_pending_until_committed() {
        let mut app = test_app();
        let original = app.world().resource::<ActiveManifest>().0.seed;
        let original_checksum = app.world().resource::<ActiveStream>().checksum;

        // Edit the candidate seed: the running simulation must be untouched.
        write_u32(&mut app, config::field::SEED, original + 1);
        app.update();
        assert_eq!(app.world().resource::<ActiveManifest>().0.seed, original);
        assert_eq!(
            app.world().resource::<ActiveStream>().checksum,
            original_checksum
        );

        // Commit: now the seed enters the deterministic manifest and the stream
        // changes to match the new seed exactly.
        app.world_mut().resource_mut::<CommitRequested>().0 = true;
        app.update();
        let committed = app.world().resource::<ActiveManifest>().0;
        assert_eq!(committed.seed, original + 1);
        let stream = app.world().resource::<ActiveStream>();
        assert_eq!(
            stream.checksum,
            model::checksum(&model::simulate(committed))
        );
        assert_ne!(stream.checksum, original_checksum);
    }

    #[test]
    fn config_persists_to_json_and_reloads_as_the_launch_manifest() {
        let path = unique_config_path();
        let _ = std::fs::remove_file(&path);

        // App 1: change knobs, then save.
        let mut app1 = test_app_with_path(path.clone());
        let saved_seed = model::DEFAULT_SEED + 7;
        write_u32(&mut app1, config::field::SEED, saved_seed);
        write_f32(&mut app1, config::field::FOG, 0.8);
        app1.world_mut().resource_mut::<PersistRequest>().save = true;
        app1.update();
        assert_eq!(app1.world().resource::<RuntimeState>().save_count, 1);
        assert!(path.exists(), "config file written");

        // App 2: same path -> startup loads it, and the persisted seed becomes
        // the launch manifest (the explicit, recorded config -> manifest path).
        let mut app2 = test_app_with_path(path.clone());
        assert_eq!(app2.world().resource::<ActiveManifest>().0.seed, saved_seed);
        assert!((read_f32(&mut app2, config::field::FOG) - 0.8).abs() < 1e-6);

        let _ = std::fs::remove_file(&path);
    }
}
