//! The authored facility as a TrenchBroom `.map`, generated from a typed layout.
//!
//! A `.map` is a plain Quake-format text file — it could be opened and edited in
//! TrenchBroom directly. We generate it from a small typed description so every
//! axis-aligned box brush has provably-correct face winding (the importer derives
//! each plane normal from the face's vertex order, and an inverted winding would
//! corrupt the reconstructed box). The generated text is materialised once into
//! `assets/maps/facility.map` (the artifact the asset loader reads); a drift test
//! keeps the committed file equal to [`facility_map`].
//!
//! Coordinates are TrenchBroom space (Quake units, Z-up). The importer converts to
//! Bevy space (Y-up) on load, dividing by the config `scale` (~39.37 units/metre),
//! so the numbers below are roughly `metres * 39`. The facility extends along +X
//! (TrenchBroom forward), which the importer maps to Bevy −Z — the controller's
//! forward — so a player spawned at yaw 0 walks straight into it.

/// One axis-aligned box in TrenchBroom units, `min`/`max` corners inclusive of the
/// solid volume between them.
#[derive(Clone, Copy, Debug)]
pub struct Box3 {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Box3 {
    pub const fn new(min: [i32; 3], max: [i32; 3]) -> Self {
        Self { min, max }
    }

    /// The six faces of the box as `.map` brush lines. Each face lists three points
    /// `A B C` such that the importer's `normal = (C-A) × (B-A)` points *outward*
    /// (Quake's convention: a point is inside the brush when behind every face).
    /// The Quake→Bevy transform is a proper rotation, so outward in this space stays
    /// outward after import.
    fn faces(&self) -> String {
        let [x1, y1, z1] = self.min;
        let [x2, y2, z2] = self.max;
        let f = |a: [i32; 3], b: [i32; 3], c: [i32; 3]| {
            format!(
                "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
                a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]
            )
        };
        let mut s = String::new();
        // +X
        s += &f([x2, y1, z1], [x2, y1, z2], [x2, y2, z1]);
        // -X
        s += &f([x1, y1, z1], [x1, y2, z1], [x1, y1, z2]);
        // +Y
        s += &f([x1, y2, z1], [x2, y2, z1], [x1, y2, z2]);
        // -Y
        s += &f([x1, y1, z1], [x1, y1, z2], [x2, y1, z1]);
        // +Z
        s += &f([x1, y1, z2], [x1, y2, z2], [x2, y1, z2]);
        // -Z
        s += &f([x1, y1, z1], [x2, y1, z1], [x1, y2, z1]);
        s
    }
}

fn brush(b: Box3) -> String {
    format!("{{\n{}}}\n", b.faces())
}

/// The structural geometry (floor, walls, stairs, platform) — always-collidable
/// brushes the importer parses for collision and we render through `observed_style`.
/// Door gaps are left open in the walls; the door leaves are separate entities.
pub fn worldspawn_solids() -> Vec<Box3> {
    vec![
        // Floor slab spanning the whole footprint (top at z=0 → Bevy y=0).
        Box3::new([-16, -272, -16], [1504, 272, 0]),
        // --- Room A shell (interior x[0,512], y[-256,256]) ---
        Box3::new([-16, -272, 0], [0, 272, 144]), // back wall (-X)
        Box3::new([0, 256, 0], [512, 272, 144]),  // left wall (+Y)
        Box3::new([0, -272, 0], [512, -256, 144]), // right wall (-Y)
        Box3::new([512, 64, 0], [528, 272, 144]), // front wall, left of door
        Box3::new([512, -272, 0], [528, -64, 144]), // front wall, right of door
        // --- Corridor shell (interior x[528,960], y[-64,64]) ---
        Box3::new([528, 64, 0], [960, 80, 144]), // left wall
        Box3::new([528, -80, 0], [960, -64, 144]), // right wall
        // --- Room B shell (interior x[976,1488], y[-256,256]) ---
        Box3::new([960, 64, 0], [976, 272, 144]), // back wall, left of door
        Box3::new([960, -272, 0], [976, -64, 144]), // back wall, right of door
        Box3::new([1488, -272, 0], [1504, 272, 144]), // far wall (+X)
        Box3::new([976, 256, 0], [1488, 272, 144]), // left wall
        Box3::new([976, -272, 0], [1488, -256, 144]), // right wall
        // --- Elevation: a three-step stair onto a raised platform in Room B ---
        Box3::new([1200, -200, 0], [1240, 200, 16]),
        Box3::new([1240, -200, 0], [1280, 200, 32]),
        Box3::new([1280, -200, 0], [1320, 200, 48]),
        Box3::new([1320, -200, 0], [1456, 200, 48]), // platform
    ]
}

/// A door: its leaf brush (filling a wall gap) plus the metadata the projection turns
/// into a `PortId`, door id, and open/closed state. The leaf is imported geometry; its
/// collision presence is gated by the *door model state*, never the map material.
pub struct DoorDef {
    pub id: u32,
    pub port: u32,
    pub open: bool,
    pub leaf: Box3,
}

pub fn doors() -> Vec<DoorDef> {
    vec![
        DoorDef {
            id: 1,
            port: 1,
            open: true, // Room A → corridor: starts open.
            leaf: Box3::new([512, -64, 0], [528, 64, 144]),
        },
        DoorDef {
            id: 2,
            port: 2,
            open: false, // corridor → Room B: starts closed (open it to pass).
            leaf: Box3::new([960, -64, 0], [976, 64, 144]),
        },
    ]
}

fn point_entity(props: &[(&str, &str)]) -> String {
    let mut s = String::from("{\n");
    for (k, v) in props {
        s += &format!("\"{k}\" \"{v}\"\n");
    }
    s += "}\n";
    s
}

/// The complete authored `.map` text.
pub fn facility_map() -> String {
    let mut s = String::new();
    s += "// Observed 2 — trenchbroom_lab feasibility facility (Phase A1).\n";
    s += "// Generated by map_source::facility_map(); see the lab README.\n";

    // worldspawn: all structural brushes.
    s += "{\n\"classname\" \"worldspawn\"\n";
    for b in worldspawn_solids() {
        s += &brush(b);
    }
    s += "}\n";

    // Player spawn.
    s += &point_entity(&[("classname", "info_player_start"), ("origin", "64 0 32")]);

    // Rooms (typed markers with explicit bounds).
    s += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "1"),
        ("kind", "room"),
        ("origin", "256 0 0"),
        ("mins", "0 -256 0"),
        ("maxs", "512 256 144"),
    ]);
    s += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "2"),
        ("kind", "corridor"),
        ("origin", "744 0 0"),
        ("mins", "528 -64 0"),
        ("maxs", "960 64 144"),
    ]);
    s += &point_entity(&[
        ("classname", "observed_room"),
        ("id", "3"),
        ("kind", "room"),
        ("origin", "1232 0 0"),
        ("mins", "976 -256 0"),
        ("maxs", "1488 256 144"),
    ]);

    // Ports (typed connections between rooms).
    s += &point_entity(&[
        ("classname", "observed_port"),
        ("id", "1"),
        ("room_a", "1"),
        ("room_b", "2"),
        ("origin", "520 0 48"),
    ]);
    s += &point_entity(&[
        ("classname", "observed_port"),
        ("id", "2"),
        ("room_a", "2"),
        ("room_b", "3"),
        ("origin", "968 0 48"),
    ]);

    // Doors (solid entities: a leaf brush + state).
    for d in doors() {
        s += "{\n\"classname\" \"observed_door\"\n";
        s += &format!("\"id\" \"{}\"\n", d.id);
        s += &format!("\"port\" \"{}\"\n", d.port);
        s += &format!("\"state\" \"{}\"\n", if d.open { "open" } else { "closed" });
        s += &brush(d.leaf);
        s += "}\n";
    }

    s
}
