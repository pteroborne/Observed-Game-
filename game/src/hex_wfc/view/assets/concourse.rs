//! Six cached local finishes for the Switching Concourse.
use super::{HexWfcVisualAssets, surface_texture};
use bevy::prelude::*;
#[derive(Clone)]
pub(super) struct ConcourseMaterials(pub(super) [Handle<StandardMaterial>; 7]);
impl ConcourseMaterials {
    pub(super) fn load(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        Self(std::array::from_fn(|i| {
            let color = if i == 6 {
                observed_style::concourse::unpowered_panel_color()
            } else {
                observed_style::concourse::colors()[i]
            };
            materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(surface_texture(
                    images,
                    observed_style::concourse::albedo(i),
                    true,
                )),
                emissive: if observed_style::concourse::emission(i) > 0.0 {
                    LinearRgba::from(color) * observed_style::concourse::emission(i)
                } else {
                    LinearRgba::BLACK
                },
                perceptual_roughness: observed_style::concourse::roughness(i),
                metallic: if i == 5 { 0.5 } else { 0.0 },
                ..default()
            })
        }))
    }
    #[cfg(test)]
    pub(super) fn for_test(dummy: &Handle<StandardMaterial>) -> Self {
        Self(std::array::from_fn(|_| dummy.clone()))
    }
}
impl HexWfcVisualAssets {
    #[cfg(test)]
    pub(in crate::hex_wfc::view) fn for_concourse_test(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let mut assets = Self::for_test(materials);
        assets.concourse = ConcourseMaterials::load(materials, images);
        assets
    }
    pub(in crate::hex_wfc::view) fn concourse_material(
        &self,
        index: usize,
    ) -> Handle<StandardMaterial> {
        self.concourse.0[index].clone()
    }
}
