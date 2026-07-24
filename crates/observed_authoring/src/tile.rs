//! Tile schema projection and exact-snap footprint validation.
//!
//! A tile `.map` contains: `worldspawn` brushes (the solid geometry), exactly
//! one `tile_meta` point entity (`archetype`, `register`, `variant`,
//! `levels`), and `tile_port` point entities (`face`, `class`). Projection is
//! importer-only — editor entities describe the tile, they never become the
//! game model.

use std::ffi::CString;

use glam::{Quat, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT, face_edge};
use observed_traversal::{ArenaSpec, ColliderShape, ColliderSpec, StableColliderId};
use quake_map::{Entity, QuakeMap};
use serde::{Deserialize, Serialize};

use crate::UNITS_PER_METER;
use crate::brush::brush_vertices;
use crate::manifest::TileKey;

/// On-boundary tolerance for the footprint check, in TrenchBroom units.
/// Authored planes are integer, so anything beyond this is a real violation.
const SNAP_EPSILON: f64 = 1.0e-3;

#[derive(Clone, Debug, PartialEq)]
pub enum TileError {
    Parse(String),
    MissingMeta,
    DuplicateMeta,
    DuplicatePort {
        face: HexFace,
    },
    MissingProperty {
        entity: &'static str,
        key: String,
    },
    UnknownFace(String),
    UnknownClass(String),
    UnknownLightKind(String),
    InvalidPort {
        face: HexFace,
        class: PortClass,
    },
    DegenerateBrush {
        index: usize,
    },
    InvalidLevels,
    /// A vertex escapes the canonical quantized-hexagon prism. Reports the
    /// offending vertex (TrenchBroom units), the violated boundary, and the
    /// exact bound so the author can fix the brush.
    FootprintViolation {
        vertex: [f64; 3],
        boundary: String,
    },
}

/// Semantic authored light source. Presentation owns its colour and energy;
/// tile sources own only placement and purpose, keeping district treatment in
/// `observed_style` instead of baking ad-hoc RGB values into geometry files.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TileLightKind {
    Practical,
}

/// One tile-local light source in world-space metres (Y-up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLight {
    pub kind: TileLightKind,
    pub position: Vec3,
}

/// A validated, world-space tile ready for placement.
#[derive(Clone, Debug, PartialEq)]
pub struct TilePrototype {
    pub key: TileKey,
    /// Relative deterministic selection weight from authored metadata.
    pub weight: u16,
    /// Vertical levels this prefab spans (1 for flats, 2 for ramp prefabs).
    pub levels: u8,
    pub signature: PortSignature,
    /// Convex hulls in tile-local world meters: origin at the cell center,
    /// level 0 floor at y = 0.
    pub hulls: Vec<Vec<Vec3>>,
    /// Semantic practicals authored against visible fixture geometry.
    pub lights: Vec<TileLight>,
}

impl TilePrototype {
    /// Collider specs for one instance of this tile, ids offset by `base_id`,
    /// hulls translated by `offset` (typically `hex_origin` of the cell).
    pub fn collider_specs(&self, base_id: u32, offset: Vec3) -> Vec<ColliderSpec> {
        self.collider_specs_with_transform(base_id, offset, Quat::IDENTITY)
    }

    /// Collider specs with translation and rotation transform applied.
    pub fn collider_specs_with_transform(
        &self,
        base_id: u32,
        offset: Vec3,
        rotation: Quat,
    ) -> Vec<ColliderSpec> {
        let rot_arr = [rotation.x, rotation.y, rotation.z, rotation.w];
        self.hulls
            .iter()
            .enumerate()
            .map(|(index, hull)| {
                let rotated_hull: Vec<Vec3> = hull.iter().map(|v| rotation * *v).collect();
                ColliderSpec {
                    id: StableColliderId(base_id + index as u32),
                    center: offset,
                    rotation: rot_arr,
                    shape: ColliderShape::ConvexHull {
                        points: rotated_hull,
                    },
                    friction: 0.8,
                }
            })
            .collect()
    }

    /// A standalone single-tile arena for labs and headless traversal tests.
    pub fn arena_spec(&self) -> ArenaSpec {
        let height = f32::from(self.levels) * TILE_LEVEL_HEIGHT;
        ArenaSpec {
            colliders: self.collider_specs(0, Vec3::ZERO),
            floor_y: 0.0,
            safety_center: Vec3::new(0.0, height * 0.5, 0.0),
            safety_half: Vec3::new(24.0, height + 12.0, 24.0),
        }
    }
}

fn cstr(value: &CString) -> String {
    value.to_string_lossy().into_owned()
}

pub(crate) fn prop(entity: &Entity, key: &str) -> Option<String> {
    entity
        .edict
        .iter()
        .find(|(k, _)| cstr(k) == key)
        .map(|(_, v)| cstr(v))
}

pub(crate) fn required(
    entity: &Entity,
    name: &'static str,
    key: &str,
) -> Result<String, TileError> {
    prop(entity, key).ok_or(TileError::MissingProperty {
        entity: name,
        key: key.to_string(),
    })
}

pub(crate) fn face_from_name(name: &str) -> Result<HexFace, TileError> {
    Ok(match name {
        "east" => HexFace::East,
        "south_east" => HexFace::SouthEast,
        "south_west" => HexFace::SouthWest,
        "west" => HexFace::West,
        "north_west" => HexFace::NorthWest,
        "north_east" => HexFace::NorthEast,
        "up" => HexFace::Up,
        "down" => HexFace::Down,
        other => return Err(TileError::UnknownFace(other.to_string())),
    })
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

pub(crate) fn class_from_name(name: &str) -> Result<PortClass, TileError> {
    Ok(match name {
        "door" => PortClass::Door,
        "ramp_open" => PortClass::RampOpen,
        "shaft_open" => PortClass::ShaftOpen,
        other => return Err(TileError::UnknownClass(other.to_string())),
    })
}

pub(crate) fn class_name(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "sealed",
        PortClass::Door => "door",
        PortClass::RampOpen => "ramp_open",
        PortClass::ShaftOpen => "shaft_open",
    }
}

/// TrenchBroom Z-up units -> world Y-up meters.
fn to_world(point: [f64; 3]) -> Vec3 {
    Vec3::new(
        (point[0] / UNITS_PER_METER) as f32,
        (point[2] / UNITS_PER_METER) as f32,
        (-point[1] / UNITS_PER_METER) as f32,
    )
}

fn parse_origin(entity: &Entity, name: &'static str) -> Result<[f64; 3], TileError> {
    let value = required(entity, name, "origin")?;
    let values = value
        .split_ascii_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TileError::MissingProperty {
            entity: name,
            key: "origin (three numbers)".to_string(),
        })?;
    if values.len() != 3 {
        return Err(TileError::MissingProperty {
            entity: name,
            key: "origin (three numbers)".to_string(),
        });
    }
    Ok([values[0], values[1], values[2]])
}

/// Canonical footprint corner in TrenchBroom units: world plan `(x, z)` maps
/// to editor `(x * S, -z * S)`.
fn tb_corner(corner: (i32, i32)) -> [f64; 2] {
    [
        f64::from(corner.0) * UNITS_PER_METER,
        f64::from(-corner.1) * UNITS_PER_METER,
    ]
}

/// Exact-snap validation: every vertex must lie inside (or on) the canonical
/// hex prism for the tile's level span.
#[derive(Clone, Copy)]
struct FootprintPrism {
    q: i16,
    r: i16,
    level: i8,
    levels: u8,
}

fn footprint_prisms(map: &QuakeMap, levels: u8) -> Result<Vec<FootprintPrism>, TileError> {
    let mut cells = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_cell") {
            continue;
        }
        let parse = |key: &str| -> Result<i16, TileError> {
            required(entity, "tile_cell", key)?
                .parse()
                .map_err(|_| TileError::MissingProperty {
                    entity: "tile_cell",
                    key: format!("{key} (integer)"),
                })
        };
        let level = parse("level")?;
        let span = prop(entity, "levels")
            .unwrap_or_else(|| "1".to_string())
            .parse::<u8>()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_cell",
                key: "levels (u8)".to_string(),
            })?;
        if span == 0 {
            return Err(TileError::InvalidLevels);
        }
        cells.push(FootprintPrism {
            q: parse("q")?,
            r: parse("r")?,
            level: i8::try_from(level).map_err(|_| TileError::MissingProperty {
                entity: "tile_cell",
                key: "level (i8)".to_string(),
            })?,
            levels: span,
        });
    }
    if cells.is_empty() {
        cells.push(FootprintPrism {
            q: 0,
            r: 0,
            level: 0,
            levels,
        });
    }
    Ok(cells)
}

fn inside_plan_footprint(vertex: [f64; 3], cell: FootprintPrism) -> bool {
    let origin_x = f64::from(i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * UNITS_PER_METER;
    let origin_y = f64::from(-i32::from(cell.r) * 12) * UNITS_PER_METER;
    let local = [vertex[0] - origin_x, vertex[1] - origin_y];
    for face in HexFace::LATERAL {
        let [a, b] = face_edge(face).map(tb_corner);
        let edge_side = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if edge_side(local) * edge_side([0.0, 0.0]).signum() < -SNAP_EPSILON * length {
            return false;
        }
    }
    true
}

fn validate_footprint(vertices: &[[f64; 3]], cells: &[FootprintPrism]) -> Result<(), TileError> {
    let level_height = f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER;
    for &vertex in vertices {
        let inside = cells.iter().copied().any(|cell| {
            let floor = f64::from(cell.level) * level_height;
            let ceiling = floor + f64::from(cell.levels) * level_height;
            vertex[2] >= floor - SNAP_EPSILON
                && vertex[2] <= ceiling + SNAP_EPSILON
                && inside_plan_footprint(vertex, cell)
        });
        if !inside {
            let any_vertical_span = cells.iter().copied().any(|cell| {
                let floor = f64::from(cell.level) * level_height;
                let ceiling = floor + f64::from(cell.levels) * level_height;
                vertex[2] >= floor - SNAP_EPSILON && vertex[2] <= ceiling + SNAP_EPSILON
            });
            if !any_vertical_span {
                return Err(TileError::FootprintViolation {
                    vertex,
                    boundary: "vertical bounds of declared tile_cell footprint".to_string(),
                });
            }
            if cells.len() == 1 {
                let cell = cells[0];
                let origin_x =
                    f64::from(i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * UNITS_PER_METER;
                let origin_y = f64::from(-i32::from(cell.r) * 12) * UNITS_PER_METER;
                let local = [vertex[0] - origin_x, vertex[1] - origin_y];
                for face in HexFace::LATERAL {
                    let [a, b] = face_edge(face).map(tb_corner);
                    let side =
                        |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
                    let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                    if side(local) * side([0.0, 0.0]).signum() < -SNAP_EPSILON * length {
                        return Err(TileError::FootprintViolation {
                            vertex,
                            boundary: format!("{} face plane", face_name(face)),
                        });
                    }
                }
            }
            return Err(TileError::FootprintViolation {
                vertex,
                boundary: "declared tile_cell footprint union".to_string(),
            });
        }
    }
    Ok(())
}

/// Parse and validate one tile `.map` text.
pub fn parse_tile(text: &str) -> Result<TilePrototype, TileError> {
    let map: QuakeMap = quake_map::parse(&mut std::io::Cursor::new(text))
        .map_err(|error| TileError::Parse(error.to_string()))?;

    let mut meta: Option<&Entity> = None;
    let mut ports = [PortClass::Sealed; 8];
    let mut seen_origin_ports = [false; 8];
    let mut worldspawn: Option<&Entity> = None;
    for entity in &map.entities {
        match prop(entity, "classname").as_deref() {
            Some("worldspawn") => worldspawn = Some(entity),
            Some("tile_meta") => {
                if meta.replace(entity).is_some() {
                    return Err(TileError::DuplicateMeta);
                }
            }
            Some("tile_port") => {
                let face = face_from_name(&required(entity, "tile_port", "face")?)?;
                let class = class_from_name(&required(entity, "tile_port", "class")?)?;
                let q = prop(entity, "q")
                    .and_then(|value| value.parse::<i16>().ok())
                    .unwrap_or(0);
                let r = prop(entity, "r")
                    .and_then(|value| value.parse::<i16>().ok())
                    .unwrap_or(0);
                let level = prop(entity, "level")
                    .and_then(|value| value.parse::<i8>().ok())
                    .unwrap_or(0);
                // TilePrototype's compatibility signature describes the
                // origin cell. Whole-room ports are retained by the richer
                // authoring schema and compiled catalog.
                if q == 0 && r == 0 && level == 0 {
                    if seen_origin_ports[face.index()] {
                        return Err(TileError::DuplicatePort { face });
                    }
                    seen_origin_ports[face.index()] = true;
                    ports[face.index()] = class;
                }
            }
            _ => {}
        }
    }
    let meta = meta.ok_or(TileError::MissingMeta)?;
    let key = TileKey {
        archetype: required(meta, "tile_meta", "archetype")?,
        register: required(meta, "tile_meta", "register")?,
        variant: required(meta, "tile_meta", "variant")?
            .parse()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_meta",
                key: "variant (u16)".to_string(),
            })?,
    };
    let weight = prop(meta, "weight")
        .unwrap_or_else(|| "1".to_string())
        .parse::<u16>()
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| TileError::MissingProperty {
            entity: "tile_meta",
            key: "weight (1..=1000)".to_string(),
        })?;
    let levels: u8 = required(meta, "tile_meta", "levels")?
        .parse()
        .map_err(|_| TileError::MissingProperty {
            entity: "tile_meta",
            key: "levels (u8)".to_string(),
        })?;
    if levels == 0 {
        return Err(TileError::InvalidLevels);
    }

    let signature =
        PortSignature::try_from_ports(ports).map_err(|invalid| TileError::InvalidPort {
            face: invalid.face,
            class: invalid.class,
        })?;

    let footprint = footprint_prisms(&map, levels)?;
    let mut hulls = Vec::new();
    if let Some(world) = worldspawn {
        for (index, brush) in world.brushes.iter().enumerate() {
            let vertices = brush_vertices(brush).ok_or(TileError::DegenerateBrush { index })?;
            validate_footprint(&vertices, &footprint)?;
            hulls.push(vertices.iter().map(|&v| to_world(v)).collect());
        }
    }

    let mut lights = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_light") {
            continue;
        }
        let origin = parse_origin(entity, "tile_light")?;
        validate_footprint(&[origin], &footprint)?;
        let kind = match prop(entity, "kind").as_deref().unwrap_or("practical") {
            "practical" => TileLightKind::Practical,
            other => return Err(TileError::UnknownLightKind(other.to_string())),
        };
        lights.push(TileLight {
            kind,
            position: to_world(origin),
        });
    }
    lights.sort_by(|a, b| {
        a.position
            .x
            .total_cmp(&b.position.x)
            .then(a.position.y.total_cmp(&b.position.y))
            .then(a.position.z.total_cmp(&b.position.z))
    });

    Ok(TilePrototype {
        key,
        weight,
        levels,
        signature,
        hulls,
        lights,
    })
}

/// Read and parse a tile `.map` from disk.
pub fn load_tile(path: &std::path::Path) -> Result<TilePrototype, TileError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| TileError::Parse(format!("{}: {error}", path.display())))?;
    parse_tile(&text)
}
