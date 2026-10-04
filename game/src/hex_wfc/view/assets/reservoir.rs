//! Ceramic materials for the Cistern's three structural surface groups.

use bevy::prelude::*;
use observed_style::{self as style, ArchitectureSurfaceRole};

use super::{MeshGroupKey, surface_texture};

#[derive(Clone)]
pub(super) struct ReservoirMaterials([Handle<StandardMaterial>; 3]);

impl ReservoirMaterials {
    pub(super) fn load(
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
