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
//! plan here is a hexagon for the same reason - two cells across, twelve
//! levels deep. Nothing in this lab is required to honour them; a shaft is
//! simply the one place where the contract and good proportion agree.
//!
//! # What the reference photographs changed
//!
//! The first version was an empty tube, and empty was the whole problem. A
//! silo is three things this had none of.
//!
//! **A great stair.** One helix wrapping the entire shaft, a full turn to a
//! level, so every floor is reached by walking around the whole building. It
//! is the spine, and without it the shaft is a hole rather than a place.
//!
//! **An inhabited wall.** Windows in every bay, some lit and some not. A tube
//! is infrastructure; a shaft with lit windows down it is somewhere people
//! are, seen from outside their rooms.
//!
//! **Tungsten against concrete.** Standing lamps on every bay, emissive and
//! never dimmed with depth, so a lamp ninety metres down burns as hard as one
//! at the top. That warm/cold pair is the entire colour scheme, and it is
//! what makes the bottom of a dark shaft legible at all.
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
/// Two cells across rather than one. Seven metres is the catalogue's own
/// apothem, and at that width the great stair fills the shaft instead of
/// hanging in it - the reference silos are wide enough that the stair reads as
/// a ribbon against a far wall.
const APOTHEM: f32 = 14.0;
const LEVELS: i32 = 12;
/// The side of a hexagon with that apothem, which is the width of one wall.
const SIDE: f32 = APOTHEM * 1.154_700_5;
/// Treads to a full turn of the great stair, which climbs exactly one level
/// per revolution.
const STEPS_PER_TURN: i32 = 26;

const CONCRETE: Color = Color::srgb(0.56, 0.58, 0.55);
/// Soffits, string courses, the underside of everything.
const CONCRETE_DEEP: Color = Color::srgb(0.36, 0.38, 0.37);
/// Stair parapets, brackets, rails.
const IRON: Color = Color::srgb(0.25, 0.27, 0.29);
/// The one warm thing in the building, and the only thing that is not dimmed
/// with depth: a lamp ninety metres down is as bright as a lamp at the top.
/// Everything else here is grey, so the whole colour scheme is concrete
/// against tungsten.
const LAMP: LinearRgba = LinearRgba::new(8.5, 4.2, 1.4, 1.0);
/// Windows are the same light seen through something, so they are dimmer and
/// a shade further into the orange.
const WINDOW: LinearRgba = LinearRgba::new(3.4, 1.9, 0.75, 1.0);
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
            ambient: 250.0,
            sun_color: Color::srgb(1.0, 0.86, 0.64),
            sun_lux: 9_000.0,
            sun_from: Vec3::new(90.0, 110.0, 34.0),
            sun_to: Vec3::new(0.0, 72.0, 0.0),
        },
        _ => Hour {
            haze: Color::srgb(0.86, 0.90, 0.94),
            zenith: Color::srgb(0.12, 0.31, 0.72),
            ambient_color: Color::srgb(0.60, 0.71, 0.90),
            ambient: 430.0,
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
        Ok("low" | "pool") => (Vec3::new(2.0, 11.0, 2.0), Vec3::ZERO),
        // Out on a landing a third of the way up, looking back down.
        // High up and out over the void, looking down across the shaft. The
        // camera has to stay inside the stair's radius or it ends up buried
        // in a tread.
        Ok("landing") => (Vec3::new(6.2, 33.0, 4.8), Vec3::new(-4.5, 9.0, -5.0)),
        _ => (Vec3::new(2.2, 2.6, 2.6), Vec3::new(0.0, 44.0, -0.6)),
    }
}

/// How much light a level still gets: one at the opening, nearly nothing at
/// the water. The exponent is what keeps the top half bright and drops the
/// bottom away quickly, which is how a shaft actually reads.
fn reach(level: i32) -> f32 {
    let t = level as f32 / (LEVELS - 1) as f32;
    0.22 + 0.78 * t.powf(1.4)
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
    let course = meshes.add(Cuboid::new(SIDE + 0.5, 0.5, 1.6));
    let pier = meshes.add(Cuboid::new(1.3, LEVEL, 1.3));
    let plate = meshes.add(Cuboid::new(SIDE * 1.45, 0.35, 3.4));
    let parapet = meshes.add(Cuboid::new(SIDE * 1.45, 1.05, 0.22));
    let bracket = meshes.add(Cuboid::new(0.35, 1.5, 2.6));
    let frame = meshes.add(Cuboid::new(1.5, 1.05, 0.3));
    let pane = meshes.add(Cuboid::new(1.12, 0.64, 0.16));
    let fixture = meshes.add(Cuboid::new(0.18, 1.9, 0.18));

    // Light is not dimmed with depth. A lamp at the bottom is as bright as a
    // lamp at the top, which is the only reason the bottom is legible at all.
    let lamp = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.8, 0.6),
        emissive: LAMP,
        ..default()
    });
    let window_lit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.6, 0.45),
        emissive: WINDOW,
        ..default()
    });
    let window_dark = materials.add(matte(Color::srgb(0.06, 0.07, 0.09)));

    for level in 0..LEVELS {
        let y = level as f32 * LEVEL;
        let lit = reach(level);
        let wall = materials.add(matte(drowned(CONCRETE, lit)));
        let deep = materials.add(matte(drowned(CONCRETE_DEEP, lit)));
        let iron = materials.add(matte(drowned(IRON, lit)));

        for face in 0..6 {
            let angle = face as f32 * FRAC_PI_3;
            let out = Vec3::new(angle.sin(), 0.0, angle.cos());
            let turn = Quat::from_rotation_y(angle);
            let mid = out * APOTHEM + Vec3::Y * (y + LEVEL * 0.5);

            commands.spawn((
                Mesh3d(panel.clone()),
                MeshMaterial3d(wall.clone()),
                Transform::from_translation(mid).with_rotation(turn),
            ));

            // Three windows to a bay. Some are lit and some are not, which is
            // the whole difference between a shaft and a tube: a tube is
            // infrastructure, a shaft with lit windows in it is somewhere
            // people are, seen from outside their rooms.
            for slot in -2..3 {
                let across = turn * Vec3::X * (slot as f32 * SIDE * 0.175);
                commands.spawn((
                    Mesh3d(frame.clone()),
                    MeshMaterial3d(deep.clone()),
                    Transform::from_translation(mid + across - out * 0.42).with_rotation(turn),
                ));
                let occupied = (level * 7 + face * 3 + (slot + 2)) % 4 != 0;
                commands.spawn((
                    Mesh3d(pane.clone()),
                    MeshMaterial3d(if occupied {
                        window_lit.clone()
                    } else {
                        window_dark.clone()
                    }),
                    Transform::from_translation(mid + across - out * 0.56).with_rotation(turn),
                ));
            }

            // A pair of standing lamps on every bay. Twelve levels of them
            // receding is what gives the depth a rhythm to be counted in.
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(fixture.clone()),
                    MeshMaterial3d(lamp.clone()),
                    Transform::from_translation(
                        mid + turn * Vec3::X * (side * SIDE * 0.44) - out * 0.62
                            + Vec3::Y * (LEVEL * 0.1),
                    )
                    .with_rotation(turn),
                ));
            }

            commands.spawn((
                Mesh3d(course.clone()),
                MeshMaterial3d(deep.clone()),
                Transform::from_translation(out * (APOTHEM - 0.45) + Vec3::Y * y)
                    .with_rotation(turn),
            ));

            // A pier on every corner, running the full height of the level.
            // For a regular hexagon the side and the circumradius are the same
            // number, so SIDE places these without any further arithmetic.
            let corner = angle + FRAC_PI_3 * 0.5;
            commands.spawn((
                Mesh3d(pier.clone()),
                MeshMaterial3d(deep.clone()),
                Transform::from_translation(
                    Vec3::new(corner.sin(), 0.0, corner.cos()) * (SIDE - 0.5)
                        + Vec3::Y * (y + LEVEL * 0.5),
                )
                .with_rotation(Quat::from_rotation_y(corner)),
            ));
        }

        // One balcony per level, stepping around the hexagon a face at a time.
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
            // Solid, not a rail. Every balcony in the reference is a wall you
            // lean on rather than a fence you see through, and it reads as
            // mass at this distance where a railing would disappear.
            commands.spawn((
                Mesh3d(parapet.clone()),
                MeshMaterial3d(deep.clone()),
                Transform::from_translation(out * (APOTHEM - 3.3) + Vec3::Y * (y + 0.5))
                    .with_rotation(turn),
            ));
            for side in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(bracket.clone()),
                    MeshMaterial3d(deep.clone()),
                    Transform::from_translation(
                        out * (APOTHEM - 1.2)
                            + Vec3::Y * (y - 0.9)
                            + turn * Vec3::X * (side * SIDE * 0.6),
                    )
                    .with_rotation(turn),
                ));
            }
            // One real light per level, so the lamps actually put something
            // warm onto the concrete instead of only glowing.
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.72, 0.42),
                    intensity: 520_000.0,
                    range: 26.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(out * (APOTHEM - 2.2) + Vec3::Y * (y + 2.2)),
            ));
        }
    }

    // The great stair. It is the one thing that makes this a silo rather than
    // a hole: a single helix wrapping the whole shaft, one turn to a level, so
    // that every floor is reached by walking around the entire building.
    let tread = meshes.add(Cuboid::new(2.9, 0.20, 2.5));
    // Wider than a tread on purpose: consecutive parapets have to overlap
    // or the outer edge of the stair reads as a ring of separate plates
    // rather than as one continuous ribbon winding down the wall.
    let riser = meshes.add(Cuboid::new(3.3, 1.30, 0.26));
    let radius = APOTHEM - 2.6;
    for step in 0..(STEPS_PER_TURN * LEVELS) {
        let turns = step as f32 / STEPS_PER_TURN as f32;
        let theta = turns * std::f32::consts::TAU;
        let y = turns * LEVEL;
        let lit = reach((y / LEVEL) as i32);
        let spin = Quat::from_rotation_y(theta);
        let at = Vec3::new(theta.sin(), 0.0, theta.cos()) * radius + Vec3::Y * y;
        commands.spawn((
            Mesh3d(tread.clone()),
            MeshMaterial3d(materials.add(matte(drowned(CONCRETE, lit)))),
            Transform::from_translation(at).with_rotation(spin),
        ));
        commands.spawn((
            Mesh3d(riser.clone()),
            MeshMaterial3d(materials.add(matte(drowned(IRON, lit)))),
            Transform::from_translation(
                at + Vec3::new(theta.sin(), 0.0, theta.cos()) * 1.35 + Vec3::Y * 0.55,
            )
            .with_rotation(spin),
        ));
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
