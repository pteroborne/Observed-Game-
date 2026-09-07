//! The Facet Monument, which the fiction calls The Welcome.
//!
//! Built for a witness so important the building would never dare change in
//! front of them. The axis runs eleven cells without deviating and terminates
//! in a raised dais with nothing on it - not *nothing left on it*, nothing was
//! ever on it. The dais is the point. It is a place to be seen standing, and
//! the guest did not come.
//!
//! # What the reference frames actually show
//!
//! Forerunner interiors are not "sci-fi temple", and they are not the stepped
//! sandstone mass I first assumed. Four things carry them:
//!
//! **Nothing is vertical.** The ribs are A-frames whose legs splay outward at
//! the floor and taper as they rise, and the taper is stepped rather than
//! smooth - three courses to a leg, each narrower than the one below.
//!
//! **The plan is chevroned.** Walkways are arrowheads and dog-legs cantilevered
//! out over a void, never rectangles. It is the single most identifying thing
//! about the geometry and the easiest to leave out.
//!
//! **Large faces are inscribed.** Every big flat surface carries recessed
//! panelling, which is what keeps a twelve-metre wall from reading as a
//! twelve-metre box.
//!
//! **Colour is complementary and lighting is flat.** Jade-green structure
//! against khaki floors and olive walls, hazy and low-contrast, punctured by a
//! very few tiny saturated points - magenta, cyan - that are the only
//! saturated things in the frame.
//!
//! # There are no lamps
//!
//! The fiction is explicit: a lamp is an object and an object has a size, and
//! whoever built this did not want you to know what size anything was. So
//! every light here is a strip recessed in the reveal between two masses.
//! Nothing is lit by a fixture you could point at.

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

use crate::{Staging, WaterPlan, matte};

/// The catalogue's cell, and the fiction's count of them.
const CELL: f32 = 8.0;
const CELLS: i32 = 11;
/// Half-width of the nave, floor to first wall course.
const HALF: f32 = 9.0;
/// Where the axis stops.
const DAIS_Z: f32 = -CELL * CELLS as f32 + 4.0;

/// Floors and walkways.
const KHAKI: Color = Color::srgb(0.63, 0.56, 0.33);
/// The wall mass. The register's own palette, which is bone rather than grey.
const BONE: Color = Color::srgb(0.60, 0.55, 0.44);
const BONE_DEEP: Color = Color::srgb(0.29, 0.27, 0.22);
/// The ribs, and the only cool thing in the building.
const JADE: Color = Color::srgb(0.17, 0.43, 0.37);
const JADE_DEEP: Color = Color::srgb(0.10, 0.26, 0.23);

/// Light in the joint: warm, wide, and never a visible fixture.
const SEAM: LinearRgba = LinearRgba::new(2.1, 1.9, 1.45, 1.0);
/// The two saturated points. There are almost none of them on purpose.
const MAGENTA: LinearRgba = LinearRgba::new(6.5, 0.6, 3.0, 1.0);
const CYAN: LinearRgba = LinearRgba::new(0.5, 3.0, 5.2, 1.0);

struct Hour {
    haze: Color,
    zenith: Color,
    ambient_color: Color,
    ambient: f32,
    sun_color: Color,
    sun_lux: f32,
}

/// `noon` is the room as it was meant to be seen - lit for a guest, the
/// longest view in the facility. `late` withdraws the fill until the seams are
/// doing all of it, which is the room admitting nobody is coming.
fn hour() -> Hour {
    match std::env::var("OBSERVED2_DAYDREAM_HOUR").as_deref() {
        Ok("late") => Hour {
            haze: Color::srgb(0.34, 0.33, 0.29),
            zenith: Color::srgb(0.10, 0.12, 0.13),
            ambient_color: Color::srgb(0.44, 0.48, 0.52),
            ambient: 95.0,
            sun_color: Color::srgb(0.86, 0.86, 0.82),
            sun_lux: 900.0,
        },
        _ => Hour {
            haze: Color::srgb(0.70, 0.68, 0.56),
            zenith: Color::srgb(0.36, 0.38, 0.34),
            ambient_color: Color::srgb(0.80, 0.80, 0.74),
            ambient: 430.0,
            sun_color: Color::srgb(0.96, 0.94, 0.86),
            sun_lux: 2_600.0,
        },
    }
}

fn view() -> (Vec3, Vec3) {
    match std::env::var("OBSERVED2_DAYDREAM_VIEW").as_deref() {
        // Low on the axis, so the polished floor doubles the whole colonnade.
        Ok("low") => (Vec3::new(0.0, 1.15, 22.0), Vec3::new(0.0, 4.4, DAIS_Z)),
        // Out on one of the cantilevered walkways, looking back across.
        Ok("gallery") => (Vec3::new(11.5, 8.4, -12.0), Vec3::new(-2.0, 3.4, -52.0)),
        _ => (Vec3::new(0.0, 3.6, 27.0), Vec3::new(0.0, 5.0, DAIS_Z)),
    }
}

/// One A-frame rib. The legs splay at the floor, step inward in three courses
/// as they rise, and carry a lintel across the top.
fn rib(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    jade: &Handle<StandardMaterial>,
    jade_deep: &Handle<StandardMaterial>,
    z: f32,
) {
    const FOOT: f32 = 7.6;
    const HEAD: f32 = 4.9;
    const RISE: f32 = 11.0;
    let lean = ((FOOT - HEAD) / RISE).atan();
    // Three courses to a leg, each narrower than the one below. A single box
    // would be a leaning post; the steps are what make it read as tapered.
    const COURSES: [(f32, f32); 3] = [(1.65, 1.95), (1.28, 1.7), (0.96, 1.5)];
    for side in [-1.0_f32, 1.0] {
        for (i, (width, depth)) in COURSES.iter().enumerate() {
            let t = (i as f32 + 0.5) / 3.0;
            let y = t * RISE;
            let x = side * (FOOT - (FOOT - HEAD) * t);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(*width, RISE / 3.0 + 0.06, *depth))),
                MeshMaterial3d(jade.clone()),
                Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_z(side * lean)),
            ));
            // The inscribed panel. Every large face carries one.
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.1, RISE / 5.0, depth * 0.5))),
                MeshMaterial3d(jade_deep.clone()),
                Transform::from_xyz(x - side * (width * 0.5 + 0.04), y, z)
                    .with_rotation(Quat::from_rotation_z(side * lean)),
            ));
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(HEAD * 2.0 + 1.1, 1.35, 2.0))),
        MeshMaterial3d(jade.clone()),
        Transform::from_xyz(0.0, RISE + 0.55, z),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(HEAD * 2.0 - 1.4, 0.42, 2.3))),
        MeshMaterial3d(jade_deep.clone()),
        Transform::from_xyz(0.0, RISE + 0.05, z),
    ));
}

/// A walkway cantilevered out over the void, bent into an arrowhead rather
/// than run straight. Two slabs meeting at an angle, which is the whole trick.
fn chevron(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    khaki: &Handle<StandardMaterial>,
    deep: &Handle<StandardMaterial>,
    at: Vec3,
    point_left: bool,
) {
    let slab = meshes.add(Cuboid::new(11.0, 0.55, 4.6));
    let soffit = meshes.add(Cuboid::new(10.0, 0.5, 3.0));
    let sweep = if point_left { 1.0 } else { -1.0 };
    for wing in [-1.0_f32, 1.0] {
        let yaw = sweep * wing * 0.42;
        let along = Quat::from_rotation_y(yaw) * Vec3::X * (wing * 5.2);
        commands.spawn((
            Mesh3d(slab.clone()),
            MeshMaterial3d(khaki.clone()),
            Transform::from_translation(at + along).with_rotation(Quat::from_rotation_y(yaw)),
        ));
        commands.spawn((
            Mesh3d(soffit.clone()),
            MeshMaterial3d(deep.clone()),
            Transform::from_translation(at + along - Vec3::Y * 0.5)
                .with_rotation(Quat::from_rotation_y(yaw)),
        ));
    }
}

#[allow(clippy::too_many_lines)]
pub fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Staging {
    let hour = hour();
    let (eye, focus) = view();
    let length = CELL * CELLS as f32 + 20.0;
    let mid = DAIS_Z * 0.5 + 6.0;

    // A weak, high, shadowless fill. The seams are the light here; this only
    // keeps two faces of a rib from being the same value.
    commands.spawn((
        DirectionalLight {
            color: hour.sun_color,
            illuminance: hour.sun_lux,
            shadow_maps_enabled: false,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.4,
            first_cascade_far_bound: 26.0,
            maximum_distance: 220.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_xyz(18.0, 40.0, 22.0).looking_at(Vec3::new(0.0, 2.0, mid), Vec3::Y),
    ));

    let khaki = materials.add(matte(KHAKI));
    let bone = materials.add(matte(BONE));
    let bone_deep = materials.add(matte(BONE_DEEP));
    let jade = materials.add(matte(JADE));
    let jade_deep = materials.add(matte(JADE_DEEP));
    let seam = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.82, 0.72),
        emissive: SEAM,
        ..default()
    });
    let magenta = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.2, 0.5),
        emissive: MAGENTA,
        ..default()
    });
    let cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.6, 0.8),
        emissive: CYAN,
        ..default()
    });

    // The walls: four courses stepping back as they rise, with a lit reveal
    // between each pair. Scale reads from the number of steps, and from
    // nothing else - there is no object in here of a known size.
    for side in [-1.0_f32, 1.0] {
        for course in 0..3 {
            let c = course as f32;
            let x = side * (HALF + 1.4 + c * 1.15);
            let y = 1.7 + c * 3.5;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(2.4, 3.4, length))),
                MeshMaterial3d(if course % 2 == 0 {
                    bone.clone()
                } else {
                    bone_deep.clone()
                }),
                Transform::from_xyz(x, y, mid),
            ));
            // The light is in the joint.
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.3, 0.22, length - 3.0))),
                MeshMaterial3d(seam.clone()),
                Transform::from_xyz(x - side * 1.25, y + 1.78, mid),
            ));
            // Inscribed panelling, so a wall this size is not read as a box.
            for bay in 0..CELLS {
                let z = 4.0 - bay as f32 * CELL;
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.14, 1.9, 3.4))),
                    MeshMaterial3d(bone_deep.clone()),
                    Transform::from_xyz(x - side * 1.22, y - 0.3, z),
                ));
            }
        }
        // The very few saturated things in the room.
        for bay in 0..4 {
            let z = -6.0 - bay as f32 * 22.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.16, 1.1, 0.5))),
                MeshMaterial3d(if bay % 2 == 0 {
                    magenta.clone()
                } else {
                    cyan.clone()
                }),
                Transform::from_xyz(side * (HALF + 0.2), 2.4, z),
            ));
        }
    }

    // A dark lid. The room is interior, and the ribs need something to be
    // silhouetted against that is not sky.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(HALF * 2.0 + 12.0, 1.2, length))),
        MeshMaterial3d(bone_deep.clone()),
        Transform::from_xyz(0.0, 14.6, mid),
    ));

    // Eleven ribs, one to a cell, holding the axis without deviating.
    for bay in 0..CELLS {
        rib(commands, meshes, &jade, &jade_deep, 4.0 - bay as f32 * CELL);
    }

    // Walkways over the void, bent into arrowheads.
    // Cantilevered from one side each, not spanning the nave. A walkway
    // across the axis is a bridge and closes the view; a walkway reaching out
    // from one wall leaves the axis open and still crosses the frame.
    chevron(
        commands,
        meshes,
        &khaki,
        &bone_deep,
        Vec3::new(-6.4, 6.6, -22.0),
        true,
    );
    chevron(
        commands,
        meshes,
        &khaki,
        &bone_deep,
        Vec3::new(6.4, 10.4, -50.0),
        false,
    );

    // The terminus: three courses shrinking to a flat top, and a slot of light
    // in the wall behind it. Nothing stands on it.
    for (i, (w, y)) in [(11.0_f32, 0.55_f32), (8.2, 1.55), (5.8, 2.5)]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w, 1.0, w))),
            MeshMaterial3d(if i == 2 { khaki.clone() } else { bone.clone() }),
            Transform::from_xyz(0.0, y, DAIS_Z),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(HALF * 2.0 + 10.0, 16.0, 2.0))),
        MeshMaterial3d(bone.clone()),
        Transform::from_xyz(0.0, 8.0, DAIS_Z - 9.0),
    ));
    // A wide slot of light behind the dais. The axis has to terminate in
    // something brighter than the air, or eleven cells of fog simply eat it
    // and the room ends in nothing rather than in the point of itself.
    for (y, w) in [(6.6_f32, 1.9_f32), (10.4, 0.55)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(HALF * 1.6, w, 0.4))),
            MeshMaterial3d(seam.clone()),
            Transform::from_xyz(0.0, y, DAIS_Z - 7.9),
        ));
    }
    // And one real light on the dais itself, the only fixture in the building,
    // hidden inside the reveal under its top course.
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.95, 0.84),
            intensity: 900_000.0,
            range: 34.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, DAIS_Z + 1.0),
    ));

    Staging {
        eye,
        focus,
        haze: hour.haze,
        zenith: hour.zenith,
        ambient_color: hour.ambient_color,
        ambient: hour.ambient,
        fov: std::f32::consts::FRAC_PI_4,
        fog_start: 52.0,
        fog_end: 240.0,
        water: Some(WaterPlan {
            centre: Vec3::new(0.0, 0.0, mid),
            size: Vec2::new(HALF * 2.0, length),
            // Not water: a polished floor. Still, so the ripple is zero, and
            // dark enough that the colonnade doubling in it stays a reflection
            // rather than becoming a second room.
            tint: Color::srgb(0.075, 0.07, 0.045),
            head_on: 0.12,
            ripple: 0.0,
        }),
    }
}
