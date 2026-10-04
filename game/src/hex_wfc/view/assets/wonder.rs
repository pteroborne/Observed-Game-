//! Surface materials shared by the two authored wonder compositions.

use bevy::prelude::*;
use observed_style::{self as style, ArchitectureSurfaceRole};

use super::{MeshGroupKey, surface_texture};

#[derive(Clone)]
pub(super) struct WonderMaterials([Handle<StandardMaterial>; 3]);

impl WonderMaterials {
    pub(super) fn from_register(register: &super::RegisterMaterials) -> Self {
        Self([
            register.floor.clone(),
            register.wall.clone(),
            register.ceiling.clone(),
        ])
    }

    pub(super) fn load_cistern(
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let ceramic = surface_texture(images, style::cistern::ceramic_albedo(), true);
        Self(
            [
                ArchitectureSurfaceRole::Floor,
                ArchitectureSurfaceRole::Wall,
                ArchitectureSurfaceRole::Ceiling,
            ]
            .map(|role| {
                let look = style::cistern::surface(role);
                materials.add(StandardMaterial {
                    base_color: look.base_color,
                    base_color_texture: Some(ceramic.clone()),
                    perceptual_roughness: style::cistern::CERAMIC_ROUGHNESS,
                    ..default()
                })
            }),
        )
    }

    pub(super) fn for_group(&self, group: MeshGroupKey) -> Handle<StandardMaterial> {
        let index = match group {
            MeshGroupKey::Floor => 0,
            MeshGroupKey::Ceiling => 2,
            _ => 1,
        };
        self.0[index].clone()
    }

    #[cfg(test)]
    pub(super) fn for_test(material: &Handle<StandardMaterial>) -> Self {
        Self(std::array::from_fn(|_| material.clone()))
    }
}

/// Cached treatments for inert conveyor markings and sample charge cores.
pub(super) fn details(materials: &mut Assets<StandardMaterial>) -> [Handle<StandardMaterial>; 4] {
    use style::chargeworks as look;
    [
        look::belt_color(),
        look::fixture_color(),
        look::warning_color(),
        look::charge_color(),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, color)| {
        materials.add(StandardMaterial {
            base_color: color,
            emissive: if i == 3 {
                LinearRgba::from(color) * 3.0
            } else {
                LinearRgba::BLACK
            },
            perceptual_roughness: 0.5,
            // The gas is translucent; the authored cage supplies collision and sight obstruction.
            alpha_mode: if i == 3 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            ..default()
        })
    })
    .collect::<Vec<_>>()
    .try_into()
    .expect("four factory detail materials")
}
