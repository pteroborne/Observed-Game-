//! The Wellshaft: the same three tools pointed the other way.
//!
//! The hall is horizontal, open at the sides, lit by ambient, and reads along
//! an axis you could walk down. This is its inverse in every one of those
//! terms - vertical, tight, lit by a single opening ninety-six metres up, and
//! read along an axis you could only fall down. It exists to prove that the
//! lab is a way of making pictures rather than one picture.
//!
//! # It is built out of the game's own numbers
//!
//! Eight metres to a level and seven metres from the centre of a cell to a
//! wall face are the seam constants the hex catalogue actually obeys, and the
//! plan here is a hexagon for the same reason. Nothing in this lab is
//! required to honour them - that is the whole point of the lab - but a shaft
//! is the one place where the contract and good proportion happen to agree.
//!
//! # The darkness is painted, not lit
//!
//! A directional light does not fall off with distance, so a sun pointed down
//! a shaft lights the bottom exactly as hard as the top and the depth
//! disappears. Every level therefore gets its own materials, dimmed and
//! cooled toward the water by a curve. It is the oldest trick in scene
//! painting and it costs one material per level.

use bevy::light::{CascadeShadowConfigBuilder, FogVolume, VolumetricLight};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_3;

use crate::{Staging, WaterPlan, matte};

/// Eight metres to a level, seven from the centre to a wall face.
const LEVEL: f32 = 8.0;
const APOTHEM: f32 = 7.0;
const LEVELS: i32 = 12;
/// The side of a hexagon with that apothem, which is the width of one wall.
const SIDE: f32 = APOTHEM * 1.154_700_5;

const CONCRETE: Color = Color::srgb(0.62, 0.66, 0.63);
const RUST: Color = Color::srgb(0.78, 0.39, 0.15);
const RUST_DEEP: Color = Color::srgb(0.45, 0.22, 0.11);
/// What every surface tends toward as it goes down. Depth is cold as well as
/// dark, and only the second of those is something a light could do.
const DROWNED: Color = Color::srgb(0.09, 0.14, 0.24);

struct Hour {
    haze: Color,
    zenith: Color,
    ambient_color: Color,
    ambient: f32,
    sun_color: Color,
    sun_lux: f32,
    sun_from: Vec3,
    sun_to: Vec3,
}

/// `noon` drops the sun straight down the shaft, so it reaches the water and
/// the landings throw their shadows onto the levels below. `late` swings it
/// off the vertical until only the top three levels are in daylight and
/// everything under them is left to the sky's own blue.
fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        Ok("late") => Hour {
            haze: Color::srgb(0.95, 0.84, 0.71),
            zenith: Color::srgb(0.20, 0.25, 0.58),
            ambient_color: Color::srgb(0.38, 0.45, 0.72),
            ambient: 260.0,
            sun_color: Color::srgb(1.0, 0.86, 0.64),
            sun_lux: 9_000.0,
            sun_from: Vec3::new(90.0, 110.0, 34.0),
            sun_to: Vec3::new(0.0, 72.0, 0.0),
        },
        _ => Hour {
            haze: Color::srgb(0.86, 0.90, 0.94),
            zenith: Color::srgb(0.12, 0.31, 0.72),
            ambient_color: Color::srgb(0.60, 0.71, 0.90),
            ambient: 470.0,
            sun_color: Color::srgb(1.0, 0.97, 0.90),
            sun_lux: 8_000.0,
            // Off the vertical by enough that the beam strikes a wall
            // partway down and the landings get to cut across it. Straight
            // down lights the whole cross-section at once, which is a flood
            // rather than a shaft.
            sun_from: Vec3::new(46.0, 128.0, 30.0),
            sun_to: Vec3::new(0.0, 10.0, 0.0),
        },
    }
}

/// Where to stand. There is only one interesting direction in a shaft, so
/// these differ mostly in how much of the water they let into the frame.
fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Looking down into the water instead of up the shaft. A shaft is
        // taller than any single frame can hold - the sky is ninety-six
        // metres from the surface, which is more than a lens can span - so
        // the reflection is the only way to see the whole of it at once,
        // upside down.
        Ok("low" | "pool") => (Vec3::new(1.1, 7.2, 1.1), Vec3::ZERO),
        // Out on a landing a third of the way up, looking back down.
        Ok("landing") => (Vec3::new(3.9, 26.4, 2.9), Vec3::new(-2.4, 4.0, -2.6)),
        _ => (Vec3::new(0.0, 2.0, 2.6), Vec3::new(0.0, 40.0, -1.0)),
    }
}

/// How much light a level still gets: one at the opening, nearly nothing at
/// the water. The exponent is what keeps the top half bright and drops the
/// bottom away quickly, which is how a shaft actually reads.
fn reach(level: i32) -> f32 {
    let t = level as f32 / (LEVELS - 1) as f32;
    0.30 + 0.70 * t.powf(1.5)
}

/// A colour at depth: dimmed toward black and pulled toward the cold that
/// deep water and long air both put into everything.
fn drowned(color: Color, reach: f32) -> Color {
    let near = LinearRgba::from(color);
    let far = LinearRgba::from(DROWNED);
    let mix = |a: f32, b: f32| a * reach + b * (1.0 - reach);
    Color::LinearRgba(LinearRgba::new(
        mix(near.red, far.red),
        mix(near.green, far.green),
        mix(near.blue, far.blue),
        1.0,
    ))
}

pub fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Staging {
    let hour = hour();
    let (eye, focus) = view();

    // Shadows are the whole point here rather than an option: a landing that
    // does not darken the level below it is a decal, not a floor.
    commands.spawn((
        DirectionalLight {
            color: hour.sun_color,
            illuminance: hour.sun_lux,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.5,
            first_cascade_far_bound: 22.0,
            maximum_distance: 230.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_translation(hour.sun_from).looking_at(hour.sun_to, Vec3::Y),
        // The hall could not carry light shafts because its aisles were open
        // and the sun arrived as an unbroken wash. This is the opposite
        // building: a closed tube with exactly one opening, which is the only
        // condition under which lit air reads as a beam rather than as murk.
        VolumetricLight,
    ));

    // The air in the shaft. Almost no absorption and heavy scattering, so the
    // volume adds light where the sun reaches it instead of subtracting light
    // everywhere.
    commands.spawn((
        FogVolume {
            fog_color: Color::srgb(0.98, 0.96, 0.92),
            density_factor: 0.017,
            absorption: 0.02,
            scattering: 1.3,
            scattering_asymmetry: 0.30,
            light_intensity: 1.1,
            ..default()
        },
        Transform::from_xyz(0.0, LEVEL * LEVELS as f32 * 0.5, 0.0).with_scale(Vec3::new(
            APOTHEM * 2.3,
            LEVEL * LEVELS as f32,
            APOTHEM * 2.3,
        )),
    ));

    let panel = meshes.add(Cuboid::new(SIDE, LEVEL, 0.8));
    let jamb = meshes.add(Cuboid::new(SIDE * 0.32, LEVEL, 0.8));
    let head = meshes.add(Cuboid::new(SIDE * 0.36, LEVEL * 0.34, 0.8));
    let course = meshes.add(Cuboid::new(SIDE + 0.5, 0.5, 1.6));
    let plate = meshes.add(Cuboid::new(SIDE * 1.45, 0.35, 3.4));
    let bracket = meshes.add(Cuboid::new(0.35, 1.5, 2.6));
    let stile = meshes.add(Cuboid::new(0.16, LEVEL, 0.16));
    let rung = meshes.add(Cuboid::new(1.0, 0.1, 0.12));

    for level in 0..LEVELS {
        let y = level as f32 * LEVEL;
        let lit = reach(level);
        let wall = materials.add(matte(drowned(CONCRETE, lit)));
        // What an opening shows. Left unbacked, a hole in the wall looks
        // straight through to the sky sphere and reads as a blazing white
        // rectangle at the bottom of a dark shaft - the exact opposite of a
        // doorway. A panel a metre behind it turns the hole back into a
        // room that is simply unlit.
        let beyond = materials.add(matte(drowned(Color::srgb(0.10, 0.11, 0.13), lit * 0.5)));
        let trim = materials.add(matte(drowned(RUST, lit)));
        let iron = materials.add(matte(drowned(RUST_DEEP, lit)));

        for face in 0..6 {
            let angle = face as f32 * FRAC_PI_3;
            let out = Vec3::new(angle.sin(), 0.0, angle.cos());
            let turn = Quat::from_rotation_y(angle);
            let mid = out * APOTHEM + Vec3::Y * (y + LEVEL * 0.5);

            // Some faces are pierced. The openings lead nowhere - they are
            // black rectangles onto nothing - but they are what stops the
            // shaft reading as a tube rather than as a building seen from
            // the inside of its one empty column.
            if level > 0 && (level * 2 + face) % 5 == 0 {
                for side in [-1.0_f32, 1.0] {
                    commands.spawn((
                        Mesh3d(jamb.clone()),
                        MeshMaterial3d(wall.clone()),
                        Transform::from_translation(mid + turn * Vec3::X * (side * SIDE * 0.34))
                            .with_rotation(turn),
                    ));
                }
                commands.spawn((
                    Mesh3d(head.clone()),
                    MeshMaterial3d(wall.clone()),
                    Transform::from_translation(mid + Vec3::Y * (LEVEL * 0.33)).with_rotation(turn),
                ));
                commands.spawn((
                    Mesh3d(panel.clone()),
                    MeshMaterial3d(beyond.clone()),
                    Transform::from_translation(
                        out * (APOTHEM + 1.4) + Vec3::Y * (y + LEVEL * 0.5),
                    )
                    .with_rotation(turn),
                ));
            } else {
                commands.spawn((
                    Mesh3d(panel.clone()),
                    MeshMaterial3d(wall.clone()),
                    Transform::from_translation(mid).with_rotation(turn),
                ));
            }

            // A band at every level line. Twelve of them stacked is what
            // gives the eye something to count the depth with.
            commands.spawn((
                Mesh3d(course.clone()),
                MeshMaterial3d(trim.clone()),
                Transform::from_translation(out * (APOTHEM - 0.45) + Vec3::Y * y)
                    .with_rotation(turn),
            ));
        }

        // One landing per level, stepping around the hexagon a face at a
        // time, so the way down is a spiral rather than a stack.
        if level > 0 {
            let angle = (level % 6) as f32 * FRAC_PI_3;
            let out = Vec3::new(angle.sin(), 0.0, angle.cos());
            let turn = Quat::from_rotation_y(angle);
            commands.spawn((
                Mesh3d(plate.clone()),
                MeshMaterial3d(iron.clone()),
                Transform::from_translation(out * (APOTHEM - 1.7) + Vec3::Y * y)
                    .with_rotation(turn),
            ));
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(bracket.clone()),
                    MeshMaterial3d(iron.clone()),
                    Transform::from_translation(
                        out * (APOTHEM - 1.2)
                            + Vec3::Y * (y - 0.9)
                            + turn * Vec3::X * (side * SIDE * 0.6),
                    )
                    .with_rotation(turn),
                ));
            }

            // And the ladder that serves it, on the same face, climbing the
            // level below.
            let ladder = out * (APOTHEM - 0.42) + Vec3::Y * (y - LEVEL * 0.5);
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(stile.clone()),
                    MeshMaterial3d(iron.clone()),
                    Transform::from_translation(ladder + turn * Vec3::X * (side * 0.45))
                        .with_rotation(turn),
                ));
            }
            for step in 0..6 {
                let rise = (step as f32 + 0.5) * (LEVEL / 6.0) - LEVEL * 0.5;
                commands.spawn((
                    Mesh3d(rung.clone()),
                    MeshMaterial3d(iron.clone()),
                    Transform::from_translation(ladder + Vec3::Y * rise).with_rotation(turn),
                ));
            }
        }
    }

    Staging {
        eye,
        focus,
        haze: hour.haze,
        zenith: hour.zenith,
        ambient_color: hour.ambient_color,
        ambient: hour.ambient,
        fog_start: 30.0,
        fog_end: 150.0,
        water: Some(WaterPlan {
            centre: Vec3::ZERO,
            size: Vec2::splat(APOTHEM * 2.6),
            // Almost black, and reflective even head-on. There is nothing
            // under this water to see, and at the bottom of a shaft the only
            // thing worth showing in it is the light that got down there.
            tint: Color::srgb(0.04, 0.10, 0.11),
            // Far more reflective head-on than water has any right to be.
            // Physically the surface should hand back its own colour when
            // looked straight down into, which is exactly the angle needed to
            // see ninety-six metres of shaft in it - so the physics loses.
            // This is the lab where that is allowed.
            head_on: 0.62,
            ripple: 0.010,
        }),
    }
}
