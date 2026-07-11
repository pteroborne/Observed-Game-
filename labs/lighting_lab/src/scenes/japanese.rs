//! Scene 1 — shoji. Shadow as material: one low warm spot outside a slatted
//! screen rakes long light blades across a dark corridor floor; the far half of
//! the opposite wall is a paper screen (soft emissive) for the diffuse
//! comparison. This is where slat shadow stability (map size/bias) gets tuned.

use super::{SceneCtx, SceneSpawned};
use bevy::light::VolumetricLight;
use bevy::prelude::*;

pub fn spawn(ctx: &mut SceneCtx) {
    // Corridor: 3.6 wide (x), 3.2 high, 26 long (z, camera looks toward -z).
    let (w, h, len) = (3.6_f32, 3.2_f32, 26.0_f32);
    let hw = w * 0.5;

    let wood_dark = ctx.matte(Color::srgb(0.09, 0.065, 0.05), 0.9);
    let floor_mat = ctx.matte(Color::srgb(0.16, 0.125, 0.095), 0.75);
    let ceiling_mat = ctx.matte(Color::srgb(0.06, 0.045, 0.04), 0.95);

    ctx.slab(
        Vec3::new(0.0, -0.1, -len * 0.5),
        Vec3::new(w + 4.0, 0.2, len + 4.0),
        floor_mat,
        "Floor",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.1, -len * 0.5),
        Vec3::new(w + 4.0, 0.2, len + 4.0),
        ceiling_mat,
        "Ceiling",
    );
    // Left wall: solid near half, paper screen far half.
    ctx.slab(
        Vec3::new(-hw - 0.1, h * 0.5, -len * 0.25),
        Vec3::new(0.2, h, len * 0.5),
        wood_dark.clone(),
        "Left wall near",
    );
    let paper = ctx.glow(LinearRgba::rgb(1.0, 0.82, 0.58), 1.6);
    ctx.slab(
        Vec3::new(-hw - 0.1, h * 0.5, -len * 0.75),
        Vec3::new(0.12, h, len * 0.5),
        paper,
        "Paper screen",
    );
    // Paper battens: a dark grid over the glow so it reads as shoji, not a panel.
    for i in 0..7 {
        let z = -len * 0.5 - 1.0 - i as f32 * (len * 0.5 - 2.0) / 6.0;
        ctx.slab(
            Vec3::new(-hw - 0.02, h * 0.5, z),
            Vec3::new(0.06, h, 0.09),
            wood_dark.clone(),
            "Paper batten",
        );
    }
    for j in 0..4 {
        let y = 0.4 + j as f32 * (h - 0.8) / 3.0;
        ctx.slab(
            Vec3::new(-hw - 0.02, y, -len * 0.75),
            Vec3::new(0.06, 0.09, len * 0.5),
            wood_dark.clone(),
            "Paper batten row",
        );
    }
    // End cap.
    ctx.slab(
        Vec3::new(0.0, h * 0.5, -len - 0.1),
        Vec3::new(w + 4.0, h, 0.2),
        wood_dark.clone(),
        "End wall",
    );

    // Right side: the slat screen — thin verticals with gaps, light beyond.
    let slat_mat = ctx.matte(Color::srgb(0.05, 0.04, 0.035), 0.95);
    let mut z = -2.0_f32;
    while z > -len + 1.0 {
        ctx.slab(
            Vec3::new(hw, h * 0.5, z),
            Vec3::new(0.09, h, 0.22),
            slat_mat.clone(),
            "Slat",
        );
        z -= 0.46;
    }
    // Rails top/bottom so the screen reads as one built thing.
    for y in [0.12_f32, h - 0.12] {
        ctx.slab(
            Vec3::new(hw, y, -len * 0.5),
            Vec3::new(0.12, 0.1, len),
            slat_mat.clone(),
            "Slat rail",
        );
    }

    // The low sun: a warm spot beyond the slats, near floor height, aimed
    // across and slightly down-corridor so the blades stretch long.
    ctx.commands.spawn((
        SceneSpawned,
        SpotLight {
            color: Color::srgb(1.0, 0.62, 0.32),
            intensity: 60_000_000.0,
            range: 45.0,
            radius: 0.05,
            shadows_enabled: true,
            inner_angle: 0.5,
            outer_angle: 0.9,
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(hw + 7.0, 1.1, -9.0).looking_at(Vec3::new(-hw, 0.2, -13.5), Vec3::Y),
        Name::new("Low sun"),
    ));

    ctx.ambient(Color::srgb(0.45, 0.55, 0.8), 14.0);
    ctx.signal_kit(Vec3::new(-0.4, 0.0, -19.0), 0.35);
    ctx.camera(
        Transform::from_xyz(0.35, 1.5, -0.6).looking_at(Vec3::new(-0.3, 0.9, -14.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.012, 0.01, 0.012), 16.0, 44.0)),
    );
}
