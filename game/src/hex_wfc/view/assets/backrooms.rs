//! Authored neutral Backrooms PBR maps; the shared style owns every finish.
use super::{
    RegisterMaterials,
    textures::{TextureKind, surface_texture_sized},
};
use bevy::prelude::*;
use observed_style::{ArchitectureSurfaceRole, backrooms as style};

pub(super) fn load(
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> RegisterMaterials {
    let sets = [
        [
            include_bytes!("../../../../../assets/textures/backrooms/carpet_albedo.png").as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/carpet_normal.png").as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/carpet_orm.png").as_slice(),
        ],
        [
            include_bytes!("../../../../../assets/textures/backrooms/wallpaper_albedo.png")
                .as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/wallpaper_normal.png")
                .as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/wallpaper_orm.png").as_slice(),
        ],
        [
            include_bytes!("../../../../../assets/textures/backrooms/acoustic_albedo.png")
                .as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/acoustic_normal.png")
                .as_slice(),
            include_bytes!("../../../../../assets/textures/backrooms/acoustic_orm.png").as_slice(),
        ],
    ];
    let mut surface = |index: usize, role| {
        let look = style::surface(role);
        let textures = sets[index]
            .into_iter()
            .zip([TextureKind::Albedo, TextureKind::Normal, TextureKind::Data])
            .map(|(bytes, kind)| {
                let slot = match kind {
                    TextureKind::Albedo => observed_assets::BACKROOMS_MATERIALS[index].albedo,
                    TextureKind::Normal => observed_assets::BACKROOMS_MATERIALS[index].normal,
                    TextureKind::Data => observed_assets::BACKROOMS_MATERIALS[index].orm,
                };
                let decoded = std::fs::read(observed_assets::assets_root().join(slot.path))
                    .ok()
                    .and_then(|data| ::image::load_from_memory(&data).ok())
                    .filter(|image| {
                        image.width() == image.height() && image.width().is_power_of_two()
                    })
                    .unwrap_or_else(|| ::image::load_from_memory(bytes).expect("committed PBR map"))
                    .into_rgba8();
                surface_texture_sized(
                    images,
                    decoded.as_raw().clone(),
                    decoded.width() as usize,
                    kind,
                )
            })
            .collect::<Vec<_>>();
        materials.add(StandardMaterial {
            base_color: look.base_color,
            emissive: look.emissive,
            base_color_texture: Some(textures[0].clone()),
            normal_map_texture: Some(textures[1].clone()),
            normal_map_channel: if role == ArchitectureSurfaceRole::Floor {
                bevy::mesh::UvChannel::Uv1
            } else {
                bevy::mesh::UvChannel::Uv0
            },
            metallic_roughness_texture: Some(textures[2].clone()),
            occlusion_texture: Some(textures[2].clone()),
            perceptual_roughness: style::roughness(role),
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(
                4.0 / style::texture_metres(role),
            )),
            ..default()
        })
    };
    let floor = surface(0, ArchitectureSurfaceRole::Floor);
    let wall = surface(1, ArchitectureSurfaceRole::Wall);
    let ceiling = surface(2, ArchitectureSurfaceRole::Ceiling);
    let ceiling_material = materials.get(&ceiling).expect("ceiling material").clone();
    let ceiling_phase = Some(std::array::from_fn(|phase| {
        let mut material = ceiling_material.clone();
        material.uv_transform.translation = Vec2::new(phase as f32 * 0.2 / 6.0, 0.0);
        materials.add(material)
    }));
    let look = style::trim();
    let trim = materials.add(StandardMaterial {
        base_color: look.base_color,
        perceptual_roughness: style::TRIM_ROUGHNESS,
        ..default()
    });
    let look = style::surface(ArchitectureSurfaceRole::PracticalFixture);
    let fixture = materials.add(StandardMaterial {
        base_color: look.base_color,
        emissive: look.emissive,
        perceptual_roughness: style::roughness(ArchitectureSurfaceRole::PracticalFixture),
        ..default()
    });
    RegisterMaterials {
        ceiling_phase,
        boundary: wall.clone(),
        floor,
        wall,
        ceiling,
        trim,
        fixture,
    }
}
