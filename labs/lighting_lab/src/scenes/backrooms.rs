//! Scene 3 — backrooms. The overlit nowhere: low ceiling, mono-yellow, a grid
//! of humming panel lights, deliberately zero shadows, repetition without
//! landmarks. Dread from evenness — this is the register the overlit district
//! (Phase 70) ships.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    let (half, h) = (12.0_f32, 2.35_f32);
    let wallpaper = ctx.matte(Color::srgb(0.60, 0.54, 0.33), 0.85);
    let carpet = ctx.matte(Color::srgb(0.42, 0.38, 0.22), 0.95);
    let ceiling_tile = ctx.matte(Color::srgb(0.55, 0.52, 0.40), 0.9);

    ctx.slab(
        Vec3::new(0.0, -0.1, 0.0),
        Vec3::new(2.0 * half, 0.2, 2.0 * half),
        carpet,
        "Carpet",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.1, 0.0),
        Vec3::new(2.0 * half, 0.2, 2.0 * half),
        ceiling_tile,
        "Ceiling",
    );
    for (pos, size) in [
        (
            Vec3::new(0.0, h * 0.5, -half),
            Vec3::new(2.0 * half, h, 0.3),
        ),
        (Vec3::new(0.0, h * 0.5, half), Vec3::new(2.0 * half, h, 0.3)),
        (
            Vec3::new(-half, h * 0.5, 0.0),
            Vec3::new(0.3, h, 2.0 * half),
        ),
        (Vec3::new(half, h * 0.5, 0.0), Vec3::new(0.3, h, 2.0 * half)),
    ] {
        ctx.slab(pos, size, wallpaper.clone(), "Wall");
    }

    // Column grid every 4 m — identical, orientation-free.
    let column = ctx.matte(Color::srgb(0.57, 0.51, 0.31), 0.85);
    let mut x = -half + 4.0;
    while x < half - 1.0 {
        let mut z = -half + 4.0;
        while z < half - 1.0 {
            ctx.slab(
                Vec3::new(x, h * 0.5, z),
                Vec3::new(0.55, h, 0.55),
                column.clone(),
                "Column",
            );
            z += 4.0;
        }
        x += 4.0;
    }

    // The panel-light grid: emissive tiles, plus unshadowed point fills spaced
    // so the light is EVEN — no pools, no direction, no shadows anywhere.
    let panel = ctx.glow(LinearRgba::rgb(1.0, 0.93, 0.62), 4.0);
    let mut x = -half + 2.2;
    while x < half - 1.0 {
        let mut z = -half + 2.2;
        while z < half - 1.0 {
            ctx.slab(
                Vec3::new(x, h + 0.02, z),
                Vec3::new(1.5, 0.06, 1.5),
                panel.clone(),
                "Light panel",
            );
            ctx.commands.spawn((
                SceneSpawned,
                PointLight {
                    color: Color::srgb(1.0, 0.93, 0.68),
                    intensity: 60_000.0,
                    range: 7.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(x, h - 0.25, z),
                Name::new("Panel fill"),
            ));
            z += 4.4;
        }
        x += 4.4;
    }

    // High warm ambient: the flat, sourceless base that makes it feel wrong.
    ctx.ambient(Color::srgb(1.0, 0.94, 0.7), 420.0);
    ctx.signal_kit(Vec3::new(-2.6, 0.0, -5.5), 0.15);
    // Camera 4° off the grid axis — almost aligned, not quite. No fog: nothing
    // recedes, it just continues.
    ctx.camera(
        Transform::from_xyz(7.5, 1.5, 9.0).looking_at(Vec3::new(-4.0, 1.25, -8.0), Vec3::Y),
        None,
    );
}
