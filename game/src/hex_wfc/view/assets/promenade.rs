//! Six cached local finishes for the Last Promenade.
use super::{HexWfcVisualAssets, surface_texture};
use bevy::prelude::*;
#[derive(Clone)]
pub(super) struct PromenadeMaterials(pub(super) [Handle<StandardMaterial>; 7]);
impl PromenadeMaterials {
    pub(super) fn load(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        Self(std::array::from_fn(|i| {
            let color = if i == 6 {
                observed_style::promenade::unpowered_panel_color()
            } else {
                observed_style::promenade::colors()[i]
            };
            materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(surface_texture(
                    images,
                    observed_style::promenade::albedo(i),
                    true,
                )),
                emissive: if observed_style::promenade::emission(i) > 0.0 {
                    LinearRgba::from(color) * observed_style::promenade::emission(i)
                } else {
                    LinearRgba::BLACK
                },
                perceptual_roughness: observed_style::promenade::roughness(i),
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
    pub(in crate::hex_wfc::view) fn for_promenade_test(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let mut assets = Self::for_test(materials);
        assets.promenade = PromenadeMaterials::load(materials, images);
        assets
    }
    pub(in crate::hex_wfc::view) fn promenade_material(
        &self,
        index: usize,
    ) -> Handle<StandardMaterial> {
        self.promenade.0[index].clone()
    }
}
