//! `OBSERVED2_CAPTURE_HEX_WFC_VERTICALS`: the facility's ways up, as a body meets them.
//!
//! Evidence for the climb compositions, one district at a time: each floor is one
//! district, and each district dresses its own climb. For every district a climb starts
//! in, two poses: just inside the foot's door looking up the flight, and on the landing
//! by its door looking back down the whole of it. Chosen deterministically from the
//! solved facility, so a before and an after are the same places.
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{ClimbPart, HexArchetype, HexCoord, HexFace, HexWfcWorld};

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

/// The pose names for a district: up from the foot, and down from the landing.
const fn names(register: ArchitectureRegister) -> (&'static str, &'static str) {
    match register {
        ArchitectureRegister::LiminalGrid => ("backrooms_up", "backrooms_down"),
        ArchitectureRegister::Monolith => ("monolith_up", "monolith_down"),
        ArchitectureRegister::Institutional => ("institutional_up", "institutional_down"),
        ArchitectureRegister::Wellshaft => ("wellshaft_up", "wellshaft_down"),
        ArchitectureRegister::InfiniteGallery => ("library_up", "library_down"),
        ArchitectureRegister::OverlitGrid => ("lumen_up", "lumen_down"),
        ArchitectureRegister::ShadowScreen => ("zen_up", "zen_down"),
        ArchitectureRegister::FacetMonument => ("monument_up", "monument_down"),
        ArchitectureRegister::Megastructure => ("reactor_up", "reactor_down"),
        ArchitectureRegister::Thinning => ("sky_up", "sky_down"),
    }
}

pub(in crate::hex_wfc) fn poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    let mut seen = std::collections::BTreeSet::new();
    let mut poses = Vec::new();
    // Feet in coordinate order, so the first climb of each district is always the same.
    for placement in world.placements.values() {
        let HexArchetype::Climb {
            part: ClimbPart::Foot,
            heading,
        } = placement.archetype
        else {
            continue;
        };
        let foot = placement.coord;
        let Some(&register) = world.architecture.get(&foot) else {
            continue;
        };
        let Some(cells) = observed_facility::hex_wfc::composition_cells(
            world.config.grid(),
            foot,
            placement.archetype,
        ) else {
            continue;
        };
        if !seen.insert(register as u8) {
            continue;
        }
        let (up, down) = names(register);
        poses.push(looking_in(up, foot, heading.opposite(), 5.6, 0.15));
        poses.push(looking_in(down, cells[3], heading, 5.6, -0.3));
    }
    poses
}
