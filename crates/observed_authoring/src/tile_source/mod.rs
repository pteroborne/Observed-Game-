//! Typed source for the locked authoring template and the full tile library.
//!
//! The library spans every [`REGISTERS`] register x the grounded hall / ramp /
//! stair-tower / room families. It supplies the topology-complete generic collision kit
//! beneath curated production modules, keeping exact WFC port coverage in code
//! without committing hundreds of mechanically derived `.map` files.
//!
//! Variant numbering (the `TileKey` schema is frozen; `variant: u16` encodes
//! direction so every key stays unique):
//! - `hall_straight`: `axis * 3 + interior` (axis 0 = E/W, 1 = SE/NW,
//!   2 = SW/NE; interior 0 = plain, 1 = colonnade, 2 = pressure).
//! - `hall_cap`, `ramp`: the door / exit face index (0..5).
//! - `hall_corner`: `low_face * 6 + high_face`.
//! - `hall_junction`: bitmask of open faces (`1 << face_index`).
//! - rooms: 0 (unique per archetype already).

mod catalog;
mod geometry;
mod halls;
mod rooms;
mod verticals;

use observed_hex::HexFace;

pub use catalog::{manifest_ron, sources, template_map};
pub use geometry::box_brush_text;
pub(crate) use geometry::hex_slab_brush;
pub use halls::{hall_cap_map, hall_corner_map, hall_junction_map, hall_straight_map};
pub use rooms::{
    room_atrium_lower_map, room_atrium_upper_map, room_double_map, room_single_map, room_wing_map,
};
pub use verticals::{
    ramp_map, stair_bottom_cap_map, stair_landing_map, stair_segment_map, stair_top_cap_map,
};

/// One complete, generic collision kit for the canonical WFC alphabet.
///
/// Production-authored modules may deliberately cover only a curated subset of
/// signatures. These generated cells preserve the hard topology/geometry
/// contract underneath that subset without requiring hundreds of derived `.map`
/// files in the repository.
///
/// **One kit per register**, since Arc O Phase 110. It used to be a single
/// institutional-derived library relabelled `generic`, consulted whenever an
/// exact register tile was missing — and since only Liminal Grid had authored
/// modules, "whenever" meant nearly always. Nine of the ten districts were built
/// entirely out of one district's geometry, which is why they read as one place
/// however they were lit or composed.
///
/// Each register now generates its own library through [`register_style`], so a
/// district's dado, its junction pylons and its stair towers are its own. The
/// `generic` copy stays underneath as a net for a register nothing was generated
/// for, and authored `.map` modules still outrank both.
pub fn compatibility_cells() -> Result<Vec<crate::TilePrototype>, crate::TileError> {
    let convert = |source: catalog::GeneratedTile, register: Option<&str>| {
        let mut tile = crate::parse_tile(&source.text)?;
        if let Some(register) = register {
            tile.key.register = register.to_string();
        }
        tile.key.archetype = compatibility_archetype(&tile).to_string();
        Ok(tile)
    };

    let mut cells = catalog::library_for(&["institutional"])
        .into_iter()
        .map(|source| convert(source, Some("generic")))
        .collect::<Result<Vec<_>, crate::TileError>>()?;
    for &register in REGISTERS {
        for source in catalog::library_for(&[register]) {
            cells.push(convert(source, None)?);
        }
    }
    Ok(cells)
}

fn compatibility_archetype(tile: &crate::TilePrototype) -> &'static str {
    match tile.key.archetype.as_str() {
        "hall_corner" => {
            let faces = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == observed_hex::PortClass::Door)
                .map(HexFace::index)
                .collect::<Vec<_>>();
            let distance = faces[0].abs_diff(faces[1]);
            if distance.min(6 - distance) == 1 {
                "hall_turn_60"
            } else {
                "hall_turn_120"
            }
        }
        "hall_junction" => {
            let doors = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == observed_hex::PortClass::Door)
                .count();
            if doors == 3 {
                "hall_junction_3way"
            } else {
                "hall_junction_4way"
            }
        }
        "ramp" => "hall_ramp",
        "stair_segment" | "stair_top" | "stair_bottom" | "stair_landing" => "stair_tower",
        // Room-cell geometry keeps the name it was generated under.
        //
        // It used to be flattened to `sanctuary` here, which was the second half
        // of backlog #15: even once a blueprint asked for `room_tri_b`, the kit
        // had already relabelled every wing to the same single-hex shape on the
        // way in, so the demand could not have been met even by accident. The
        // authored whole-room `.map` modules are a different path and still map
        // to `sanctuary` below.
        archetype if ROOM_CELL_ARCHETYPES.contains(&archetype) => ROOM_CELL_ARCHETYPES
            .iter()
            .find(|name| **name == archetype)
            .expect("just matched"),
        archetype if archetype.starts_with("room_") => "sanctuary",
        "hall_straight" => "hall_straight",
        "hall_cap" => "hall_cap",
        "expanse" => "expanse",
        unexpected => panic!("unmapped compatibility archetype {unexpected}"),
    }
}

/// The nine architecture registers the library is authored for.
/// The per-cell room geometry the blueprints ask for, generated in every
/// register. Kept as a list so [`compatibility_archetype`] can hand each name
/// straight through with a `'static` lifetime instead of leaking a `String`.
pub(crate) const ROOM_CELL_ARCHETYPES: &[&str] = &[
    "room_single",
    "room_double_west",
    "room_double_east",
    "room_double_nw",
    "room_double_se",
    "room_tri_a",
    "room_tri_b",
    "room_tri_c",
    "room_fork_a",
    "room_fork_b",
    "room_fork_c",
    "room_fork_d",
    "room_atrium_lower",
    "room_atrium_upper",
];

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
    // Liminal Grid was absent for the whole of Arc L and Arc M, on the reasoning
    // that it is the one district authored as `.map` modules and so needs no
    // generated kit. That held only while the authored corpus covered every
    // demand the solver could make. It stopped holding the moment `Expanse`
    // arrived (backlog #20): the district whose identity *is* open space had no
    // exact tiles for it and fell through to a fallback drawn in another
    // district's style. A generated kit under authored modules is a floor, not a
    // replacement — authored tiles outweigh it and still win selection.
    "liminal_grid",
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
        // Vast and open: the lowest trim of any district that has one, and slim
        // pylons, so nothing in a Liminal hall interrupts the run of the walls.
        "liminal_grid" => (6.0, 9.0),
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
