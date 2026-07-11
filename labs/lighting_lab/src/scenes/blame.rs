//! Scene 6 — megastructure. The BLAME! void: a space too large to see the
//! edges of, one faint distant light, gantry struts silhouetted against it, a
//! narrow walkway forward. The figure is small and the dark is structural.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    let near_black = ctx.matte(Color::srgb(0.035, 0.04, 0.05), 0.9);
    let strut_mat = ctx.matte(Color::srgb(0.02, 0.022, 0.028), 0.95);

    // A vast implied volume: only a far backdrop plane (lit by the distant
    // light), a floor far below, and the walkway exist. Fog owns the rest.
    ctx.slab(
        Vec3::new(0.0, 8.0, -95.0),
        Vec3::new(120.0, 60.0, 1.0),
        near_black.clone(),
        "Far backdrop",
    );
    ctx.slab(
        Vec3::new(0.0, -14.0, -50.0),
        Vec3::new(120.0, 1.0, 130.0),
        near_black,
        "Abyss floor",
    );

    // The walkway: narrow, railed, running toward the light.
    let walk = ctx.matte(Color::srgb(0.05, 0.055, 0.065), 0.8);
    ctx.slab(
        Vec3::new(0.0, -0.15, -40.0),
        Vec3::new(2.4, 0.3, 85.0),
        walk.clone(),
        "Walkway",
    );
    for side in [-1.0_f32, 1.0] {
        ctx.slab(
            Vec3::new(side * 1.15, 0.55, -40.0),
            Vec3::new(0.07, 0.07, 85.0),
            walk.clone(),
            "Rail",
        );
        let mut z = -6.0_f32;
        while z > -80.0 {
            ctx.slab(
                Vec3::new(side * 1.15, 0.25, z),
                Vec3::new(0.06, 0.6, 0.06),
                walk.clone(),
                "Rail post",
            );
            z -= 4.0;
        }
    }

    // Gantry struts: great diagonals crossing between camera and the light —
    // they exist only as silhouettes.
    let strut_specs: [(Vec3, Vec3, f32); 6] = [
        (Vec3::new(-14.0, 4.0, -34.0), Vec3::new(1.4, 34.0, 1.4), 0.5),
        (Vec3::new(10.0, 6.0, -46.0), Vec3::new(1.8, 44.0, 1.8), -0.6),
        (
            Vec3::new(-4.0, 10.0, -58.0),
            Vec3::new(1.2, 50.0, 1.2),
            0.35,
        ),
        (
            Vec3::new(18.0, 2.0, -60.0),
            Vec3::new(2.2, 40.0, 2.2),
            -0.25,
        ),
        (
            Vec3::new(-20.0, 0.0, -70.0),
            Vec3::new(2.6, 52.0, 2.6),
            0.15,
        ),
        (
            Vec3::new(4.0, 14.0, -72.0),
            Vec3::new(1.0, 46.0, 1.0),
            -0.45,
        ),
    ];
    for (pos, size, tilt) in strut_specs {
        ctx.slab_at(
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_z(tilt)),
            size,
            strut_mat.clone(),
            "Strut",
        );
    }
    // Horizontal crossmembers.
    for (y, z, tilt) in [(9.0_f32, -50.0_f32, 0.08_f32), (16.0, -64.0, -0.05)] {
        ctx.slab_at(
            Transform::from_translation(Vec3::new(0.0, y, z))
                .with_rotation(Quat::from_rotation_z(tilt)),
            Vec3::new(70.0, 1.6, 1.6),
            strut_mat.clone(),
            "Crossmember",
        );
    }

    // The one light: far, pale, huge range — it lights the backdrop and the
    // fog, and the struts read as absence against it.
    ctx.commands.spawn((
        SceneSpawned,
        PointLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            intensity: 900_000_000.0,
            range: 160.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, -84.0),
        Name::new("Distant light"),
    ));

    ctx.ambient(Color::srgb(0.3, 0.4, 0.6), 1.2);
    ctx.signal_kit(Vec3::new(0.0, 0.0, -10.0), 0.0);
    ctx.camera(
        Transform::from_xyz(0.0, 1.5, 2.0).looking_at(Vec3::new(0.5, 4.0, -60.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.006, 0.008, 0.014), 12.0, 95.0)),
    );
}
