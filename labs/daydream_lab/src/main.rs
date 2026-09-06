//! A scene built with every contract switched off.
//!
//! No seam, no port signature, no hull budget, no Legibility Contract, no
//! catalogue. The question this lab exists to answer is *is the look worth
//! wanting*, and the only way to answer it honestly is to build the look
//! without first bending it to fit the building.
//!
//! The vocabulary is the metaphysical-painting one that runs from de Chirico
//! through a hundred prerendered adventure games: a colonnade to a vanishing
//! point, a round-headed arch onto bright haze, still water, a checkerboard,
//! one sphere. Flat planes, three hues, no shadow to speak of.
//!
//! # Two tricks are doing most of the work
//!
//! **The void is fog, not a skybox.** The clear colour and the fog colour are
//! the same pale value, so the arch opens onto a distance that never resolves.
//! Nothing is modelled beyond the wall.
//!
//! **The water is a real planar reflection.** A second camera sits at the main
//! camera's position reflected through the plane `y = 0`, renders the hall to
//! a texture, and the water shader samples that texture at each fragment's
//! screen position. It replaced an earlier trick that built the whole hall
//! twice - once upright, once scaled `-1` in Y - which cost a second copy of
//! every draw and could only ever reflect what had been duplicated.
//!
//! # Two hours, not one setting
//!
//! `OBSERVED2_DAYDREAM_HOUR` chooses between two pictures of the same room.
//! `noon` is ambient-led and shadowless, opening onto a cold white void.
//! `late` collapses the ambient and rakes one warm sun across the colonnade,
//! so the piers stripe the aisle. Neither is a variation on the other.
//!
//! # What volumetric light shafts ran into
//!
//! Shafts were tried and taken back out. Bevy will draw them - `FogVolume`
//! plus `VolumetricLight` plus `VolumetricFog` on the camera - but this
//! building cannot produce them, because the nave has no side walls. The
//! aisles are open, so the sun arrives as an unbroken wash rather than
//! through openings, and there is nothing to cut it into bars of lit air.
//! Every density that made the air visible also filled the arch with murk.
//! Shafts here would need a clerestory first: a solid upper wall with holes
//! in it. That is an architectural change, not a lighting one.
//!
//! # Shadows are off at noon on purpose
//!
//! A cascade boundary drew a hard seam across the hall, and everything past it
//! blew out. Losing shadows cost nothing at noon, because that look is
//! ambient-led: form comes from which way a face is turned, not from what is
//! thrown onto it. `late` needs them, and gets cascades long enough to reach
//! past the far wall so the seam has nowhere to fall.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::math::reflection_matrix;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::window::{PrimaryWindow, WindowResized, WindowResolution};

/// Everything about the light, which is the only thing that changes between
/// the two paintings this hall can be.
///
/// `noon` is the scene as first built: ambient-led, shadowless, opening onto a
/// cold white void. `late` is the same architecture at the end of the day -
/// the ambient collapses, one warm sun rakes across the colonnade, and the
/// piers cut it into bars of lit air. Neither is a variation on the other;
/// they are different pictures of the same room.
struct Hour {
    /// Fills the clear colour, the distance fog, and the water's far fade, so
    /// the opening in the far wall has no far side.
    haze: Color,
    ambient_color: Color,
    ambient: f32,
    sun_color: Color,
    sun_lux: f32,
    /// The sun is a direction, but it is easier to author as two points.
    sun_from: Vec3,
    sun_to: Vec3,
    /// Off unless the light needs to be blocked by something to read.
    shadows: bool,
}

fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        Ok("late") => Hour {
            haze: Color::srgb(0.98, 0.87, 0.74),
            ambient_color: Color::srgb(0.46, 0.52, 0.68),
            ambient: 235.0,
            sun_color: Color::srgb(1.0, 0.87, 0.66),
            sun_lux: 15_000.0,
            sun_from: Vec3::new(34.0, 16.0, -14.0),
            sun_to: Vec3::new(0.0, 3.4, -20.0),
            shadows: true,
        },
        _ => Hour {
            haze: Color::srgb(0.80, 0.86, 0.92),
            ambient_color: Color::srgb(0.88, 0.87, 0.92),
            ambient: 560.0,
            sun_color: Color::srgb(1.0, 0.95, 0.88),
            sun_lux: 3_400.0,
            sun_from: Vec3::new(14.0, 26.0, 8.0),
            sun_to: Vec3::new(0.0, 0.0, -22.0),
            shadows: false,
        },
    }
}
/// The warm mass everything is cut from.
const SALMON: Color = Color::srgb(0.87, 0.37, 0.28);
/// The same mass in shadow, for the reveals.
const SALMON_DEEP: Color = Color::srgb(0.60, 0.21, 0.18);
/// The cool half of the palette: water, pier feet, and the wall beyond the arch.
const TEAL: Color = Color::srgb(0.10, 0.40, 0.46);
/// The one object that is neither wall nor water.
const CREAM: Color = Color::srgb(0.93, 0.86, 0.74);

const HALL_HALF: f32 = 14.0;
/// Half-width of the open channel. The floor stops here; the water starts.
/// It is wide enough that the colonnade stands *in* the water rather than
/// beside it, which is the whole reason the reflection is worth having.
const CHANNEL: f32 = 5.9;
/// Where the far wall stands. Near enough that the arch is the subject.
const FAR: f32 = -33.0;
/// Springing line of the arch.
const SPRING: f32 = 5.2;
const ARCH_R: f32 = 3.4;
/// Half-width of the roofed nave. Outside it the aisles are open to the sky,
/// so the colonnade is backlit and the haze leaks in between the piers.
const NAVE: f32 = 5.9;

const FOG_START: f32 = 40.0;
const FOG_END: f32 = 130.0;

/// The water is the only thing on this layer, so the reflection camera - which
/// draws everything *else* - can be told to skip it. Without that the mirror
/// would be trying to reflect itself.
const WATER_LAYER: usize = 1;

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
    .insert_resource(ClearColor(hour().haze))
    .insert_resource(GlobalAmbientLight {
        color: hour().ambient_color,
        brightness: hour().ambient,
        ..default()
    })
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
    /// The colour of the water itself, seen through the reflection.
    tint: Vec4,
    /// The colour everything fades to. Matches the fog and the clear colour.
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

/// Marks the camera the player is actually looking through.
#[derive(Component)]
struct MainCamera;

/// The texture the reflection is rendered into, kept so it can be resized with
/// the window.
#[derive(Resource)]
struct ReflectionTarget(Handle<Image>);

/// Where to stand. `OBSERVED2_DAYDREAM_VIEW` picks one of three; the default
/// is the one down the axis, which is the composition the scene was built for.
fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Down at the waterline, where the reflection is longer than the hall.
        Ok("low") => (Vec3::new(0.0, 1.35, 17.0), Vec3::new(0.0, 4.8, FAR)),
        // From an aisle, so the colonnade is read across rather than through.
        Ok("aisle") => (Vec3::new(10.5, 3.2, 7.0), Vec3::new(-2.0, 4.6, -20.0)),
        _ => (Vec3::new(0.0, 2.7, 20.0), Vec3::new(0.0, 5.2, FAR)),
    }
}

/// A two-tone checkerboard, generated rather than loaded. Cells are drawn a
/// few texels across so linear filtering can soften the far end of the hall
/// instead of tearing it into moire.
fn checkerboard(images: &mut Assets<Image>, a: [u8; 3], b: [u8; 3]) -> Handle<Image> {
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

fn matte(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 1.0,
        reflectance: 0.02,
        ..default()
    }
}

/// A cuboid whose checker cells come out square in world space regardless of
/// how long the slab is.
fn checkered(width: f32, depth: f32) -> bevy::math::Affine2 {
    const CELL: f32 = 3.0;
    bevy::math::Affine2::from_scale(Vec2::new(width / (CELL * 8.0), depth / (CELL * 8.0)))
}

/// The same fog on every camera, so the reflected hall fades exactly as the
/// real one does.
fn haze(hour: &Hour) -> DistanceFog {
    DistanceFog {
        color: hour.haze,
        falloff: FogFalloff::Linear {
            start: FOG_START,
            end: FOG_END,
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

#[allow(clippy::too_many_lines)]
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let hour = hour();
    let (eye, focus) = view();
    let camera_transform = Transform::from_translation(eye).looking_at(focus, Vec3::Y);
    let projection = PerspectiveProjection::default();

    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Just enough bloom that the opening blows out the way a bright
        // exterior does when the interior is what the eye is adapted to.
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        Projection::Perspective(projection.clone()),
        camera_transform,
        haze(&hour),
        RenderLayers::from_layers(&[0, WATER_LAYER]),
        MainCamera,
    ));

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
            clear_color: ClearColorConfig::Custom(hour.haze),
            ..default()
        },
        // The water shader samples this texture and hands the result to the
        // main camera's own tonemapping, so it must not be tonemapped twice.
        Tonemapping::None,
        RenderTarget::Image(reflection.clone().into()),
        reflect_transform,
        reflect_projection,
        haze(&hour),
        RenderLayers::layer(0),
        ReflectionCamera,
    ));

    // One distant source, raking across the colonnade rather than down it,
    // so the piers cut the light into bars.
    //
    // Shadows are back on, but only because the shafts need them - a
    // volumetric light has to know what it is blocked by. The seam that made
    // them unusable before was a cascade boundary falling inside the hall;
    // the cascades below reach past the far wall, so there is no boundary
    // left to see.
    commands.spawn((
        DirectionalLight {
            color: hour.sun_color,
            illuminance: hour.sun_lux,
            shadow_maps_enabled: hour.shadows,
            ..default()
        },
        // Cascades that reach past the far wall. The seam that made
        // shadows unusable at first was a cascade boundary falling inside
        // the hall, with everything past it unshadowed and blown out.
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.4,
            first_cascade_far_bound: 24.0,
            maximum_distance: 190.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_translation(hour.sun_from).looking_at(hour.sun_to, Vec3::Y),
    ));

    let floor_tex = checkerboard(&mut images, [224, 180, 162], [186, 118, 100]);
    let salmon = materials.add(matte(SALMON));
    let salmon_deep = materials.add(matte(SALMON_DEEP));
    let teal = materials.add(matte(TEAL));
    let cream = materials.add(matte(CREAM));

    let deck = meshes.add(Cuboid::new(HALL_HALF - CHANNEL, 0.4, 150.0));
    let floor_mat = materials.add(StandardMaterial {
        base_color_texture: Some(floor_tex),
        perceptual_roughness: 0.96,
        reflectance: 0.03,
        uv_transform: checkered(HALL_HALF - CHANNEL, 150.0),
        ..default()
    });
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            Mesh3d(deck.clone()),
            MeshMaterial3d(floor_mat.clone()),
            Transform::from_xyz(side * (CHANNEL + (HALL_HALF - CHANNEL) * 0.5), -0.2, -45.0),
        ));
    }

    // The nave is roofed; the aisles are not. Everything overhead is a
    // silhouette against the haze that comes in from the sides.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(NAVE * 2.0, 0.6, 150.0))),
        MeshMaterial3d(salmon_deep.clone()),
        Transform::from_xyz(0.0, 11.15, -45.0),
    ));

    // The colonnades, and the beams that tie them across.
    let pier = meshes.add(Cuboid::new(1.4, 9.6, 1.4));
    let cap = meshes.add(Cuboid::new(1.9, 0.5, 1.9));
    let base = meshes.add(Cuboid::new(1.8, 1.1, 1.8));
    let beam = meshes.add(Cuboid::new(NAVE * 2.0, 0.7, 1.1));
    for i in 0..9 {
        let z = 2.0 - i as f32 * 4.6;
        for side in [-1.0_f32, 1.0] {
            let x = side * 4.4;
            commands.spawn((
                Mesh3d(pier.clone()),
                MeshMaterial3d(salmon.clone()),
                Transform::from_xyz(x, 4.8, z),
            ));
            commands.spawn((
                Mesh3d(cap.clone()),
                MeshMaterial3d(salmon_deep.clone()),
                Transform::from_xyz(x, 9.85, z),
            ));
            // A cool block at the foot of every pier. Two hues at full chroma
            // is the whole colour scheme; the third is the haze.
            commands.spawn((
                Mesh3d(base.clone()),
                MeshMaterial3d(teal.clone()),
                Transform::from_xyz(x, 0.55, z),
            ));
        }
        commands.spawn((
            Mesh3d(beam.clone()),
            MeshMaterial3d(salmon_deep.clone()),
            Transform::from_xyz(0.0, 10.45, z),
        ));
    }

    // The far wall, and the arch cut into it. Nothing is modelled beyond the
    // opening - what shows through is the clear colour, which is the fog
    // colour, so the distance never resolves.
    let wall_block = meshes.add(Cuboid::new(5.0, 15.0, 1.2));
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            Mesh3d(wall_block.clone()),
            MeshMaterial3d(teal.clone()),
            Transform::from_xyz(side * (ARCH_R + 2.5), 7.5, FAR),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, SPRING, 1.2))),
            MeshMaterial3d(teal.clone()),
            Transform::from_xyz(side * (ARCH_R + 0.5), SPRING * 0.5, FAR),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(ARCH_R * 2.0 + 2.0, 5.6, 1.2))),
        MeshMaterial3d(teal.clone()),
        Transform::from_xyz(0.0, SPRING + ARCH_R + 2.8, FAR),
    ));
    // The arch ring: short chords swept over a half circle.
    let voussoir = meshes.add(Cuboid::new(0.66, 1.05, 1.3));
    for i in 0..17 {
        let t = i as f32 / 16.0;
        let a = std::f32::consts::PI * t;
        commands.spawn((
            Mesh3d(voussoir.clone()),
            MeshMaterial3d(salmon_deep.clone()),
            Transform::from_xyz(-a.cos() * ARCH_R, SPRING + a.sin() * ARCH_R, FAR)
                .with_rotation(Quat::from_rotation_z(-a + std::f32::consts::FRAC_PI_2)),
        ));
    }

    // The one object in the room, standing in the water off the axis so that
    // it does not block the opening and its reflection hangs under it.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.7, 2.4, 1.7))),
        MeshMaterial3d(teal.clone()),
        Transform::from_xyz(-2.1, 1.2, -6.5),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.05).mesh().ico(5).unwrap())),
        MeshMaterial3d(cream.clone()),
        Transform::from_xyz(-2.1, 3.5, -6.5),
    ));

    // The waterline itself, alone on its own render layer.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(CHANNEL * 2.0, 150.0))),
        MeshMaterial3d(water_materials.add(WaterMaterial {
            settings: WaterSettings {
                tint: linear(Color::srgb(0.07, 0.30, 0.34)),
                haze: linear(hour.haze),
                params: Vec4::new(FOG_START, FOG_END, 0.10, 0.0075),
            },
            reflection,
        })),
        Transform::from_xyz(0.0, 0.0, -45.0),
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
    target: Res<ReflectionTarget>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
) {
    if resized.read().last().is_none() {
        return;
    }
    let Some(window) = windows.iter().next() else {
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
