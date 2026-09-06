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
//! **The water is a mirror made of geometry.** Bevy has no cheap planar
//! reflection, so the colonnade is built twice - once upright and once scaled
//! `-1` in Y - with a translucent sheet at the waterline between them. It is
//! the oldest trick there is and it costs one extra draw per reflected object.

use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;

/// Pale haze. Clear colour and fog share it, so the opening has no far side.
const HAZE: Color = Color::srgb(0.80, 0.86, 0.92);
/// The warm mass everything is cut from.
const SALMON: Color = Color::srgb(0.87, 0.37, 0.28);
/// The same mass in shadow, for the reveals.
const SALMON_DEEP: Color = Color::srgb(0.60, 0.21, 0.18);
/// The cool half of the palette: water, and the wall beyond the arch.
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
    .insert_resource(ClearColor(HAZE))
    .insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.88, 0.87, 0.92),
        brightness: 560.0,
        ..default()
    })
    .add_systems(Startup, setup)
    .add_systems(Update, shoot);
    app.run();
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

/// Mirroring through the waterline flips the winding of every triangle, so
/// the reflected copy of the hall needs its own materials with culling off.
/// Without this the water reflects nothing and reads as a hole in the floor.
fn matte(color: Color, mirrored: bool) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 1.0,
        reflectance: 0.02,
        double_sided: mirrored,
        cull_mode: if mirrored { None } else { default() },
        ..default()
    }
}

/// A cuboid whose checker cells come out square in world space regardless of
/// how long the slab is.
fn checkered(width: f32, depth: f32) -> bevy::math::Affine2 {
    const CELL: f32 = 3.0;
    bevy::math::Affine2::from_scale(Vec2::new(width / (CELL * 8.0), depth / (CELL * 8.0)))
}

/// Where to stand. `OBSERVED2_DAYDREAM_VIEW` picks one of three; the default
/// is the one down the axis, which is the composition the scene was built for.
fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Down at the waterline, where the reflection is longer than the hall.
        Ok("low") => (Vec3::new(0.0, 0.7, 17.0), Vec3::new(0.0, 4.6, FAR)),
        // From an aisle, so the colonnade is read across rather than through.
        Ok("aisle") => (Vec3::new(10.5, 3.2, 7.0), Vec3::new(-2.0, 4.6, -20.0)),
        _ => (Vec3::new(0.0, 2.7, 20.0), Vec3::new(0.0, 5.2, FAR)),
    }
}

#[allow(clippy::too_many_lines)]
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (eye, focus) = view();
    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Just enough bloom that the opening blows out the way a bright
        // exterior does when the interior is what the eye is adapted to.
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        Transform::from_translation(eye).looking_at(focus, Vec3::Y),
        DistanceFog {
            color: HAZE,
            falloff: FogFalloff::Linear {
                start: 40.0,
                end: 130.0,
            },
            ..default()
        },
    ));

    // One distant source, high and soft, casting nothing. Shadows were the
    // first thing to go: a cascade boundary put a hard seam across the hall,
    // and the look this is chasing is shadowless anyway - form comes from
    // which way a face is turned, not from what is thrown onto it.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.88),
            illuminance: 3_400.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(14.0, 26.0, 8.0).looking_at(Vec3::new(0.0, 0.0, -22.0), Vec3::Y),
    ));

    let floor_tex = checkerboard(&mut images, [224, 180, 162], [186, 118, 100]);
    let mut hue = |c: Color| {
        [
            materials.add(matte(c, false)),
            materials.add(matte(c, true)),
        ]
    };
    let salmon_pair = hue(SALMON);
    let salmon_deep_pair = hue(SALMON_DEEP);
    let teal_pair = hue(TEAL);
    let cream_pair = hue(CREAM);

    // Everything is built twice: once upright, once mirrored through y = 0, so
    // the sheet of water at the waterline has something to reflect. There is
    // no floor over the channel - if there were, it would bury the mirror.
    let deck = meshes.add(Cuboid::new(HALL_HALF - CHANNEL, 0.4, 150.0));
    let deck_uv = checkered(HALL_HALF - CHANNEL, 150.0);
    let pier = meshes.add(Cuboid::new(1.4, 9.6, 1.4));
    let cap = meshes.add(Cuboid::new(1.9, 0.5, 1.9));
    let base = meshes.add(Cuboid::new(1.8, 1.1, 1.8));
    let beam = meshes.add(Cuboid::new(NAVE * 2.0, 0.7, 1.1));
    let roof = meshes.add(Cuboid::new(NAVE * 2.0, 0.6, 150.0));
    let voussoir = meshes.add(Cuboid::new(0.66, 1.05, 1.3));
    let wall_block = meshes.add(Cuboid::new(5.0, 15.0, 1.2));
    let plinth = meshes.add(Cuboid::new(1.7, 2.4, 1.7));
    let orb = meshes.add(Sphere::new(1.05).mesh().ico(5).unwrap());

    for (index, mirror) in [1.0_f32, -1.0].into_iter().enumerate() {
        let m = mirror;
        let salmon = &salmon_pair[index];
        let salmon_deep = &salmon_deep_pair[index];
        let teal = &teal_pair[index];
        let cream = &cream_pair[index];
        let at = move |x: f32, y: f32, z: f32| Transform {
            translation: Vec3::new(x, y * m, z),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0, m, 1.0),
        };

        let floor_mat = materials.add(StandardMaterial {
            base_color_texture: Some(floor_tex.clone()),
            perceptual_roughness: 0.96,
            reflectance: 0.03,
            uv_transform: deck_uv,
            double_sided: mirror < 0.0,
            cull_mode: if mirror < 0.0 { None } else { default() },
            ..default()
        });
        // The decks are horizontal and sit at the waterline, so a mirrored
        // copy would only stack a second slab on the first.
        if mirror > 0.0 {
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(deck.clone()),
                    MeshMaterial3d(floor_mat.clone()),
                    at(side * (CHANNEL + (HALL_HALF - CHANNEL) * 0.5), -0.2, -45.0),
                ));
            }
        }

        // The colonnades, and the beams that tie them across. The beams are
        // what make this an interior rather than a ruin.
        // The nave is roofed; the aisles are not. Everything overhead is a
        // silhouette against the haze that comes in from the sides.
        commands.spawn((
            Mesh3d(roof.clone()),
            MeshMaterial3d(salmon_deep.clone()),
            at(0.0, 11.15, -45.0),
        ));
        for i in 0..9 {
            let z = 2.0 - i as f32 * 4.6;
            for side in [-1.0_f32, 1.0] {
                let x = side * 4.4;
                commands.spawn((
                    Mesh3d(pier.clone()),
                    MeshMaterial3d(salmon.clone()),
                    at(x, 4.8, z),
                ));
                commands.spawn((
                    Mesh3d(cap.clone()),
                    MeshMaterial3d(salmon_deep.clone()),
                    at(x, 9.85, z),
                ));
                // A cool block at the foot of every pier. Two hues at full
                // chroma is the whole colour scheme; the third is the haze.
                commands.spawn((
                    Mesh3d(base.clone()),
                    MeshMaterial3d(teal.clone()),
                    at(x, 0.55, z),
                ));
            }
            commands.spawn((
                Mesh3d(beam.clone()),
                MeshMaterial3d(salmon_deep.clone()),
                at(0.0, 10.45, z),
            ));
        }

        // The far wall, and the arch cut into it. Nothing is modelled beyond
        // the opening - what shows through is the clear colour, which is the
        // fog colour, so the distance never resolves.
        for side in [-1.0_f32, 1.0] {
            commands.spawn((
                Mesh3d(wall_block.clone()),
                MeshMaterial3d(teal.clone()),
                at(side * (ARCH_R + 2.5), 7.5, FAR),
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(ARCH_R * 2.0 + 2.0, 5.6, 1.2))),
            MeshMaterial3d(teal.clone()),
            at(0.0, SPRING + ARCH_R + 2.8, FAR),
        ));
        for side in [-1.0_f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, SPRING, 1.2))),
                MeshMaterial3d(teal.clone()),
                at(side * (ARCH_R + 0.5), SPRING * 0.5, FAR),
            ));
        }
        // The arch ring: short chords swept over a half circle.
        for i in 0..17 {
            let t = i as f32 / 16.0;
            let a = std::f32::consts::PI * t;
            commands.spawn((
                Mesh3d(voussoir.clone()),
                MeshMaterial3d(salmon_deep.clone()),
                Transform {
                    translation: Vec3::new(-a.cos() * ARCH_R, (SPRING + a.sin() * ARCH_R) * m, FAR),
                    rotation: Quat::from_rotation_z((-a + std::f32::consts::FRAC_PI_2) * m),
                    scale: Vec3::new(1.0, m, 1.0),
                },
            ));
        }

        // The one object in the room, standing in the water on the axis so
        // that its reflection hangs directly under it.
        commands.spawn((
            Mesh3d(plinth.clone()),
            MeshMaterial3d(teal.clone()),
            at(-2.1, 1.2, -6.5),
        ));
        commands.spawn((
            Mesh3d(orb.clone()),
            MeshMaterial3d(cream.clone()),
            at(-2.1, 3.5, -6.5),
        ));
    }

    // The waterline itself: dark, smooth, and translucent enough that the
    // mirrored half reads as a reflection rather than as a basement.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(CHANNEL * 2.0, 0.02, 150.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.05, 0.19, 0.25, 0.45),
            perceptual_roughness: 0.04,
            metallic: 0.4,
            reflectance: 0.7,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, -45.0),
    ));
}

fn shoot(mut commands: Commands, mut done: Local<bool>, mut frames: Local<u32>) {
    *frames += 1;
    if *frames < 90 || *done {
        return;
    }
    *done = true;
    let path = std::env::var("OBSERVED2_CAPTURE").unwrap_or_else(|_| "daydream.png".to_string());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
