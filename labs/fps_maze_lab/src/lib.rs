mod lab;
// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `maze` path. This lab is the debug projection.
pub mod maze {
    pub use observed_match::maze::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::MazeRuntime;
pub use maze::MazeLayout;

pub struct FpsMazePlugin;

impl Plugin for FpsMazePlugin {
    fn build(&self, app: &mut App) {
        let mut runtime = MazeRuntime::default();
        let maze = lab::initial_maze(&runtime);
        let collision = lab::initial_collision(&maze);
        lab::spawn_player(&mut runtime, &maze);
        app.insert_resource(runtime)
            .insert_resource(maze)
            .insert_resource(collision)
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::toggle_grab,
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
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.014, 0.02)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Spatial Maze Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsMazePlugin);

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
    mut runtime: ResMut<MazeRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Angled overview of the whole generated maze: rooms, corridors, and the
        // gold spine route, embedded as real geometry.
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
    use lab::{MazeCam, MazeUiRoot};
    use observation_lab::model::ROOM_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsMazePlugin);
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
        assert_eq!(count::<MazeCam>(&mut app), 1);
        assert_eq!(count::<MazeUiRoot>(&mut app), 1);
        let maze = app.world().resource::<MazeLayout>();
        assert!(maze.navigable());
        assert_eq!(maze.reachable_rooms(), ROOM_COUNT);
        assert!(!maze.rooms_overlap());
    }

    #[test]
    fn regenerating_the_layout_keeps_it_navigable_and_leak_free() {
        let mut app = test_app();
        let cams = count::<MazeCam>(&mut app);
        for seed in [2u64, 9, 100, 7777] {
            {
                let world = app.world().resource::<MazeRuntime>().world.clone();
                let new_maze = MazeLayout::generate(&world.graph, seed);
                assert!(new_maze.navigable(), "seed {seed} stays navigable");
                *app.world_mut().resource_mut::<MazeLayout>() = new_maze;
            }
            app.update();
            assert_eq!(count::<MazeCam>(&mut app), cams);
            assert_eq!(count::<MazeUiRoot>(&mut app), 1);
        }
    }
}
