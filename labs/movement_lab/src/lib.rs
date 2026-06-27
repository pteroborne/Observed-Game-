mod input;
mod lab;
mod simulation;

use bevy::{
    input::InputSystems,
    prelude::*,
    time::Fixed,
    window::{PresentMode, WindowResolution},
};

pub use lab::{MovementCounters, MovementLabRuntime};
pub use simulation::{
    GroundContact, MovementBody, MovementConfig, MovementStep, MovementWorld, MovingPlatform,
    PlatformId, SolidRect, SupportId, SurfaceSegment, step_body,
};

#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FixedMovementSet {
    Platforms,
    Intent,
    Simulation,
}

pub struct MovementLabPlugin;

impl Plugin for MovementLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(MovementWorld::authored_course())
            .init_resource::<MovementConfig>()
            .init_resource::<MovementLabRuntime>()
            .init_resource::<MovementCounters>()
            .init_resource::<input::HumanInputBuffer>()
            .configure_sets(
                FixedUpdate,
                (
                    FixedMovementSet::Platforms,
                    FixedMovementSet::Intent,
                    FixedMovementSet::Simulation,
                )
                    .chain(),
            )
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(PreUpdate, input::sample_hardware_input.after(InputSystems))
            .add_systems(
                FixedUpdate,
                lab::advance_platforms.in_set(FixedMovementSet::Platforms),
            )
            .add_systems(
                FixedUpdate,
                input::prepare_player_intents.in_set(FixedMovementSet::Intent),
            )
            .add_systems(
                FixedUpdate,
                lab::simulate_players.in_set(FixedMovementSet::Simulation),
            )
            .add_systems(
                Update,
                (
                    lab::handle_lab_shortcuts,
                    lab::perform_reset,
                    lab::update_platform_visuals,
                    lab::present_players,
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
        Transform::from_xyz(0.0, -70.0, 1000.0),
        Name::new("Movement Lab Camera"),
    ));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Movement Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MovementLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::HumanInputBuffer,
        lab::{MovementLabUiRoot, PLAYER_COUNT, PlayerVisual},
    };
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use player_input::PlayerIntent;

    const DT: f32 = 1.0 / 60.0;

    fn flat_world() -> MovementWorld {
        MovementWorld {
            segments: vec![SurfaceSegment {
                start: Vec2::new(-500.0, 0.0),
                end: Vec2::new(500.0, 0.0),
            }],
            solids: Vec::new(),
            platforms: Vec::new(),
            elapsed: 0.0,
            bounds_min: Vec2::new(-1000.0, -500.0),
            bounds_max: Vec2::new(1000.0, 500.0),
        }
    }

    fn grounded_body(position_x: f32) -> MovementBody {
        let mut body = MovementBody::new(Vec2::new(position_x, 34.0));
        body.position = Vec2::new(position_x, 34.0);
        body.grounded = true;
        body.contact = Some(GroundContact {
            support: SupportId::Segment(0),
            normal: Vec2::Y,
        });
        body
    }

    #[test]
    fn walk_run_acceleration_and_deceleration_are_predictable() {
        let world = flat_world();
        let config = MovementConfig::default();
        let mut walker = grounded_body(0.0);
        let mut runner = grounded_body(0.0);

        for _ in 0..30 {
            step_body(
                &mut walker,
                PlayerIntent {
                    movement: Vec2::X,
                    ..default()
                },
                &world,
                config,
                DT,
            );
            step_body(
                &mut runner,
                PlayerIntent {
                    movement: Vec2::X,
                    sprint_held: true,
                    ..default()
                },
                &world,
                config,
                DT,
            );
        }

        assert!((walker.velocity.x - config.walk_speed).abs() < 0.1);
        assert!((runner.velocity.x - config.run_speed).abs() < 0.1);
        let before_release = runner.velocity.x;
        step_body(&mut runner, PlayerIntent::default(), &world, config, DT);
        assert!(runner.velocity.x < before_release);
        assert!(runner.velocity.x > 0.0);
    }

    #[test]
    fn jump_falls_and_lands_back_on_the_floor() {
        let world = flat_world();
        let config = MovementConfig::default();
        let mut body = grounded_body(0.0);

        let jump = step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );
        assert!(jump.jumped);
        assert!(body.velocity.y > 0.0);
        assert!(!body.grounded);

        let mut saw_fall = false;
        let mut landed = false;
        for _ in 0..180 {
            let report = step_body(&mut body, PlayerIntent::default(), &world, config, DT);
            saw_fall |= body.velocity.y < 0.0;
            landed |= report.landed;
            if body.grounded {
                break;
            }
        }
        assert!(saw_fall);
        assert!(landed);
        assert!(body.grounded);
        assert!((body.position.y - 34.0).abs() < 0.1);
    }

    #[test]
    fn coyote_time_allows_jump_just_after_leaving_ground() {
        let mut world = flat_world();
        let config = MovementConfig::default();
        let mut body = grounded_body(0.0);

        world.segments.clear();
        step_body(&mut body, PlayerIntent::default(), &world, config, DT);
        assert!(!body.grounded);
        assert!(body.coyote_remaining > 0.0);

        let report = step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );
        assert!(report.jumped);
        assert!(body.velocity.y > 0.0);
    }

    #[test]
    fn buffered_jump_fires_when_landing() {
        let world = flat_world();
        let config = MovementConfig::default();
        let mut body = MovementBody::new(Vec2::new(0.0, 72.0));
        body.position = Vec2::new(0.0, 72.0);
        body.velocity.y = -170.0;

        let first = step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );
        assert!(!first.jumped);

        let mut buffered_jump = false;
        for _ in 0..12 {
            buffered_jump |=
                step_body(&mut body, PlayerIntent::default(), &world, config, DT).jumped;
            if buffered_jump {
                break;
            }
        }
        assert!(buffered_jump);
        assert!(body.velocity.y > 0.0);
        assert!(!body.grounded);
    }

    #[test]
    fn slope_contact_raises_body_and_exposes_normal() {
        let world = MovementWorld::authored_course();
        let config = MovementConfig::default();
        let mut body = MovementBody::new(Vec2::new(-610.0, -220.0));
        body.position = Vec2::new(-610.0, -220.0);

        for _ in 0..75 {
            step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::X,
                    ..default()
                },
                &world,
                config,
                DT,
            );
        }

        assert!(body.position.y > -150.0);
        assert!(body.grounded);
        let normal = body.contact.expect("slope contact").normal;
        assert!(normal.x < -0.1);
        assert!(normal.y > 0.5);
    }

    #[test]
    fn stairs_use_authored_step_height() {
        let world = MovementWorld::authored_course();
        let config = MovementConfig::default();
        let mut body = MovementBody::new(Vec2::new(-180.0, -86.0));
        body.position = Vec2::new(-180.0, -86.0);
        body.grounded = true;
        body.contact = Some(GroundContact {
            support: SupportId::Segment(2),
            normal: Vec2::Y,
        });
        let mut steps = 0;

        for _ in 0..100 {
            let report = step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::X,
                    ..default()
                },
                &world,
                config,
                DT,
            );
            steps += usize::from(report.stepped_up);
        }

        assert!(steps >= 4);
        assert!(body.position.x > 40.0);
        assert!(body.position.y > 0.0);
        assert!(body.grounded);
    }

    #[test]
    fn grounded_body_inherits_moving_platform_delta() {
        let mut world = MovementWorld::authored_course();
        let config = MovementConfig::default();
        let platform = world.platforms[0];
        let mut body = MovementBody::new(Vec2::new(
            platform.center.x,
            platform.center.y + platform.half_size.y + 34.0,
        ));
        body.position = body.spawn_position;
        body.grounded = true;
        body.contact = Some(GroundContact {
            support: SupportId::Platform(platform.id),
            normal: Vec2::Y,
        });

        world.advance_platforms(DT);
        let expected_delta = world.platforms[0].delta();
        let before = body.position;
        step_body(&mut body, PlayerIntent::default(), &world, config, DT);

        assert!((body.position.y - before.y - expected_delta.y).abs() < 0.2);
        assert!(matches!(
            body.contact.map(|contact| contact.support),
            Some(SupportId::Platform(_))
        ));
    }

    #[test]
    fn leaving_bounds_respawns_at_authored_spawn() {
        let world = flat_world();
        let config = MovementConfig::default();
        let mut body = MovementBody::new(Vec2::new(-50.0, 34.0));
        body.position = Vec2::new(0.0, -800.0);

        let report = step_body(&mut body, PlayerIntent::default(), &world, config, DT);
        assert!(report.respawned);
        assert_eq!(body.position, body.spawn_position);
        assert_eq!(body.respawns, 1);
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(MovementLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn repeated_reset_preserves_entity_counts_and_restores_state() {
        let mut app = test_app();
        let initial_visuals = count::<PlayerVisual>(&mut app);
        let initial_roots = count::<MovementLabUiRoot>(&mut app);
        assert_eq!(initial_visuals, PLAYER_COUNT);
        assert_eq!(initial_roots, 1);

        for reset_count in 1..=10 {
            app.world_mut()
                .resource_mut::<MovementLabRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerVisual>(&mut app), initial_visuals);
            assert_eq!(count::<MovementLabUiRoot>(&mut app), initial_roots);
            assert_eq!(
                app.world().resource::<MovementLabRuntime>().reset_count,
                reset_count
            );
            assert_eq!(
                *app.world().resource::<HumanInputBuffer>(),
                HumanInputBuffer::default()
            );

            let world = app.world_mut();
            let mut query = world.query::<(&MovementBody, &PlayerIntent)>();
            assert!(query.iter(world).all(|(body, intent)| {
                body.position == body.spawn_position
                    && body.velocity == Vec2::ZERO
                    && intent.is_neutral()
            }));
        }
    }
}
