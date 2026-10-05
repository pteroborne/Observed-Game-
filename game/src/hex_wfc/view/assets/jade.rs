//! Six cached local finishes for the Jade Nave.
use super::{HexWfcVisualAssets, surface_texture};
use bevy::prelude::*;
#[derive(Clone)]
pub(super) struct JadeMaterials(pub(super) [Handle<StandardMaterial>; 7]);
impl JadeMaterials {
    pub(super) fn load(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        Self(std::array::from_fn(|i| {
            let color = if i == 6 {
                observed_style::jade::unpowered_panel_color()
            } else {
                observed_style::jade::colors()[i]
            };
            materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(surface_texture(
                    images,
                    observed_style::jade::albedo(i),
                    true,
                )),
                emissive: if observed_style::jade::emission(i) > 0.0 {
                    LinearRgba::from(color) * observed_style::jade::emission(i)
                } else {
                    LinearRgba::BLACK
                },
                perceptual_roughness: observed_style::jade::roughness(i),
                metallic: if i == 3 { 0.6 } else { 0.0 },
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
    pub(in crate::hex_wfc::view) fn for_jade_test(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let mut assets = Self::for_test(materials);
        assets.jade = JadeMaterials::load(materials, images);
        assets
    }
    pub(in crate::hex_wfc::view) fn jade_material(&self, index: usize) -> Handle<StandardMaterial> {
        self.jade.0[index].clone()
    }
}
