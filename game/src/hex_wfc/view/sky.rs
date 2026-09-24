//! The sky outside the facility: a dome darkest straight down, and a cloud sea in two
//! layers below the lattice. Seen only where the building opens onto it.
//!
//! The colours and the cloud texture are `observed_style::open_air`'s, the same ones
//! the Architect's cutaway and `labs/vista_lab` draw, so all three views of the same
//! air agree. Two layers at different depths, drifting at different speeds, are the
//! parallax that makes the drop read as a drop.
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine2;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_style::open_air::{CLOUD_TEXTURE_SIZE, SkyRole, cloud_rgba, sky, sky_along};

use crate::GameState;
use crate::view::components::GameCam;

/// The dome's radius: inside the play camera's far plane, outside everything else.
const DOME_RADIUS: f32 = 900.0;

/// The cloud layers below the lattice: depth, opacity, tiling, drift in tiles/second.
const LAYERS: [(f32, f32, f32, Vec2); 2] = [
    (-24.0, 0.62, 16.0, Vec2::new(0.0022, 0.0009)),
    (-60.0, 0.8, 9.0, Vec2::new(-0.0011, 0.0016)),
];

#[derive(Component)]
pub(in crate::hex_wfc) struct SkyDome;

#[derive(Component)]
pub(in crate::hex_wfc) struct CloudLayer {
    material: Handle<StandardMaterial>,
    tiles: f32,
    drift: Vec2,
}

/// Spawn the dome and the cloud sea, centred on the facility.
pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    center: Vec3,
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
        SkyDome,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::from_translation(center).with_scale(Vec3::splat(DOME_RADIUS)),
        NotShadowCaster,
        NotShadowReceiver,
        DespawnOnExit(GameState::HexWfc),
        Name::new("Sky dome"),
    ));

    let texture = images.add(cloud_image());
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
            DespawnOnExit(GameState::HexWfc),
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
pub(in crate::hex_wfc) fn follow_camera(
    camera: Query<&Transform, With<GameCam>>,
    mut dome: Query<&mut Transform, (With<SkyDome>, Without<GameCam>)>,
) {
    let (Ok(eye), Ok(mut dome)) = (camera.single(), dome.single_mut()) else {
        return;
    };
    dome.translation = eye.translation;
}

pub(in crate::hex_wfc) fn drift_clouds(
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
