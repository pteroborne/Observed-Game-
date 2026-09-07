//! The Shadow Screen: paper walls, and light that has to come through them.
//!
//! Everything else in this lab is lit by something you could walk to - a sun,
//! a lamp, a strip in a reveal. Here the light source is a *surface*: runs of
//! paper panels with the low sun behind them, glowing the whole length of the
//! room. You never see out, except once, at the very end.
//!
//! # This is the room the hall wanted to be
//!
//! Volumetric shafts were built for the colonnade and taken out again, because
//! its aisles were open and the sun arrived as an unbroken wash with nothing to
//! cut it. The condition they needed is exactly this: solid walls with a
//! regular grid of openings, and a dark interior behind. The paper is told not
//! to cast, so the sun pours through it; the timber between the panels *does*
//! cast, so what pours through arrives already striped. The shafts are not
//! decoration here, they are the subject.
//!
//! # Depth is made of layers, not of distance
//!
//! Three rooms deep, separated by cross-walls with a single opening each, so
//! the view runs through one doorway into another and another. That is what
//! makes this architecture read: not a long room, but a short one repeated
//! behind itself, every layer dimmer than the last.
//!
//! # Two things that are geometry and one that is not
//!
//! The kumiko grid on the paper is far too fine to model - roughly a hundred
//! cells to a panel - so it is drawn into the emissive texture. The stiles and
//! rails that hold each panel *are* geometry, because they are what casts.
//! Tatami is the third: mats are read almost entirely from the black banding
//! between them, so the banding is modelled and the mats themselves are a
//! single flat field.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{CascadeShadowConfigBuilder, FogVolume, NotShadowCaster, VolumetricLight};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::{Staging, matte, scramble};

/// Half-width of the rooms, and how far back the three of them run.
const ROOM_X: f32 = 3.1;
const BACK: f32 = -11.4;
/// Low, the way this architecture is.
const CEILING: f32 = 2.62;
/// Head height of the sliding screens. Everything above it is transom.
const HEAD: f32 = 1.78;
/// The two cross-walls, and the width of the opening left in each.
const CROSS: [f32; 2] = [-3.6, -7.5];
const DOOR: f32 = 1.7;
/// A tatami mat, near enough. The banding between them is the only thing that
/// actually reads at this distance.
const MAT: f32 = 0.92;

const TIMBER: Color = Color::srgb(0.072, 0.056, 0.042);
const TIMBER_LIT: Color = Color::srgb(0.20, 0.145, 0.095);
const PLASTER: Color = Color::srgb(0.20, 0.175, 0.145);
const TATAMI: Color = Color::srgb(0.40, 0.355, 0.235);

/// The paper. Strong, because it is standing in for the sun.
const PAPER: LinearRgba = LinearRgba::new(3.4, 2.75, 1.85, 1.0);
/// Panels not facing the sun. Same wall, a quarter of the light.
const PAPER_SHADE: LinearRgba = LinearRgba::new(0.30, 0.265, 0.225, 1.0);
/// The garden, seen once, through the last doorway. The entire cool half of
/// the palette is this.
const GARDEN: LinearRgba = LinearRgba::new(0.30, 0.62, 0.34, 1.0);

struct Hour {
    haze: Color,
    ambient_color: Color,
    ambient: f32,
    sun_from: Vec3,
    air: f32,
}

fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        // Almost gone, and round far enough that only a shoulder of the wall
        // is still carrying it.
        Ok("late") => Hour {
            haze: Color::srgb(0.15, 0.115, 0.085),
            ambient_color: Color::srgb(0.30, 0.30, 0.36),
            ambient: 16.0,
            sun_from: Vec3::new(15.0, 2.4, -13.0),
            air: 0.042,
        },
        _ => Hour {
            haze: Color::srgb(0.20, 0.16, 0.12),
            ambient_color: Color::srgb(0.36, 0.34, 0.33),
            ambient: 27.0,
            sun_from: Vec3::new(13.0, 4.6, -4.0),
            air: 0.028,
        },
    }
}

fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Sitting on the mats, which is the height the proportions are for.
        Ok("low") => (Vec3::new(0.75, 0.58, 2.3), Vec3::new(-0.2, 0.95, BACK)),
        // Into the corner, so the two runs of screens are read against each
        // other rather than one at a time.
        Ok("corner") => (Vec3::new(1.9, 1.42, 1.4), Vec3::new(-3.0, 1.15, -5.6)),
        _ => (Vec3::new(0.85, 1.24, 2.6), Vec3::new(-0.15, 1.05, BACK)),
    }
}

/// The kumiko grid, drawn rather than built. Roughly a hundred cells to a
/// panel is far past the point where modelling each bar would be sensible, and
/// the grid casts nothing worth seeing at this fineness anyway.
fn kumiko(images: &mut Assets<Image>) -> Handle<Image> {
    const N: usize = 256;
    const ACROSS: usize = 9;
    const UP: usize = 13;
    let mut data = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let cx = (x * ACROSS) % N < ACROSS * 2;
            let cy = (y * UP) % N < UP * 2;
            let edge = x < 3 || y < 3 || x >= N - 3 || y >= N - 3;
            // Paper is not uniform; it is fibrous and slightly blotchy, and
            // backlighting is exactly the condition that shows it.
            let fibre = (scramble(x as i32 / 2, y as i32 / 2, 0x71C4) % 26) as u8;
            let v: u8 = if cx || cy || edge {
                40
            } else {
                214 + fibre / 3
            };
            data.extend_from_slice(&[v, v, v, 255]);
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
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()
    });
    images.add(image)
}

/// One sliding panel: a sheet of lit paper in a frame. The sheet casts
/// nothing - it is the aperture - and the frame casts everything.
struct Panel<'a> {
    paper: &'a Handle<StandardMaterial>,
    frame: &'a Handle<StandardMaterial>,
    at: Vec3,
    width: f32,
    height: f32,
    yaw: f32,
}

fn shoji(commands: &mut Commands, meshes: &mut Assets<Mesh>, p: &Panel) {
    let turn = Quat::from_rotation_y(p.yaw);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(p.width - 0.07, p.height - 0.07, 0.025))),
        MeshMaterial3d(p.paper.clone()),
        Transform::from_translation(p.at).with_rotation(turn),
        NotShadowCaster,
    ));
    let stile = meshes.add(Cuboid::new(0.048, p.height, 0.055));
    let rail = meshes.add(Cuboid::new(p.width, 0.048, 0.055));
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            Mesh3d(stile.clone()),
            MeshMaterial3d(p.frame.clone()),
            Transform::from_translation(p.at + turn * Vec3::X * (side * (p.width * 0.5 - 0.024)))
                .with_rotation(turn),
        ));
    }
    for rise in [-0.5_f32, 0.0, 0.5] {
        commands.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(p.frame.clone()),
            Transform::from_translation(p.at + Vec3::Y * (rise * (p.height - 0.048)))
                .with_rotation(turn),
        ));
    }
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

    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.86, 0.63),
            illuminance: 17_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: 0.1,
            first_cascade_far_bound: 9.0,
            maximum_distance: 46.0,
            overlap_proportion: 0.2,
        }
        .build(),
        VolumetricLight,
        Transform::from_translation(hour.sun_from).looking_at(Vec3::new(-2.0, 0.7, -5.5), Vec3::Y),
    ));
    commands.spawn((
        FogVolume {
            fog_color: Color::srgb(1.0, 0.94, 0.84),
            density_factor: hour.air,
            absorption: 0.02,
            scattering: 1.5,
            scattering_asymmetry: 0.4,
            light_intensity: 0.95,
            ..default()
        },
        Transform::from_xyz(0.0, CEILING * 0.5, BACK * 0.5).with_scale(Vec3::new(
            ROOM_X * 2.2,
            CEILING * 1.1,
            -BACK * 1.15,
        )),
    ));

    let grid = kumiko(images);
    let timber = materials.add(matte(TIMBER));
    let timber_lit = materials.add(matte(TIMBER_LIT));
    let plaster = materials.add(matte(PLASTER));
    let mats = materials.add(StandardMaterial {
        base_color: TATAMI,
        perceptual_roughness: 0.95,
        reflectance: 0.02,
        ..default()
    });
    let lit_paper = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.82, 0.7),
        emissive: PAPER,
        emissive_texture: Some(grid.clone()),
        base_color_texture: Some(grid.clone()),
        ..default()
    });
    let shade_paper = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.66, 0.58),
        emissive: PAPER_SHADE,
        emissive_texture: Some(grid.clone()),
        base_color_texture: Some(grid),
        ..default()
    });

    // Floor and ceiling. The tatami is one field; the banding between mats is
    // what is actually modelled, because it is what the eye reads.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(ROOM_X * 2.0, 0.3, -BACK + 4.0))),
        MeshMaterial3d(mats),
        Transform::from_xyz(0.0, -0.15, BACK * 0.5 + 1.4),
    ));
    let band_long = meshes.add(Cuboid::new(0.035, 0.012, -BACK + 4.0));
    let band_cross = meshes.add(Cuboid::new(MAT, 0.012, 0.035));
    for i in -3..=3 {
        commands.spawn((
            Mesh3d(band_long.clone()),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(i as f32 * MAT, 0.006, BACK * 0.5 + 1.4),
        ));
    }
    // Mats are twice as long as they are wide and are laid so that four
    // corners never meet, so the cross banding has to step by one mat between
    // neighbouring columns. Running it straight across reads as floor tiles.
    for col in -3i32..3 {
        let offset = if col.rem_euclid(2) == 0 { 0.0 } else { MAT };
        for i in 0..9 {
            commands.spawn((
                Mesh3d(band_cross.clone()),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(
                    (col as f32 + 0.5) * MAT,
                    0.006,
                    1.6 + offset - i as f32 * MAT * 2.0,
                ),
            ));
        }
    }
    // A slatted ceiling, close overhead.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(ROOM_X * 2.0, 0.2, -BACK + 4.0))),
        MeshMaterial3d(timber.clone()),
        Transform::from_xyz(0.0, CEILING + 0.1, BACK * 0.5 + 1.4),
    ));
    for i in 0..22 {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.05, 0.05, -BACK + 3.6))),
            MeshMaterial3d(timber_lit.clone()),
            Transform::from_xyz(
                -ROOM_X + 0.2 + i as f32 * 0.28,
                CEILING - 0.03,
                BACK * 0.5 + 1.4,
            ),
        ));
    }

    // The two long walls, panel by panel. The sun is out beyond the right-hand
    // run, so that one is the light and the other is only its echo.
    let bays = ((-BACK + 3.0) / 0.92) as i32;
    for i in 0..bays {
        let z = 2.2 - i as f32 * 0.92;
        for side in [-1.0_f32, 1.0] {
            let sunward = side > 0.0;
            shoji(
                commands,
                meshes,
                &Panel {
                    paper: if sunward { &lit_paper } else { &shade_paper },
                    frame: &timber,
                    at: Vec3::new(side * ROOM_X, HEAD * 0.5, z),
                    width: 0.9,
                    height: HEAD,
                    yaw: std::f32::consts::FRAC_PI_2,
                },
            );
            // Transom above the head rail, which is what stops the wall
            // reading as a fence and starts it reading as a room.
            shoji(
                commands,
                meshes,
                &Panel {
                    paper: if sunward { &lit_paper } else { &shade_paper },
                    frame: &timber,
                    at: Vec3::new(side * ROOM_X, HEAD + 0.44, z),
                    width: 0.9,
                    height: 0.5,
                    yaw: std::f32::consts::FRAC_PI_2,
                },
            );
        }
    }
    // The heavy horizontal that separates screen from transom, and the one
    // above it. These are the darkest lines in the picture.
    for side in [-1.0_f32, 1.0] {
        for y in [HEAD + 0.09, CEILING - 0.12] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.13, 0.13, -BACK + 3.6))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(side * ROOM_X, y, BACK * 0.5 + 1.4),
            ));
        }
    }

    // Two cross-walls, each with one opening, so the room is three rooms.
    for (n, z) in CROSS.iter().enumerate() {
        let flank = (ROOM_X * 2.0 - DOOR) * 0.5;
        for side in [-1.0_f32, 1.0] {
            let cx = side * (DOOR * 0.5 + flank * 0.5);
            let panels = (flank / 0.9).max(1.0).round() as i32;
            for k in 0..panels {
                let x = side * (DOOR * 0.5) + side * (k as f32 + 0.5) * (flank / panels as f32);
                shoji(
                    commands,
                    meshes,
                    &Panel {
                        paper: &shade_paper,
                        frame: &timber,
                        at: Vec3::new(x, HEAD * 0.5, *z),
                        width: flank / panels as f32,
                        height: HEAD,
                        yaw: 0.0,
                    },
                );
            }
            // Solid plaster over the flanking panels, and a post at the jamb.
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(flank, CEILING - HEAD - 0.1, 0.16))),
                MeshMaterial3d(plaster.clone()),
                Transform::from_xyz(cx, HEAD + (CEILING - HEAD) * 0.5, *z),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.17, CEILING, 0.19))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(side * DOOR * 0.5, CEILING * 0.5, *z),
            ));
        }
        // The lintel over the opening.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(DOOR + 0.35, 0.16, 0.2))),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(0.0, HEAD + 0.08, *z),
        ));
        // A threshold strip on the floor, one per layer.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(ROOM_X * 2.0, 0.03, 0.13))),
            MeshMaterial3d(timber_lit.clone()),
            Transform::from_xyz(0.0, 0.015, *z),
        ));
        let _ = n;
    }

    // The far wall, and the one moment the outside is admitted: a strip of
    // garden through the last opening, which is all the cool colour there is.
    for i in -3..=3 {
        shoji(
            commands,
            meshes,
            &Panel {
                paper: &shade_paper,
                frame: &timber,
                at: Vec3::new(i as f32 * 0.9, HEAD * 0.5, BACK),
                width: 0.9,
                height: HEAD,
                yaw: 0.0,
            },
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.15, 0.62, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.55, 0.38),
            emissive: GARDEN,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.92, BACK - 0.5),
        NotShadowCaster,
    ));

    // Dust, only where the light is. Dust you cannot see is not dust.
    let mote = meshes.add(Cuboid::new(0.0055, 0.0055, 0.0055));
    let lit_air = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.78),
        emissive: LinearRgba::new(1.05, 0.85, 0.58, 1.0),
        ..default()
    });
    for i in 0..190 {
        let a = scramble(i, 1, 0xD1CE);
        let b = scramble(i, 2, 0x4F7B);
        let c = scramble(i, 3, 0x9C02);
        let x = -ROOM_X + (a % 1000) as f32 / 1000.0 * ROOM_X * 2.0;
        let y = 0.2 + (b % 1000) as f32 / 1000.0 * (CEILING - 0.9);
        let z = 2.0 - (c % 1000) as f32 / 1000.0 * 7.0;
        commands.spawn((
            Mesh3d(mote.clone()),
            MeshMaterial3d(lit_air.clone()),
            Transform::from_xyz(x, y, z),
            NotShadowCaster,
        ));
    }

    Staging {
        eye,
        focus,
        haze: hour.haze,
        zenith: hour.haze,
        ambient_color: hour.ambient_color,
        ambient: hour.ambient,
        fov: 0.95,
        fog_start: 7.0,
        fog_end: 23.0,
        // The mats have a sheen but they are matting, not a mirror, and the
        // rig's water carries no texture. A reflection here would cost the
        // banding, which is worth more than the doubling.
        water: None,
    }
}
