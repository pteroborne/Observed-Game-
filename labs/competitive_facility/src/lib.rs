mod lab;
// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `model` path. This lab is the debug projection.
pub mod model {
    pub use observed_match::facility::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::CompRuntime;
pub use model::CompetitiveFacility;

pub struct CompetitiveFacilityPlugin;

impl Plugin for CompetitiveFacilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CompetitiveFacility::authored())
            .init_resource::<CompRuntime>()
            .init_resource::<lab::RoundTimer>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::simulate,
                    lab::perform_reset,
                    lab::present_teams,
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
        Name::new("Competitive Facility Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.012, 0.02)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Competitive Facility".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(CompetitiveFacilityPlugin);

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
    mut facility: ResMut<CompetitiveFacility>,
    mut runtime: ResMut<CompRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Run the match to a dramatic mid-point: the leader near the exit, the
        // collapse eating the back of the spine, a laggard already absorbed.
        runtime.running = false;
        for _ in 0..7 {
            facility.advance_round(&[]);
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
    use crate::lab::{CompUiRoot, MemberDot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::{PLAYER_COUNT, START_ROOM, TEAM_COUNT};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(CompetitiveFacilityPlugin);
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
        assert_eq!(count::<MemberDot>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<CompUiRoot>(&mut app), 1);
        assert!(app.world().resource::<CompetitiveFacility>().connected());
    }

    #[test]
    fn repeated_reset_restores_the_entrance() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut facility = app.world_mut().resource_mut::<CompetitiveFacility>();
                facility.advance_round(&[]);
                facility.advance_round(&[]);
            }
            app.world_mut()
                .resource_mut::<CompRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<MemberDot>(&mut app), PLAYER_COUNT);
            assert_eq!(count::<CompUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<CompRuntime>().reset_count,
                reset_count
            );
            let facility = app.world().resource::<CompetitiveFacility>();
            assert_eq!(facility.round, 0);
            assert!((0..TEAM_COUNT).all(|i| facility.team_room(i).0 == START_ROOM));
        }
    }
}
