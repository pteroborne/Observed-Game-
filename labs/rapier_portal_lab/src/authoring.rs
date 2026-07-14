//! TrenchBroom source and pure projection for two reusable hallway modules.

use std::path::Path;

use bevy::math::Vec3;
use bevy_trenchbroom::{
    brush::{Brush, ConvexHull},
    config::TrenchBroomConfig,
    qmap::{QuakeMapEntities, QuakeMapEntity},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArchitectureKind {
    Gantry,
    Colonnade,
}

impl ArchitectureKind {
    pub const ALL: [Self; 2] = [Self::Gantry, Self::Colonnade];

    pub fn label(self) -> &'static str {
        match self {
            Self::Gantry => "gantry",
            Self::Colonnade => "colonnade",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConvexHullData {
    pub points: Vec<[f32; 3]>,
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredModule {
    pub kind: ArchitectureKind,
    pub hulls: Vec<ConvexHullData>,
    pub length: f32,
}

#[derive(Clone, Copy)]
struct Box3 {
    min: [i32; 3],
    max: [i32; 3],
}

impl Box3 {
    const fn new(min: [i32; 3], max: [i32; 3]) -> Self {
        Self { min, max }
    }

    fn brush(self) -> String {
        let [x1, y1, z1] = self.min;
        let [x2, y2, z2] = self.max;
        let f = |a: [i32; 3], b: [i32; 3], c: [i32; 3]| face(a, b, c);
        format!(
            "{{\n{}{}{}{}{}{} }}\n",
            f([x2, y1, z1], [x2, y1, z2], [x2, y2, z1]),
            f([x1, y1, z1], [x1, y2, z1], [x1, y1, z2]),
            f([x1, y2, z1], [x2, y2, z1], [x1, y2, z2]),
            f([x1, y1, z1], [x1, y1, z2], [x2, y1, z1]),
            f([x1, y1, z2], [x1, y2, z2], [x2, y1, z2]),
            f([x1, y1, z1], [x2, y1, z1], [x1, y2, z1]),
        )
    }
}

fn face(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> String {
    format!(
        "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
        a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]
    )
}

/// Triangular prism rising along +X. The same convex brush is collision and render data.
fn rising_ramp(x1: i32, x2: i32, y1: i32, y2: i32, height: i32) -> String {
    let a = [x1, y1, 0];
    let b = [x1, y2, 0];
    let c = [x2, y1, 0];
    let d = [x2, y2, 0];
    let e = [x2, y1, height];
    let f = [x2, y2, height];
    format!(
        "{{\n{}{}{}{}{} }}\n",
        face(a, c, b),
        face(a, e, c),
        face(b, d, f),
        face(c, e, d),
        face(a, b, e),
    )
}

fn module_start(kind: &str) -> String {
    format!("{{\n\"classname\" \"observed_module\"\n\"kind\" \"{kind}\"\n")
}

fn gantry_entity() -> String {
    let mut out = module_start("gantry");
    // Twenty-metre shell with an always-walkable ground bypass.
    for brush in [
        Box3::new([0, -220, -16], [800, 220, 0]),
        Box3::new([0, 204, 0], [800, 220, 180]),
        Box3::new([0, -220, 0], [800, -204, 180]),
        // Raised route occupies the left side, leaving the centre/right bypass clear.
        Box3::new([240, -180, 0], [370, -40, 96]),
        Box3::new([450, -180, 0], [690, -40, 96]),
        // Thin landing after the jump gap.
        Box3::new([700, -180, 0], [780, -40, 96]),
    ] {
        out += &brush.brush();
    }
    out += &rising_ramp(60, 240, -180, -40, 96);
    out += "}\n";
    out
}

fn colonnade_entity() -> String {
    let mut out = module_start("colonnade");
    for brush in [
        Box3::new([0, -320, -16], [800, 320, 0]),
        Box3::new([0, 304, 0], [800, 320, 200]),
        Box3::new([0, -320, 0], [800, -304, 200]),
    ] {
        out += &brush.brush();
    }
    // Repeated columns establish the pseudo-room rhythm; |Y| < 90 is the clear lane.
    for x in [150, 320, 490, 660] {
        for y in [-220, 220] {
            out += &Box3::new([x - 22, y - 22, 0], [x + 22, y + 22, 200]).brush();
        }
    }
    out += "}\n";
    out
}

pub fn module_map() -> String {
    let mut out = String::from("// Observed 2 authored portal module kit.\n");
    out += "{\n\"classname\" \"worldspawn\"\n}\n";
    out += &gantry_entity();
    out += &colonnade_entity();
    out
}

pub fn materialize(path: &Path) -> std::io::Result<()> {
    let source = module_map();
    if std::fs::read_to_string(path).ok().as_deref() != Some(source.as_str()) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, source)?;
    }
    Ok(())
}

fn prop(entity: &QuakeMapEntity, key: &str) -> Option<String> {
    entity.properties.get(key).cloned()
}

fn hull(brush: &Brush) -> Option<ConvexHullData> {
    let mut points = Vec::<[f32; 3]>::new();
    for (vertex, _) in brush.calculate_vertices() {
        let point = vertex.as_vec3().to_array();
        if !points.iter().any(|candidate| {
            candidate[0].to_bits() == point[0].to_bits()
                && candidate[1].to_bits() == point[1].to_bits()
                && candidate[2].to_bits() == point[2].to_bits()
        }) {
            points.push(point);
        }
    }
    if points.len() < 4 {
        return None;
    }
    points.sort_by_key(|point| (point[0].to_bits(), point[1].to_bits(), point[2].to_bits()));
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for point in &points {
        min = min.min(Vec3::from_array(*point));
        max = max.max(Vec3::from_array(*point));
    }
    Some(ConvexHullData { points, min, max })
}

pub fn authored_modules() -> Vec<AuthoredModule> {
    let config = TrenchBroomConfig::new("observed2_rapier_portal_lab");
    let parsed =
        quake_map::parse(&mut std::io::Cursor::new(module_map())).expect("typed module map parses");
    let entities = QuakeMapEntities::from_quake_map(parsed, &config);
    let mut modules = Vec::new();
    for entity in entities
        .iter()
        .filter(|entity| entity.classname().unwrap_or("") == "observed_module")
    {
        let kind = match prop(entity, "kind").as_deref() {
            Some("gantry") => ArchitectureKind::Gantry,
            Some("colonnade") => ArchitectureKind::Colonnade,
            other => panic!("unknown authored module kind {other:?}"),
        };
        let hulls: Vec<_> = entity.brushes.iter().filter_map(hull).collect();
        let min_z = hulls
            .iter()
            .map(|hull| hull.min.z)
            .fold(f32::INFINITY, f32::min);
        let max_z = hulls
            .iter()
            .map(|hull| hull.max.z)
            .fold(f32::NEG_INFINITY, f32::max);
        modules.push(AuthoredModule {
            kind,
            hulls,
            length: max_z - min_z,
        });
    }
    modules.sort_by_key(|module| match module.kind {
        ArchitectureKind::Gantry => 0,
        ArchitectureKind::Colonnade => 1,
    });
    modules
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapier3d::prelude::{ColliderBuilder, Vector};

    #[test]
    fn imports_one_gantry_and_one_colonnade_as_convex_brush_sets() {
        let modules = authored_modules();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].kind, ArchitectureKind::Gantry);
        assert_eq!(modules[1].kind, ArchitectureKind::Colonnade);
        assert!(modules[0].hulls.iter().any(|hull| hull.points.len() == 6));
        assert!(modules[1].hulls.len() >= 11);
    }

    #[test]
    fn every_authored_brush_builds_a_rapier_convex_hull() {
        for module in authored_modules() {
            for hull in module.hulls {
                let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
                assert!(ColliderBuilder::convex_hull(&points).is_some());
            }
        }
    }

    #[test]
    fn committed_trenchbroom_module_kit_matches_typed_source() {
        assert_eq!(
            include_str!("../assets/maps/portal_modules.map"),
            module_map()
        );
    }
}
