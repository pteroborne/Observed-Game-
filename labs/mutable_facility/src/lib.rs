mod lab;
// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `model` path. This lab is the debug projection.
pub mod model {
    pub use observed_match::mutable::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::FacRuntime;
pub use model::MutableFacility;

pub struct MutableFacilityPlugin;

impl Plugin for MutableFacilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MutableFacility::authored())
            .init_resource::<FacRuntime>()
            .init_resource::<lab::AdvanceTimer>()
            .init_resource::<lab::DecohereTimer>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::simulate,
                    lab::perform_reset,
                    lab::present_team,
                    lab::draw_debug,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.55,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 40.0, 1000.0),
        Name::new("Mutable Facility Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.016, 0.024)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Mutable Facility".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MutableFacilityPlugin);

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

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut facility: ResMut<MutableFacility>,
    mut runtime: ResMut<FacRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Walk the team partway along the spine while the structure decoheres
        // behind them, then freeze for a legible mid-run shot.
        runtime.running = false;
        for _ in 0..4 {
            facility.advance();
            facility.decohere();
        }
        for _ in 0..3 {
            facility.decohere();
        }
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.4 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::{FacUiRoot, TeamDot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::TEAM_SIZE;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(MutableFacilityPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_dot_per_member() {
        let mut app = test_app();
        assert_eq!(count::<TeamDot>(&mut app), TEAM_SIZE);
        assert_eq!(count::<FacUiRoot>(&mut app), 1);
        assert!(app.world().resource::<MutableFacility>().connected());
    }

    #[test]
    fn repeated_reset_restores_the_entrance() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut facility = app.world_mut().resource_mut::<MutableFacility>();
                facility.advance();
                facility.decohere();
            }
            app.world_mut().resource_mut::<FacRuntime>().reset_requested = true;
            app.update();

            assert_eq!(count::<TeamDot>(&mut app), TEAM_SIZE);
            assert_eq!(count::<FacUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<FacRuntime>().reset_count,
                reset_count
            );
            let facility = app.world().resource::<MutableFacility>();
            assert_eq!(facility.steps, 0);
            assert!(
                facility
                    .team_rooms()
                    .iter()
                    .all(|r| r.0 == model::START_ROOM)
            );
        }
    }
}
