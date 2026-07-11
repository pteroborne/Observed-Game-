//! Scene 2 — monolith. Mass and weight: raw concrete volumes, no fill light,
//! one hard cool shaft through a single high aperture landing as a sharp quad
//! on the floor. Depth comes from shadow gradation, not detail.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    let concrete = ctx.matte(Color::srgb(0.30, 0.30, 0.29), 0.95);
    let concrete_dark = ctx.matte(Color::srgb(0.22, 0.22, 0.215), 0.95);

    // Hall: 20 × 9 × 20.
    let (hw, h) = (10.0_f32, 9.0_f32);
    ctx.slab(
        Vec3::new(0.0, -0.15, 0.0),
        Vec3::new(2.0 * hw + 6.0, 0.3, 2.0 * hw + 6.0),
        concrete.clone(),
        "Floor",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.15, 0.0),
        Vec3::new(2.0 * hw + 6.0, 0.3, 2.0 * hw + 6.0),
        concrete_dark.clone(),
        "Ceiling",
    );
    // Back and right walls solid; the left wall carries the aperture.
    ctx.slab(
        Vec3::new(0.0, h * 0.5, -hw - 0.4),
        Vec3::new(2.0 * hw + 6.0, h, 0.8),
        concrete.clone(),
        "Back wall",
    );
    ctx.slab(
        Vec3::new(hw + 0.4, h * 0.5, 0.0),
        Vec3::new(0.8, h, 2.0 * hw + 6.0),
        concrete.clone(),
        "Right wall",
    );
    // Left wall in four pieces framing a 3.2 × 2.2 aperture high at z = -2.
    let ap = (3.2_f32, 2.2_f32);
    let (ap_y, ap_z) = (5.6_f32, -2.0_f32);
    ctx.slab(
        Vec3::new(
            -hw - 0.4,
            h * 0.5,
            -hw * 0.5 + (ap_z - ap.0 * 0.5 - (-hw)) * 0.5 - hw * 0.25,
        ),
        Vec3::new(0.8, h, (ap_z - ap.0 * 0.5) - (-hw) + 3.0),
        concrete.clone(),
        "Left wall A",
    );
    ctx.slab(
        Vec3::new(-hw - 0.4, h * 0.5, (ap_z + ap.0 * 0.5 + hw + 3.0) * 0.5),
        Vec3::new(0.8, h, hw + 3.0 - (ap_z + ap.0 * 0.5)),
        concrete.clone(),
        "Left wall B",
    );
    ctx.slab(
        Vec3::new(-hw - 0.4, (ap_y - ap.1 * 0.5) * 0.5, ap_z),
        Vec3::new(0.8, ap_y - ap.1 * 0.5, ap.0),
        concrete.clone(),
        "Left wall below aperture",
    );
    ctx.slab(
        Vec3::new(-hw - 0.4, (ap_y + ap.1 * 0.5 + h) * 0.5, ap_z),
        Vec3::new(0.8, h - (ap_y + ap.1 * 0.5), ap.0),
        concrete.clone(),
        "Left wall above aperture",
    );

    // Interior masses: three monoliths and a deep-reveal passage slab.
    ctx.slab(
        Vec3::new(2.5, 3.0, -4.5),
        Vec3::new(2.2, 6.0, 5.0),
        concrete_dark.clone(),
        "Monolith A",
    );
    ctx.slab(
        Vec3::new(-3.0, 2.25, 5.0),
        Vec3::new(4.5, 4.5, 2.4),
        concrete_dark.clone(),
        "Monolith B",
    );
    ctx.slab(
        Vec3::new(6.5, 1.4, 4.0),
        Vec3::new(3.2, 2.8, 3.2),
        concrete_dark.clone(),
        "Monolith C",
    );
    // A beam overhead to catch the shaft's upper edge.
    ctx.slab(
        Vec3::new(0.0, 7.4, 1.5),
        Vec3::new(2.0 * hw, 1.2, 1.6),
        concrete_dark,
        "Beam",
    );

    // The one light: a hard, tight, cool shaft through the aperture.
    ctx.commands.spawn((
        SceneSpawned,
        SpotLight {
            color: Color::srgb(0.92, 0.95, 1.0),
            intensity: 300_000_000.0,
            range: 60.0,
            radius: 0.02,
            shadows_enabled: true,
            inner_angle: 0.16,
            outer_angle: 0.22,
            ..default()
        },
        Transform::from_xyz(-hw - 9.0, 10.5, ap_z).looking_at(Vec3::new(3.0, 0.0, 2.5), Vec3::Y),
        Name::new("Hard shaft"),
    ));

    ctx.ambient(Color::srgb(0.5, 0.55, 0.65), 3.0);
    ctx.signal_kit(Vec3::new(1.5, 0.0, 8.5), -0.4);
    ctx.camera(
        Transform::from_xyz(7.5, 1.3, 11.5).looking_at(Vec3::new(-2.0, 2.2, -2.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.008, 0.008, 0.01), 20.0, 70.0)),
    );
}
