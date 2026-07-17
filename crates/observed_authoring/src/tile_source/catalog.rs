//! The single registry the committed assets are derived from: every generated
//! tile (file name, map text, key, levels, ports) in one list, so the `.map`
//! files and `manifest.ron` can never disagree.

use observed_hex::HexFace;

use super::geometry::{FLOOR_TOP, hex_slab_brush, level_units, tile_meta, wall_brush, worldspawn};
use super::halls::{hall_cap_map, hall_corner_map, hall_junction_map, hall_straight_map};
use super::rooms::{room_atrium_lower_map, room_atrium_upper_map, room_single_map, room_wing_map};
use super::verticals::{
    ramp_map, shaft_bottom_cap_map, shaft_landing_map, shaft_segment_map, shaft_top_cap_map,
};
use super::{REGISTERS, face_name};

pub(crate) struct GeneratedTile {
    pub file: String,
    pub text: String,
    pub archetype: String,
    pub register: &'static str,
    pub variant: u16,
    pub levels: u8,
    pub ports: Vec<(&'static str, &'static str)>,
}

fn door_ports(faces: &[HexFace]) -> Vec<(&'static str, &'static str)> {
    faces
        .iter()
        .map(|&face| (face_name(face), "door"))
        .collect()
}

/// Corner pairs: the six 60-degree and six 120-degree combinations (opposite
/// pairs are straights). Ordered exactly as the committed file set.
fn corner_pairs() -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for step in [1, 2] {
        for i in 0..6 {
            let j = (i + step) % 6;
            pairs.push((i.min(j), i.max(j)));
        }
    }
    pairs
}

#[allow(clippy::too_many_lines)]
pub(crate) fn library() -> Vec<GeneratedTile> {
    let mut tiles = Vec::new();
    let mut push = |file: String,
                    text: String,
                    archetype: &str,
                    register: &'static str,
                    variant: u16,
                    levels: u8,
                    ports: Vec<(&'static str, &'static str)>| {
        tiles.push(GeneratedTile {
            file,
            text,
            archetype: archetype.to_string(),
            register,
            variant,
            levels,
            ports,
        });
    };

    for &reg in REGISTERS {
        // Straight halls: three axes x three interior readings.
        for (axis, low, high) in [
            (HexFace::East, 0, 3),
            (HexFace::SouthEast, 1, 4),
            (HexFace::SouthWest, 2, 5),
        ] {
            for interior in 0..=2u16 {
                push(
                    format!("{reg}_hall_straight_{low}_{high}_v{interior}.map"),
                    hall_straight_map(reg, interior, axis),
                    "hall_straight",
                    reg,
                    axis.index() as u16 * 3 + interior,
                    1,
                    door_ports(&[axis, axis.opposite()]),
                );
            }
        }
        // Dead-end caps: one per door face.
        for (i, face) in HexFace::LATERAL.into_iter().enumerate() {
            push(
                format!("{reg}_hall_cap_{i}.map"),
                hall_cap_map(reg, face),
                "hall_cap",
                reg,
                i as u16,
                1,
                door_ports(&[face]),
            );
        }
        // Corners: every non-opposite door pair (mitered diagonals included).
        for (i, j) in corner_pairs() {
            let f1 = HexFace::LATERAL[i];
            let f2 = HexFace::LATERAL[j];
            push(
                format!("{reg}_hall_corner_{i}_{j}.map"),
                hall_corner_map(reg, f1, f2),
                "hall_corner",
                reg,
                (i * 6 + j) as u16,
                1,
                door_ports(&[f1, f2]),
            );
        }
        // Junctions: all 3-way and 4-way door combinations.
        for i in 0..6 {
            for j in (i + 1)..6 {
                for k in (j + 1)..6 {
                    let faces = [
                        HexFace::LATERAL[i],
                        HexFace::LATERAL[j],
                        HexFace::LATERAL[k],
                    ];
                    push(
                        format!("{reg}_hall_junction_{i}_{j}_{k}.map"),
                        hall_junction_map(reg, &faces),
                        "hall_junction",
                        reg,
                        (1 << i) | (1 << j) | (1 << k),
                        1,
                        door_ports(&faces),
                    );
                    for l in (k + 1)..6 {
                        let faces = [
                            HexFace::LATERAL[i],
                            HexFace::LATERAL[j],
                            HexFace::LATERAL[k],
                            HexFace::LATERAL[l],
                        ];
                        push(
                            format!("{reg}_hall_junction_{i}_{j}_{k}_{l}.map"),
                            hall_junction_map(reg, &faces),
                            "hall_junction",
                            reg,
                            (1 << i) | (1 << j) | (1 << k) | (1 << l),
                            1,
                            door_ports(&faces),
                        );
                    }
                }
            }
        }
        // Ramps: an explicit prefab per exit direction.
        for (i, exit) in HexFace::LATERAL.into_iter().enumerate() {
            let mut ports = door_ports(&[exit.opposite()]);
            ports.push(("up", "ramp_open"));
            push(
                format!("{reg}_ramp_{i}.map"),
                ramp_map(reg, exit),
                "ramp",
                reg,
                i as u16,
                2,
                ports,
            );
        }
        // Shaft family.
        push(
            format!("{reg}_shaft.map"),
            shaft_segment_map(reg),
            "shaft",
            reg,
            0,
            1,
            vec![("up", "shaft_open"), ("down", "shaft_open")],
        );
        push(
            format!("{reg}_shaft_top.map"),
            shaft_top_cap_map(reg),
            "shaft_top",
            reg,
            0,
            1,
            vec![("down", "shaft_open")],
        );
        push(
            format!("{reg}_shaft_bottom.map"),
            shaft_bottom_cap_map(reg),
            "shaft_bottom",
            reg,
            0,
            1,
            vec![("up", "shaft_open")],
        );
        for (i, face) in HexFace::LATERAL.into_iter().enumerate() {
            let mut ports = vec![("up", "shaft_open"), ("down", "shaft_open")];
            ports.extend(door_ports(&[face]));
            push(
                format!("{reg}_shaft_landing_{i}.map"),
                shaft_landing_map(reg, face),
                "shaft_landing",
                reg,
                i as u16,
                1,
                ports,
            );
        }
        // Rooms: single, blueprint strip / triangle / diamond cells, atrium.
        push(
            format!("{reg}_room_single.map"),
            room_single_map(reg),
            "room_single",
            reg,
            0,
            1,
            door_ports(&HexFace::LATERAL),
        );
        let wings: [(&str, &[HexFace]); 11] = [
            ("room_double_west", &[HexFace::East]),
            ("room_double_east", &[HexFace::West]),
            ("room_double_nw", &[HexFace::SouthEast]),
            ("room_double_se", &[HexFace::NorthWest]),
            ("room_tri_a", &[HexFace::East, HexFace::SouthEast]),
            ("room_tri_b", &[HexFace::West, HexFace::SouthWest]),
            ("room_tri_c", &[HexFace::NorthWest, HexFace::NorthEast]),
            ("room_fork_a", &[HexFace::East, HexFace::SouthEast]),
            (
                "room_fork_b",
                &[HexFace::West, HexFace::SouthWest, HexFace::SouthEast],
            ),
            (
                "room_fork_c",
                &[HexFace::NorthWest, HexFace::NorthEast, HexFace::East],
            ),
            ("room_fork_d", &[HexFace::West, HexFace::NorthWest]),
        ];
        for (archetype, open) in wings {
            let doors: Vec<HexFace> = HexFace::LATERAL
                .into_iter()
                .filter(|face| !open.contains(face))
                .collect();
            push(
                format!("{reg}_{archetype}.map"),
                room_wing_map(reg, archetype, open),
                archetype,
                reg,
                0,
                1,
                door_ports(&doors),
            );
        }
        let mut lower_ports = vec![("up", "shaft_open")];
        lower_ports.extend(door_ports(&HexFace::LATERAL));
        push(
            format!("{reg}_room_atrium_lower.map"),
            room_atrium_lower_map(reg),
            "room_atrium_lower",
            reg,
            0,
            1,
            lower_ports,
        );
        let mut upper_ports = vec![("down", "shaft_open")];
        upper_ports.extend(door_ports(&HexFace::LATERAL));
        push(
            format!("{reg}_room_atrium_upper.map"),
            room_atrium_upper_map(reg),
            "room_atrium_upper",
            reg,
            0,
            1,
            upper_ports,
        );
    }
    tiles
}

/// The locked authoring template: sealed hexagonal cell, one level.
pub fn template_map() -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    for face in HexFace::LATERAL {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from(
        "// Observed 2 hex tile template (Arc L). Copy me; keep every vertex\n// inside the canonical footprint — the importer hard-fails otherwise.\n",
    );
    out += &worldspawn(&brushes);
    out += &tile_meta("template", "institutional", 0, 1);
    out
}

/// Every generated asset: the template, the Phase 89 seed tiles, the full
/// library, and the manifest.
pub fn sources() -> Vec<(String, String)> {
    let mut s = vec![
        ("template.map".to_string(), template_map()),
        (
            "hall_straight_ew.map".to_string(),
            super::hall_straight_ew_map(),
        ),
        ("hall_cap_e.map".to_string(), super::hall_cap_e_map()),
        ("ramp_e.map".to_string(), super::ramp_e_map()),
        ("shaft.map".to_string(), super::shaft_map()),
    ];
    for tile in library() {
        s.push((tile.file, tile.text));
    }
    s.push(("manifest.ron".to_string(), manifest_ron()));
    s
}

pub fn manifest_ron() -> String {
    let mut out = String::from(
        "// Observed 2 hex tile manifest (Arc L). Tiles ARE the catalog.\n(\n    tiles: [\n",
    );
    for tile in library() {
        let ports = tile
            .ports
            .iter()
            .map(|(face, class)| format!("(face: \"{face}\", class: \"{class}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        out += &format!(
            "        (\n            key: (archetype: \"{}\", register: \"{}\", variant: {}),\n            map_path: \"{}\",\n            levels: {},\n            ports: [{ports}],\n        ),\n",
            tile.archetype, tile.register, tile.variant, tile.file, tile.levels
        );
    }
    out += "    ],\n)\n";
    out
}
