//! Cached local wonder finishes; ordinary Zen keeps its existing register materials.
use super::surface_texture;
use bevy::prelude::*;
#[derive(Clone)]
pub(super) struct RainMaterials(pub(super) [Handle<StandardMaterial>; 7]);
impl RainMaterials {
    pub(super) fn load(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        Self(std::array::from_fn(|i| {
            materials.add(StandardMaterial {
                base_color: observed_style::rain_court::colors()[i],
                base_color_texture: Some(surface_texture(
                    images,
                    observed_style::rain_court::albedo(i),
                    true,
                )),
                perceptual_roughness: if i == 3 { 0.24 } else { 0.82 },
                metallic: if i == 3 { 0.12 } else { 0.0 },
                ..default()
            })
        }))
    }
    #[cfg(test)]
    pub(super) fn for_test(dummy: &Handle<StandardMaterial>) -> Self {
        Self(std::array::from_fn(|_| dummy.clone()))
    }
}
