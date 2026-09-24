//! Open air under the deck. Air is roughly a third of every facility, so it is a
//! large share of the frame and has to say *drop*, not *nothing*.
//!
//! Orthographic projection has no parallax, so depth comes from atmosphere
//! instead, in four layers from the back:
//!
//! 1. a camera-fixed backdrop, deepest straight down the middle of the view and
//!    hazing toward its edges;
//! 2. the active deck's shadow far below, soft-edged and offset along the key
//!    light, so the deck visibly hangs over something;
//! 3. a thin world-fixed cloud layer over that shadow, which pans with the deck
//!    and so reads as belonging to the same space rather than to the screen;
//! 4. a sawn underside on every built cell of the active deck, so its edges are
//!    cliffs with thickness rather than stickers.
//!
//! Lower decks shown for context are hazed by storey (see `observed_style::architect::hazed`).
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_style::architect::{Role, color};

/// The backdrop quad, parented to the board camera and resized to its view.
#[derive(Component)]
pub(crate) struct SkyBackdrop;

/// How far below the active deck the cloud layer and the cast shadow lie.
pub(crate) const CLOUD_DEPTH: f32 = 34.0;
/// Where the backdrop sits in front of the far plane, in camera space.
pub(crate) const BACKDROP_DISTANCE: f32 = 980.0;

/// The key light's direction of travel, matching `view::setup`'s studio key.
fn light_direction() -> Vec3 {
    -Vec3::new(40.0, 90.0, 20.0).normalize()
}

/// Where a point on the deck casts onto the cloud layer below it.
#[must_use]
pub(crate) fn shadow_offset() -> Vec3 {
    let d = light_direction();
    d * ((CLOUD_DEPTH - 0.6) / -d.y)
}

pub(crate) fn spawn(
    commands: &mut Commands,
    camera: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    let backdrop = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    let backdrop = commands
        .spawn((
            SkyBackdrop,
            Mesh3d(meshes.add(backdrop_mesh())),
            MeshMaterial3d(backdrop),
            Transform::from_xyz(0.0, 0.0, -BACKDROP_DISTANCE),
            NotShadowCaster,
            RenderLayers::layer(0),
            Name::new("Sky backdrop"),
        ))
        .id();
    commands.entity(camera).add_child(backdrop);

    let clouds = materials.add(StandardMaterial {
        base_color: color(Role::SkyHaze).lighter(0.04).with_alpha(0.34),
        base_color_texture: Some(images.add(cloud_texture())),
        uv_transform: Affine2::from_scale(Vec2::splat(6.0)),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1600.0, 1600.0))),
        MeshMaterial3d(clouds),
        // Above the shadow, so the shadow reads as lying beneath the haze.
        Transform::from_xyz(0.0, -CLOUD_DEPTH + 1.5, 0.0),
        NotShadowCaster,
        RenderLayers::layer(0),
        Name::new("Cloud layer below the deck"),
    ));
}

/// A unit quad in camera space (x right, y up), coloured as a radial well.
fn backdrop_mesh() -> Mesh {
    const COLUMNS: u32 = 32;
    const ROWS: u32 = 20;
    let deep = color(Role::SkyDeep).to_linear();
    let haze = color(Role::SkyHaze).to_linear();
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for row in 0..=ROWS {
        for column in 0..=COLUMNS {
            #[allow(clippy::cast_precision_loss)]
            let (u, v) = (
                column as f32 / COLUMNS as f32 - 0.5,
                row as f32 / ROWS as f32 - 0.5,
            );
            positions.push([u, v, 0.0]);
            // Slightly below centre: the eye looks down and forward into the drop.
            let r = Vec2::new(u * 1.15, (v + 0.06) * 1.45).length();
            let t = smoothstep(0.12, 0.78, r);
            let mix = |a: f32, b: f32| a + (b - a) * t;
            colors.push([
                mix(deep.red, haze.red),
                mix(deep.green, haze.green),
                mix(deep.blue, haze.blue),
                1.0,
            ]);
        }
    }
    let mut indices = Vec::new();
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let i = row * (COLUMNS + 1) + column;
            let up = i + COLUMNS + 1;
            indices.extend_from_slice(&[i, i + 1, up, i + 1, up + 1, up]);
        }
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The shared open-air cloud texture ([`observed_style::open_air::cloud_rgba`]),
/// sampled so it tiles.
fn cloud_texture() -> Image {
    const SIZE: u32 = observed_style::open_air::CLOUD_TEXTURE_SIZE;
    let data = observed_style::open_air::cloud_rgba();
    let mut image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cast_shadow_lands_on_the_cloud_layer_along_the_key_light() {
        let offset = shadow_offset();
        assert!((offset.y + CLOUD_DEPTH - 0.6).abs() < 1e-4);
        assert!(offset.x < 0.0 && offset.z < 0.0, "{offset}");
    }

    #[test]
    fn the_backdrop_is_deepest_in_the_middle() {
        let mesh = backdrop_mesh();
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("backdrop is vertex coloured");
        };
        let luminance = |c: &[f32; 4]| c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
        let corner = luminance(&colors[0]);
        let middle = luminance(&colors[colors.len() / 2]);
        assert!(middle < corner * 0.5, "middle {middle}, corner {corner}");
    }
}
