//! Typed source for the locked authoring template and the seed tiles.
//!
//! Committed `.map` assets are pinned to this generator by test — the editable
//! files can be opened in TrenchBroom, but they never drift from source.
//! Coordinates are TrenchBroom units (Z-up, [`crate::UNITS_PER_METER`] per
//! meter); the canonical hexagon corners land on integers by construction.
//!
//! Seed-tile limitation (Phase 89): doorways are authored only on the flat
//! East/West faces (axis-aligned jambs). Diagonal-face doors need mitered
//! pieces and arrive with the full tile library (Phase 91).

use observed_hex::{HexFace, TILE_LEVEL_HEIGHT, face_edge};

use crate::UNITS_PER_METER;

/// One level's height in TrenchBroom units (128).
fn level_units() -> f64 {
    f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER
}

/// Canonical corner in TB units for a lateral face's edge (A, B order).
fn tb_edge(face: HexFace) -> ([f64; 2], [f64; 2]) {
    let [a, b] = face_edge(face);
    let convert = |(x, z): (i32, i32)| {
        [
            f64::from(x) * UNITS_PER_METER,
            f64::from(-z) * UNITS_PER_METER,
        ]
    };
    (convert(a), convert(b))
}

fn fmt(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn plane_line(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> String {
    format!(
        "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
        fmt(a[0]),
        fmt(a[1]),
        fmt(a[2]),
        fmt(b[0]),
        fmt(b[1]),
        fmt(b[2]),
        fmt(c[0]),
        fmt(c[1]),
        fmt(c[2])
    )
}

/// Plane through the vertical quad (a2, b2) x [z0, z1], wound so the outward
/// normal points away from `interior_hint` (a 2D point inside the solid).
fn side_plane(a2: [f64; 2], b2: [f64; 2], z0: f64, interior_hint: [f64; 2]) -> String {
    let a = [a2[0], a2[1], z0];
    let b = [b2[0], b2[1], z0];
    let c = [a2[0], a2[1], z0 + 64.0];
    // normal = cross(c - a, b - a) = (-h*dy, h*dx, 0) with d = b - a.
    let normal = [-(b2[1] - a2[1]), b2[0] - a2[0]];
    let toward_hint = [interior_hint[0] - a2[0], interior_hint[1] - a2[1]];
    if normal[0] * toward_hint[0] + normal[1] * toward_hint[1] > 0.0 {
        // Normal points inward — flip winding.
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

/// Horizontal plane at `z`, outward normal up (`top = true`) or down.
fn flat_plane(z: f64, top: bool) -> String {
    let a = [0.0, 0.0, z];
    let b = [64.0, 0.0, z];
    let c = [0.0, 64.0, z];
    // cross(c-a, b-a) = (0,0,-64*64) points down; (a b c) order is down.
    if top {
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

/// A full-footprint hexagonal slab from z0 to z1 (floor/ceiling).
fn hex_slab_brush(z0: f64, z1: f64) -> String {
    let mut out = String::from("{\n");
    for face in HexFace::LATERAL {
        let (a, b) = tb_edge(face);
        out += &side_plane(a, b, z0, [0.0, 0.0]);
    }
    out += &flat_plane(z0, false);
    out += &flat_plane(z1, true);
    out += "}\n";
    out
}

/// Inward offset of a 2D edge by `t` units (both endpoints moved along the
/// inward normal).
fn offset_inward(a: [f64; 2], b: [f64; 2], t: f64) -> ([f64; 2], [f64; 2]) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let outward = [-d[1] / length, d[0] / length];
    // The hexagon center is the origin; outward has positive dot with a.
    let sign = if outward[0] * a[0] + outward[1] * a[1] > 0.0 {
        -1.0
    } else {
        1.0
    };
    let m = [outward[0] * sign * t, outward[1] * sign * t];
    ([a[0] + m[0], a[1] + m[1]], [b[0] + m[0], b[1] + m[1]])
}

/// Wall thickness in TB units (0.5 m).
const WALL: f64 = 8.0;

/// A full sealed wall on `face`, mitered against its neighbor face planes.
fn wall_brush(face: HexFace, z0: f64, z1: f64) -> String {
    let (a, b) = tb_edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let lateral = HexFace::LATERAL;
    let prev = lateral[(face.index() + 5) % 6];
    let next = lateral[(face.index() + 1) % 6];
    let (pa, pb) = tb_edge(prev);
    let (na, nb) = tb_edge(next);
    let mid = [(ia[0] + ib[0]) * 0.5, (ia[1] + ib[1]) * 0.5];
    let mut out = String::from("{\n");
    out += &side_plane(a, b, z0, [0.0, 0.0]); // outer face
    out += &side_plane(ia, ib, z0, [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]); // inner
    out += &side_plane(pa, pb, z0, mid); // miter at corner A
    out += &side_plane(na, nb, z0, mid); // miter at corner B
    out += &flat_plane(z0, false);
    out += &flat_plane(z1, true);
    out += "}\n";
    out
}

fn box_brush(min: [f64; 3], max: [f64; 3]) -> String {
    let mut out = String::from("{\n");
    out += &side_plane([max[0], min[1]], [max[0], max[1]], min[2], [min[0], min[1]]);
    out += &side_plane([min[0], min[1]], [min[0], max[1]], min[2], [max[0], max[1]]);
    out += &side_plane([min[0], max[1]], [max[0], max[1]], min[2], [min[0], min[1]]);
    out += &side_plane([min[0], min[1]], [max[0], min[1]], min[2], [max[0], max[1]]);
    out += &flat_plane(min[2], false);
    out += &flat_plane(max[2], true);
    out += "}\n";
    out
}

/// Integer box brush text (public for brush-math tests).
pub fn box_brush_text(min: [i32; 3], max: [i32; 3]) -> String {
    box_brush(min.map(f64::from), max.map(f64::from))
}

fn point_entity(props: &[(&str, &str)]) -> String {
    let mut out = String::from("{\n");
    for (key, value) in props {
        out += &format!("\"{key}\" \"{value}\"\n");
    }
    out += "}\n";
    out
}

fn tile_meta(archetype: &str, variant: u16, levels: u8) -> String {
    point_entity(&[
        ("classname", "tile_meta"),
        ("archetype", archetype),
        ("register", "institutional"),
        ("variant", &variant.to_string()),
        ("levels", &levels.to_string()),
    ])
}

fn tile_port(face: &str, class: &str) -> String {
    point_entity(&[("classname", "tile_port"), ("face", face), ("class", class)])
}

/// Doorway constants (TB units): a 4.5 m wide, 4 m tall opening.
const DOOR_HALF_WIDTH: f64 = 36.0;
const DOOR_TOP: f64 = 72.0; // above the 8-unit floor slab: 4 m of clearance
const FLOOR_TOP: f64 = 8.0;
const EAST_X: f64 = 112.0;

/// An East or West wall with a centered doorway: two jambs plus a header.
/// Axis-aligned because the flat faces sit at x = +-112.
fn door_wall_east_west(east: bool, z0: f64, z1: f64, sill: f64, top: f64) -> String {
    let (outer, inner) = if east {
        (EAST_X, EAST_X - WALL)
    } else {
        (-EAST_X, -EAST_X + WALL)
    };
    let (x_min, x_max) = (outer.min(inner), outer.max(inner));
    let mut out = String::new();
    out += &box_brush([x_min, -64.0, z0], [x_max, -DOOR_HALF_WIDTH, z1]);
    out += &box_brush([x_min, DOOR_HALF_WIDTH, z0], [x_max, 64.0, z1]);
    if top < z1 {
        out += &box_brush([x_min, -DOOR_HALF_WIDTH, top], [x_max, DOOR_HALF_WIDTH, z1]);
    }
    if sill > z0 {
        out += &box_brush(
            [x_min, -DOOR_HALF_WIDTH, z0],
            [x_max, DOOR_HALF_WIDTH, sill],
        );
    }
    out
}

fn worldspawn(brushes: &str) -> String {
    format!("{{\n\"classname\" \"worldspawn\"\n{brushes}}}\n")
}

/// The locked authoring template: the canonical floor slab and a sealed shell,
/// ready to copy per tile. Boundary walls sit exactly on the quantized corners.
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
    out += &tile_meta("template", 0, 1);
    out
}

/// Straight hall: doors East and West.
pub fn hall_straight_ew_map() -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    for face in [
        HexFace::SouthEast,
        HexFace::SouthWest,
        HexFace::NorthWest,
        HexFace::NorthEast,
    ] {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &door_wall_east_west(true, 0.0, h, FLOOR_TOP, DOOR_TOP);
    brushes += &door_wall_east_west(false, 0.0, h, FLOOR_TOP, DOOR_TOP);
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Observed 2 seed tile: straight hall, doors E/W.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_straight", 0, 1);
    out += &tile_port("east", "door");
    out += &tile_port("west", "door");
    out
}

/// Dead-end cap: one door East.
pub fn hall_cap_e_map() -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    for face in [
        HexFace::SouthEast,
        HexFace::SouthWest,
        HexFace::West,
        HexFace::NorthWest,
        HexFace::NorthEast,
    ] {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &door_wall_east_west(true, 0.0, h, FLOOR_TOP, DOOR_TOP);
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Observed 2 seed tile: dead-end cap, door E.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_cap", 0, 1);
    out += &tile_port("east", "door");
    out
}

/// The two-level ramp prefab, `RampUp(East)`: a sloped floor rising a full
/// level West -> East across the flats, entered by a West door at level 0 and
/// exited by an East door at level 1. Anchored at the LOW cell; the head
/// cell above contributes no geometry of its own. This tile's ports are the
/// LOW cell's: West door + Up ramp_open (the paired `RampHead(East)` variant
/// carries Down ramp_open + East door).
pub fn ramp_e_map() -> String {
    let h = level_units();
    let top = 2.0 * h;
    // Sloped slab: hexagon sides + flat bottom + sloped top plane rising from
    // (x=-112, z=8) to (x=+112, z=136) — rise 128 over run 224, ~29.7 degrees.
    let mut ramp = String::from("{\n");
    for face in HexFace::LATERAL {
        let (a, b) = tb_edge(face);
        ramp += &side_plane(a, b, 0.0, [0.0, 0.0]);
    }
    ramp += &flat_plane(0.0, false);
    // Sloped top: three integer points, outward (upward) normal.
    ramp += &plane_line(
        [-EAST_X, -64.0, FLOOR_TOP],
        [-EAST_X, 64.0, FLOOR_TOP],
        [EAST_X, 0.0, FLOOR_TOP + h],
    );
    ramp += "}\n";

    let mut brushes = ramp;
    for face in [
        HexFace::SouthEast,
        HexFace::SouthWest,
        HexFace::NorthWest,
        HexFace::NorthEast,
    ] {
        brushes += &wall_brush(face, 0.0, top);
    }
    // West door at level 0 (sill at the slab top), East door at level 1
    // (sill where the ramp meets the East face).
    brushes += &door_wall_east_west(false, 0.0, top, FLOOR_TOP, DOOR_TOP);
    brushes += &door_wall_east_west(true, 0.0, top, FLOOR_TOP + h, DOOR_TOP + h);
    brushes += &hex_slab_brush(top - FLOOR_TOP, top);
    let mut out = String::from("// Observed 2 seed tile: RampUp(East) two-level prefab.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("ramp", 0, 2);
    out += &tile_port("west", "door");
    out += &tile_port("up", "ramp_open");
    out
}

/// A wellshaft segment: sealed lateral shell, open above and below, with
/// three cantilevered landing ledges reading as the climb path.
pub fn shaft_map() -> String {
    let h = level_units();
    let mut brushes = String::new();
    for face in HexFace::LATERAL {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &box_brush([-56.0, -32.0, 24.0], [-8.0, 32.0, 32.0]);
    brushes += &box_brush([8.0, -32.0, 64.0], [56.0, 32.0, 72.0]);
    brushes += &box_brush([-56.0, -32.0, 104.0], [-8.0, 32.0, 112.0]);
    let mut out = String::from("// Observed 2 seed tile: wellshaft segment, open Up/Down.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft", 0, 1);
    out += &tile_port("up", "shaft_open");
    out += &tile_port("down", "shaft_open");
    out
}

/// The seed manifest, typed. Committed `assets/tiles/manifest.ron` is pinned
/// to this by test.
pub fn manifest_ron() -> String {
    let entry = |archetype: &str, map: &str, levels: u8, ports: &[(&str, &str)]| {
        let ports = ports
            .iter()
            .map(|(face, class)| format!("(face: \"{face}\", class: \"{class}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "        (\n            key: (archetype: \"{archetype}\", register: \"institutional\", variant: 0),\n            map_path: \"{map}\",\n            levels: {levels},\n            ports: [{ports}],\n        ),\n"
        )
    };
    let mut out = String::from(
        "// Observed 2 hex tile manifest (Arc L). Tiles ARE the catalog.\n(\n    tiles: [\n",
    );
    out += &entry(
        "hall_straight",
        "hall_straight_ew.map",
        1,
        &[("east", "door"), ("west", "door")],
    );
    out += &entry("hall_cap", "hall_cap_e.map", 1, &[("east", "door")]);
    out += &entry(
        "ramp",
        "ramp_e.map",
        2,
        &[("west", "door"), ("up", "ramp_open")],
    );
    out += &entry(
        "shaft",
        "shaft.map",
        1,
        &[("up", "shaft_open"), ("down", "shaft_open")],
    );
    out += "    ],\n)\n";
    out
}

/// Write the template, seed tiles, and manifest under `dir` when the content
/// differs (labs call this so the committed assets stay openable in
/// TrenchBroom without drifting from the typed source).
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

/// Every generated asset as `(file name, content)`.
pub fn sources() -> Vec<(&'static str, String)> {
    vec![
        ("template.map", template_map()),
        ("hall_straight_ew.map", hall_straight_ew_map()),
        ("hall_cap_e.map", hall_cap_e_map()),
        ("ramp_e.map", ramp_e_map()),
        ("shaft.map", shaft_map()),
        ("manifest.ron", manifest_ron()),
    ]
}
