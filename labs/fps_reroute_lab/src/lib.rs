mod lab;
pub mod reroute;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::RerouteRuntime;
pub use reroute::RerouteMaze;

pub struct FpsReroutePlugin;

impl Plugin for FpsReroutePlugin {
    fn build(&self, app: &mut App) {
        let mut runtime = RerouteRuntime::default();
        let maze = RerouteMaze::authored(1);
        lab::spawn_player(&mut runtime, &maze);
        let collision = lab::RerouteCollision(maze.arena(3.4));
        app.insert_resource(maze)
            .insert_resource(runtime)
            .insert_resource(collision)
            .init_resource::<lab::DecohereTimer>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::toggle_grab,
                    lab::reconcile,
                    lab::present_camera,
                    lab::draw_maze,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.01, 0.018)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Rerouting Passages".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsReroutePlugin);

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
    mut maze: ResMut<RerouteMaze>,
    mut runtime: ResMut<RerouteRuntime>,
    mut collision: ResMut<lab::RerouteCollision>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto = false;
        // Reroute the maze a few times off-camera (it visibly differs from the
        // authored layout), then leave one reroute pending and keep it visible
        // (top-down) so the magenta "waiting to swap" tiles show in the shot.
        for _ in 0..3 {
            maze.decohere();
            maze.try_commit(&std::collections::HashSet::new(), None);
        }
        collision.0 = maze.arena(3.4);
        maze.decohere();
        runtime.top_down = true;
        runtime.camera_override =
            Some(Transform::from_xyz(0.0, 72.0, 56.0).looking_at(Vec3::ZERO, Vec3::Y));
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
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use lab::{RerouteCam, RerouteUiRoot};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsReroutePlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_and_ui_over_a_navigable_maze() {
        let mut app = test_app();
        assert_eq!(count::<RerouteCam>(&mut app), 1);
        assert_eq!(count::<RerouteUiRoot>(&mut app), 1);
        let maze = app.world().resource::<RerouteMaze>();
        assert!(maze.navigable());
        assert!(maze.in_sync());
    }

    #[test]
    fn the_reconcile_system_commits_a_reroute_when_nothing_is_in_view() {
        let mut app = test_app();
        app.world_mut().resource_mut::<RerouteMaze>().decohere();
        assert!(!app.world().resource::<RerouteMaze>().in_sync());
        {
            // Park the player far outside the grid so nothing is in view.
            let mut runtime = app.world_mut().resource_mut::<RerouteRuntime>();
            runtime.auto = false;
            runtime.top_down = false;
            runtime.body.position = Vec3::splat(10_000.0);
        }
        app.update();
        let maze = app.world().resource::<RerouteMaze>();
        assert!(maze.in_sync(), "the reroute committed off-camera");
        assert!(maze.commit_count >= 1);
        assert!(maze.navigable());
    }

    #[test]
    fn the_reconcile_system_defers_while_the_maze_is_watched() {
        let mut app = test_app();
        app.world_mut().resource_mut::<RerouteMaze>().decohere();
        {
            let mut runtime = app.world_mut().resource_mut::<RerouteRuntime>();
            runtime.auto = false;
            runtime.top_down = true; // the map view sees everything
        }
        app.update();
        let maze = app.world().resource::<RerouteMaze>();
        assert!(!maze.in_sync(), "a watched reroute stays pending");
        assert!(maze.deferred_count >= 1);
    }
}
