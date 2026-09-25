//! The sky outside the facility: a dome darkest straight down, a large moon and a
//! field of stars above, and a cloud sea in two layers below the lattice. Seen only
//! where the building opens onto it.
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
use observed_style::open_air::{
    CLOUD_TEXTURE_SIZE, MOON_ANGULAR_DIAMETER, MOON_TEXTURE_SIZE, SkyRole, cloud_rgba, halo_rgba,
    moon_disc, moon_halo, moon_rgba, sky, sky_along, stars, toward_moon,
};

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
    let dome = commands
        .spawn((
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
        ))
        .id();
    // Children of the dome, in its unit-sphere frame: at infinity, and with the eye.
    for body in heavens(meshes, materials, images) {
        commands.spawn((body, ChildOf(dome)));
    }

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

/// What hangs in the sky: the moon, its halo and the stars, each placed in the dome's
/// unit-sphere frame just inside it, so the dome's scale puts them at the horizon's
/// distance and the dome's position keeps them with the eye.
fn heavens(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Vec<impl Bundle> {
    let toward = Vec3::from_array(toward_moon());
    let facing = |at: Vec3| Transform::from_translation(at).looking_at(Vec3::ZERO, Vec3::Y);
    // Far enough inside the unit dome that even the halo's corners never reach it:
    // a quad that pokes through the dome is clipped along the dome's facets.
    let (moon_at, halo_at) = (0.72, 0.7);
    let moon_width = moon_at * MOON_ANGULAR_DIAMETER;
    let halo_width = halo_at * MOON_ANGULAR_DIAMETER * 2.4;
    let sky_body =
        |texture: Option<Handle<Image>>, color: LinearRgba, alpha: AlphaMode| StandardMaterial {
            base_color: Color::LinearRgba(color),
            base_color_texture: texture,
            alpha_mode: alpha,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        };
    vec![
        (
            Mesh3d(meshes.add(Rectangle::new(halo_width, halo_width))),
            MeshMaterial3d(materials.add(sky_body(
                Some(images.add(square_image(halo_rgba(), MOON_TEXTURE_SIZE))),
                moon_halo(),
                AlphaMode::Add,
            ))),
            facing(toward * halo_at),
            NotShadowCaster,
            Name::new("Moon halo"),
        ),
        (
            Mesh3d(meshes.add(Rectangle::new(moon_width, moon_width))),
            MeshMaterial3d(materials.add(sky_body(
                Some(images.add(square_image(moon_rgba(), MOON_TEXTURE_SIZE))),
                moon_disc(),
                AlphaMode::Blend,
            ))),
            facing(toward * moon_at),
            NotShadowCaster,
            Name::new("Moon"),
        ),
        (
            Mesh3d(meshes.add(star_field())),
            MeshMaterial3d(materials.add(sky_body(None, LinearRgba::WHITE, AlphaMode::Add))),
            Transform::IDENTITY,
            NotShadowCaster,
            Name::new("Stars"),
        ),
    ]
}

/// Every star as a tiny quad facing the eye, brightness in its vertex colour.
fn star_field() -> Mesh {
    let field = stars(1_800);
    let mut positions = Vec::with_capacity(field.len() * 4);
    let mut colors = Vec::with_capacity(field.len() * 4);
    let mut indices = Vec::with_capacity(field.len() * 6);
    for star in field {
        let direction = Vec3::from_array(star.direction);
        let right = Vec3::Y.cross(direction).normalize_or(Vec3::X);
        let up = direction.cross(right);
        let at = direction * 0.97;
        let half = 0.97 * star.size * 0.5;
        let base = u32::try_from(positions.len()).expect("star count fits");
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((at + (right * x + up * y) * half).to_array());
            colors.push([
                star.brightness,
                star.brightness,
                star.brightness * 1.08,
                1.0,
            ]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

fn square_image(data: Vec<u8>, size: u32) -> Image {
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
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
