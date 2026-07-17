//! Typed source for the locked authoring template and the full tile library.
//!
//! Committed `.map` assets are pinned to this generator by test — the editable
//! files can be opened in TrenchBroom, but they never drift from source. The
//! library spans every [`REGISTERS`] register x the hall / ramp / shaft / room
//! families; [`catalog::library`] is the single registry both the generated
//! files and `manifest.ron` are derived from, so they cannot disagree.
//!
//! Variant numbering (the `TileKey` schema is frozen; `variant: u16` encodes
//! direction so every key stays unique):
//! - `hall_straight`: `axis * 3 + interior` (axis 0 = E/W, 1 = SE/NW,
//!   2 = SW/NE; interior 0 = plain, 1 = colonnade, 2 = pressure).
//! - `hall_cap`, `ramp`, `shaft_landing`: the door / exit face index (0..5).
//! - `hall_corner`: `low_face * 6 + high_face`.
//! - `hall_junction`: bitmask of open faces (`1 << face_index`).
//! - shafts and rooms: 0 (unique per archetype already).

mod catalog;
mod geometry;
mod halls;
mod rooms;
mod verticals;

use observed_hex::HexFace;

pub use catalog::{manifest_ron, sources, template_map};
pub use geometry::box_brush_text;
pub use halls::{hall_cap_map, hall_corner_map, hall_junction_map, hall_straight_map};
pub use rooms::{
    room_atrium_lower_map, room_atrium_upper_map, room_double_map, room_single_map, room_wing_map,
};
pub use verticals::{
    ramp_map, shaft_bottom_cap_map, shaft_landing_map, shaft_segment_map, shaft_top_cap_map,
};

/// The nine architecture registers the library is authored for.
pub const REGISTERS: &[&str] = &[
    "shadow_screen",
    "monolith",
    "overlit_grid",
    "institutional",
    "facet_monument",
    "megastructure",
    "wellshaft",
    "infinite_gallery",
    "thinning",
];

/// Register-specific interior parameters, in TB units. `trim_height` is the
/// wainscot band along walls (0 = bare); `pylon_radius` sizes the junction
/// waypoint pylon. This is how the same floor plan reads differently per
/// district without any schema change.
#[derive(Clone, Copy)]
pub(crate) struct RegisterStyle {
    pub trim_height: f64,
    pub pylon_radius: f64,
}

pub(crate) fn register_style(register: &str) -> RegisterStyle {
    let (trim_height, pylon_radius) = match register {
        "shadow_screen" => (16.0, 10.0),
        "monolith" => (32.0, 16.0),
        "overlit_grid" => (4.0, 8.0),
        "facet_monument" => (24.0, 14.0),
        "megastructure" => (48.0, 16.0),
        "wellshaft" => (20.0, 12.0),
        "infinite_gallery" => (8.0, 10.0),
        "thinning" => (0.0, 8.0),
        // institutional and the template default: a modest dado rail.
        _ => (12.0, 12.0),
    };
    RegisterStyle {
        trim_height,
        pylon_radius,
    }
}

/// Seed tiles (Phase 89): kept byte-identical to their institutional
/// counterparts so the locked template stays a faithful teaching copy.
pub fn hall_straight_ew_map() -> String {
    hall_straight_map("institutional", 0, HexFace::East)
}

pub fn hall_cap_e_map() -> String {
    hall_cap_map("institutional", HexFace::East)
}

pub fn ramp_e_map() -> String {
    ramp_map("institutional", HexFace::East)
}

pub fn shaft_map() -> String {
    shaft_segment_map("institutional")
}

/// Regenerate the committed tile assets (only rewrites files that changed).
pub fn materialize(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, content) in sources() {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            std::fs::write(path, content)?;
        }
    }
    Ok(())
}

pub(crate) fn face_name(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "east",
        HexFace::SouthEast => "south_east",
        HexFace::SouthWest => "south_west",
        HexFace::West => "west",
        HexFace::NorthWest => "north_west",
        HexFace::NorthEast => "north_east",
        HexFace::Up => "up",
        HexFace::Down => "down",
    }
}
