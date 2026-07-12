//! Typed source for the editable Quake `.map` course.
//!
//! Coordinates are TrenchBroom/Quake units (Z-up). `bevy_trenchbroom` converts
//! them to metres and Bevy's Y-up convention. The course extends along +X in the
//! editor, which becomes -Z in Bevy.

use std::path::Path;

#[derive(Clone, Copy, Debug)]
struct Box3 {
    min: [i32; 3],
    max: [i32; 3],
}

impl Box3 {
    const fn new(min: [i32; 3], max: [i32; 3]) -> Self {
        Self { min, max }
    }

    fn faces(self) -> String {
        let [x1, y1, z1] = self.min;
        let [x2, y2, z2] = self.max;
        let face = |a: [i32; 3], b: [i32; 3], c: [i32; 3]| {
            format!(
                "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
                a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]
            )
        };
        [
            face([x2, y1, z1], [x2, y1, z2], [x2, y2, z1]),
            face([x1, y1, z1], [x1, y2, z1], [x1, y1, z2]),
            face([x1, y2, z1], [x2, y2, z1], [x1, y2, z2]),
            face([x1, y1, z1], [x1, y1, z2], [x2, y1, z1]),
            face([x1, y1, z2], [x1, y2, z2], [x2, y1, z2]),
            face([x1, y1, z1], [x2, y1, z1], [x1, y2, z1]),
        ]
        .concat()
    }
}

fn face(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> String {
    format!(
        "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
        a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]
    )
}

fn box_brush(b: Box3) -> String {
    format!("{{\n{}}}\n", b.faces())
}

/// A triangular prism whose top is a genuine sloped plane, not an AABB staircase.
fn ramp_brush() -> String {
    let (x1, x2) = (1120, 1360);
    let (y1, y2) = (-200, 200);
    let (z0, z1) = (0, 96);
    let a = [x1, y1, z0];
    let b = [x1, y2, z0];
    let c = [x2, y1, z0];
    let d = [x2, y2, z0];
    let e = [x2, y1, z1];
    let f = [x2, y2, z1];
    let mut out = String::from("{\n");
    out += &face(a, c, b); // bottom (-Z)
    out += &face(a, e, c); // -Y side
    out += &face(b, d, f); // +Y side
    out += &face(c, e, d); // +X end
    out += &face(a, b, e); // sloped top
    out += "}\n";
    out
}

fn point_entity(props: &[(&str, &str)]) -> String {
    let mut out = String::from("{\n");
    for (key, value) in props {
        out += &format!("\"{key}\" \"{value}\"\n");
    }
    out += "}\n";
    out
}

fn structural_boxes() -> Vec<Box3> {
    vec![
        Box3::new([-16, -272, -16], [1504, 272, 0]),
        Box3::new([-16, -272, 0], [0, 272, 160]),
        Box3::new([0, 256, 0], [512, 272, 160]),
        Box3::new([0, -272, 0], [512, -256, 160]),
        Box3::new([512, 64, 0], [528, 272, 160]),
        Box3::new([512, -272, 0], [528, -64, 160]),
        Box3::new([528, 64, 0], [960, 80, 160]),
        Box3::new([528, -80, 0], [960, -64, 160]),
        Box3::new([960, 64, 0], [976, 272, 160]),
        Box3::new([960, -272, 0], [976, -64, 160]),
        Box3::new([1488, -272, 0], [1504, 272, 200]),
        Box3::new([976, 256, 0], [1488, 272, 200]),
        Box3::new([976, -272, 0], [1488, -256, 200]),
        Box3::new([1360, -200, 0], [1488, 200, 96]),
    ]
}

pub fn course_map() -> String {
    let mut out = String::from("// Observed 2 Rapier authoring course.\n");
    out += "{\n\"classname\" \"worldspawn\"\n";
    for solid in structural_boxes() {
        out += &box_brush(solid);
    }
    out += &ramp_brush();
    out += "}\n";

    out += &point_entity(&[("classname", "info_player_start"), ("origin", "64 0 32")]);
    out += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "1"),
        ("kind", "room"),
        ("mins", "0 -256 0"),
        ("maxs", "512 256 160"),
    ]);
    out += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "2"),
        ("kind", "corridor"),
        ("mins", "528 -64 0"),
        ("maxs", "960 64 160"),
    ]);
    out += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "3"),
        ("kind", "room"),
        ("mins", "976 -256 0"),
        ("maxs", "1488 256 200"),
    ]);
    out += &point_entity(&[
        ("classname", "observed_port"),
        ("id", "1"),
        ("room_a", "1"),
        ("room_b", "2"),
        ("origin", "520 0 48"),
    ]);
    out += &point_entity(&[
        ("classname", "observed_port"),
        ("id", "2"),
        ("room_a", "2"),
        ("room_b", "3"),
        ("origin", "968 0 48"),
    ]);

    out += "{\n\"classname\" \"observed_door\"\n\"id\" \"2\"\n";
    out += "\"port\" \"2\"\n\"state\" \"closed\"\n";
    out += &box_brush(Box3::new([960, -64, 0], [976, 64, 160]));
    out += "}\n";
    out
}

/// Keep a real `.map` beside the lab so it can be opened directly in TrenchBroom.
pub fn materialize(path: &Path) -> std::io::Result<()> {
    let source = course_map();
    if std::fs::read_to_string(path).ok().as_deref() != Some(source.as_str()) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, source)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_course_is_valid_quake_map_text() {
        quake_map::parse(&mut std::io::Cursor::new(course_map())).expect("course map parses");
    }

    #[test]
    fn committed_editable_map_does_not_drift_from_typed_source() {
        assert_eq!(
            include_str!("../assets/maps/rapier_course.map"),
            course_map()
        );
    }
}
