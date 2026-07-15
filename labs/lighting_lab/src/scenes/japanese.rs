//! Scene 1 — Shadow Screen. One sparse, dominant screen interrupts an otherwise
//! calm room shell. A low warm source and six floor blades make the occlusion
//! grammar readable without turning every surface into visual noise.

use super::{SceneCtx, SceneSpawned};
use bevy::prelude::*;

pub(crate) const CORRIDOR_LENGTH: f32 = 30.0;
pub(crate) const SCREEN_LENGTH: f32 = 9.0;
pub(crate) const FLOOR_BLADE_COUNT: usize = 6;

pub fn spawn(ctx: &mut SceneCtx) {
    let (w, h, len) = (6.8_f32, 3.8_f32, CORRIDOR_LENGTH);
    let hw = w * 0.5;

    let calm_wall = ctx.matte(Color::srgb(0.095, 0.075, 0.065), 0.92);
    let floor_mat = ctx.matte(Color::srgb(0.14, 0.105, 0.08), 0.78);
    let ceiling_mat = ctx.matte(Color::srgb(0.055, 0.045, 0.042), 0.96);
    let screen_mat = ctx.matte(Color::srgb(0.035, 0.03, 0.028), 0.96);

    ctx.slab(
        Vec3::new(0.0, -0.1, -len * 0.5),
        Vec3::new(w + 2.0, 0.2, len + 2.0),
        floor_mat,
        "Shadow Screen floor",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.1, -len * 0.5),
        Vec3::new(w + 2.0, 0.2, len + 2.0),
        ceiling_mat,
        "Calm ceiling",
    );
    // The opposite wall is deliberately uninterrupted: the screen is the only
    // high-frequency architectural read in the room.
    ctx.slab(
        Vec3::new(-hw - 0.1, h * 0.5, -len * 0.5),
        Vec3::new(0.2, h, len),
        calm_wall.clone(),
        "Calm wall",
    );

    let screen_center_z = -13.5_f32;
    let screen_near = screen_center_z + SCREEN_LENGTH * 0.5;
    let screen_far = screen_center_z - SCREEN_LENGTH * 0.5;
    let near_len = -screen_near;
    let far_len = len + screen_far;
    ctx.slab(
        Vec3::new(hw + 0.1, h * 0.5, screen_near * 0.5),
        Vec3::new(0.2, h, near_len),
        calm_wall.clone(),
        "Calm wall before screen",
    );
    ctx.slab(
        Vec3::new(hw + 0.1, h * 0.5, screen_far - far_len * 0.5),
        Vec3::new(0.2, h, far_len),
        calm_wall.clone(),
        "Calm wall after screen",
    );

    // One contiguous screen occupies 30% of the eligible wall (inside the
    // catalogue's 20–35% target), with no secondary grid or paper wall.
    let mut z = screen_near + 0.25;
    while z > screen_far - 0.25 {
        ctx.slab(
            Vec3::new(hw, h * 0.5, z),
            Vec3::new(0.11, h, 0.18),
            screen_mat.clone(),
            "Shadow screen slat",
        );
        z -= 0.62;
    }
    for y in [0.12_f32, h - 0.12] {
        ctx.slab(
            Vec3::new(hw, y, screen_center_z),
            Vec3::new(0.14, 0.12, SCREEN_LENGTH),
            screen_mat.clone(),
            "Shadow screen rail",
        );
    }
    for z in [screen_near, screen_far] {
        ctx.slab(
            Vec3::new(hw, h * 0.5, z),
            Vec3::new(0.15, h, 0.15),
            screen_mat.clone(),
            "Shadow screen jamb",
        );
    }

    ctx.slab(
        Vec3::new(0.0, h * 0.5, -len - 0.1),
        Vec3::new(w + 2.0, h, 0.2),
        calm_wall,
        "Calm end wall",
    );

    // Physical light makes the shadows; these restrained floor-only guides
    // guarantee six legible blades even when capture shadow sampling changes.
    let blade = ctx.glow(LinearRgba::rgb(1.0, 0.48, 0.16), 0.75);
    for i in 0..FLOOR_BLADE_COUNT {
        let z = screen_near - 0.8 - i as f32 * 1.25;
        ctx.slab_at(
            Transform::from_xyz(0.2, 0.012, z).with_rotation(Quat::from_rotation_y(-0.08)),
            Vec3::new(w - 0.7, 0.018, 0.16),
            blade.clone(),
            "Floor light blade",
        );
    }

    ctx.commands.spawn((
        SceneSpawned,
        SpotLight {
            color: Color::srgb(1.0, 0.62, 0.32),
            intensity: 42_000_000.0,
            range: 34.0,
            radius: 0.05,
            // The six authored blades carry the floor read. Disabling sampled
            // shadows keeps the walls and ceiling calm across GPU backends.
            shadows_enabled: false,
            inner_angle: 0.30,
            outer_angle: 0.52,
            ..default()
        },
        Transform::from_xyz(hw + 6.0, 0.9, screen_center_z + 1.5)
            .looking_at(Vec3::new(0.0, 0.02, screen_center_z - 1.0), Vec3::Y),
        Name::new("Shadow screen low source"),
    ));

    ctx.ambient(Color::srgb(0.42, 0.50, 0.68), 220.0);
    ctx.signal_kit(Vec3::new(-0.6, 0.0, -23.0), 0.25);
    ctx.camera(
        Transform::from_xyz(-0.65, 1.55, -1.0).looking_at(Vec3::new(0.2, 0.75, -16.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.012, 0.01, 0.012), 18.0, 46.0)),
    );
}
