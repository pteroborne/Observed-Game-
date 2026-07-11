//! Scene 7 — wellshaft. Warm pools in buried dark: a vertical well, ring
//! platforms descending, one caged warm practical per level with a tight
//! range so each pool is an island. Verticality points DOWN.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    let concrete = ctx.matte(Color::srgb(0.24, 0.235, 0.225), 0.95);
    let concrete_dark = ctx.matte(Color::srgb(0.16, 0.155, 0.15), 0.95);

    // The well: 11 × 11 shaft, 36 deep. Camera stands on the top ring.
    let (half, depth) = (5.5_f32, 36.0_f32);

    // Shaft walls (four slabs the full depth).
    for (pos, size) in [
        (
            Vec3::new(0.0, -depth * 0.5 + 3.0, -half - 0.4),
            Vec3::new(2.0 * half + 2.0, depth + 8.0, 0.8),
        ),
        (
            Vec3::new(0.0, -depth * 0.5 + 3.0, half + 0.4),
            Vec3::new(2.0 * half + 2.0, depth + 8.0, 0.8),
        ),
        (
            Vec3::new(-half - 0.4, -depth * 0.5 + 3.0, 0.0),
            Vec3::new(0.8, depth + 8.0, 2.0 * half + 2.0),
        ),
        (
            Vec3::new(half + 0.4, -depth * 0.5 + 3.0, 0.0),
            Vec3::new(0.8, depth + 8.0, 2.0 * half + 2.0),
        ),
    ] {
        ctx.slab(pos, size, concrete.clone(), "Shaft wall");
    }

    // Ring platforms every 5 m down: four ledge slabs leaving a central void,
    // each level's practical staggered a quarter-turn from the last.
    let cage = ctx.matte(Color::srgb(0.1, 0.09, 0.08), 0.8);
    for level in 0..7 {
        let y = -(level as f32) * 5.0;
        let ledge = 1.7_f32;
        for (pos, size) in [
            (
                Vec3::new(0.0, y - 0.15, -half + ledge * 0.5),
                Vec3::new(2.0 * half, 0.3, ledge),
            ),
            (
                Vec3::new(0.0, y - 0.15, half - ledge * 0.5),
                Vec3::new(2.0 * half, 0.3, ledge),
            ),
            (
                Vec3::new(-half + ledge * 0.5, y - 0.15, 0.0),
                Vec3::new(ledge, 0.3, 2.0 * half - 2.0 * ledge),
            ),
            (
                Vec3::new(half - ledge * 0.5, y - 0.15, 0.0),
                Vec3::new(ledge, 0.3, 2.0 * half - 2.0 * ledge),
            ),
        ] {
            ctx.slab(pos, size, concrete_dark.clone(), "Ring ledge");
        }

        // The caged practical: a small warm emissive lamp + tight-range light,
        // staggered around the well so the descent spirals.
        let corner = [
            Vec3::new(-half + 1.0, 0.0, -half + 1.0),
            Vec3::new(half - 1.0, 0.0, -half + 1.0),
            Vec3::new(half - 1.0, 0.0, half - 1.0),
            Vec3::new(-half + 1.0, 0.0, half - 1.0),
        ][level % 4];
        let lamp_pos = corner + Vec3::new(0.0, y + 1.6, 0.0);
        let lamp = ctx.glow(LinearRgba::rgb(1.0, 0.55, 0.2), 8.0);
        ctx.slab(lamp_pos, Vec3::new(0.22, 0.3, 0.22), lamp, "Practical");
        // Cage bars.
        for (dx, dz) in [(-0.18_f32, 0.0_f32), (0.18, 0.0), (0.0, -0.18), (0.0, 0.18)] {
            ctx.slab(
                lamp_pos + Vec3::new(dx, 0.0, dz),
                Vec3::new(0.03, 0.42, 0.03),
                cage.clone(),
                "Cage bar",
            );
        }
        ctx.commands.spawn((
            SceneSpawned,
            PointLight {
                color: Color::srgb(1.0, 0.6, 0.25),
                intensity: 260_000.0,
                range: 7.5,
                shadows_enabled: level < 2, // shadow budget: only the top pools cast
                ..default()
            },
            Transform::from_translation(lamp_pos),
            Name::new("Practical light"),
        ));
    }

    ctx.ambient(Color::srgb(0.4, 0.45, 0.55), 3.0);
    ctx.signal_kit(Vec3::new(0.6, 0.0, 3.2), 2.8);
    // Camera on the top ring, looking down the well: pools of warm receding
    // into black.
    ctx.camera(
        Transform::from_xyz(-half + 2.0, 1.7, half - 2.2)
            .looking_at(Vec3::new(1.5, -22.0, -1.5), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.008, 0.007, 0.006), 14.0, 46.0)),
    );
}
