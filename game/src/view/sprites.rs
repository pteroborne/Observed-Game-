//! Helpers for 2.5D sprite placeholders in the 3D match scene.
//!
//! `bevy_sprite3d` builds a mesh from loaded image dimensions, so callers must check
//! image readiness before spawning. Missing or not-yet-loaded images keep the
//! procedural fallback path alive.

use bevy::prelude::*;
use bevy_sprite3d::prelude::Sprite3d;

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

pub(crate) fn sprite3d_components(
    image: Handle<Image>,
    treatment: &observed_style::Treatment,
    pixels_per_metre: f32,
) -> (Sprite, Sprite3d, BillboardSprite) {
    (
        Sprite { image, ..default() },
        Sprite3d {
            pixels_per_metre,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            emissive: treatment.emissive,
            pivot: Some(Vec2::new(0.5, 0.0)),
            double_sided: true,
            ..default()
        },
        BillboardSprite,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_faces_camera_on_xz_plane() {
        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(1.0, 3.0, 0.0)).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.0001);

        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(0.0, 3.0, -1.0)).unwrap();
        assert!((yaw - std::f32::consts::PI).abs() < 0.0001);

        assert!(yaw_toward_camera(Vec3::ZERO, Vec3::ZERO).is_none());
    }
}
