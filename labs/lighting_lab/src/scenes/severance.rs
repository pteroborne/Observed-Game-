//! Scene 4 — Institutional. A deterministic rectilinear set combines a route
//! with four 90° turns, a square decision room, and a much taller empty-factory
//! expanse. The cutaway camera makes the complete spatial grammar auditable in
//! one capture while cold, even panels retain the maintained-institution read.

use std::collections::BTreeMap;

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

const TILE: f32 = 4.0;
const HALL_HEIGHT: f32 = 3.0;
const FACTORY_HEIGHT: f32 = 7.2;

pub(crate) const ROUTE: [(i32, i32); 16] = [
    (0, 2),
    (0, 1),
    (0, 0),
    (0, -1),
    (0, -2),
    (1, -2),
    (2, -2),
    (3, -2),
    (4, -2),
    (4, -3),
    (4, -4),
    (4, -5),
    (5, -5),
    (6, -5),
    (6, -6),
    (6, -7),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Zone {
    Hall,
    DecisionRoom,
    FactoryExpanse,
}

pub(crate) fn route_turn_count() -> usize {
    ROUTE
        .windows(3)
        .filter(|points| {
            let before = (points[1].0 - points[0].0, points[1].1 - points[0].1);
            let after = (points[2].0 - points[1].0, points[2].1 - points[1].1);
            before != after
        })
        .count()
}

pub(crate) fn plan() -> BTreeMap<(i32, i32), Zone> {
    let mut cells = BTreeMap::new();
    for cell in ROUTE {
        cells.insert(cell, Zone::Hall);
    }
    for x in -1..=1 {
        for z in -3..=-1 {
            cells.insert((x, z), Zone::DecisionRoom);
        }
    }
    for x in 5..=10 {
        for z in -10..=-7 {
            cells.insert((x, z), Zone::FactoryExpanse);
        }
    }
    cells
}

pub fn spawn(ctx: &mut SceneCtx) {
    debug_assert_eq!(route_turn_count(), 4);
    let white = ctx.matte(Color::srgb(0.80, 0.82, 0.81), 0.58);
    let factory_white = ctx.matte(Color::srgb(0.68, 0.71, 0.70), 0.72);
    let trim = ctx.matte(Color::srgb(0.52, 0.57, 0.55), 0.58);
    let hall_floor = ctx.matte(Color::srgb(0.12, 0.25, 0.18), 0.96);
    let room_floor = ctx.matte(Color::srgb(0.15, 0.29, 0.21), 0.94);
    let factory_floor = ctx.matte(Color::srgb(0.20, 0.25, 0.22), 0.88);
    let panel = ctx.glow(LinearRgba::rgb(0.90, 0.96, 0.96), 2.4);
    let factory_panel = ctx.glow(LinearRgba::rgb(0.82, 0.88, 0.86), 1.55);
    let cells = plan();

    for (&(x, z), &zone) in &cells {
        let center = Vec3::new(x as f32 * TILE, 0.0, z as f32 * TILE);
        let (height, floor, ceiling, floor_name, ceiling_name) = match zone {
            Zone::Hall => (
                HALL_HEIGHT,
                hall_floor.clone(),
                panel.clone(),
                "Institutional hall floor",
                "Institutional hall ceiling panel",
            ),
            Zone::DecisionRoom => (
                HALL_HEIGHT,
                room_floor.clone(),
                panel.clone(),
                "Decision room floor",
                "Decision room ceiling panel",
            ),
            Zone::FactoryExpanse => (
                FACTORY_HEIGHT,
                factory_floor.clone(),
                factory_panel.clone(),
                "Empty factory floor",
                "Empty factory ceiling panel",
            ),
        };
        ctx.slab(
            center + Vec3::new(0.0, -0.1, 0.0),
            Vec3::new(TILE, 0.2, TILE),
            floor,
            floor_name,
        );
        ctx.slab(
            center + Vec3::new(0.0, height + 0.04, 0.0),
            if zone == Zone::FactoryExpanse {
                Vec3::new(0.7, 0.06, TILE - 0.55)
            } else {
                Vec3::new(0.9, 0.06, TILE - 0.7)
            },
            ceiling,
            ceiling_name,
        );

        if (x + z).rem_euclid(2) == 0 {
            let (intensity, range) = if zone == Zone::FactoryExpanse {
                (38_000.0, 8.5)
            } else {
                (25_000.0, 6.0)
            };
            ctx.commands.spawn((
                SceneSpawned,
                PointLight {
                    color: Color::srgb(0.90, 0.95, 0.94),
                    intensity,
                    range,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(center + Vec3::new(0.0, height - 0.35, 0.0)),
                Name::new("Institutional panel fill"),
            ));
        }

        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if cells.contains_key(&(x + dx, z + dz)) {
                continue;
            }
            let wall_material = if zone == Zone::FactoryExpanse {
                factory_white.clone()
            } else {
                white.clone()
            };
            let wall_center =
                center + Vec3::new(dx as f32 * TILE * 0.5, height * 0.5, dz as f32 * TILE * 0.5);
            let wall_size = if dx != 0 {
                Vec3::new(0.18, height, TILE + 0.04)
            } else {
                Vec3::new(TILE + 0.04, height, 0.18)
            };
            ctx.slab(
                wall_center,
                wall_size,
                wall_material,
                "Institutional boundary wall",
            );

            let trim_center = wall_center.with_y(1.0);
            let trim_size = if dx != 0 {
                Vec3::new(0.21, 0.07, TILE + 0.04)
            } else {
                Vec3::new(TILE + 0.04, 0.07, 0.21)
            };
            ctx.slab(
                trim_center,
                trim_size,
                trim.clone(),
                "Institutional wainscot",
            );
        }
    }

    // Sparse columns and high beams communicate factory scale without turning
    // the expanse into an obstacle course.
    let structure = ctx.matte(Color::srgb(0.42, 0.47, 0.45), 0.68);
    for x in [5.3_f32, 7.5, 9.7] {
        for z in [-7.3_f32, -9.7] {
            ctx.slab(
                Vec3::new(x * TILE, FACTORY_HEIGHT * 0.5, z * TILE),
                Vec3::new(0.32, FACTORY_HEIGHT, 0.32),
                structure.clone(),
                "Empty factory column",
            );
        }
    }
    for z in [-7.3_f32, -9.7] {
        ctx.slab(
            Vec3::new(7.5 * TILE, FACTORY_HEIGHT - 0.55, z * TILE),
            Vec3::new(18.0, 0.34, 0.34),
            structure.clone(),
            "Empty factory roof beam",
        );
    }

    ctx.ambient(Color::srgb(0.84, 0.91, 0.90), 185.0);
    ctx.signal_kit(Vec3::new(0.0, 0.0, -8.0), -0.55);
    // A cutaway proof camera: all turns, the decision room, and the scale jump
    // into the empty factory are visible in a single deterministic capture.
    ctx.camera(
        Transform::from_xyz(-13.0, 20.0, 14.0).looking_at(Vec3::new(17.0, 0.4, -23.0), Vec3::Y),
        None,
    );
}
