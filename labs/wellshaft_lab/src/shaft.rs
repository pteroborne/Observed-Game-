//! Deterministic geometry for the wellshaft: a hexagonal central pillar, six
//! landing directions with one outward threshold bridge each, and a spiral stair
//! whose treads are **cantilevered from the pillar** — each tread springs from
//! the pillar face, winds a fixed angle around it, and closes onto the tread
//! below, so the flight reads as a solid staircase bolted to the core rather
//! than a row of blocks floating in the shaft. A short guard rail lines the open
//! (outward) edge of every tread so the descent cannot be walked off by
//! accident.
//!
//! The production controller consumes AABBs. Visible tread tops therefore are the
//! controller's real collision heights; rotated tread/guard footprints use their
//! conservative world-space AABBs. The pillar renders as a true hexagonal prism
//! and uses a conservative square collision core that sits inboard of every
//! tread's inner edge, so the walkable band never touches it.

use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};
use observed_traversal::Aabb3;

pub const HALF: f32 = 15.5;
pub const LEVELS: usize = 6;
pub const LEVEL_H: f32 = 3.0;
pub const PILLAR_RADIUS: f32 = 6.0;
/// Conservative square collision core for the pillar. Its corners (≈ r 4.95) stay
/// inboard of every tread's inner edge, so the controller resolves against the
/// treads, never against a lip poking up through them.
pub const PILLAR_COLLISION_HALF: f32 = 3.3;
pub const LANDING_RADIUS: f32 = 9.6;
pub const LANDING_HALF: f32 = 1.6;
pub const BRIDGE_END_RADIUS: f32 = 14.5;
pub const BRIDGE_WIDTH: f32 = 1.8;

/// Treads span this radial band. The inner edge overlaps the pillar (< its hex
/// radius) so each tread visibly springs from the core; the outer edge overlaps
/// the landing platforms so the flight and landings share continuous floor.
pub const TREAD_INNER_RADIUS: f32 = 4.8;
pub const TREAD_OUTER_RADIUS: f32 = 9.2;
pub const TREAD_MID_RADIUS: f32 = (TREAD_INNER_RADIUS + TREAD_OUTER_RADIUS) * 0.5;
pub const TREAD_RADIAL_HALF: f32 = (TREAD_OUTER_RADIUS - TREAD_INNER_RADIUS) * 0.5;
/// Eight treads create seven legal-height risers per flight; the first tread
/// meets its lower landing and the last meets the upper landing.
pub const STEPS_PER_FLIGHT: usize = 8;
pub const STEP_RISE: f32 = LEVEL_H / (STEPS_PER_FLIGHT - 1) as f32;
/// Tangential half-width of a tread. Chosen so adjacent treads overlap along the
/// whole radial band (contiguous, no gaps) while the exposed run stays walkable.
pub const TREAD_TANGENTIAL_HALF: f32 = 0.7;
/// Vertical closure of a tread: one riser plus a small lip, so each tread's front
/// face closes down onto the tread below (no open risers) with minimal overhang.
pub const TREAD_CLOSURE: f32 = STEP_RISE + 0.12;

pub const GUARD_HEIGHT: f32 = 1.0;
pub const GUARD_THICKNESS: f32 = 0.24;

pub const DOOR_WIDTH: f32 = 2.4;
pub const DOOR_HEIGHT: f32 = 2.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Floor,
    Pillar,
    Landing(usize),
    Bridge(usize),
    Step,
    /// A short parapet on a tread's outward edge — collidable, keeps the descent
    /// from being walked off sideways.
    Guard,
    /// Three non-colliding pieces form a threshold frame at this level.
    Doorway(usize),
    Lamp(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaftPart {
    pub center: Vec3,
    /// Local cuboid half-extents. Pillar uses X as its hex-prism radius.
    pub half: Vec3,
    pub yaw: f32,
    pub part: Part,
}

impl ShaftPart {
    fn cuboid(center: Vec3, half: Vec3, yaw: f32, part: Part) -> Self {
        Self {
            center,
            half,
            yaw,
            part,
        }
    }
}

pub fn level_top(level: usize) -> f32 {
    level as f32 * LEVEL_H
}

/// Signed angle of a level's landing/bridge/threshold, measured so consecutive
/// levels wind a constant +60° around the pillar (the spiral's pitch).
pub fn level_angle(level: usize) -> f32 {
    level as f32 * TAU / LEVELS as f32
}

/// Six radial directions, one per landing/bridge/threshold.
pub fn level_direction(level: usize) -> Vec2 {
    Vec2::from_angle(level_angle(level))
}

pub fn landing_center(level: usize) -> Vec2 {
    level_direction(level) * LANDING_RADIUS
}

/// Regroup point biased toward the bridge, clear of the stair's top tread.
pub fn landing_rest(level: usize) -> Vec2 {
    landing_center(level) + level_direction(level) * 0.55
}

fn tangent(direction: Vec2) -> Vec2 {
    Vec2::new(-direction.y, direction.x)
}

/// Bevy yaw that rotates a cuboid's local +X axis onto an X/Z direction.
fn direction_yaw(direction: Vec2) -> f32 {
    (-direction.y).atan2(direction.x)
}

/// Angle of the `step`-th tread of the flight rising from `lower_level` to the
/// next level: evenly spaced across the 60° the spiral turns between landings.
fn tread_angle(lower_level: usize, step: usize) -> f32 {
    let a0 = level_angle(lower_level);
    let a1 = level_angle(lower_level + 1);
    a0 + (a1 - a0) * (step as f32 + 0.5) / STEPS_PER_FLIGHT as f32
}

/// World-space XZ centre of a tread's footprint (mid-band radius).
pub fn stair_center(lower_level: usize, step: usize) -> Vec2 {
    Vec2::from_angle(tread_angle(lower_level, step)) * TREAD_MID_RADIUS
}

pub fn build() -> Vec<ShaftPart> {
    let mut parts = Vec::new();
    let total_height = level_top(LEVELS - 1) + LEVEL_H;

    parts.push(ShaftPart::cuboid(
        Vec3::new(0.0, -0.15, 0.0),
        Vec3::new(HALF, 0.15, HALF),
        0.0,
        Part::Floor,
    ));
    parts.push(ShaftPart::cuboid(
        Vec3::new(0.0, total_height * 0.5, 0.0),
        Vec3::new(PILLAR_RADIUS, total_height * 0.5, PILLAR_RADIUS),
        0.0,
        Part::Pillar,
    ));

    for level in 0..LEVELS {
        let top = level_top(level);
        let direction = level_direction(level);
        let landing = landing_center(level);
        let yaw = direction_yaw(direction);

        parts.push(ShaftPart::cuboid(
            Vec3::new(landing.x, top - 0.15, landing.y),
            Vec3::new(LANDING_HALF, 0.15, LANDING_HALF),
            yaw,
            Part::Landing(level),
        ));

        let bridge_start = LANDING_RADIUS + LANDING_HALF * 0.65;
        let bridge_length = BRIDGE_END_RADIUS - bridge_start;
        let bridge_center = direction * (bridge_start + bridge_length * 0.5);
        parts.push(ShaftPart::cuboid(
            Vec3::new(bridge_center.x, top - 0.15, bridge_center.y),
            Vec3::new(bridge_length * 0.5, 0.15, BRIDGE_WIDTH * 0.5),
            yaw,
            Part::Bridge(level),
        ));

        let threshold_center = direction * BRIDGE_END_RADIUS;
        let threshold_tangent = tangent(direction);
        for sign in [-1.0, 1.0] {
            let jamb = threshold_center + threshold_tangent * DOOR_WIDTH * 0.5 * sign;
            parts.push(ShaftPart::cuboid(
                Vec3::new(jamb.x, top + DOOR_HEIGHT * 0.5, jamb.y),
                Vec3::new(0.07, DOOR_HEIGHT * 0.5, 0.07),
                yaw,
                Part::Doorway(level),
            ));
        }
        parts.push(ShaftPart::cuboid(
            Vec3::new(threshold_center.x, top + DOOR_HEIGHT, threshold_center.y),
            Vec3::new(DOOR_WIDTH * 0.5 + 0.07, 0.07, 0.07),
            direction_yaw(threshold_tangent),
            Part::Doorway(level),
        ));

        let lamp = landing + tangent(direction) * 0.85;
        parts.push(ShaftPart::cuboid(
            Vec3::new(lamp.x, top + 2.05, lamp.y),
            Vec3::splat(0.18),
            0.0,
            Part::Lamp(level),
        ));

        if level + 1 < LEVELS {
            for step in 0..STEPS_PER_FLIGHT {
                let angle = tread_angle(level, step);
                let u = Vec2::from_angle(angle);
                let center = u * TREAD_MID_RADIUS;
                let step_top = top + step as f32 * STEP_RISE;
                let center_y = step_top - TREAD_CLOSURE * 0.5;
                parts.push(ShaftPart::cuboid(
                    Vec3::new(center.x, center_y, center.y),
                    Vec3::new(
                        TREAD_RADIAL_HALF,
                        TREAD_CLOSURE * 0.5,
                        TREAD_TANGENTIAL_HALF,
                    ),
                    direction_yaw(u),
                    Part::Step,
                ));

                // Guard only the mid-flight treads. The first and last treads abut
                // the landings; railing them would seal the flight off from the
                // walkway out to the threshold bridge.
                if step > 0 && step + 1 < STEPS_PER_FLIGHT {
                    let guard_xz = u * (TREAD_OUTER_RADIUS + GUARD_THICKNESS * 0.5);
                    parts.push(ShaftPart::cuboid(
                        Vec3::new(guard_xz.x, step_top + GUARD_HEIGHT * 0.5, guard_xz.y),
                        Vec3::new(
                            GUARD_THICKNESS * 0.5,
                            GUARD_HEIGHT * 0.5,
                            TREAD_TANGENTIAL_HALF,
                        ),
                        direction_yaw(u),
                        Part::Guard,
                    ));
                }
            }
        }
    }

    parts
}

fn rotated_aabb_half(part: &ShaftPart) -> Vec3 {
    let (sin, cos) = part.yaw.sin_cos();
    Vec3::new(
        cos.abs() * part.half.x + sin.abs() * part.half.z,
        part.half.y,
        sin.abs() * part.half.x + cos.abs() * part.half.z,
    )
}

pub fn solids(parts: &[ShaftPart]) -> Vec<Aabb3> {
    parts
        .iter()
        .filter(|part| {
            matches!(
                part.part,
                Part::Floor
                    | Part::Pillar
                    | Part::Landing(_)
                    | Part::Bridge(_)
                    | Part::Step
                    | Part::Guard
            )
        })
        .map(|part| {
            if part.part == Part::Pillar {
                // Conservative square core, inboard of the treads.
                Aabb3::from_center_half(
                    part.center,
                    Vec3::new(PILLAR_COLLISION_HALF, part.half.y, PILLAR_COLLISION_HALF),
                )
            } else {
                Aabb3::from_center_half(part.center, rotated_aabb_half(part))
            }
        })
        .collect()
}

pub fn spawn_xz() -> Vec2 {
    landing_rest(LEVELS - 1)
}

pub fn spawn_height() -> f32 {
    level_top(LEVELS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_traversal::{FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
    use player_input::PlayerIntent;

    fn count(parts: &[ShaftPart], predicate: impl Fn(Part) -> bool) -> usize {
        parts.iter().filter(|part| predicate(part.part)).count()
    }

    fn climb_to(
        body: &mut FpsBody,
        target: Vec2,
        target_feet: f32,
        arena: &FpsArena,
        config: &FpsConfig,
    ) {
        for _ in 0..360 {
            let here = Vec2::new(body.position.x, body.position.z);
            let delta = target - here;
            let feet = body.position.y - config.half_height;
            if delta.length() < 0.72 && feet >= target_feet - 0.05 {
                return;
            }
            let direction = delta.normalize_or_zero();
            body.yaw = direction.x.atan2(-direction.y);
            step_body(
                body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..Default::default()
                },
                arena,
                config,
                FIXED_DT,
            );
        }
        panic!(
            "body did not climb to {target:?} at {target_feet}; stopped at {:?}",
            body.position
        );
    }

    fn descend_to(
        body: &mut FpsBody,
        target: Vec2,
        target_feet: f32,
        arena: &FpsArena,
        config: &FpsConfig,
    ) {
        for _ in 0..360 {
            let here = Vec2::new(body.position.x, body.position.z);
            let delta = target - here;
            if delta.length() < 0.72 {
                return;
            }
            let direction = delta.normalize_or_zero();
            body.yaw = direction.x.atan2(-direction.y);
            step_body(
                body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..Default::default()
                },
                arena,
                config,
                FIXED_DT,
            );
        }
        panic!(
            "body did not descend to {target:?} at {target_feet}; stopped at {:?}",
            body.position
        );
    }

    #[test]
    fn geometry_is_deterministic_and_uses_all_six_threshold_directions() {
        let first = build();
        assert_eq!(first, build());
        assert_eq!(count(&first, |part| part == Part::Pillar), 1);
        assert_eq!(
            count(&first, |part| matches!(part, Part::Doorway(_))),
            LEVELS * 3
        );
        assert_eq!(count(&first, |part| matches!(part, Part::Lamp(_))), LEVELS);
        assert_eq!(
            count(&first, |part| part == Part::Step),
            (LEVELS - 1) * STEPS_PER_FLIGHT
        );
        // One guard rail per mid-flight tread (end treads abut landings).
        assert_eq!(
            count(&first, |part| part == Part::Guard),
            (LEVELS - 1) * (STEPS_PER_FLIGHT - 2)
        );
        for level in 0..LEVELS {
            assert_eq!(count(&first, |part| part == Part::Landing(level)), 1);
            assert_eq!(count(&first, |part| part == Part::Bridge(level)), 1);
            assert_eq!(count(&first, |part| part == Part::Doorway(level)), 3);
            assert_eq!(count(&first, |part| part == Part::Lamp(level)), 1);
            for other in 0..level {
                assert!(level_direction(level).distance(level_direction(other)) > 0.9);
            }
        }
    }

    #[test]
    fn treads_spring_from_the_pillar_and_stay_contiguous() {
        // Inner edge overlaps the pillar (springs from the core), and the pillar's
        // collision corner stays inboard of it (no lip through the treads).
        const { assert!(TREAD_INNER_RADIUS < PILLAR_RADIUS) };
        assert!(PILLAR_COLLISION_HALF * 2.0_f32.sqrt() <= TREAD_INNER_RADIUS + 0.1);

        // Adjacent treads overlap along the whole radial band: their tangential
        // half-widths sum to more than the widest (outer-edge) angular spacing.
        let dtheta = TAU / LEVELS as f32 / STEPS_PER_FLIGHT as f32;
        let outer_spacing = TREAD_OUTER_RADIUS * dtheta;
        assert!(
            2.0 * TREAD_TANGENTIAL_HALF > outer_spacing,
            "treads gap at the outer edge: {outer_spacing}"
        );
        // The exposed tangential run at the walking band is a legal footing.
        let run = TREAD_MID_RADIUS * dtheta;
        assert!(
            run >= FpsConfig::default().radius * 2.0,
            "run too short: {run}"
        );
    }

    #[test]
    fn collision_keeps_visible_tread_heights_and_controller_step_limit() {
        let parts = build();
        let collidable = count(&parts, |part| {
            matches!(
                part,
                Part::Floor
                    | Part::Pillar
                    | Part::Landing(_)
                    | Part::Bridge(_)
                    | Part::Step
                    | Part::Guard
            )
        });
        assert_eq!(solids(&parts).len(), collidable);
        assert!(STEP_RISE <= FpsConfig::default().step_height);

        let spawn = spawn_xz();
        let supported = parts.iter().any(|part| {
            part.part == Part::Landing(LEVELS - 1)
                && (part.center.y + part.half.y - spawn_height()).abs() < 0.02
                && (spawn.x - part.center.x).abs() <= LANDING_HALF
                && (spawn.y - part.center.z).abs() <= LANDING_HALF
        });
        assert!(supported, "top spawn must stand on its visible landing");
    }

    #[test]
    fn production_controller_walks_the_hex_spiral_in_both_directions_without_jumping() {
        let config = FpsConfig::default();
        let parts = build();
        let arena = FpsArena {
            solids: solids(&parts),
            floor_y: 0.0,
            floor_half: HALF,
        };
        let spawn = spawn_xz();
        let mut body = FpsBody::spawned(
            Vec3::new(spawn.x, spawn_height() + config.half_height, spawn.y),
            0.0,
        );
        let mut landing_heights = Vec::new();

        for upper_level in (1..LEVELS).rev() {
            let lower_level = upper_level - 1;
            for step in (0..STEPS_PER_FLIGHT).rev() {
                descend_to(
                    &mut body,
                    stair_center(lower_level, step),
                    level_top(lower_level) + step as f32 * STEP_RISE,
                    &arena,
                    &config,
                );
            }
            descend_to(
                &mut body,
                landing_rest(lower_level),
                level_top(lower_level),
                &arena,
                &config,
            );
            for _ in 0..30 {
                step_body(
                    &mut body,
                    PlayerIntent::default(),
                    &arena,
                    &config,
                    FIXED_DT,
                );
            }
            landing_heights.push(body.position.y - config.half_height);
        }

        let feet = body.position.y - config.half_height;
        assert!(
            feet <= 0.05,
            "spiral must reach the bottom, got {feet}; landings {landing_heights:?}"
        );
        for (index, height) in landing_heights.iter().enumerate() {
            let expected = level_top(LEVELS - 2 - index);
            assert!(
                (height - expected).abs() < 0.08,
                "landing {index} expected {expected}, got {height}"
            );
        }

        let mut ascent_heights = Vec::new();
        for lower_level in 0..LEVELS - 1 {
            for step in 0..STEPS_PER_FLIGHT {
                climb_to(
                    &mut body,
                    stair_center(lower_level, step),
                    level_top(lower_level) + step as f32 * STEP_RISE,
                    &arena,
                    &config,
                );
            }
            climb_to(
                &mut body,
                landing_rest(lower_level + 1),
                level_top(lower_level + 1),
                &arena,
                &config,
            );
            for _ in 0..30 {
                step_body(
                    &mut body,
                    PlayerIntent::default(),
                    &arena,
                    &config,
                    FIXED_DT,
                );
            }
            ascent_heights.push(body.position.y - config.half_height);
        }
        assert!(
            (body.position.y - config.half_height - spawn_height()).abs() < 0.08,
            "spiral must return to the top; ascents {ascent_heights:?}"
        );
    }
}
