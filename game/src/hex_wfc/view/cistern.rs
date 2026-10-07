//! Presentation of the authored reservoir. One reflected camera for the viewed
//! floor, a bounded half-resolution target, and surfaces owned by resident cells.
//! This draws water; observation continues to use the simulation's sight rays.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::reflection_matrix;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::window::PrimaryWindow;
use observed_hex::{HexCoord, hex_origin};

use crate::{GameState, view::components::GameCam};

const WATER_LAYER: usize = 29;

#[derive(Component)]
pub(super) struct ReservoirCell(pub HexCoord);
#[derive(Component)]
struct ReservoirSurface(HexCoord);
#[derive(Component)]
struct ReflectionCamera;

#[derive(Clone, ShaderType)]
struct WaterSettings {
    tint: Vec4,
    haze: Vec4,
    params: Vec4,
}
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct WaterMaterial {
    #[uniform(0)]
    settings: WaterSettings,
    #[texture(1)]
    #[sampler(2)]
    reflection: Handle<Image>,
}
impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/cistern_water.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
#[derive(Resource)]
struct ReservoirAssets {
    image: Handle<Image>,
    material: Handle<WaterMaterial>,
    mesh: Handle<Mesh>,
}

pub(in crate::hex_wfc) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
        .add_systems(OnEnter(GameState::HexWfc), setup)
        .add_systems(
            PostUpdate,
            sync.before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(OnExit(GameState::HexWfc), cleanup);
}
fn linear(color: Color) -> Vec4 {
    let c = LinearRgba::from(color);
    Vec4::new(c.red, c.green, c.blue, c.alpha)
}

/// Limit reflection cost without stretching it relative to the main viewport.
fn reflection_extent(width: u32, height: u32) -> UVec2 {
    let size = UVec2::new(width.max(1), height.max(1)).as_vec2();
    let scale = 0.5_f32.min(960.0 / size.x).min(600.0 / size.y);
    (size * scale).round().max(Vec2::ONE).as_uvec2()
}

fn surface_mesh() -> Mesh {
    let mut positions = vec![[0.0, 0.0, 0.0]];
    positions.extend(observed_hex::CORNERS.map(|(x, z)| [x as f32, 0.0, z as f32]));
    let indices: Vec<u32> = (0..6).flat_map(|i| [0, (i + 1) % 6 + 1, i + 1]).collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 7])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; 7])
    .with_inserted_indices(Indices::U32(indices))
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let image = images.add(Image::new_target_texture(
        640,
        400,
        TextureFormat::Rgba16Float,
        None,
    ));
    let material = materials.add(WaterMaterial {
        settings: WaterSettings {
            tint: linear(observed_style::cistern::water_tint()),
            haze: Vec4::ZERO,
            params: Vec4::new(
                12.0,
                60.0,
                observed_style::cistern::WATER_REFLECTANCE,
                observed_style::cistern::WATER_RIPPLE,
            ),
        },
        reflection: image.clone(),
    });
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Camera {
            order: -2,
            is_active: false,
            invert_culling: true,
            ..default()
        },
        Tonemapping::None,
        RenderTarget::Image(image.clone().into()),
        RenderLayers::layer(0),
        ReflectionCamera,
        DespawnOnExit(GameState::HexWfc),
    ));
    commands.insert_resource(ReservoirAssets {
        image,
        material,
        mesh: meshes.add(surface_mesh()),
    });
}

/// Matrix composition preserves the negative determinant of the reflection.
fn reflected(
    main: &Transform,
    lens: &PerspectiveProjection,
    height: f32,
) -> (Transform, Projection) {
    let mirror = Mat4::from_translation(Vec3::Y * height * 2.0)
        * Mat4::from_mat3a(reflection_matrix(Vec3::Y));
    let transform = Transform::from_matrix(mirror * main.to_matrix());
    let normal = (main.compute_affine().matrix3.inverse() * Vec3::NEG_Y).normalize();
    let projection = Projection::Perspective(PerspectiveProjection {
        near_clip_plane: normal.extend(height - main.translation.y),
        ..lens.clone()
    });
    (transform, projection)
}

type MainCamera<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Projection,
        &'static Camera,
        Option<&'static DistanceFog>,
    ),
    (With<GameCam>, Without<ReflectionCamera>),
>;
type MirrorCamera<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut Projection,
        &'static mut Camera,
    ),
    (With<ReflectionCamera>, Without<GameCam>),
>;

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    assets: Option<Res<ReservoirAssets>>,
    cells: Query<(Entity, &ReservoirCell)>,
    new_cells: Query<(Entity, &ReservoirCell), Added<ReservoirCell>>,
    main: MainCamera,
    mut mirror: MirrorCamera,
    mut surfaces: Query<(&ReservoirSurface, &mut Visibility)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (parent, cell) in &new_cells {
        let origin = Vec3::from_array(hex_origin(cell.0));
        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_translation(origin + Vec3::Y * observed_style::cistern::WATER_HEIGHT),
            RenderLayers::layer(WATER_LAYER),
            ReservoirSurface(cell.0),
            ChildOf(parent),
            NotShadowCaster,
            crate::hex_wfc::view::NeverShadowCaster,
            NotShadowReceiver,
        ));
    }
    let Ok((pose, Projection::Perspective(lens), camera, fog)) = main.single() else {
        return;
    };
    let Ok((entity, mut mirror_pose, mut mirror_lens, mut mirror_camera)) = mirror.single_mut()
    else {
        return;
    };
    let closest = cells
        .iter()
        .map(|(_, c)| c.0)
        .filter(|c| {
            let origin = Vec3::from_array(hex_origin(*c));
            pose.translation.y >= origin.y
                && pose.translation.y < origin.y + observed_hex::TILE_LEVEL_HEIGHT
        })
        .min_by(|a, b| {
            pose.translation
                .distance_squared(Vec3::from_array(hex_origin(*a)))
                .total_cmp(
                    &pose
                        .translation
                        .distance_squared(Vec3::from_array(hex_origin(*b))),
                )
        })
        .filter(|c| pose.translation.distance(Vec3::from_array(hex_origin(*c))) < 35.0);
    for (surface, mut visibility) in &mut surfaces {
        *visibility = if closest.is_some_and(|cell| surface.0.level == cell.level) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    mirror_camera.is_active = camera.is_active && closest.is_some();
    commands
        .entity(entity)
        .insert(fog.cloned().unwrap_or_default());
    if let Some(cell) = closest {
        let height = f32::from(cell.level) * observed_hex::TILE_LEVEL_HEIGHT
            + observed_style::cistern::WATER_HEIGHT;
        let (next_pose, next_lens) = reflected(pose, lens, height);
        *mirror_pose = next_pose;
        *mirror_lens = next_lens;
    }
    // The main camera sees default geometry plus water. Other render layers
    // belong to the desk/cards/map and stay out of this first-person mirror.
    if let Some(window) = windows.iter().next()
        && let Some(mut image) = images.get_mut(&assets.image)
    {
        let UVec2 {
            x: width,
            y: height,
        } = reflection_extent(window.physical_width(), window.physical_height());
        if image.width() != width || image.height() != height {
            image.resize(Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
        }
    }
    if let Some(mut material) = materials.get_mut(&assets.material)
        && let Some(fog) = fog
    {
        material.settings.haze = linear(fog.color);
        if let FogFalloff::Linear { start, end } = fog.falloff {
            material.settings.params.x = start;
            material.settings.params.y = end;
        }
    }
}
fn cleanup(mut commands: Commands, cameras: Query<Entity, With<GameCam>>) {
    commands.remove_resource::<ReservoirAssets>();
    for camera in &cameras {
        commands.entity(camera).remove::<RenderLayers>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reflection_target_preserves_aspect_at_large_and_portrait_sizes() {
        assert_eq!(reflection_extent(1280, 800), UVec2::new(640, 400));
        assert_eq!(reflection_extent(3840, 2160), UVec2::new(960, 540));
        assert_eq!(reflection_extent(1080, 1920), UVec2::new(338, 600));
        assert_eq!(reflection_extent(0, 0), UVec2::ONE);
    }
    #[test]
    fn reflection_uses_the_reservoirs_storey_and_reverses_winding() {
        let main =
            Transform::from_xyz(3.0, 18.0, 5.0).looking_at(Vec3::new(5.0, 17.0, 2.0), Vec3::Y);
        let (mirror, _) = reflected(&main, &PerspectiveProjection::default(), 16.65);
        assert!((mirror.translation.y - 15.3).abs() < 0.001);
        assert!(mirror.to_matrix().determinant() < 0.0);
    }
}
