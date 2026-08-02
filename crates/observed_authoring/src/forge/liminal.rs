//! The Liminal Grid horizontal kit: one source per rotational topology class.
//!
//! Two physical layouts - weight 3 plain repetition, weight 2 sparse piers -
//! make the 7x7 district coherent without making every cell byte-identical.
//! All structure stays convex brush-per-hull; six-door sanctuary cells remain
//! under the 28-hull budget, which is why the vertical variants spend their
//! hulls on the aperture ring and stay deliberately spare.

use super::entities::{
    Meta, PORT_SHORT, ceiling_fixture, lateral_port, tile_cell, vertical_port, worldspawn,
};
use super::geometry::{FLOOR_TOP, LEVEL, P2, corners, door_wall, hex_slab, prism, wall};
use super::{Builder, GENERATED_NOTE};

const LIMINAL_REGISTER: &str = "liminal_grid";

/// Which way a cell opens vertically, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    Up,
    Down,
}

impl Vertical {
    #[must_use]
    fn slug(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

#[must_use]
fn square(center: P2, half: f64) -> Vec<P2> {
    vec![
        (center.0 - half, center.1 - half),
        (center.0 + half, center.1 - half),
        (center.0 + half, center.1 + half),
        (center.0 - half, center.1 + half),
    ]
}

#[must_use]
fn faces_from_mask(mask: u8) -> Vec<usize> {
    (0..6).filter(|face| mask & (1 << face) != 0).collect()
}

/// Six convex trapezoids around a centred vertical hex aperture.
#[must_use]
fn hex_opening_slab(z0: f64, z1: f64) -> String {
    const OPENING_SCALE: f64 = 0.30;
    let outer = corners();
    let inner: Vec<P2> = outer
        .iter()
        .map(|(x, y)| (x * OPENING_SCALE, y * OPENING_SCALE))
        .collect();
    let mut brushes = String::new();
    for face in 0..6 {
        let next = (face + 1) % 6;
        brushes.push_str(&prism(
            &[outer[face], outer[next], inner[next], inner[face]],
            z0,
            z1,
            None,
            0.0,
            0.0,
        ));
    }
    brushes
}

#[must_use]
fn liminal_shell(door_faces: &[usize], vertical: Option<Vertical>) -> String {
    let mut brushes = String::from("// Liminal Grid full floor/ceiling envelope\n");
    if vertical == Some(Vertical::Down) {
        brushes.push_str(&hex_opening_slab(0.0, FLOOR_TOP));
    } else {
        brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    }
    if vertical == Some(Vertical::Up) {
        brushes.push_str(&hex_opening_slab(LEVEL - FLOOR_TOP, LEVEL));
    } else {
        brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 2.0));
    }
    for face in 0..6 {
        if door_faces.contains(&face) {
            // The floor slab supplies the 0.5 m sill, leaving exactly three
            // surround hulls: two jamb masses and one lintel.
            brushes.push_str(&door_wall(
                face,
                0.0,
                LEVEL,
                0.0,
                super::geometry::DOOR_TOP,
                4.0,
                5.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    brushes
}

#[must_use]
fn liminal_layout(layout: u32, vertical: Option<Vertical>) -> (String, String) {
    let mut brushes = String::new();
    let mut lights = String::new();
    // Vertical cells stay spare: their aperture ring already costs six hulls.
    let mut fixture_positions: Vec<P2> = if vertical.is_some() {
        vec![(42.0, 0.0)]
    } else {
        vec![(-42.0, 0.0), (42.0, 0.0)]
    };
    if layout == 0 && vertical.is_none() {
        fixture_positions.push((0.0, 0.0));
    }
    for (x, y) in fixture_positions {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL, 22.0, 5.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    if layout == 1 {
        // Square grounded piers keep the centre and every threshold ray open.
        // The vertical variant uses one pier to preserve its hull budget.
        let centers: Vec<P2> = if vertical.is_some() {
            vec![(-34.0, -30.0)]
        } else {
            vec![(-34.0, -30.0), (34.0, 30.0)]
        };
        for center in centers {
            brushes.push_str(&prism(
                &square(center, 5.0),
                FLOOR_TOP,
                LEVEL - FLOOR_TOP,
                None,
                2.0,
                0.0,
            ));
        }
    }
    (brushes, lights)
}

#[must_use]
pub fn liminal_cell(
    name: &str,
    archetype: &str,
    door_mask: u8,
    variant: i32,
    layout: u32,
    vertical: Option<Vertical>,
) -> String {
    let door_faces = faces_from_mask(door_mask);
    let mut brushes = liminal_shell(&door_faces, vertical);
    let (layout_brushes, lights) = liminal_layout(layout, vertical);
    brushes.push_str(&layout_brushes);
    let layout_name = if layout == 0 {
        "plain fluorescent repetition"
    } else {
        "sparse partition piers"
    };
    let mut out = format!("// Liminal Grid {archetype}: {layout_name}.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{name}"),
            archetype,
            variant,
            1,
            if layout == 0 { 3 } else { 2 },
        )
        .with_register(LIMINAL_REGISTER)
        .with_register_scope(LIMINAL_REGISTER)
        .emit(),
    );
    out.push_str(&tile_cell(
        0,
        0,
        0,
        1,
        if vertical == Some(Vertical::Down) {
            "open"
        } else {
            "solid"
        },
    ));
    for &face in &door_faces {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", PORT_SHORT[face]),
            0,
            0,
            0,
        ));
    }
    if let Some(vertical) = vertical {
        out.push_str(&vertical_port(
            vertical.slug(),
            "shaft_open",
            &format!("{}_shaft", vertical.slug()),
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// One generated liminal tile: its file stem and everything needed to build it.
///
/// Materialised as data rather than as closures because the registry has to be
/// enumerable - the byte-identity gate iterates it, and so will `gen-tiles`.
#[derive(Clone, Debug)]
pub struct LiminalTile {
    pub name: String,
    pub archetype: &'static str,
    pub mask: u8,
    pub variant: i32,
    pub layout: u32,
    pub vertical: Option<Vertical>,
}

impl LiminalTile {
    #[must_use]
    pub fn build(&self) -> String {
        liminal_cell(
            &self.name,
            self.archetype,
            self.mask,
            self.variant,
            self.layout,
            self.vertical,
        )
    }
}

/// One source per rotational topology class, each in two layouts.
///
/// The mask lists are orbit representatives: every other rotation of the same
/// door topology is generated by the compiler's sixfold expansion, so authoring
/// them here would compile duplicates.
#[must_use]
pub fn tiles() -> Vec<LiminalTile> {
    let horizontal: [(&'static str, &[u8]); 6] = [
        ("hall_cap", &[0b00_0001]),
        ("hall_straight", &[0b00_1001]),
        ("hall_turn_60", &[0b10_0001]),
        ("hall_turn_120", &[0b01_0001]),
        (
            "hall_junction_3way",
            &[0b00_0111, 0b00_1011, 0b00_1101, 0b01_0101],
        ),
        ("hall_junction_4way", &[0b00_1111, 0b01_0111, 0b01_1011]),
    ];
    let sanctuary: [(u8, Option<Vertical>); 6] = [
        (0b11_1111, None),
        (0b00_1111, None),
        (0b00_0111, None),
        (0b01_1111, None),
        (0b11_1111, Some(Vertical::Up)),
        (0b11_1111, Some(Vertical::Down)),
    ];

    let mut out = Vec::new();
    for (archetype, masks) in horizontal {
        for (orbit, &mask) in masks.iter().enumerate() {
            for layout in 0..2 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let variant = (orbit as i32) * 2 + layout as i32;
                out.push(LiminalTile {
                    name: format!("liminal_grid_{archetype}_o{orbit}_l{layout}"),
                    archetype,
                    mask,
                    variant,
                    layout,
                    vertical: None,
                });
            }
        }
    }
    for (orbit, (mask, vertical)) in sanctuary.into_iter().enumerate() {
        for layout in 0..2 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let variant = (orbit as i32) * 2 + layout as i32;
            let vertical_name = vertical.map_or("lateral", Vertical::slug);
            out.push(LiminalTile {
                name: format!("liminal_grid_sanctuary_{vertical_name}_o{orbit}_l{layout}"),
                archetype: "sanctuary",
                mask,
                variant,
                layout,
                vertical,
            });
        }
    }
    out
}

/// Generate every liminal tile, as `(stem, text)` pairs.
///
/// Not a [`Builder`] list: these are data-driven rather than one function per
/// file, so there is no `fn() -> String` to name. The gate below takes the same
/// shape by another route.
#[must_use]
pub fn generated() -> Vec<(String, String)> {
    tiles()
        .into_iter()
        .map(|tile| (tile.name.clone(), tile.build()))
        .collect()
}

/// Empty: this family has no per-file builder functions.
#[must_use]
pub fn builders() -> Vec<Builder> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same gate the other families get, by the data-driven route.
    #[test]
    fn every_liminal_tile_reproduces_its_committed_file() {
        for (name, produced) in generated() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles/authored")
                .join(format!("{name}.map"));
            let committed = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                .replace("\r\n", "\n");
            if committed == produced {
                continue;
            }
            let at = committed
                .lines()
                .zip(produced.lines())
                .position(|(a, b)| a != b);
            match at {
                Some(index) => {
                    let expected: Vec<&str> = committed.lines().collect();
                    let actual: Vec<&str> = produced.lines().collect();
                    panic!(
                        "{name}.map diverges at line {}:\n  committed: {}\n  forge:     {}",
                        index + 1,
                        expected[index],
                        actual[index]
                    );
                }
                None => panic!(
                    "{name}.map length differs: committed {} lines, forge {}",
                    committed.lines().count(),
                    produced.lines().count()
                ),
            }
        }
    }

    /// Variants index the compiled catalog, so a collision within one archetype
    /// would have two sources claiming the same slot.
    #[test]
    fn variants_are_unique_within_an_archetype() {
        let mut seen: Vec<(&str, i32)> = tiles()
            .iter()
            .map(|tile| (tile.archetype, tile.variant))
            .collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(total, seen.len(), "two liminal tiles share a variant slot");
    }

    /// Every generated name has to be distinct, or one silently overwrites
    /// another when the registry is written to disk.
    #[test]
    fn generated_names_are_distinct() {
        let mut names: Vec<String> = tiles().into_iter().map(|tile| tile.name).collect();
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(total, names.len());
    }
}
