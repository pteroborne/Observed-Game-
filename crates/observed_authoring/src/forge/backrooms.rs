//! Low-clearance editions of the Backrooms kit. Existing sources retain their
//! identities; these editions have explicit low ports and ceiling brush tags.

use std::ffi::CString;

use quake_map::Entity;

use super::geometry::{FLOOR_TOP, translate};

fn property(entity: &Entity, key: &str) -> Option<String> {
    entity
        .edict
        .iter()
        .find(|(name, _)| name.to_bytes() == key.as_bytes())
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

fn set(entity: &mut Entity, key: &str, value: &str) {
    if let Some((_, previous)) = entity
        .edict
        .iter_mut()
        .find(|(name, _)| name.to_bytes() == key.as_bytes())
    {
        *previous = CString::new(value).expect("source value");
    } else {
        entity.edict.push((
            CString::new(key).expect("key"),
            CString::new(value).expect("value"),
        ));
    }
}

/// Produce an independent source with low external doorway clearances. Flat
/// ordinary modules compress their upper construction to a three-metre room;
/// climbs and wonders keep their interiors and receive a threshold collar.
#[must_use]
pub fn lower_source(source: &str) -> Option<String> {
    let mut map = quake_map::parse(&mut std::io::Cursor::new(source)).ok()?;
    let meta = map
        .entities
        .iter()
        .position(|entity| property(entity, "classname").as_deref() == Some("tile_meta"))?;
    let scope = property(&map.entities[meta], "register_scope")
        .or_else(|| property(&map.entities[meta], "register"))?;
    if scope != "all" && !scope.split(',').any(|register| register == "liminal_grid") {
        return None;
    }
    let archetype = property(&map.entities[meta], "archetype")?;
    let levels = property(&map.entities[meta], "levels").unwrap_or_else(|| "1".to_string());
    let id = property(&map.entities[meta], "id").unwrap_or_else(|| format!("compat/{archetype}"));
    let variant: u16 = property(&map.entities[meta], "variant")?.parse().ok()?;
    set(&mut map.entities[meta], "id", &format!("{id}_low"));
    set(
        &mut map.entities[meta],
        "variant",
        &(variant.checked_add(5000)?).to_string(),
    );
    set(&mut map.entities[meta], "register", "liminal_grid");
    set(&mut map.entities[meta], "register_scope", "liminal_grid");
    let exceptional = levels != "1"
        || [
            "climb",
            "cistern",
            "well",
            "rain",
            "charge",
            "nave",
            "concourse",
            "promenade",
        ]
        .iter()
        .any(|word| archetype.contains(word));
    let world = map
        .entities
        .iter()
        .position(|entity| property(entity, "classname").as_deref() == Some("worldspawn"))?;
    let already_low = id.starts_with("authored/back_");
    if already_low {
        let blocked = ["back_baffle", "back_cut", "back_recess"]
            .iter()
            .any(|name| id.ends_with(name));
        set(
            &mut map.entities[meta],
            "weight",
            if blocked { "9" } else { "3" },
        );
    }
    if !exceptional && already_low {
        map.entities[world].brushes.retain(|brush| {
            !bounds(brush).is_some_and(|(min, max)| {
                min[2] >= 52.0
                    && max[2] - min[2] <= 12.0
                    && (max[0] - min[0]).max(max[1] - min[1]) > 30.0
            })
        });
        let ceiling = super::geometry::hex_slab(56.0, 58.0, 0.0, 0.0)
            .replace("__TB_empty", "observed_ceiling");
        let cap = quake_map::parse(&mut std::io::Cursor::new(super::entities::worldspawn(
            &ceiling,
        )))
        .ok()?;
        map.entities[world]
            .brushes
            .extend(cap.entities[0].brushes.clone());
    }
    if !exceptional && !already_low {
        // The service void disappears, rather than squeezing its ribs and
        // fixture housings into the doorway's required body clearance.
        map.entities[world].brushes.retain(|brush| {
            !bounds(brush).is_some_and(|(min, max)| lower_height(min[2]) >= lower_height(max[2]))
        });
    }
    if !exceptional {
        for brush in &mut map.entities[world].brushes {
            let (min, max) = bounds(brush)?;
            let horizontal =
                max[2] - min[2] <= 12.0 && (max[0] - min[0]).max(max[1] - min[1]) > 30.0;
            let ceiling = horizontal && min[2] >= if already_low { 52.0 } else { 120.0 };
            let trim = horizontal && already_low && min[2] >= 52.0 && max[2] <= 56.0;
            for surface in brush.iter_mut() {
                if ceiling {
                    surface.texture = CString::new(if trim {
                        "observed_trim"
                    } else {
                        "observed_ceiling"
                    })
                    .expect("tag");
                }
                if !already_low {
                    for point in &mut surface.half_space {
                        point[2] = lower_height(point[2]);
                    }
                }
            }
        }
    }
    let mut collars = String::new();
    for entity in &mut map.entities {
        let class = property(entity, "classname").unwrap_or_default();
        if class == "tile_port"
            && property(entity, "class").as_deref() == Some("door")
            && property(entity, "level").as_deref().unwrap_or("0") == "0"
        {
            set(entity, "class", "low_door");
            let level: f64 = property(entity, "level")
                .unwrap_or_else(|| "0".to_string())
                .parse()
                .ok()?;
            if let Some(origin) = property(entity, "origin") {
                let mut p = point(&origin)?;
                p[2] = level * 128.0 + 32.0;
                set(entity, "origin", &format!("{} {} {}", p[0], p[1], p[2]));
            }
            if exceptional || already_low {
                let face = super::geometry::FACE_NAMES
                    .iter()
                    .position(|&name| property(entity, "face").as_deref() == Some(name))?;
                let q: i32 = property(entity, "q")
                    .unwrap_or_else(|| "0".to_string())
                    .parse()
                    .ok()?;
                let r: i32 = property(entity, "r")
                    .unwrap_or_else(|| "0".to_string())
                    .parse()
                    .ok()?;
                let (x, y) = super::geometry::cell_origin(q, r);
                // Only the header mass is added; jambs/floors remain the original
                // authored geometry. Its three-metre underside meets the low kit.
                let header = super::geometry::band(face, 0.0, 8.0, 56.0, 72.0);
                collars.push_str(&translate(&header, x, y, level * 128.0));
            }
        } else if class == "tile_light"
            && !exceptional
            && let Some(origin) = property(entity, "origin")
        {
            let mut p = point(&origin)?;
            p[2] = if already_low {
                p[2].min(52.0)
            } else {
                lower_height(p[2]).min(52.0)
            };
            set(entity, "origin", &format!("{} {} {}", p[0], p[1], p[2]));
        }
    }
    if !collars.is_empty() {
        let extra = quake_map::parse(&mut std::io::Cursor::new(super::entities::worldspawn(
            &collars,
        )))
        .ok()?;
        map.entities[world]
            .brushes
            .extend(extra.entities[0].brushes.clone());
    }
    let mut bytes = Vec::new();
    map.write_to(&mut bytes).ok()?;
    Some(format!(
        "// Backrooms low-clearance edition.\n{}{}",
        super::GENERATED_NOTE,
        String::from_utf8(bytes).ok()?
    ))
}

fn point(value: &str) -> Option<[f64; 3]> {
    let values = value
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<Vec<f64>, _>>()
        .ok()?;
    values.try_into().ok()
}

fn lower_height(height: f64) -> f64 {
    if height <= FLOOR_TOP {
        height
    } else {
        if height <= 72.0 {
            FLOOR_TOP + (height - FLOOR_TOP) * 0.75
        } else if height <= 120.0 {
            56.0
        } else {
            56.0 + (height - 120.0) * 0.25
        }
    }
}

fn bounds(brush: &quake_map::Brush) -> Option<([f64; 3], [f64; 3])> {
    let vertices = crate::brush::brush_vertices(brush)?;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Some((min, max))
}

/// Low editions only of the curated Backrooms families; other districts and
/// retired design experiments cannot silently enter the production lottery.
#[must_use]
pub fn generated(sources: &[(String, String)]) -> Vec<(String, String)> {
    sources
        .iter()
        .filter(|(name, _)| {
            name.starts_with("liminal_grid_")
                || name.starts_with("back_")
                || name.starts_with("climb_foot")
                || name.starts_with("cistern_")
                || super::rooms::builders()
                    .iter()
                    .any(|(room, _)| name == room)
        })
        .filter_map(|(name, source)| {
            lower_source(source).map(|source| (format!("{name}_low"), source))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_hex::{HexFace, PortClass};

    #[test]
    fn low_flat_has_a_real_ceiling_and_preserves_floor_and_topology() {
        let source =
            super::super::liminal::liminal_cell("proof", "hall_straight", 0b001001, 1, 0, None);
        let original = crate::parse_tile(&source).expect("original");
        let low = crate::parse_tile(&lower_source(&source).expect("low source")).expect("low tile");
        assert_eq!(low.signature, original.signature.lowered());
        assert_eq!(low.signature.port(HexFace::East), PortClass::LowDoor);
        assert!(low.surfaces.contains(&Some(crate::HullSurface::Ceiling)));
        assert!(
            low.hulls
                .iter()
                .any(|hull| hull.iter().all(|p| p.y >= 3.5 && p.y <= 3.72))
        );
        assert_eq!(low.hulls[0], original.hulls[0]);
    }

    #[test]
    fn low_headers_clear_a_standing_capsule_at_both_thresholds() {
        use glam::{Vec2, Vec3};
        use observed_traversal::{
            FpsBody, FpsConfig,
            rapier_controller::{RapierTraversalScene, step_character},
        };
        use player_input::PlayerIntent;
        let source =
            super::super::liminal::liminal_cell("proof", "hall_straight", 0b001001, 1, 0, None);
        let tile = crate::parse_tile(&lower_source(&source).expect("low source")).expect("tile");
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        for sign in [-1.0f32, 1.0] {
            let mut body = FpsBody::spawned(
                Vec3::new(sign * 6.7, 0.5 + config.half_height, 0.0),
                -sign * std::f32::consts::FRAC_PI_2,
            );
            for _ in 0..1200 {
                if body.position.x * sign < -6.6 {
                    break;
                }
                step_character(
                    &scene,
                    &mut body,
                    PlayerIntent {
                        movement: Vec2::Y,
                        ..PlayerIntent::default()
                    },
                    &config,
                    1.0 / 60.0,
                );
            }
            assert!(
                body.position.x * sign < -6.0,
                "low doorway blocked the body at {:?}",
                body.position
            );
            assert!((body.position.y - config.half_height - 0.5).abs() < 0.05);
        }
    }
}
