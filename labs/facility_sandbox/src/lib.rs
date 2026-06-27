mod app;
pub mod facility;
mod sandbox;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    time::Fixed,
    window::{PresentMode, WindowResolution},
};
use climbing_lab::ClimbBody;

use app::AppState;
use facility::{Facility, ItemLocation, PIT_MAX, PIT_MIN};
use sandbox::{PlayerBody, SandboxCamera, SandboxRuntime};

pub use facility::Facility as FacilityModel;

pub struct FacilitySandboxPlugin;

impl Plugin for FacilitySandboxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(Facility::authored())
            .init_resource::<SandboxRuntime>()
            .init_resource::<sandbox::SandboxBuilt>()
            .init_resource::<sandbox::HumanInput>()
            .add_systems(Startup, setup_camera)
            .add_systems(
                PreUpdate,
                sandbox::sample_input
                    .after(InputSystems)
                    .run_if(in_state(AppState::Sandbox)),
            )
            .add_systems(
                FixedUpdate,
                sandbox::step_players.run_if(in_state(AppState::Sandbox)),
            )
            .add_systems(
                Update,
                (sandbox::gameplay_shortcuts, sandbox::update_objective)
                    .chain()
                    .run_if(in_state(AppState::Sandbox)),
            )
            .add_systems(
                Update,
                (
                    sandbox::present_players,
                    sandbox::present_items,
                    sandbox::draw_scene,
                    sandbox::update_hud,
                    sandbox::follow_camera,
                )
                    .run_if(app::in_sandbox_or_paused),
            );
        app::register(app);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        SandboxCamera,
        Projection::Orthographic(OrthographicProjection {
            scale: 3.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(1500.0, 140.0, 1000.0),
        Name::new("Facility Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.030)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Facility Sandbox".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FacilitySandboxPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

/// Drives the app into the sandbox (skipping the menus), arranges an integrated
/// mid-objective showcase, captures it, and exits.
#[allow(clippy::too_many_arguments)]
fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut facility: ResMut<Facility>,
    mut runtime: ResMut<SandboxRuntime>,
    mut bodies: Query<(&PlayerBody, &mut ClimbBody)>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match state.get() {
        AppState::Boot | AppState::MainMenu => next.set(AppState::Loading),
        AppState::Sandbox => {
            if request.phase == 0 {
                // Power the door, bridge the pit, spread the squad through the rooms.
                facility.battery.location = ItemLocation::Socketed;
                facility.door_opened = true;
                facility.jack.location =
                    ItemLocation::Deployed(Vec2::new((PIT_MIN + PIT_MAX) * 0.5, 0.0));
                facility.pit_bridged = true;
                runtime.spectator = true;
                let showcase = [
                    Vec2::new(900.0, 246.0),
                    Vec2::new(1600.0, 34.0),
                    Vec2::new(2070.0, 34.0),
                    Vec2::new(2880.0, 34.0),
                ];
                for (body, mut climb) in &mut bodies {
                    if let Some(target) = showcase.get(body.0.index()) {
                        climb.position = *target;
                        climb.velocity = Vec2::ZERO;
                    }
                }
                request.phase = 1;
            } else if request.phase == 1 && elapsed >= 1.0 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(request.path.clone()));
                request.phase = 2;
            } else if request.phase == 2 && elapsed >= 1.9 {
                exit.write(AppExit::Success);
            }
        }
        AppState::Loading | AppState::Paused => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxOwned;
    use bevy::{
        asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin, state::app::StatesPlugin,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
            StatesPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FacilitySandboxPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn set_state(app: &mut App, state: AppState) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
        app.update();
    }

    #[test]
    fn entering_and_leaving_the_sandbox_leaks_no_entities() {
        let mut app = test_app();
        // Boot: nothing sandbox-owned yet.
        assert_eq!(count::<SandboxOwned>(&mut app), 0);

        // Loading builds the sandbox (4 players + 2 items + HUD root).
        set_state(&mut app, AppState::Loading);
        let built = count::<SandboxOwned>(&mut app);
        assert_eq!(count::<PlayerBody>(&mut app), 4);
        assert_eq!(built, 7);

        // Returning to the main menu tears everything down.
        set_state(&mut app, AppState::MainMenu);
        assert_eq!(count::<SandboxOwned>(&mut app), 0);
        assert_eq!(count::<PlayerBody>(&mut app), 0);

        // And the facility model is reset for a fresh run.
        assert!(!app.world().resource::<Facility>().door_opened);
    }

    #[test]
    fn repeated_enter_leave_is_stable() {
        let mut app = test_app();
        for _ in 0..5 {
            set_state(&mut app, AppState::Loading);
            assert_eq!(count::<PlayerBody>(&mut app), 4);
            set_state(&mut app, AppState::MainMenu);
            assert_eq!(count::<SandboxOwned>(&mut app), 0);
        }
        // The persistent camera is never torn down or duplicated.
        assert_eq!(count::<SandboxCamera>(&mut app), 1);
    }
}
