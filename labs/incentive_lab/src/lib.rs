mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::IncentiveRuntime;
pub use model::IncentiveWorld;

pub struct IncentiveLabPlugin;

impl Plugin for IncentiveLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IncentiveWorld::authored())
            .init_resource::<IncentiveRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::simulate,
                    lab::perform_reset,
                    lab::present_rooms,
                    lab::present_members,
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
        Transform::from_xyz(0.0, -10.0, 1000.0),
        Name::new("Incentive Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.020, 0.020)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Incentive Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(IncentiveLabPlugin);

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
    mut world: ResMut<IncentiveWorld>,
    mut runtime: ResMut<IncentiveRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Let the authored spread-vs-clumped matchup play out, then freeze.
        for _ in 0..200 {
            world.tick(1.0 / 30.0);
        }
        runtime.running = false;
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
    use crate::lab::{IncentiveUiRoot, MemberDot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::TOTAL_MEMBERS;
    use observed_core::TeamId;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(IncentiveLabPlugin);
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
        assert_eq!(count::<MemberDot>(&mut app), TOTAL_MEMBERS);
        assert_eq!(count::<IncentiveUiRoot>(&mut app), 1);
    }

    #[test]
    fn repeated_reset_restores_a_fresh_round() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<IncentiveWorld>();
                for _ in 0..30 {
                    world.tick(1.0 / 30.0);
                }
            }
            app.world_mut()
                .resource_mut::<IncentiveRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<MemberDot>(&mut app), TOTAL_MEMBERS);
            assert_eq!(count::<IncentiveUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<IncentiveRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<IncentiveWorld>();
            assert_eq!(world.score_of(TeamId(0)), 0.0);
            assert_eq!(world.tick_count, 0);
        }
    }
}
