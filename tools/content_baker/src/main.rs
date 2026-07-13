//! Deterministic TrenchBroom `.map` to runtime convex-module baker.

use std::{collections::BTreeMap, env, fs, path::PathBuf};

use bevy_trenchbroom::{
    brush::{Brush, ConvexHull as TrenchBroomConvexHull},
    config::TrenchBroomConfig,
    qmap::{QuakeMapEntities, QuakeMapEntity},
};
use observed_content::{BakedModule, ConvexHull as BakedConvexHull, sha256};

fn main() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("tool lives under tools")
        .to_owned();
    let source = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("labs/rapier_portal_lab/assets/maps/portal_modules.map"));
    let output = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("assets/content/modules"));

    let bytes = fs::read(&source)
        .unwrap_or_else(|error| panic!("cannot read source map {}: {error}", source.display()));
    let modules = bake(&bytes).unwrap_or_else(|error| panic!("bake failed: {error}"));
    fs::create_dir_all(&output)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", output.display()));
    println!("source_sha256={}", sha256(&bytes));
    for module in modules {
        let path = output.join(format!("{}.json", module.module_id));
        let encoded = module.canonical_json();
        fs::write(&path, &encoded)
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
        println!(
            "module={} hulls={} sha256={} path={}",
            module.module_id,
            module.hulls.len(),
            sha256(&encoded),
            path.display()
        );
    }
}

fn bake(source: &[u8]) -> Result<Vec<BakedModule>, String> {
    let config = TrenchBroomConfig::new("observed2_content_baker");
    let parsed = quake_map::parse(&mut std::io::Cursor::new(source))
        .map_err(|error| format!("map parse error: {error:?}"))?;
    let entities = QuakeMapEntities::from_quake_map(parsed, &config);
    let mut modules = BTreeMap::<String, Vec<BakedConvexHull>>::new();
    for entity in entities
        .iter()
        .filter(|entity| entity.classname().unwrap_or("") == "observed_module")
    {
        let kind = property(entity, "kind")
            .ok_or_else(|| "observed_module is missing `kind`".to_string())?;
        let hulls = modules.entry(kind).or_default();
        for brush in &entity.brushes {
            if let Some(points) = brush_points(brush) {
                hulls.push(BakedConvexHull {
                    id: hulls.len() as u32,
                    points,
                });
            }
        }
    }
    let modules: Vec<_> = modules
        .into_iter()
        .map(|(module_id, hulls)| BakedModule {
            schema_version: observed_content::SUPPORTED_SCHEMA_VERSION,
            module_id,
            hulls,
        })
        .collect();
    if modules.is_empty() {
        return Err("map contains no observed_module entities".into());
    }
    for module in &modules {
        module.validate().map_err(|error| error.to_string())?;
    }
    Ok(modules)
}

fn property(entity: &QuakeMapEntity, key: &str) -> Option<String> {
    entity.properties.get(key).cloned()
}

fn brush_points(brush: &Brush) -> Option<Vec<[f32; 3]>> {
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
    (points.len() >= 4).then_some(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_source_bakes_deterministically() {
        let source =
            include_bytes!("../../../labs/rapier_portal_lab/assets/maps/portal_modules.map");
        let a = bake(source).unwrap();
        let b = bake(source).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.iter()
                .map(|module| module.module_id.as_str())
                .collect::<Vec<_>>(),
            vec!["colonnade", "gantry"]
        );
        assert!(a.iter().all(|module| !module.hulls.is_empty()));
    }
}
