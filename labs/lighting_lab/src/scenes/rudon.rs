//! Scene 9 — thinning. Decaying variety (Rudon's Plane): one long corridor
//! where everything that makes a place a place — trim, fixtures, saturation,
//! light — thins with distance until only fog is left. The decay implies the
//! corridor never ends; it just stops offering.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;
use observed_style as style;

pub fn spawn(ctx: &mut SceneCtx) {
    let (w, h, len) = (4.2_f32, 3.1_f32, 90.0_f32);
    let hw = w * 0.5;
    let seg = 3.0_f32;
    let segments = (len / seg) as usize;

    // The warm start palette: Reactor's temperature, decaying to dead grey.
    let reactor = style::district(style::District::Reactor);
    let warm = reactor.light_color.to_srgba();

    for i in 0..segments {
        let z0 = -(i as f32) * seg;
        let zc = z0 - seg * 0.5;
        // Variety fades: t = 0 (full) → 1 (featureless).
        let t = (i as f32 / (segments as f32 * 0.7)).min(1.0);
        let sat = 1.0 - t;

        let wall_color = Color::srgb(
            0.30 * sat + 0.16 * t,
            0.22 * sat + 0.16 * t,
            0.16 * sat + 0.16 * t,
        );
        let wall = ctx.matte(wall_color, 0.9);
        let floor_color = Color::srgb(
            0.22 * sat + 0.12 * t,
            0.17 * sat + 0.12 * t,
            0.13 * sat + 0.12 * t,
        );
        let floor = ctx.matte(floor_color, 0.85);

        ctx.slab(Vec3::new(0.0, -0.1, zc), Vec3::new(w, 0.2, seg + 0.05), floor, "Floor seg");
        ctx.slab(
            Vec3::new(0.0, h + 0.1, zc),
            Vec3::new(w, 0.2, seg + 0.05),
            wall.clone(),
            "Ceiling seg",
        );
        for sx in [-1.0_f32, 1.0] {
            ctx.slab(
                Vec3::new(sx * (hw + 0.1), h * 0.5, zc),
                Vec3::new(0.2, h, seg + 0.05),
                wall.clone(),
                "Wall seg",
            );
        }

        // Trim ribs: dense at the start, skipped more and more often.
        let trim_every = 1 + (t * 5.0) as usize;
        if i % trim_every == 0 && sat > 0.08 {
            let trim = ctx.matte(
                Color::srgb(0.5 * sat + 0.18 * t, 0.35 * sat + 0.18 * t, 0.22 * sat + 0.18 * t),
                0.7,
            );
            for sx in [-1.0_f32, 1.0] {
                for y in [0.15_f32, h - 0.2] {
                    ctx.slab(
                        Vec3::new(sx * (hw + 0.02), y, zc),
                        Vec3::new(0.05, 0.08, seg * 0.9),
                        trim.clone(),
                        "Trim",
                    );
                }
            }
        }

        // Fixtures: warm and frequent near the start; sparser, dimmer, and
        // colder until they stop appearing at all.
        let fixture_every = 2 + (t * 6.0) as usize;
        if i % fixture_every == 0 && t < 0.85 {
            let strength = (1.0 - t) * (1.0 - t);
            let fixture_color = Color::srgb(
                warm.red * sat + 0.5 * t,
                warm.green * sat + 0.5 * t,
                warm.blue * sat + 0.5 * t,
            );
            let lamp = ctx.glow(
                LinearRgba::rgb(1.0 * strength + 0.2, 0.7 * strength + 0.2, 0.4 * strength + 0.2),
                4.0 * strength + 0.4,
            );
            ctx.slab(
                Vec3::new(0.0, h - 0.08, zc),
                Vec3::new(0.9, 0.1, 0.35),
                lamp,
                "Fixture",
            );
            ctx.commands.spawn((
                SceneSpawned,
                PointLight {
                    color: fixture_color,
                    intensity: 160_000.0 * strength + 4_000.0,
                    range: 9.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, h - 0.5, zc),
                Name::new("Fixture light"),
            ));
        }
    }

    // No end wall: the fog is the end.
    ctx.ambient(Color::srgb(0.8, 0.75, 0.7), 20.0);
    ctx.signal_kit(Vec3::new(-0.7, 0.0, -7.5), 0.3);
    ctx.camera(
        Transform::from_xyz(0.0, 1.5, 1.5).looking_at(Vec3::new(0.0, 1.35, -30.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.055, 0.052, 0.05), 22.0, 74.0)),
    );
}
