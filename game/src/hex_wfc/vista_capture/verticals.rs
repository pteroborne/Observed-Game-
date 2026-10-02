//! `OBSERVED2_CAPTURE_HEX_WFC_VERTICALS`: the facility's ways up, as a body meets them.
//!
//! Evidence for the vertical tiles. Each pose stands the runner in the cell beside a
//! ramp or a stair tower and looks in through the doorway between them, the way a body
//! arriving at it does: the foot of a ramp, the top of one looking down, the door of a
//! tower on the ground, and one high up. Chosen deterministically from the solved
//! facility, so a before and an after are the same places.
use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexWfcWorld};

use observed_hex::{FLOOR_SLAB_TOP, hex_origin};

use super::{Stage, VistaPose, face_dir};

/// Standing just inside `cell`'s open lateral `face`, `from_centre` metres out from the
/// cell's centre, looking in across the cell, eyes at `pitch`.
fn looking_in(
    name: &'static str,
    cell: HexCoord,
    face: HexFace,
    from_centre: f32,
    pitch: f32,
) -> VistaPose {
    let out = face_dir(face);
    let o = Vec3::from_array(hex_origin(cell));
    VistaPose {
        name,
        cell,
        feet: o + Vec3::new(out.x * from_centre, FLOOR_SLAB_TOP, out.y * from_centre),
        // Facing back in, the opposite way to `pose`'s look out through the face.
        yaw: (-out.x).atan2(out.y),
        pitch,
        stage: Stage::Nothing,
    }
}

/// The first open lateral face of `cell` whose neighbour opens back onto it.
fn way_in(world: &HexWfcWorld, cell: HexCoord) -> Option<HexFace> {
    let placement = world.placements.get(&cell)?;
    HexFace::LATERAL.into_iter().find(|&face| {
        placement.is_open(face)
            && world
                .config
                .grid()
                .neighbor(cell, face)
                .is_some_and(|next| {
                    world
                        .placements
                        .get(&next)
                        .is_some_and(|other| other.space.built() && other.is_open(face.opposite()))
                })
    })
}

pub(in crate::hex_wfc) fn poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    let of = |archetype: HexArchetype| {
        world
            .placements
            .values()
            .filter(move |placement| placement.archetype == archetype && placement.space.built())
            .map(|placement| placement.coord)
    };
    let mut poses = Vec::new();
    // The foot of a ramp, then the foot of one higher up.
    let ramps: Vec<HexCoord> = of(HexArchetype::RampUp).collect();
    for (name, ramp) in [
        ("ramp_foot_low", ramps.iter().min_by_key(|c| (c.level, **c))),
        (
            "ramp_foot_high",
            ramps
                .iter()
                .max_by_key(|c| (c.level, std::cmp::Reverse(**c))),
        ),
    ] {
        if let Some(&ramp) = ramp
            && let Some(face) = way_in(world, ramp)
        {
            poses.push(looking_in(name, ramp, face, 5.6, 0.3));
        }
    }
    // The top of a ramp, looking back down it from its head.
    if let Some(head) = of(HexArchetype::RampHead).min_by_key(|c| (c.level, *c))
        && let Some(face) = way_in(world, head)
    {
        poses.push(looking_in("ramp_head", head, face, 5.6, -0.35));
    }
    // A stair tower's door on the ground floor, and one high up.
    let towers: Vec<HexCoord> = of(HexArchetype::Shaft).collect();
    for (name, tower) in [
        ("tower_low", towers.iter().min_by_key(|c| (c.level, **c))),
        (
            "tower_high",
            towers
                .iter()
                .max_by_key(|c| (c.level, std::cmp::Reverse(**c))),
        ),
    ] {
        if let Some(&tower) = tower
            && let Some(face) = way_in(world, tower)
        {
            poses.push(looking_in(name, tower, face, 5.6, 0.25));
        }
    }
    poses
}
