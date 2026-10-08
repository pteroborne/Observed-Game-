//! Actual capsule travel, rather than requested velocity, guards slope cadence.
use glam::{Quat, Vec2, Vec3};
use player_input::PlayerIntent;

use super::{RapierTraversalScene, step_character_with_settings};
use crate::{
    ArenaSpec, ColliderSpec, FIXED_DT, FpsBody, StableColliderId, TraversalRuntimeProfile,
};

fn ramp(degrees: f32) -> RapierTraversalScene {
    let mut surface =
        ColliderSpec::cuboid(StableColliderId(0), Vec3::ZERO, Vec3::new(30.0, 0.5, 40.0));
    surface.rotation = Quat::from_rotation_x(degrees.to_radians()).to_array();
    RapierTraversalScene::from_arena_spec(&ArenaSpec {
        colliders: vec![surface],
        floor_y: 0.0,
        safety_center: Vec3::ZERO,
        safety_half: Vec3::splat(100.0),
    })
}

#[test]
fn grounded_ramp_travel_preserves_walk_sprint_and_analog_surface_speed() {
    let profile = TraversalRuntimeProfile::canonical_hex();
    let config = profile.controller();
    let mut failures = Vec::new();
    for degrees in [0.0_f32, 20.0, 35.0, 48.0] {
        for (label, movement, sprint) in [
            ("up", Vec2::Y, false),
            ("down", -Vec2::Y, false),
            ("across", Vec2::X, false),
            ("diagonal", Vec2::ONE.normalize(), false),
            ("sprint", Vec2::Y, true),
            ("bot", Vec2::Y * 0.35, false),
        ] {
            let scene = ramp(degrees);
            let mut body = FpsBody::spawned(Vec3::new(0.0, 3.0, 0.0), 0.0);
            let intent = PlayerIntent {
                movement,
                sprint_held: sprint,
                ..Default::default()
            };
            // Settle, then accelerate on the same continuous surface.
            for _ in 0..90 {
                step_character_with_settings(
                    &scene,
                    &mut body,
                    PlayerIntent::default(),
                    &config,
                    profile.rapier(),
                    FIXED_DT,
                );
            }
            for _ in 0..60 {
                step_character_with_settings(
                    &scene,
                    &mut body,
                    intent,
                    &config,
                    profile.rapier(),
                    FIXED_DT,
                );
            }
            let start = body.position;
            for tick in 0..60 {
                step_character_with_settings(
                    &scene,
                    &mut body,
                    intent,
                    &config,
                    profile.rapier(),
                    FIXED_DT,
                );
                if !body.grounded {
                    failures.push(format!(
                        "{degrees} {label} lost support at tick {tick}: {:?}",
                        body.position
                    ));
                }
            }
            let actual = body.position.distance(start);
            let expected = movement.length()
                * if sprint {
                    config.run_speed
                } else {
                    config.walk_speed
                };
            eprintln!(
                "ramp={degrees:>2} mode={label:<8} surface_speed={actual:.4} expected={expected:.4} m/s"
            );
            // Descending also includes Rapier's small contact/snap corrections.
            // Near the 50-degree climb cutoff Rapier's conservative shape casts
            // may reject individual contacts; retain at least 90% of target speed.
            let tolerance = if degrees > 45.0 {
                expected * 0.10
            } else if label == "down" {
                expected * 0.02
            } else {
                0.025
            };
            if (actual - expected).abs() >= tolerance {
                failures.push(format!("{degrees} {label}: {actual} vs {expected}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn slope_compensation_does_not_turn_walls_into_walkable_ramps() {
    let profile = TraversalRuntimeProfile::canonical_hex();
    let config = profile.controller();
    let mut scene = ramp(35.0);
    scene
        .apply_collider_delta(&crate::ColliderDelta {
            upserted: vec![ColliderSpec::cuboid(
                StableColliderId(1),
                Vec3::new(0.0, 5.0, -2.0),
                Vec3::new(5.0, 10.0, 0.2),
            )],
            ..Default::default()
        })
        .expect("wall inserted");
    let mut body = FpsBody::spawned(Vec3::new(0.0, 3.0, 0.0), 0.0);
    for _ in 0..90 {
        step_character_with_settings(
            &scene,
            &mut body,
            PlayerIntent::default(),
            &config,
            profile.rapier(),
            FIXED_DT,
        );
    }
    for _ in 0..240 {
        step_character_with_settings(
            &scene,
            &mut body,
            PlayerIntent {
                movement: Vec2::Y,
                sprint_held: true,
                ..Default::default()
            },
            &config,
            profile.rapier(),
            FIXED_DT,
        );
    }
    assert!(
        body.position.z > -1.5,
        "capsule crossed the ramp's end wall"
    );
    assert!(body.position.y < 4.0, "capsule climbed the vertical wall");
    let report = step_character_with_settings(
        &scene,
        &mut body,
        PlayerIntent {
            jump_pressed: true,
            ..Default::default()
        },
        &config,
        profile.rapier(),
        FIXED_DT,
    );
    assert!(report.jumped);
    assert!(!body.grounded);
    assert!(body.velocity.y > 0.0);
}

#[test]
fn solid_actor_base_does_not_sink_into_a_ramp() {
    let profile = TraversalRuntimeProfile::canonical_hex();
    let mut config = profile.controller();
    config.radius = 1.35;
    config.half_height = 1.4;
    config.walk_speed = 2.5;
    for degrees in [20.0_f32, 35.0] {
        let scene = ramp(degrees);
        let mut body = FpsBody::spawned(Vec3::new(0.0, 4.0, 0.0), 0.0);
        let intent = PlayerIntent {
            movement: Vec2::Y,
            ..Default::default()
        };
        for tick in 0..240 {
            let movement = if tick < 90 {
                PlayerIntent::default()
            } else {
                intent
            };
            super::step_solid_character_with_settings(
                &scene,
                &mut body,
                movement,
                &config,
                profile.rapier(),
                FIXED_DT,
            );
            if tick >= 90 {
                let highest_support = 0.5 / degrees.to_radians().cos()
                    - (body.position.z - config.radius) * degrees.to_radians().tan();
                assert!(
                    body.position.y - config.half_height >= highest_support - 0.01,
                    "{degrees}: solid base below support: {:?}",
                    body.position
                );
                assert!(body.grounded, "{degrees} degrees, tick {tick}: {body:?}");
            }
        }
    }
}
