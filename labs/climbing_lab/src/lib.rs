mod input;
mod lab;
mod simulation;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    time::Fixed,
    window::{PresentMode, WindowResolution},
};

pub use lab::{ClimbCounters, ClimbLabRuntime};
pub use simulation::{
    ClimbBody, ClimbConfig, ClimbMode, ClimbSolid, ClimbStep, ClimbWorld, GrappleId, GrappleSocket,
    Ladder, LadderId, Ledge, LedgeId, step_body,
};

#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ClimbSet {
    Intent,
    Simulate,
}

pub struct ClimbLabPlugin;

impl Plugin for ClimbLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(ClimbWorld::authored_course())
            .init_resource::<ClimbConfig>()
            .init_resource::<ClimbLabRuntime>()
            .init_resource::<ClimbCounters>()
            .init_resource::<input::HumanInput>()
            .configure_sets(FixedUpdate, (ClimbSet::Intent, ClimbSet::Simulate).chain())
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(PreUpdate, input::sample_hardware.after(InputSystems))
            .add_systems(
                FixedUpdate,
                input::distribute_intents.in_set(ClimbSet::Intent),
            )
            .add_systems(FixedUpdate, lab::simulate_bodies.in_set(ClimbSet::Simulate))
            .add_systems(
                Update,
                (
                    lab::handle_shortcuts,
                    lab::perform_reset,
                    lab::present_bodies,
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
            scale: 1.45,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, -120.0, 1000.0),
        Name::new("Climbing Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Climbing Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ClimbLabPlugin);

    // Optional headless-friendly evidence capture: render the authored showcase
    // for a moment, save a screenshot to the given path, then exit.
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, taken: false })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    taken: bool,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if !request.taken && elapsed >= 0.6 {
        let path = request.path.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        request.taken = true;
    } else if request.taken && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::{BodyVisual, ClimbLabUiRoot, PLAYER_COUNT};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::PlayerIntent;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(ClimbLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_the_authored_showcase() {
        let mut app = test_app();
        assert_eq!(count::<BodyVisual>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<ClimbLabUiRoot>(&mut app), 1);

        let world = app.world_mut();
        let mut query = world.query::<&ClimbBody>();
        let modes: Vec<&'static str> = query.iter(world).map(|body| body.mode.label()).collect();
        assert!(modes.contains(&"LADDER"));
        assert!(modes.contains(&"LEDGE-HANG"));
        assert!(modes.contains(&"GRAPPLE"));
        assert!(modes.contains(&"FREE · GROUNDED"));
    }

    #[test]
    fn repeated_reset_restores_modes_without_leaking_entities() {
        let mut app = test_app();
        let initial_bodies = count::<BodyVisual>(&mut app);
        let initial_roots = count::<ClimbLabUiRoot>(&mut app);

        for reset_count in 1..=10 {
            {
                let world = app.world_mut();
                let mut query = world.query::<&mut ClimbBody>();
                for mut body in query.iter_mut(world) {
                    body.position += Vec2::new(40.0, -30.0);
                    body.mode = ClimbMode::Free { grounded: false };
                }
            }
            app.world_mut()
                .resource_mut::<ClimbLabRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<BodyVisual>(&mut app), initial_bodies);
            assert_eq!(count::<ClimbLabUiRoot>(&mut app), initial_roots);
            assert_eq!(
                app.world().resource::<ClimbLabRuntime>().reset_count,
                reset_count
            );

            let world = app.world_mut();
            let mut query = world.query::<(&ClimbBody, &PlayerIntent)>();
            assert!(query.iter(world).all(|(body, intent)| {
                body.position == body.spawn_position && *intent == PlayerIntent::default()
            }));
        }
    }
}
