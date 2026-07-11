//! Scene 8 — babel. Infinity by repetition: a hexagonal gallery with a warm
//! center lamp; through two opposite doorways, identical galleries recede,
//! each dimmer than the last. Borges by mirrored geometry — the repetition is
//! staged, which is exactly what the game's doorway previews do for real.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;
use std::f32::consts::TAU;

pub fn spawn(ctx: &mut SceneCtx) {
    // Three galleries in a row along z; the camera stands in the nearest and
    // sees the chain through aligned doorways.
    let radius = 4.6_f32;
    let apothem = radius * (TAU / 12.0).cos();
    let spacing = apothem * 2.0 + 0.4; // adjacent hexes share the doorway wall
    for depth in 0..3 {
        gallery(ctx, Vec3::new(0.0, 0.0, -(depth as f32) * spacing), depth);
    }

    ctx.ambient(Color::srgb(0.7, 0.55, 0.4), 8.0);
    ctx.signal_kit(Vec3::new(-1.6, 0.0, 2.4), -0.5);
    ctx.camera(
        Transform::from_xyz(0.6, 1.5, 3.4).looking_at(Vec3::new(0.0, 1.3, -8.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.014, 0.010, 0.007), 10.0, 34.0)),
    );
}

/// One hexagonal gallery: six walls (two with doorway openings on the ±z
/// faces), shelf ribs, and the center lamp — dimmer with each `depth`.
fn gallery(ctx: &mut SceneCtx, center: Vec3, depth: usize) {
    let radius = 4.6_f32;
    let h = 4.4_f32;
    let dim = [1.0_f32, 0.55, 0.3][depth.min(2)];

    let wall = ctx.matte(Color::srgb(0.30 * dim, 0.24 * dim, 0.17 * dim), 0.85);
    let shelf = ctx.matte(Color::srgb(0.14 * dim, 0.10 * dim, 0.07 * dim), 0.9);
    let floor = ctx.matte(Color::srgb(0.22 * dim, 0.18 * dim, 0.13 * dim), 0.8);

    ctx.slab(
        center + Vec3::new(0.0, -0.1, 0.0),
        Vec3::new(2.3 * radius, 0.2, 2.3 * radius),
        floor,
        "Gallery floor",
    );
    ctx.slab(
        center + Vec3::new(0.0, h + 0.1, 0.0),
        Vec3::new(2.3 * radius, 0.2, 2.3 * radius),
        wall.clone(),
        "Gallery ceiling",
    );

    // Six walls; the two facing ±z get doorway openings (split into two piers
    // + a lintel), the other four carry shelf ribs.
    let apothem = radius * (TAU / 12.0).cos();
    let side_len = radius; // hexagon side length equals circumradius
    for i in 0..6 {
        let angle = TAU * (i as f32 + 0.5) / 6.0; // faces at 30°, 90°, …
        let normal = Vec2::new(angle.cos(), angle.sin());
        let wall_center = center + Vec3::new(normal.x * apothem, h * 0.5, normal.y * apothem);
        let yaw = -angle; // rotate the slab to lie along the face
        let is_door_face = normal.y.abs() > 0.95;
        if is_door_face {
            // Two piers flanking a 1.5-wide, 2.8-high doorway.
            let pier = (side_len - 1.5) * 0.5;
            for s in [-1.0_f32, 1.0] {
                let along = Vec3::new(-normal.y, 0.0, normal.x) * (s * (1.5 * 0.5 + pier * 0.5));
                ctx.slab_at(
                    Transform::from_translation(wall_center + along)
                        .with_rotation(Quat::from_rotation_y(yaw)),
                    Vec3::new(pier, h, 0.35),
                    wall.clone(),
                    "Door pier",
                );
            }
            ctx.slab_at(
                Transform::from_translation(
                    wall_center + Vec3::Y * (2.8 - h * 0.5 + (h - 2.8) * 0.5),
                ),
                Vec3::new(1.6, h - 2.8, 0.35),
                wall.clone(),
                "Door lintel",
            );
        } else {
            ctx.slab_at(
                Transform::from_translation(wall_center).with_rotation(Quat::from_rotation_y(yaw)),
                Vec3::new(side_len + 0.4, h, 0.35),
                wall.clone(),
                "Gallery wall",
            );
            // Shelf ribs: five horizontal lines — the books are implied.
            for row in 0..5 {
                let y = 0.5 + row as f32 * 0.75;
                ctx.slab_at(
                    Transform::from_translation(
                        center
                            + Vec3::new(
                                normal.x * (apothem - 0.25),
                                y,
                                normal.y * (apothem - 0.25),
                            ),
                    )
                    .with_rotation(Quat::from_rotation_y(yaw)),
                    Vec3::new(side_len - 0.6, 0.07, 0.3),
                    shelf.clone(),
                    "Shelf",
                );
            }
        }
    }

    // The lamp: a warm globe at center height, its light dimming with depth.
    let lamp = ctx.glow(LinearRgba::rgb(1.0, 0.7, 0.35), 5.0 * dim);
    ctx.slab(
        center + Vec3::new(0.0, h - 1.1, 0.0),
        Vec3::new(0.4, 0.4, 0.4),
        lamp,
        "Gallery lamp",
    );
    ctx.commands.spawn((
        SceneSpawned,
        PointLight {
            color: Color::srgb(1.0, 0.72, 0.4),
            intensity: 300_000.0 * dim,
            range: 12.0,
            shadows_enabled: depth == 0,
            ..default()
        },
        Transform::from_translation(center + Vec3::new(0.0, h - 1.3, 0.0)),
        Name::new("Gallery light"),
    ));
}
