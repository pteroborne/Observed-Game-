//! The sky from inside it: a dome that is darkest straight down, and a cloud sea in
//! two layers far below every deck.
//!
//! The cutaway had to fake depth with atmosphere because an orthographic camera has
//! no parallax. A first-person eye has parallax for free, so the two cloud layers sit
//! at different depths and drift at different speeds, and the drop reads as a drop
//! because the near layer slides over the far one as you move.
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine2;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_style::open_air::{CLOUD_TEXTURE_SIZE, SkyRole, cloud_rgba, sky, sky_along};

use super::{Lab, VistaCamera, VistaEntity};

/// Radius of the dome that carries the sky gradient. Inside the camera's far plane.
const DOME_RADIUS: f32 = 1_100.0;

#[derive(Component)]
pub(super) struct SkyDome;

/// One drifting cloud layer: its material, its tiling, and how fast it moves.
#[derive(Component)]
pub(super) struct CloudLayer {
    material: Handle<StandardMaterial>,
    tiles: f32,
    drift: Vec2,
}

/// The cloud layers below the lattice: depth, opacity, tiling, drift in tiles/second.
const LAYERS: [(f32, f32, f32, Vec2); 2] = [
    (-22.0, 0.62, 16.0, Vec2::new(0.0022, 0.0009)),
    (-58.0, 0.8, 9.0, Vec2::new(-0.0011, 0.0016)),
];

pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    lab: &Lab,
) {
    let mut dome = Sphere::new(1.0).mesh().uv(64, 32);
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        dome.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        let colors: Vec<[f32; 4]> = positions
            .iter()
            .map(|p| {
                let c = sky_along(p[1]);
                [c.red, c.green, c.blue, 1.0]
            })
            .collect();
        dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    commands.spawn((
        VistaEntity,
        SkyDome,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(DOME_RADIUS)),
        NotShadowCaster,
        NotShadowReceiver,
        Name::new("Sky dome"),
    ));

    let texture = images.add(cloud_image());
    let (center, _) = lab.eye();
    for (depth, opacity, tiles, drift) in LAYERS {
        let material = materials.add(StandardMaterial {
            base_color: sky(SkyRole::Cloud).with_alpha(opacity),
            base_color_texture: Some(texture.clone()),
            uv_transform: Affine2::from_scale(Vec2::splat(tiles)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            VistaEntity,
            CloudLayer {
                material: material.clone(),
                tiles,
                drift,
            },
            Mesh3d(meshes.add(Plane3d::default().mesh().size(4_000.0, 4_000.0))),
            MeshMaterial3d(material),
            Transform::from_xyz(center.x, depth, center.z),
            NotShadowCaster,
            NotShadowReceiver,
            Name::new("Cloud sea"),
        ));
    }
}

fn cloud_image() -> Image {
    let size = CLOUD_TEXTURE_SIZE;
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        cloud_rgba(),
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

/// The dome is centred on the eye, so the horizon is always at eye height.
pub(super) fn follow_camera(
    camera: Query<&Transform, With<VistaCamera>>,
    mut dome: Query<&mut Transform, (With<SkyDome>, Without<VistaCamera>)>,
) {
    let (Ok(eye), Ok(mut dome)) = (camera.single(), dome.single_mut()) else {
        return;
    };
    dome.translation = eye.translation;
}

pub(super) fn drift_clouds(
    time: Res<Time>,
    layers: Query<&CloudLayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for layer in &layers {
        if let Some(mut material) = materials.get_mut(&layer.material) {
            material.uv_transform = Affine2::from_scale_angle_translation(
                Vec2::splat(layer.tiles),
                0.0,
                (layer.drift * t).fract(),
            );
        }
    }
}
