//! Tile schema projection and exact-snap footprint validation.
//!
//! A tile `.map` contains: `worldspawn` brushes (the solid geometry), exactly
//! one `tile_meta` point entity (`archetype`, `register`, `variant`,
//! `levels`), and `tile_port` point entities (`face`, `class`). Projection is
//! importer-only — editor entities describe the tile, they never become the
//! game model.

use std::ffi::CString;

use glam::Vec3;
use observed_hex::{HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT, face_edge};
use observed_traversal::{ArenaSpec, ColliderShape, ColliderSpec, StableColliderId};
use quake_map::{Entity, QuakeMap};

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
    MissingProperty {
        entity: &'static str,
        key: String,
    },
    UnknownFace(String),
    UnknownClass(String),
    InvalidPort {
        face: HexFace,
        class: PortClass,
    },
    DegenerateBrush {
        index: usize,
    },
    /// A vertex escapes the canonical quantized-hexagon prism. Reports the
    /// offending vertex (TrenchBroom units), the violated boundary, and the
    /// exact bound so the author can fix the brush.
    FootprintViolation {
        vertex: [f64; 3],
        boundary: String,
    },
}

/// A validated, world-space tile ready for placement.
#[derive(Clone, Debug, PartialEq)]
pub struct TilePrototype {
    pub key: TileKey,
    /// Vertical levels this prefab spans (1 for flats, 2 for ramp prefabs).
    pub levels: u8,
    pub signature: PortSignature,
    /// Convex hulls in tile-local world meters: origin at the cell center,
    /// level 0 floor at y = 0.
    pub hulls: Vec<Vec<Vec3>>,
}

impl TilePrototype {
    /// Collider specs for one instance of this tile, ids offset by `base_id`,
    /// hulls translated by `offset` (typically `hex_origin` of the cell).
    pub fn collider_specs(&self, base_id: u32, offset: Vec3) -> Vec<ColliderSpec> {
        self.hulls
            .iter()
            .enumerate()
            .map(|(index, hull)| ColliderSpec {
                id: StableColliderId(base_id + index as u32),
                center: offset,
                rotation: [0.0, 0.0, 0.0, 1.0],
                shape: ColliderShape::ConvexHull {
                    points: hull.clone(),
                },
                friction: 0.8,
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

fn prop(entity: &Entity, key: &str) -> Option<String> {
    entity
        .edict
        .iter()
        .find(|(k, _)| cstr(k) == key)
        .map(|(_, v)| cstr(v))
}

fn required(entity: &Entity, name: &'static str, key: &str) -> Result<String, TileError> {
    prop(entity, key).ok_or(TileError::MissingProperty {
        entity: name,
        key: key.to_string(),
    })
}

fn face_from_name(name: &str) -> Result<HexFace, TileError> {
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

fn class_from_name(name: &str) -> Result<PortClass, TileError> {
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
fn validate_footprint(vertices: &[[f64; 3]], levels: u8) -> Result<(), TileError> {
    let ceiling = f64::from(levels) * f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER;
    for &vertex in vertices {
        if vertex[2] < -SNAP_EPSILON || vertex[2] > ceiling + SNAP_EPSILON {
            return Err(TileError::FootprintViolation {
                vertex,
                boundary: format!("vertical bounds 0..{ceiling} units (levels {levels})"),
            });
        }
        for face in HexFace::LATERAL {
            let [a, b] = face_edge(face).map(tb_corner);
            // Cross product side test against the centroid (origin); the
            // vertex must be on the same side as the interior, or on the edge.
            let edge_side =
                |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
            let interior = edge_side([0.0, 0.0]);
            let this = edge_side([vertex[0], vertex[1]]);
            // Normalize by edge length so the tolerance is in units.
            let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            if this * interior.signum() < -SNAP_EPSILON * length {
                let [ax, ay] = a;
                let [bx, by] = b;
                return Err(TileError::FootprintViolation {
                    vertex,
                    boundary: format!(
                        "{} face plane through ({ax}, {ay}) - ({bx}, {by})",
                        face_name(face)
                    ),
                });
            }
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
                ports[face.index()] = class;
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
    let levels: u8 = required(meta, "tile_meta", "levels")?
        .parse()
        .map_err(|_| TileError::MissingProperty {
            entity: "tile_meta",
            key: "levels (u8)".to_string(),
        })?;

    let signature =
        PortSignature::try_from_ports(ports).map_err(|invalid| TileError::InvalidPort {
            face: invalid.face,
            class: invalid.class,
        })?;

    let mut hulls = Vec::new();
    if let Some(world) = worldspawn {
        for (index, brush) in world.brushes.iter().enumerate() {
            let vertices = brush_vertices(brush).ok_or(TileError::DegenerateBrush { index })?;
            validate_footprint(&vertices, levels)?;
            hulls.push(vertices.iter().map(|&v| to_world(v)).collect());
        }
    }

    Ok(TilePrototype {
        key,
        levels,
        signature,
        hulls,
    })
}

/// Read and parse a tile `.map` from disk.
pub fn load_tile(path: &std::path::Path) -> Result<TilePrototype, TileError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| TileError::Parse(format!("{}: {error}", path.display())))?;
    parse_tile(&text)
}
