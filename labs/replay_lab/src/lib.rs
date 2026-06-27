mod lab;
pub mod tape;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::Spectator;
pub use tape::Tape;

pub struct ReplayLabPlugin;

impl Plugin for ReplayLabPlugin {
    fn build(&self, app: &mut App) {
        let tape = Tape::record();
        let view = lab::View(tape.replay_to(0));
        app.insert_resource(tape)
            .insert_resource(view)
            .init_resource::<Spectator>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::advance_cursor,
                    lab::update_view,
                    lab::present_racers,
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
            scale: 0.95,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, -50.0, 1000.0),
        Name::new("Replay Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.018, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Replay / Spectator Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ReplayLabPlugin);

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
    tape: Res<Tape>,
    mut request: ResMut<CaptureRequest>,
    mut spectator: ResMut<Spectator>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Park the spectator mid-replay, paused, so the scrubber and an
        // in-progress match are both legible.
        spectator.playing = false;
        spectator.cursor = tape.len() as f32 * 0.6;
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
    use crate::lab::{Racer, ReplayUiRoot, View};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use competition_lab::model::TEAM_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(ReplayLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_racer_per_team_and_a_view() {
        let mut app = test_app();
        assert_eq!(count::<Racer>(&mut app), TEAM_COUNT);
        assert_eq!(count::<ReplayUiRoot>(&mut app), 1);
        assert!(app.world().contains_resource::<View>());
    }

    #[test]
    fn seeking_reproduces_state_at_the_cursor() {
        let mut app = test_app();
        let len = app.world().resource::<Tape>().len();

        // Seek to the end (paused): the replayed view is the finished match.
        {
            let mut spectator = app.world_mut().resource_mut::<Spectator>();
            spectator.playing = false;
            spectator.cursor = len as f32;
        }
        app.update();
        assert!(app.world().resource::<View>().0.finished);

        // Seek back to the start: the view is the fresh match again.
        {
            let mut spectator = app.world_mut().resource_mut::<Spectator>();
            spectator.cursor = 0.0;
        }
        app.update();
        let view = app.world().resource::<View>();
        assert!(!view.0.finished);
        assert!(view.0.teams.iter().all(|t| t.progress == 0.0));
    }
}
