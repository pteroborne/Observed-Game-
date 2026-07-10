//! Helpers for 2.5D sprite placeholders in the 3D match scene.
//!
//! `bevy_sprite3d` builds a mesh from loaded image dimensions, so callers must check
//! image readiness before spawning. Missing or not-yet-loaded images keep the
//! procedural fallback path alive.

use bevy::prelude::*;
use bevy_sprite3d::prelude::Sprite3d;
use super::actor_metadata::SpriteMetadata;

use crate::view::components::{BillboardSprite, GameCam};

pub(crate) const ACTOR_PIXELS_PER_METRE: f32 = 64.0;
pub(crate) const DEVICE_PIXELS_PER_METRE: f32 = 80.0;

pub(crate) fn ready_image(
    images: &Assets<Image>,
    image: &Option<Handle<Image>>,
) -> Option<Handle<Image>> {
    image
        .as_ref()
        .filter(|handle| images.get(*handle).is_some())
        .cloned()
}

pub(crate) fn sprite3d_components_with_pivot(
    image: Handle<Image>,
    treatment: &observed_style::Treatment,
    pixels_per_metre: f32,
    pivot: Vec2,
) -> (Sprite, Sprite3d, BillboardSprite) {
    (
        Sprite { image, ..default() },
        Sprite3d {
            pixels_per_metre,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            emissive: treatment.emissive,
            pivot: Some(pivot),
            double_sided: true,
            ..default()
        },
        BillboardSprite,
    )
}

pub(crate) fn sprite3d_components(
    image: Handle<Image>,
    treatment: &observed_style::Treatment,
    pixels_per_metre: f32,
) -> (Sprite, Sprite3d, BillboardSprite) {
    sprite3d_components_with_pivot(image, treatment, pixels_per_metre, Vec2::new(0.5, 0.0))
}

pub(crate) fn yaw_toward_camera(sprite: Vec3, camera: Vec3) -> Option<f32> {
    let to_camera = Vec2::new(camera.x - sprite.x, camera.z - sprite.z);
    (to_camera.length_squared() > 0.0001).then(|| to_camera.x.atan2(to_camera.y))
}

pub(crate) fn face_billboard_sprites(
    camera: Query<&GlobalTransform, (With<GameCam>, Without<BillboardSprite>)>,
    mut sprites: Query<&mut Transform, With<BillboardSprite>>,
) {
    let Some(camera_transform) = camera.iter().next() else {
        return;
    };
    let camera_pos = camera_transform.translation();
    for mut transform in &mut sprites {
        if let Some(yaw) = yaw_toward_camera(transform.translation, camera_pos) {
            transform.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

pub(crate) fn actor_frame(
    meta: &SpriteMetadata,
    clip_name: &str,
    rel_angle: f32,
    animation_step: usize,
) -> usize {
    let dir_count = meta.directional_count.count();

    // 1. Determine direction index
    let dir_index = if dir_count <= 1 {
        0
    } else {
        // Normalize relative angle to [-PI, PI]
        let mut angle = rel_angle;
        while angle > std::f32::consts::PI {
            angle -= 2.0 * std::f32::consts::PI;
        }
        while angle < -std::f32::consts::PI {
            angle += 2.0 * std::f32::consts::PI;
        }

        let sector = (angle + std::f32::consts::PI / dir_count as f32) / (2.0 * std::f32::consts::PI);
        let mut idx = (sector * dir_count as f32).floor() as i32;
        if idx < 0 {
            idx += dir_count as i32;
        }
        (idx as usize) % dir_count
    };

    // 2. Map clip name to steps
    let steps = meta.clips.get(clip_name).or_else(|| meta.clips.get("idle"));
    let logical_step = if let Some(steps) = steps {
        if !steps.is_empty() {
            steps[animation_step % steps.len()]
        } else {
            0
        }
    } else {
        0
    };

    // Final flat atlas index
    logical_step * dir_count + dir_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use super::super::actor_metadata::DirectionalCount;

    #[test]
    fn yaw_faces_camera_on_xz_plane() {
        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(1.0, 3.0, 0.0)).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.0001);

        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(0.0, 3.0, -1.0)).unwrap();
        assert!((yaw - std::f32::consts::PI).abs() < 0.0001);

        assert!(yaw_toward_camera(Vec3::ZERO, Vec3::ZERO).is_none());
    }

    #[test]
    fn actor_frame_quantization_and_clips() {
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), vec![0]);
        clips.insert("walk".to_string(), vec![0, 1, 2]);

        let meta = SpriteMetadata {
            name: "test_guard".to_string(),
            image_path: "test_guard.png".to_string(),
            frames: vec![], // not used by actor_frame directly
            pivot: (0.5, 0.0),
            pixels_per_metre: 64.0,
            directional_count: DirectionalCount::Way8,
            clips,
            default_material_role: "Rival".to_string(),
        };

        // 8-way directional sectors:
        // Angle 0 is facing camera -> index 0
        assert_eq!(actor_frame(&meta, "idle", 0.0, 0), 0);
        assert_eq!(actor_frame(&meta, "idle", 0.2, 0), 0);
        assert_eq!(actor_frame(&meta, "idle", -0.2, 0), 0);

        // Angle PI is facing away -> index 4
        assert_eq!(actor_frame(&meta, "idle", std::f32::consts::PI, 0), 4);
        assert_eq!(actor_frame(&meta, "idle", -std::f32::consts::PI, 0), 4);

        // Walk clip steps: walk maps to indices [0, 1, 2]
        // Step 0 -> logical_step 0 * 8 + dir
        // Step 1 -> logical_step 1 * 8 + dir
        // Step 2 -> logical_step 2 * 8 + dir
        // Step 3 (wraps) -> logical_step 0 * 8 + dir
        assert_eq!(actor_frame(&meta, "walk", 0.0, 0), 0);
        assert_eq!(actor_frame(&meta, "walk", 0.0, 1), 8);
        assert_eq!(actor_frame(&meta, "walk", 0.0, 2), 16);
        assert_eq!(actor_frame(&meta, "walk", 0.0, 3), 0);
    }
}
