//! Scenes built with every authoring contract switched off.
//!
//! No seam, no port signature, no hull budget, no Legibility Contract, no
//! catalogue. The question this lab exists to answer is *is the look worth
//! wanting*, and the only way to answer it honestly is to build the look
//! without first bending it to fit the building.
//!
//! # The rig and the room
//!
//! This file is the rig: a camera, a mirror, a sky, and fog. It knows nothing
//! about what it is pointed at. Each scene module builds its own geometry,
//! spawns its own sun, and hands back a [`Staging`] describing where to stand
//! and what colour the air is. `OBSERVED2_DAYDREAM_SCENE` chooses between
//! them, `OBSERVED2_DAYDREAM_HOUR` and `OBSERVED2_DAYDREAM_VIEW` mean whatever
//! the chosen scene decides they mean.
//!
//! - [`hall`] - a colonnade standing in water, two arched walls deep.
//! - [`shaft`] - the same tools pointed straight up a hexagonal well.
//! - [`monument`] - eleven cells of held axis, ending on an empty dais.
//!
//! # Two tricks carry every scene
//!
//! **The void is fog, not a skybox.** The sky is a gradient on a sphere lit by
//! nothing and fogged by nothing, and its bottom is the exact colour the fog
//! fades to. So a hole in a wall opens onto a distance that never resolves,
//! and nothing has to be modelled behind it.
//!
//! **The water is a real planar reflection.** A second camera sits at the main
//! camera's position reflected through the plane `y = 0`, with its near plane
//! skewed onto that same plane, and renders into a texture the water shader
//! samples at each fragment's screen position. It replaced an earlier trick
//! that built the whole room twice, once scaled `-1` in Y - which cost a
//! second copy of every draw and could only reflect what had been duplicated.
//!
//! Every scene's mirror is the plane `y = 0`. That is the one thing the rig
//! insists on.

mod hall;
mod monument;
mod shaft;

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::reflection_matrix;
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::window::{PrimaryWindow, WindowResized, WindowResolution};

/// The water is the only thing on this layer, so the reflection camera - which
/// draws everything *else* - can be told to skip it. Without that the mirror
/// would be trying to reflect itself.
const WATER_LAYER: usize = 1;

/// What a scene hands back to the rig that draws it.
pub struct Staging {
    pub eye: Vec3,
    pub focus: Vec3,
    /// Fills the clear colour, the distance fog, the water's far fade, and the
    /// bottom of the sky, so the horizon is the one place with no edge.
    pub haze: Color,
    /// The top of the sky. The ramp between this and `haze` is the only thing
    /// overhead, and it is what the water reflects when it reflects nothing.
    pub zenith: Color,
    pub ambient_color: Color,
    pub ambient: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    /// A scene without water gets no reflection camera at all.
    pub water: Option<WaterPlan>,
}

/// A sheet of water. It always lies on `y = 0`, because that is the plane the
/// reflection is built around.
pub struct WaterPlan {
    pub centre: Vec3,
    pub size: Vec2,
    pub tint: Color,
    /// How much is reflected when looked at straight down into. Schlick takes
    /// it the rest of the way at grazing angles.
    pub head_on: f32,
    pub ripple: f32,
}

fn scene() -> &'static str {
    match std::env::var("OBSERVED2_DAYDREAM_SCENE").as_deref() {
        Ok("shaft") => "shaft",
        Ok("monument") => "monument",
        _ => "hall",
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Daydream".into(),
            resolution: WindowResolution::new(1600, 900),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(MaterialPlugin::<WaterMaterial>::default())
    .add_systems(Startup, setup)
    .add_systems(Update, (follow_with_reflection, refit_reflection_target))
    .add_systems(Update, shoot);
    app.run();
}

/// Everything the water shader needs in one uniform.
///
/// These have to travel as a single `ShaderType` rather than as three fields
/// each tagged `#[uniform(0)]` - separate fields all claiming binding zero
/// collide, and only one of them survives to reach the shader.
#[derive(Clone, ShaderType)]
struct WaterSettings {
    tint: Vec4,
    haze: Vec4,
    /// x: fog start, y: fog end, z: reflectance head-on, w: ripple amplitude.
    params: Vec4,
}

/// The water surface. See `assets/shaders/daydream_water.wgsl`.
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
        "shaders/daydream_water.wgsl".into()
    }
}

/// Marks the camera that renders the reflected world into the water's texture.
#[derive(Component)]
struct ReflectionCamera;

/// Marks the camera the viewer is actually looking through.
#[derive(Component)]
struct MainCamera;

/// The texture the reflection is rendered into, kept so it can be resized with
/// the window.
#[derive(Resource)]
struct ReflectionTarget(Handle<Image>);

/// A two-tone checkerboard, generated rather than loaded. Cells are drawn a
/// few texels across so linear filtering can soften the far end of a room
/// instead of tearing it into moire.
pub fn checkerboard(images: &mut Assets<Image>, a: [u8; 3], b: [u8; 3]) -> Handle<Image> {
    const CELLS: usize = 8;
    const TEXELS: usize = 16;
    const N: usize = CELLS * TEXELS;
    let mut data = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let c = if (x / TEXELS + y / TEXELS).is_multiple_of(2) {
                a
            } else {
                b
            };
            data.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: N as u32,
            height: N as u32,
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
        ..default()
    });
    images.add(image)
}

/// A cuboid whose checker cells come out square in world space regardless of
/// how long the slab is.
pub fn checkered(width: f32, depth: f32) -> bevy::math::Affine2 {
    const CELL: f32 = 3.0;
    bevy::math::Affine2::from_scale(Vec2::new(width / (CELL * 8.0), depth / (CELL * 8.0)))
}

/// A vertical ramp from the zenith down to the horizon, and flat haze below
/// it. One pixel wide: the sphere's own latitude does all the work.
fn sky_gradient(images: &mut Assets<Image>, zenith: Color, horizon: Color) -> Handle<Image> {
    const N: usize = 128;
    let top = LinearRgba::from(zenith);
    let bottom = LinearRgba::from(horizon);
    let mut data = Vec::with_capacity(N * 4);
    for row in 0..N {
        // v runs 0 at the zenith to 1 at the nadir. The horizon sits at the
        // halfway row, and everything below it is held at the haze colour so
        // the distance fog has something exact to fade into.
        //
        // The ramp is deliberately slow near the top. A room seen from close
        // to the ground shows only the first thirty degrees of sky, and a
        // gradient that reached the horizon colour early was indistinguishable
        // from a flat fill - which cost an hour of looking for a sky that was
        // rendering perfectly all along.
        let v = row as f32 / (N - 1) as f32;
        let t = (v * 2.0).min(1.0);
        let eased = t.powf(1.15);
        let mix = |a: f32, b: f32| a + (b - a) * eased;
        let srgb = Srgba::from(Color::LinearRgba(LinearRgba::new(
            mix(top.red, bottom.red),
            mix(top.green, bottom.green),
            mix(top.blue, bottom.blue),
            1.0,
        )));
        data.extend_from_slice(&[
            (srgb.red * 255.0) as u8,
            (srgb.green * 255.0) as u8,
            (srgb.blue * 255.0) as u8,
            255,
        ]);
    }
    let mut image = Image::new(
        Extent3d {
            width: 1,
            height: N as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    });
    images.add(image)
}

/// The texture the reflection camera draws into. It matches the window, so the
/// water shader can sample it at the fragment's own screen position.
fn reflection_image(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut image = Image::new_uninit(
        Extent3d {
            width: size.x.max(1),
            height: size.y.max(1),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage |=
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(image)
}

/// A colour as the shader wants it: linear, not sRGB.
fn linear(color: Color) -> Vec4 {
    let c = LinearRgba::from(color);
    Vec4::new(c.red, c.green, c.blue, c.alpha)
}

pub fn matte(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 1.0,
        reflectance: 0.02,
        ..default()
    }
}

/// The same fog on every camera, so a reflected room fades exactly as the real
/// one does.
fn haze(staging: &Staging) -> DistanceFog {
    DistanceFog {
        color: staging.haze,
        falloff: FogFalloff::Linear {
            start: staging.fog_start,
            end: staging.fog_end,
        },
        ..default()
    }
}

/// Place a camera at `main` reflected through the waterline, and skew its near
/// plane onto that same plane so nothing below the water is drawn into the
/// reflection.
///
/// The reflection has to be composed as a matrix and only then turned back
/// into a `Transform`. Reflection carries a negative determinant, and
/// composing `Transform`s cannot represent that.
fn reflected(main: &Transform, projection: &PerspectiveProjection) -> (Transform, Projection) {
    let transform =
        Transform::from_matrix(Mat4::from_mat3a(reflection_matrix(Vec3::Y)) * main.to_matrix());

    // Signed distance from the camera to the waterline, and the waterline's
    // normal expressed in the main camera's view space: together they are the
    // oblique near plane.
    let to_plane = InfinitePlane3d::new(Vec3::Y)
        .signed_distance(Isometry3d::IDENTITY, Vec3::ZERO - main.translation);
    let view_from_world = main.compute_affine().matrix3.inverse();
    let normal = (view_from_world * Vec3::NEG_Y).normalize();

    (
        transform,
        Projection::Perspective(PerspectiveProjection {
            near_clip_plane: normal.extend(to_plane),
            ..projection.clone()
        }),
    )
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // The room goes up first: it owns its own geometry and its own sun, and
    // tells the rig where to stand and what colour the air is.
    let staging = match scene() {
        "shaft" => shaft::build(&mut commands, &mut meshes, &mut materials),
        "monument" => monument::build(&mut commands, &mut meshes, &mut materials),
        _ => hall::build(&mut commands, &mut meshes, &mut materials, &mut images),
    };

    commands.insert_resource(ClearColor(staging.haze));
    commands.insert_resource(GlobalAmbientLight {
        color: staging.ambient_color,
        brightness: staging.ambient,
        ..default()
    });

    let camera_transform =
        Transform::from_translation(staging.eye).looking_at(staging.focus, Vec3::Y);
    let projection = PerspectiveProjection::default();

    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Just enough bloom that an opening blows out the way a bright
        // exterior does when the interior is what the eye is adapted to.
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        Projection::Perspective(projection.clone()),
        camera_transform,
        haze(&staging),
        // With no shadows at all in some hours, nothing was darkening the
        // inside corners - where a pier meets the water, where a beam meets
        // the roof. Screen-space occlusion puts that contact back. It costs
        // MSAA, which SSAO cannot run alongside, so the edges are handed to
        // temporal antialiasing instead; a still capture gives it ninety
        // frames to settle.
        Msaa::Off,
        // Lit air, for the scenes that have an opening narrow enough to make
        // beams out of it. Harmless where no `FogVolume` is spawned.
        bevy::light::VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        },
        ScreenSpaceAmbientOcclusion {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
            constant_object_thickness: 3.0,
        },
        TemporalAntiAliasing::default(),
        RenderLayers::from_layers(&[0, WATER_LAYER]),
        MainCamera,
    ));

    // The sky: a sphere large enough to sit outside everything, lit by nothing
    // and fogged by nothing.
    let sky = sky_gradient(&mut images, staging.zenith, staging.haze);
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(420.0).mesh().uv(48, 32))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(sky),
            unlit: true,
            fog_enabled: false,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_translation(staging.focus.with_y(0.0)),
        // Without this the sky is a four-hundred-metre opaque shell standing
        // between the sun and the room, and every shadow-casting hour renders
        // the whole building in silhouette.
        NotShadowCaster,
        NotShadowReceiver,
    ));

    let Some(water) = staging.water.as_ref() else {
        return;
    };

    let size = windows.iter().next().map_or(UVec2::new(1600, 900), |w| {
        UVec2::new(w.physical_width(), w.physical_height())
    });
    let reflection = reflection_image(&mut images, size);
    commands.insert_resource(ReflectionTarget(reflection.clone()));

    let (reflect_transform, reflect_projection) = reflected(&camera_transform, &projection);
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            // Reflecting the world flips the winding of every triangle, so
            // backface culling has to be inverted to match.
            invert_culling: true,
            clear_color: ClearColorConfig::Custom(staging.haze),
            ..default()
        },
        // The water shader samples this texture and hands the result to the
        // main camera's own tonemapping, so it must not be tonemapped twice.
        Tonemapping::None,
        RenderTarget::Image(reflection.clone().into()),
        reflect_transform,
        reflect_projection,
        haze(&staging),
        RenderLayers::layer(0),
        ReflectionCamera,
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(water.size.x, water.size.y))),
        MeshMaterial3d(water_materials.add(WaterMaterial {
            settings: WaterSettings {
                tint: linear(water.tint),
                haze: linear(staging.haze),
                params: Vec4::new(
                    staging.fog_start,
                    staging.fog_end,
                    water.head_on,
                    water.ripple,
                ),
            },
            reflection,
        })),
        Transform::from_translation(water.centre.with_y(0.0)),
        RenderLayers::layer(WATER_LAYER),
    ));
}

/// The main camera's pose and lens, borrowed disjointly from the reflection
/// camera's.
type MainCameraQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static Projection),
    (With<MainCamera>, Without<ReflectionCamera>),
>;

/// Keep the reflection camera pinned to the main camera's mirror image.
fn follow_with_reflection(
    main: MainCameraQuery,
    mut reflection: Query<(&mut Transform, &mut Projection), With<ReflectionCamera>>,
) {
    let Ok((main_transform, Projection::Perspective(main_projection))) = main.single() else {
        return;
    };
    let Ok((mut transform, mut projection)) = reflection.single_mut() else {
        return;
    };
    let (next_transform, next_projection) = reflected(main_transform, main_projection);
    *transform = next_transform;
    *projection = next_projection;
}

/// The water samples the reflection at screen position, so the reflection
/// texture has to stay the same size as the window.
fn refit_reflection_target(
    mut resized: MessageReader<WindowResized>,
    target: Option<Res<ReflectionTarget>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
) {
    if resized.read().last().is_none() {
        return;
    }
    let (Some(target), Some(window)) = (target, windows.iter().next()) else {
        return;
    };
    let Some(mut image) = images.get_mut(&target.0) else {
        return;
    };
    image.resize(Extent3d {
        width: window.physical_width().max(1),
        height: window.physical_height().max(1),
        depth_or_array_layers: 1,
    });
}

fn shoot(mut commands: Commands, mut done: Local<bool>, mut frames: Local<u32>) {
    *frames += 1;
    if *frames < 90 || *done {
        return;
    }
    *done = true;
    let path = std::env::var("OBSERVED2_CAPTURE").unwrap_or_else(|_| "daydream.png".to_string());
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}
