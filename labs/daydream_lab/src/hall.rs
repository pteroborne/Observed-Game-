//! The colonnade: the scene this lab was built to answer a question about.
//!
//! A colonnade standing in water, roofed over the nave and open at the
//! aisles. Two arched walls, one seen through the other - one opening onto
//! haze is a wall with a hole in it; a second opening seen through the first
//! is a building that continues. A flight of steps climbing out of the water
//! to the first threshold, its lowest treads submerged.
//!
//! # Two hours, not one setting
//!
//! `noon` is ambient-led and shadowless, opening onto a cold white void.
//! `late` collapses the ambient and rakes one warm sun across the colonnade,
//! so the piers stripe the aisle. Neither is a variation on the other.
//!
//! # Shadows are off at noon on purpose
//!
//! A cascade boundary drew a hard seam across the hall, and everything past
//! it blew out. Losing shadows cost nothing at noon, because that look is
//! ambient-led: form comes from which way a face is turned, not from what is
//! thrown onto it. `late` needs them, and gets cascades long enough to reach
//! past the far wall so the seam has nowhere to fall.
//!
//! # What volumetric light shafts ran into
//!
//! Shafts were tried here and taken back out. Bevy will draw them -
//! `FogVolume` plus `VolumetricLight` plus `VolumetricFog` on the camera -
//! but this building cannot produce them, because the nave has no side walls.
//! The aisles are open, so the sun arrives as an unbroken wash rather than
//! through openings, and there is nothing to cut it into bars of lit air.
//! Every density that made the air visible also filled the arch with murk.
//! Shafts need an opening to come through; see [`crate::shaft`], which is
//! nothing but an opening.

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

use crate::{Staging, WaterPlan, checkerboard, checkered, matte};

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
/// The second wall, seen through the first.
const SECOND: f32 = -58.0;
/// The floor level of the room between the two walls, and the top of the
/// flight of steps that climbs to it out of the water.
const LANDING: f32 = 1.9;
/// Half-width of the roofed nave. Outside it the aisles are open to the sky,
/// so the colonnade is backlit and the haze leaks in between the piers.
const NAVE: f32 = 5.9;

const FOG_START: f32 = 40.0;
const FOG_END: f32 = 130.0;

struct Hour {
    haze: Color,
    zenith: Color,
    ambient_color: Color,
    ambient: f32,
    sun_color: Color,
    sun_lux: f32,
    sun_from: Vec3,
    sun_to: Vec3,
    shadows: bool,
}

fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        Ok("late") => Hour {
            haze: Color::srgb(0.98, 0.87, 0.74),
            zenith: Color::srgb(0.14, 0.17, 0.44),
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
            zenith: Color::srgb(0.05, 0.18, 0.52),
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

/// Where to stand. The default is the one down the axis, which is the
/// composition the scene was built for.
fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Down at the waterline, where the reflection is longer than the hall.
        Ok("low") => (Vec3::new(0.0, 1.35, 17.0), Vec3::new(0.0, 4.8, FAR)),
        // From an aisle, so the colonnade is read across rather than through.
        Ok("aisle") => (Vec3::new(10.5, 3.2, 7.0), Vec3::new(-2.0, 4.6, -20.0)),
        _ => (Vec3::new(0.0, 2.7, 20.0), Vec3::new(0.0, 5.2, FAR)),
    }
}

/// One wall with a round-headed opening cut through it.
struct Arch {
    z: f32,
    /// The height the opening starts from, so a wall can stand on a landing.
    sill: f32,
    radius: f32,
    /// How far above the sill the arch springs.
    spring: f32,
    wall: Handle<StandardMaterial>,
    ring: Handle<StandardMaterial>,
}

fn arched_wall(commands: &mut Commands, meshes: &mut Assets<Mesh>, arch: &Arch) {
    let spring = arch.sill + arch.spring;
    let jamb = meshes.add(Cuboid::new(1.0, arch.spring, 1.2));
    let flank = meshes.add(Cuboid::new(5.0, 15.0, 1.2));
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            Mesh3d(flank.clone()),
            MeshMaterial3d(arch.wall.clone()),
            Transform::from_xyz(side * (arch.radius + 2.5), arch.sill + 7.5, arch.z),
        ));
        commands.spawn((
            Mesh3d(jamb.clone()),
            MeshMaterial3d(arch.wall.clone()),
            Transform::from_xyz(
                side * (arch.radius + 0.5),
                arch.sill + arch.spring * 0.5,
                arch.z,
            ),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(arch.radius * 2.0 + 2.0, 5.6, 1.2))),
        MeshMaterial3d(arch.wall.clone()),
        Transform::from_xyz(0.0, spring + arch.radius + 2.8, arch.z),
    ));
    // The ring itself: short chords swept over a half circle.
    let voussoir = meshes.add(Cuboid::new(0.66, 1.05, 1.3));
    for i in 0..17 {
        let t = i as f32 / 16.0;
        let a = std::f32::consts::PI * t;
        commands.spawn((
            Mesh3d(voussoir.clone()),
            MeshMaterial3d(arch.ring.clone()),
            Transform::from_xyz(
                -a.cos() * arch.radius,
                spring + a.sin() * arch.radius,
                arch.z,
            )
            .with_rotation(Quat::from_rotation_z(-a + std::f32::consts::FRAC_PI_2)),
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
            color: hour.sun_color,
            illuminance: hour.sun_lux,
            shadow_maps_enabled: hour.shadows,
            ..default()
        },
        // Cascades that reach past the far wall. The seam that made shadows
        // unusable at first was a cascade boundary falling inside the hall,
        // with everything past it unshadowed and blown out.
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

    let floor_tex = checkerboard(images, [224, 180, 162], [186, 118, 100]);
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

    // Two arched walls, one behind the other. Nothing is modelled past the
    // second - what shows through both openings is the sky, so the distance
    // never resolves into anything you could walk to.
    arched_wall(
        commands,
        meshes,
        &Arch {
            z: FAR,
            sill: 0.0,
            radius: ARCH_R,
            spring: SPRING,
            wall: teal.clone(),
            ring: salmon_deep.clone(),
        },
    );
    arched_wall(
        commands,
        meshes,
        &Arch {
            z: SECOND,
            sill: LANDING,
            // Smaller than the first, not larger. What has to be true is that
            // the whole of the second ring falls inside the cone the first
            // opening allows through, or it reads as nothing at all - an
            // earlier pass made it wider and the only visible result was that
            // the sky beyond changed shape.
            radius: 2.6,
            spring: 3.2,
            wall: salmon_deep.clone(),
            ring: teal.clone(),
        },
    );

    // The floor of the room between the two walls, and the flight of steps
    // that climbs to it out of the water. The lowest treads are under the
    // surface, which is what makes the water read as having a depth rather
    // than being a plane with things standing on it.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(HALL_HALF * 2.0, 1.8, 39.0))),
        MeshMaterial3d(salmon.clone()),
        Transform::from_xyz(0.0, LANDING - 0.9, -44.5),
    ));
    const TREADS: i32 = 7;
    for i in 0..TREADS {
        let rise = LANDING * (TREADS - i) as f32 / TREADS as f32;
        let z = -24.4 + i as f32 * 1.3;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(7.0, rise + 1.6, 1.3))),
            MeshMaterial3d(if i % 2 == 0 {
                salmon.clone()
            } else {
                salmon_deep.clone()
            }),
            Transform::from_xyz(0.0, rise - (rise + 1.6) * 0.5, z),
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

    Staging {
        eye,
        focus,
        haze: hour.haze,
        zenith: hour.zenith,
        ambient_color: hour.ambient_color,
        ambient: hour.ambient,
        fov: std::f32::consts::FRAC_PI_4,
        fog_start: FOG_START,
        fog_end: FOG_END,
        water: Some(WaterPlan {
            centre: Vec3::new(0.0, 0.0, -45.0),
            size: Vec2::new(CHANNEL * 2.0, 150.0),
            tint: Color::srgb(0.07, 0.30, 0.34),
            head_on: 0.10,
            ripple: 0.0075,
        }),
    }
}
