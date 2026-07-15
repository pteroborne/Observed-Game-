//! Shared environment-asset helpers used by both the isolated Place renderer and
//! the continuous full-WFC renderer.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::assets::{ContentScene, asset_present};

pub(crate) const SURFACE_UV_REPEAT_PER_METRE: f32 = 0.25;

pub(crate) fn load_repeating_texture(
    asset_server: &AssetServer,
    path: &'static str,
) -> Option<Handle<Image>> {
    asset_present(path).then(|| {
        asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.sampler =
                bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    ..default()
                });
        })
    })
}

pub(crate) fn load_content_scene(
    asset_server: &AssetServer,
    manifest: &observed_content::ContentManifest,
    id: &str,
) -> Option<ContentScene> {
    manifest
        .assets
        .iter()
        .find(|asset| asset.id == id)
        .filter(|asset| asset_present(&asset.path))
        .map(|asset| ContentScene {
            scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(asset.path.clone())),
            scale: asset.scale,
        })
}

fn push_cuboid_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    uv_span: Vec2,
) {
    let start = positions.len() as u32;
    positions.extend_from_slice(&verts);
    normals.extend_from_slice(&[normal; 4]);
    uvs.extend_from_slice(&[
        [0.0, 0.0],
        [uv_span.x, 0.0],
        [uv_span.x, uv_span.y],
        [0.0, uv_span.y],
    ]);
    indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}

/// Build a cuboid with world-unit UVs so imported structural textures tile instead
/// of stretching once across a full cell wall or floor.
pub(crate) fn cuboid_mesh(size: Vec3) -> Mesh {
    let h = size * 0.5;
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let xy = Vec2::new(size.x, size.y) * SURFACE_UV_REPEAT_PER_METRE;
    let zy = Vec2::new(size.z, size.y) * SURFACE_UV_REPEAT_PER_METRE;
    let xz = Vec2::new(size.x, size.z) * SURFACE_UV_REPEAT_PER_METRE;

    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, h.z],
            [h.x, -h.y, h.z],
            [h.x, h.y, h.z],
            [-h.x, h.y, h.z],
        ],
        [0.0, 0.0, 1.0],
        xy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [h.x, -h.y, -h.z],
            [-h.x, -h.y, -h.z],
            [-h.x, h.y, -h.z],
            [h.x, h.y, -h.z],
        ],
        [0.0, 0.0, -1.0],
        xy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [h.x, -h.y, h.z],
            [h.x, -h.y, -h.z],
            [h.x, h.y, -h.z],
            [h.x, h.y, h.z],
        ],
        [1.0, 0.0, 0.0],
        zy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, -h.z],
            [-h.x, -h.y, h.z],
            [-h.x, h.y, h.z],
            [-h.x, h.y, -h.z],
        ],
        [-1.0, 0.0, 0.0],
        zy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, h.y, h.z],
            [h.x, h.y, h.z],
            [h.x, h.y, -h.z],
            [-h.x, h.y, -h.z],
        ],
        [0.0, 1.0, 0.0],
        xz,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, -h.z],
            [h.x, -h.y, -h.z],
            [h.x, -h.y, h.z],
            [-h.x, -h.y, h.z],
        ],
        [0.0, -1.0, 0.0],
        xz,
    );

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
