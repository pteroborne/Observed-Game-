//! The Overlit Grid, which everyone else calls the Backrooms.
//!
//! Every other room in this lab is generous: tall, axial, and built around a
//! bright thing at the end of a long view. This one is the refusal of all
//! three. The ceiling is at head height, there is no axis to stand on, and
//! there is nothing at the end because there is no end - the far wall is
//! another gap into another identical room.
//!
//! # One hue, and no relief from it
//!
//! Walls, carpet and ceiling are the same yellow at three values. There is no
//! complementary accent anywhere, which is the single hardest rule here: every
//! instinct says to put one cool thing in frame to relieve it, and relief is
//! exactly what the room must not offer.
//!
//! # The ceiling is the light
//!
//! Not a sun, not a fixture you could walk up to - a field of weak recessed
//! panels in a suspended grid, covering the entire plan. Nothing casts a
//! shadow worth the name, so form comes only from which way a face is turned,
//! and every corner is the same brightness as every other corner.
//!
//! # There is deliberately no reflection
//!
//! The rig's mirror is optional and this is the scene that proves it. Wet
//! carpet would be *beautiful* - a floor doubling all that yellow would make a
//! genuinely handsome picture - and that is precisely why it is wrong. The
//! reference is dry, matte, low-pile commercial carpet that gives nothing
//! back. Handing this room a mirror would be answering a question nobody
//! asked.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::{Staging, matte};

/// Half-extent of the floor plate. There is no reason for it to stop here
/// other than that the fog does.
const HALF: f32 = 30.0;
/// Head height plus a hand. Everything unnerving about this room is this
/// number against the two above.
const CEILING: f32 = 2.92;
/// The plan is laid out on this module, and so is the ceiling grid.
const BAY: f32 = 4.0;
const BAYS: i32 = 15;

const WALL: Color = Color::srgb(0.80, 0.73, 0.41);
const CARPET: Color = Color::srgb(0.63, 0.54, 0.28);
const TILE: Color = Color::srgb(0.79, 0.75, 0.55);
/// The one piece of trim in the building, and the only reason it reads as an
/// office rather than a box.
const SKIRT: Color = Color::srgb(0.48, 0.43, 0.23);
/// Weak. These are not bright lights; there are simply no others.
const PANEL: LinearRgba = LinearRgba::new(1.45, 1.34, 0.82, 1.0);
/// A stopped panel is not a dimmer panel. It is off, and it stays the
/// colour of the plastic in front of a tube that no longer lights.
const PANEL_TIRED: LinearRgba = LinearRgba::new(0.09, 0.08, 0.05, 1.0);

struct Hour {
    haze: Color,
    ambient_color: Color,
    ambient: f32,
    /// How many panels have stopped. The lights never go out here; some of
    /// them just stop, and nobody comes.
    dead_in: i32,
}

fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        Ok("late") => Hour {
            haze: Color::srgb(0.55, 0.49, 0.26),
            ambient_color: Color::srgb(0.70, 0.64, 0.38),
            ambient: 150.0,
            dead_in: 3,
        },
        _ => Hour {
            haze: Color::srgb(0.74, 0.68, 0.40),
            ambient_color: Color::srgb(0.88, 0.82, 0.54),
            ambient: 620.0,
            dead_in: 11,
        },
    }
}

fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Facing into a pocket, where three stubs overlap and the way through
        // is not obvious.
        Ok("pocket") => (Vec3::new(6.0, 1.60, 9.0), Vec3::new(-7.0, 1.45, -12.0)),
        // Down on the carpet. The ceiling gets very close from here.
        Ok("low") => (Vec3::new(0.0, 0.55, 12.0), Vec3::new(-2.0, 1.30, -14.0)),
        _ => (Vec3::new(1.5, 1.62, 13.0), Vec3::new(-4.0, 1.34, -14.0)),
    }
}

/// A cheap deterministic hash, so the plan is irregular but the same irregular
/// every run. Nothing here should look designed, and nothing should move.
fn scramble(x: i32, y: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B) ^ salt;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    h
}

/// A flat field with a little value noise in it: carpet, mostly. Generated
/// rather than loaded so the whole scene stays inside one file.
fn speckle(images: &mut Assets<Image>, base: [u8; 3], grit: i16) -> Handle<Image> {
    const N: usize = 128;
    let mut data = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let n = scramble(x as i32, y as i32, 0x5A17) % 256;
            let d = (n as i16 - 128) * grit / 128;
            for c in base {
                data.push((i16::from(c) + d).clamp(0, 255) as u8);
            }
            data.push(255);
        }
    }
    repeating(images, data, N as u32)
}

/// A pale field ruled into squares: the suspended ceiling.
fn tile_grid(images: &mut Assets<Image>, base: [u8; 3], line: [u8; 3]) -> Handle<Image> {
    const N: usize = 128;
    let mut data = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let edge = x < 3 || y < 3 || x >= N - 3 || y >= N - 3;
            let c = if edge { line } else { base };
            let n = scramble(x as i32, y as i32, 0x11B7) % 12;
            for v in c {
                data.push((u16::from(v) + u16::from(n as u8)).min(255) as u8);
            }
            data.push(255);
        }
    }
    repeating(images, data, N as u32)
}

fn repeating(images: &mut Assets<Image>, data: Vec<u8>, n: u32) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
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

fn tiled(width: f32, depth: f32, cell: f32) -> bevy::math::Affine2 {
    bevy::math::Affine2::from_scale(Vec2::new(width / cell, depth / cell))
}

#[allow(clippy::too_many_lines)]
pub fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Staging {
    let hour = hour();
    let (eye, focus) = view();
    let span = HALF * 2.0;

    // No sun at all. Not a dim one - none. The ceiling is the only source, and
    // anything directional would immediately imply an outside.
    let carpet = materials.add(StandardMaterial {
        base_color: CARPET,
        base_color_texture: Some(speckle(images, [161, 138, 71], 24)),
        uv_transform: tiled(span, span, 1.6),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let ceiling = materials.add(StandardMaterial {
        base_color: TILE,
        base_color_texture: Some(tile_grid(images, [201, 191, 140], [150, 145, 110])),
        uv_transform: tiled(span, span, 0.62),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let wall = materials.add(StandardMaterial {
        base_color: WALL,
        base_color_texture: Some(speckle(images, [204, 186, 105], 12)),
        uv_transform: tiled(BAY, CEILING, 0.9),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let skirt = materials.add(matte(SKIRT));
    let lit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.84, 0.66),
        emissive: PANEL,
        ..default()
    });
    let tired = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.38, 0.30),
        emissive: PANEL_TIRED,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(span, 0.4, span))),
        MeshMaterial3d(carpet),
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(span, 0.3, span))),
        MeshMaterial3d(ceiling),
        Transform::from_xyz(0.0, CEILING + 0.15, 0.0),
    ));

    // The lights, on the same module as the plan. Every fourth-ish one has
    // stopped, which is the only event that has ever happened in this room.
    let panel = meshes.add(Cuboid::new(1.15, 0.06, 0.58));
    for i in -BAYS..=BAYS {
        for j in -BAYS..=BAYS {
            let at = Vec3::new(i as f32 * BAY * 0.5, CEILING - 0.02, j as f32 * BAY * 0.5);
            if at.x.abs() > HALF || at.z.abs() > HALF {
                continue;
            }
            let dead = scramble(i, j, 0x9051) % 16 >= hour.dead_in as u32;
            commands.spawn((
                Mesh3d(panel.clone()),
                MeshMaterial3d(if dead { tired.clone() } else { lit.clone() }),
                Transform::from_translation(at),
            ));
        }
    }
    // A sparse scatter of real lights so the emissive panels put something on
    // the carpet. One per panel would be hundreds; this is enough to fill.
    for i in -2..=2 {
        for j in -2..=2 {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.96, 0.76),
                    intensity: 340_000.0,
                    range: 22.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(i as f32 * 11.0, CEILING - 0.35, j as f32 * 11.0),
            ));
        }
    }

    // The plan. Square columns and half-length wall stubs on an irregular
    // scatter - never a colonnade, never a corridor. What makes this room
    // frightening is that it is neither open nor closed: every sightline ends
    // on a stub with a gap beside it, and the gap leads somewhere identical.
    let column = meshes.add(Cuboid::new(1.5, CEILING, 1.5));
    let column_skirt = meshes.add(Cuboid::new(1.62, 0.13, 1.62));
    let stub = meshes.add(Cuboid::new(BAY * 1.5, CEILING, 0.55));
    let stub_skirt = meshes.add(Cuboid::new(BAY * 1.5 + 0.1, 0.13, 0.67));
    for i in -7..=7 {
        for j in -7..=7 {
            let at = Vec3::new(i as f32 * BAY, CEILING * 0.5, j as f32 * BAY);
            if at.x.abs() > HALF - 3.0 || at.z.abs() > HALF - 3.0 {
                continue;
            }
            let roll = scramble(i, j, 0x2C4D) % 100;
            let (body, foot, yaw) = if roll < 14 {
                (column.clone(), column_skirt.clone(), 0.0)
            } else if roll < 30 {
                (stub.clone(), stub_skirt.clone(), 0.0)
            } else if roll < 44 {
                (
                    stub.clone(),
                    stub_skirt.clone(),
                    std::f32::consts::FRAC_PI_2,
                )
            } else {
                continue;
            };
            let turn = Quat::from_rotation_y(yaw);
            commands.spawn((
                Mesh3d(body),
                MeshMaterial3d(wall.clone()),
                Transform::from_translation(at).with_rotation(turn),
            ));
            commands.spawn((
                Mesh3d(foot),
                MeshMaterial3d(skirt.clone()),
                Transform::from_translation(at.with_y(0.065)).with_rotation(turn),
            ));
        }
    }

    Staging {
        eye,
        focus,
        haze: hour.haze,
        // There is no sky. If the rig's sphere is ever visible through a gap
        // it must be the same yellow as everything else, because an outside
        // would ruin the only claim this room makes.
        zenith: hour.haze,
        ambient_color: hour.ambient_color,
        ambient: hour.ambient,
        // A wide lens. The ceiling is at head height, and a normal lens turns
        // that into a corridor rather than into a room too wide to be this low.
        fov: 1.16,
        fog_start: 11.0,
        fog_end: 52.0,
        // Deliberately none. See the module docs: a mirror here would make it
        // beautiful, and beautiful is the wrong answer.
        water: None,
    }
}
