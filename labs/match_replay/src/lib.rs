mod lab;
pub mod tape;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{Spectator, View};
pub use tape::Tape;

pub struct MatchReplayPlugin;

impl Plugin for MatchReplayPlugin {
    fn build(&self, app: &mut App) {
        let tape = Tape::record();
        app.insert_resource(View(tape.replay_to(0)))
            .insert_resource(tape)
            .init_resource::<Spectator>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::advance_cursor,
                    lab::update_view,
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
            scale: 1.7,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 60.0, 1000.0),
        Name::new("Spectator Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.016, 0.026)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Match Replay / Spectator".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MatchReplayPlugin);

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
        // Pause partway through the recorded match: the leader near the exit, the
        // collapse mid-spine, a team or two already absorbed, the scrubber partway
        // with passed events behind the cursor.
        spectator.playing = false;
        spectator.cursor = 7.0_f32.min(tape.len() as f32);
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
    use crate::lab::{MemberDot, ReplayUiRoot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use competitive_facility::model::{PLAYER_COUNT, TEAM_COUNT};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(MatchReplayPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_dot_per_member_and_a_recorded_tape() {
        let mut app = test_app();
        assert_eq!(count::<MemberDot>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<ReplayUiRoot>(&mut app), 1);
        assert!(!app.world().resource::<Tape>().is_empty());
    }

    #[test]
    fn seeking_reproduces_the_exact_recorded_state() {
        let mut app = test_app();
        let len = app.world().resource::<Tape>().len();
        for round in [0usize, 1, len / 2, len.saturating_sub(1), len] {
            app.world_mut().resource_mut::<Spectator>().cursor = round as f32;
            app.world_mut().resource_mut::<Spectator>().playing = false;
            app.update();

            let tape = app.world().resource::<Tape>();
            let expected = tape.replay_to(round);
            let view = app.world().resource::<View>();
            let rooms_view: Vec<u32> = (0..TEAM_COUNT).map(|i| view.0.team_room(i).0).collect();
            let rooms_exp: Vec<u32> = (0..TEAM_COUNT).map(|i| expected.team_room(i).0).collect();
            assert_eq!(rooms_view, rooms_exp, "seek to {round} must match the tape");
            assert_eq!(view.0.escaped_count(), expected.escaped_count());
            assert_eq!(view.0.finished, expected.finished);
        }
    }

    #[test]
    fn restart_returns_the_cursor_to_the_start() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Spectator>().cursor = 5.0;
        app.world_mut().resource_mut::<Spectator>().reset_count = 0;
        // Mirror the R key: cursor to 0, playing, bump count.
        {
            let mut spectator = app.world_mut().resource_mut::<Spectator>();
            spectator.cursor = 0.0;
            spectator.reset_count += 1;
        }
        app.update();
        let view = app.world().resource::<View>();
        assert!(
            (0..TEAM_COUNT)
                .all(|i| view.0.team_room(i).0 == competitive_facility::model::START_ROOM)
        );
        assert_eq!(count::<MemberDot>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<ReplayUiRoot>(&mut app), 1);
    }
}
