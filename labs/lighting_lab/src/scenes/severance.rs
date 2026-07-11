//! Scene 4 — lumon. The pristine institution: a long white corridor in perfect
//! one-point perspective, continuous cool strip light, green carpet, one green
//! door far away. Distinct from backrooms by symmetry, cleanliness, and cold —
//! the dread is that someone maintains this.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    let (w, h, len) = (2.4_f32, 2.6_f32, 34.0_f32);
    let hw = w * 0.5;

    let white = ctx.matte(Color::srgb(0.86, 0.87, 0.88), 0.55);
    let carpet = ctx.matte(Color::srgb(0.16, 0.30, 0.22), 0.95);

    ctx.slab(
        Vec3::new(0.0, -0.1, -len * 0.5),
        Vec3::new(w, 0.2, len),
        carpet,
        "Carpet",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.1, -len * 0.5),
        Vec3::new(w + 1.0, 0.2, len),
        white.clone(),
        "Ceiling",
    );
    for sx in [-1.0_f32, 1.0] {
        ctx.slab(
            Vec3::new(sx * (hw + 0.1), h * 0.5, -len * 0.5),
            Vec3::new(0.2, h, len),
            white.clone(),
            "Wall",
        );
        // Waist trim: the institutional wainscot line, perfectly straight.
        let trim_mat = ctx_trim(ctx);
        ctx.slab(
            Vec3::new(sx * (hw + 0.02), 1.0, -len * 0.5),
            Vec3::new(0.04, 0.06, len),
            trim_mat,
            "Wainscot trim",
        );
    }
    ctx.slab(
        Vec3::new(0.0, h * 0.5, -len - 0.1),
        Vec3::new(w + 1.0, h, 0.2),
        white.clone(),
        "End wall",
    );

    // The one accent: a Lumon-green door at the far end, right wall.
    let green = ctx.matte(Color::srgb(0.13, 0.34, 0.24), 0.6);
    ctx.slab(
        Vec3::new(hw + 0.05, 1.05, -len + 4.0),
        Vec3::new(0.12, 2.1, 0.95),
        green,
        "Green door",
    );

    // Continuous ceiling strip: an unbroken emissive line down the center,
    // plus evenly spaced unshadowed fills. Even, cold, shadowless.
    let strip = ctx.glow(LinearRgba::rgb(0.92, 0.98, 1.0), 5.0);
    ctx.slab(
        Vec3::new(0.0, h - 0.02, -len * 0.5),
        Vec3::new(0.35, 0.05, len - 0.6),
        strip,
        "Ceiling strip",
    );
    let mut z = -1.5_f32;
    while z > -len + 1.0 {
        ctx.commands.spawn((
            SceneSpawned,
            PointLight {
                color: Color::srgb(0.92, 0.97, 1.0),
                intensity: 45_000.0,
                range: 6.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, h - 0.3, z),
            Name::new("Strip fill"),
        ));
        z -= 3.0;
    }

    ctx.ambient(Color::srgb(0.9, 0.96, 1.0), 260.0);
    ctx.signal_kit(Vec3::new(0.0, 0.0, -26.0), 0.0);
    // Dead-center one-point perspective: the corridor vanishes into itself.
    ctx.camera(
        Transform::from_xyz(0.0, 1.35, 0.0).looking_at(Vec3::new(0.0, 1.3, -len), Vec3::Y),
        None,
    );
}

fn ctx_trim(ctx: &mut SceneCtx) -> Handle<StandardMaterial> {
    ctx.matte(Color::srgb(0.70, 0.72, 0.73), 0.5)
}
