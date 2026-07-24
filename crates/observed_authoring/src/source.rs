//! First-class TrenchBroom source metadata and authoring-contract validation.
//!
//! Arc L maps remain valid as `authoring_version = 1` compatibility sources.
//! New hand-authored modules opt into version 2, which adds stable IDs,
//! explicit footprint cells, spatial ports, and geometry budgets. The richer
//! schema is deliberately importer-only: editor entities compile to these pure
//! records and never enter simulation as entities.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use glam::{Vec2, Vec3};
use observed_hex::{CORNERS, HexFace, PortClass, TILE_LEVEL_HEIGHT, face_edge};
use quake_map::{Entity, QuakeMap};
use serde::{Deserialize, Serialize};

use crate::UNITS_PER_METER;
use crate::tile::{
    TileError, TilePrototype, class_from_name, face_from_name, parse_tile, prop, required,
};

const PORT_EPSILON_UNITS: f64 = 1.0;
const MIN_HEADROOM_METERS: f32 = 2.2;
const CELL_HULL_BUDGET: usize = 32;
const ROOM_HULL_BUDGET: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ModuleKind {
    Cell,
    Room,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RotationPolicy {
    None,
    SixFold,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FloorPolicy {
    Solid,
    Ramp,
    Open,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ModuleCell {
    pub q: i16,
    pub r: i16,
    pub level: i8,
    pub levels: u8,
    pub floor: FloorPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModulePort {
    pub cell: ModuleCellRef,
    pub face: HexFace,
    pub class: PortClass,
    /// TrenchBroom Z-up coordinates in integer editor units.
    pub origin: Option<[f64; 3]>,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ModuleCellRef {
    pub q: i16,
    pub r: i16,
    pub level: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredModule {
    pub authoring_version: u8,
    pub id: String,
    pub kind: ModuleKind,
    pub archetype: String,
    pub room_role: Option<String>,
    /// `all` means structural geometry is shared by every architecture
    /// register. Legacy maps contain their one historical register here.
    pub register_scope: Vec<String>,
    pub rotation: RotationPolicy,
    pub weight: u16,
    pub footprint: Vec<ModuleCell>,
    pub ports: Vec<ModulePort>,
    pub prototype: TilePrototype,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceError {
    Tile(TileError),
    Parse(String),
    InvalidProperty {
        entity: &'static str,
        detail: String,
    },
    DuplicateCell(ModuleCellRef),
    DuplicatePort {
        cell: ModuleCellRef,
        face: HexFace,
    },
    CellModuleFootprint,
    RoomRoleMissing,
    PortCellMissing(ModuleCellRef),
    PortOnInternalFace {
        cell: ModuleCellRef,
        face: HexFace,
    },
    MissingPortOrigin {
        cell: ModuleCellRef,
        face: HexFace,
    },
    PortOriginMismatch {
        cell: ModuleCellRef,
        face: HexFace,
        expected: [f64; 3],
        actual: [f64; 3],
    },
    DisconnectedFootprint,
    ComplexityBudget {
        hulls: usize,
        maximum: usize,
    },
    FloorGap(ModuleCellRef),
    Headroom {
        cell: ModuleCellRef,
        meters: f32,
    },
    MissingRampSurface(ModuleCellRef),
    RampTooSteep {
        cell: ModuleCellRef,
        slope: f32,
    },
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SourceError {}

impl From<TileError> for SourceError {
    fn from(value: TileError) -> Self {
        Self::Tile(value)
    }
}

fn parse_i16(entity: &Entity, name: &'static str, key: &str) -> Result<i16, SourceError> {
    required(entity, name, key)?
        .parse()
        .map_err(|_| SourceError::InvalidProperty {
            entity: name,
            detail: format!("{key} must be an i16"),
        })
}

fn parse_i8(entity: &Entity, name: &'static str, key: &str) -> Result<i8, SourceError> {
    required(entity, name, key)?
        .parse()
        .map_err(|_| SourceError::InvalidProperty {
            entity: name,
            detail: format!("{key} must be an i8"),
        })
}

fn parse_origin(value: &str) -> Option<[f64; 3]> {
    let values: Vec<f64> = value
        .split_ascii_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    (values.len() == 3).then_some([values[0], values[1], values[2]])
}

fn cell_ref(entity: &Entity, required_coordinates: bool) -> Result<ModuleCellRef, SourceError> {
    if !required_coordinates && prop(entity, "q").is_none() {
        return Ok(ModuleCellRef {
            q: 0,
            r: 0,
            level: 0,
        });
    }
    Ok(ModuleCellRef {
        q: parse_i16(entity, "tile_port", "q")?,
        r: parse_i16(entity, "tile_port", "r")?,
        level: parse_i8(entity, "tile_port", "level")?,
    })
}

fn expanded_cells(footprint: &[ModuleCell]) -> BTreeSet<ModuleCellRef> {
    let mut cells = BTreeSet::new();
    for cell in footprint {
        for offset in 0..cell.levels {
            cells.insert(ModuleCellRef {
                q: cell.q,
                r: cell.r,
                level: cell.level.saturating_add_unsigned(offset),
            });
        }
    }
    cells
}

fn neighbor(cell: ModuleCellRef, face: HexFace) -> ModuleCellRef {
    let (dq, dr, dl) = face.delta();
    ModuleCellRef {
        q: cell.q.saturating_add(dq as i16),
        r: cell.r.saturating_add(dr as i16),
        level: cell.level.saturating_add(dl as i8),
    }
}

fn expected_port_origin(cell: ModuleCellRef, face: HexFace) -> [f64; 3] {
    let world_x = i32::from(cell.q) * 14 + i32::from(cell.r) * 7;
    let world_z = i32::from(cell.r) * 12;
    if face.is_lateral() {
        let [a, b] = face_edge(face);
        return [
            (f64::from(world_x) + f64::from(a.0 + b.0) * 0.5) * UNITS_PER_METER,
            -(f64::from(world_z) + f64::from(a.1 + b.1) * 0.5) * UNITS_PER_METER,
            (f64::from(cell.level) * f64::from(TILE_LEVEL_HEIGHT) + 3.0) * UNITS_PER_METER,
        ];
    }
    let level = match face {
        HexFace::Up => i16::from(cell.level) + 1,
        HexFace::Down => i16::from(cell.level),
        _ => unreachable!(),
    };
    [
        f64::from(world_x) * UNITS_PER_METER,
        -f64::from(world_z) * UNITS_PER_METER,
        f64::from(level) * f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER,
    ]
}

fn validate_ports(module: &AuthoredModule) -> Result<(), SourceError> {
    let cells = expanded_cells(&module.footprint);
    let mut seen = BTreeSet::new();
    for port in &module.ports {
        if !cells.contains(&port.cell) {
            return Err(SourceError::PortCellMissing(port.cell));
        }
        if !seen.insert((port.cell, port.face)) {
            return Err(SourceError::DuplicatePort {
                cell: port.cell,
                face: port.face,
            });
        }
        let prefab_ramp_handoff = module.kind == ModuleKind::Cell
            && port.face == HexFace::Up
            && port.class == PortClass::RampOpen;
        if module.authoring_version >= 2
            && cells.contains(&neighbor(port.cell, port.face))
            && !prefab_ramp_handoff
        {
            return Err(SourceError::PortOnInternalFace {
                cell: port.cell,
                face: port.face,
            });
        }
        if module.authoring_version >= 2 {
            let actual = port.origin.ok_or(SourceError::MissingPortOrigin {
                cell: port.cell,
                face: port.face,
            })?;
            let expected = expected_port_origin(port.cell, port.face);
            if actual
                .iter()
                .zip(expected)
                .any(|(a, b)| (a - b).abs() > PORT_EPSILON_UNITS)
            {
                return Err(SourceError::PortOriginMismatch {
                    cell: port.cell,
                    face: port.face,
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(())
}

fn validate_connectivity(module: &AuthoredModule) -> Result<(), SourceError> {
    let cells = expanded_cells(&module.footprint);
    let Some(&start) = cells.first() else {
        return Err(SourceError::DisconnectedFootprint);
    };
    let mut reached = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        for face in HexFace::ALL {
            let next = neighbor(cell, face);
            if cells.contains(&next) && reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    (reached.len() == cells.len())
        .then_some(())
        .ok_or(SourceError::DisconnectedFootprint)
}

#[derive(Clone, Copy)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

fn bounds(hull: &[Vec3]) -> Bounds {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &point in hull {
        min = min.min(point);
        max = max.max(point);
    }
    Bounds { min, max }
}

fn plan_origin(cell: ModuleCellRef) -> Vec3 {
    Vec3::new(
        f32::from(cell.q) * 14.0 + f32::from(cell.r) * 7.0,
        f32::from(cell.level) * TILE_LEVEL_HEIGHT,
        f32::from(cell.r) * 12.0,
    )
}

fn covers_plan_point(bounds: Bounds, point: Vec3) -> bool {
    bounds.min.x <= point.x + 0.05
        && bounds.max.x >= point.x - 0.05
        && bounds.min.z <= point.z + 0.05
        && bounds.max.z >= point.z - 0.05
}

fn point_in_plan_hull(hull: &[Vec3], point: Vec2) -> bool {
    let mut points = hull
        .iter()
        .map(|point| Vec2::new(point.x, point.z))
        .collect::<Vec<_>>();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return false;
    }
    let cross = |origin: Vec2, a: Vec2, b: Vec2| (a - origin).perp_dot(b - origin);
    let mut lower = Vec::new();
    for candidate in points.iter().copied() {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], candidate) <= 0.0
        {
            lower.pop();
        }
        lower.push(candidate);
    }
    let mut upper = Vec::new();
    for candidate in points.iter().rev().copied() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], candidate) <= 0.0
        {
            upper.pop();
        }
        upper.push(candidate);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
        .iter()
        .copied()
        .zip(lower.iter().copied().cycle().skip(1))
        .take(lower.len())
        .all(|(a, b)| cross(a, b, point) >= -0.02)
}

fn floor_top_at(module: &AuthoredModule, bounds: &[Bounds], point: Vec3) -> Option<f32> {
    module
        .prototype
        .hulls
        .iter()
        .zip(bounds)
        .filter(|(hull, b)| {
            covers_plan_point(**b, point)
                && point_in_plan_hull(hull, Vec2::new(point.x, point.z))
                && b.min.y <= point.y + 0.1
                && b.max.y >= point.y
                && b.max.y <= point.y + 1.5
        })
        .map(|(_, b)| b.max.y)
        .max_by(f32::total_cmp)
}

fn validate_floor_and_headroom(module: &AuthoredModule) -> Result<(), SourceError> {
    let hull_bounds: Vec<Bounds> = module.prototype.hulls.iter().map(|h| bounds(h)).collect();
    for authored_cell in &module.footprint {
        if authored_cell.floor == FloorPolicy::Open {
            continue;
        }
        let cell = ModuleCellRef {
            q: authored_cell.q,
            r: authored_cell.r,
            level: authored_cell.level,
        };
        let center = plan_origin(cell);
        let floor_top =
            floor_top_at(module, &hull_bounds, center).ok_or(SourceError::FloorGap(cell))?;
        for (x, z) in CORNERS {
            let sample = center + Vec3::new(x as f32 * 0.78, 0.0, z as f32 * 0.78);
            floor_top_at(module, &hull_bounds, sample).ok_or(SourceError::FloorGap(cell))?;
        }
        let overhead = hull_bounds
            .iter()
            .copied()
            .zip(&module.prototype.hulls)
            .filter(|(b, hull)| {
                covers_plan_point(*b, center)
                    && point_in_plan_hull(hull, Vec2::new(center.x, center.z))
                    && b.min.y > floor_top + 0.05
            })
            .map(|(b, _)| b.min.y)
            .min_by(f32::total_cmp);
        if let Some(overhead) = overhead {
            let headroom = overhead - floor_top;
            if headroom < MIN_HEADROOM_METERS {
                return Err(SourceError::Headroom {
                    cell,
                    meters: headroom,
                });
            }
        }
    }
    Ok(())
}

fn validate_ramps(module: &AuthoredModule) -> Result<(), SourceError> {
    for authored_cell in &module.footprint {
        if authored_cell.floor != FloorPolicy::Ramp {
            continue;
        }
        let cell = ModuleCellRef {
            q: authored_cell.q,
            r: authored_cell.r,
            level: authored_cell.level,
        };
        let origin = plan_origin(cell);
        let candidate = module
            .prototype
            .hulls
            .iter()
            .map(|hull| (hull, bounds(hull)))
            .filter(|(_, b)| covers_plan_point(*b, origin) && b.min.y <= origin.y + 0.1)
            .max_by(|(_, a), (_, b)| (a.max.y - a.min.y).total_cmp(&(b.max.y - b.min.y)))
            .filter(|(_, b)| b.max.y - b.min.y >= TILE_LEVEL_HEIGHT - 0.6)
            .ok_or(SourceError::MissingRampSurface(cell))?;
        let plan_run = ((candidate.1.max.x - candidate.1.min.x).powi(2)
            + (candidate.1.max.z - candidate.1.min.z).powi(2))
        .sqrt();
        let slope = (candidate.1.max.y - candidate.1.min.y) / plan_run.max(0.01);
        if slope > 0.65 {
            return Err(SourceError::RampTooSteep { cell, slope });
        }
    }
    Ok(())
}

pub fn validate_module(module: &AuthoredModule) -> Result<(), SourceError> {
    let maximum = match module.kind {
        ModuleKind::Cell => CELL_HULL_BUDGET,
        ModuleKind::Room => ROOM_HULL_BUDGET,
    };
    if module.authoring_version >= 2 && module.prototype.hulls.len() > maximum {
        return Err(SourceError::ComplexityBudget {
            hulls: module.prototype.hulls.len(),
            maximum,
        });
    }
    validate_connectivity(module)?;
    validate_ports(module)?;
    if module.authoring_version >= 2 {
        validate_floor_and_headroom(module)?;
        validate_ramps(module)?;
    }
    Ok(())
}

/// Parse a legacy or version-2 module from one `.map` source.
pub fn parse_authored_module(text: &str) -> Result<AuthoredModule, SourceError> {
    let prototype = parse_tile(text)?;
    let map: QuakeMap = quake_map::parse(&mut std::io::Cursor::new(text))
        .map_err(|error| SourceError::Parse(error.to_string()))?;
    let meta = map
        .entities
        .iter()
        .find(|entity| prop(entity, "classname").as_deref() == Some("tile_meta"))
        .ok_or(TileError::MissingMeta)?;
    let authoring_version = prop(meta, "authoring_version")
        .unwrap_or_else(|| "1".to_string())
        .parse::<u8>()
        .map_err(|_| SourceError::InvalidProperty {
            entity: "tile_meta",
            detail: "authoring_version must be a u8".to_string(),
        })?;
    let kind = match prop(meta, "kind").as_deref().unwrap_or("cell") {
        "cell" => ModuleKind::Cell,
        "room" => ModuleKind::Room,
        other => {
            return Err(SourceError::InvalidProperty {
                entity: "tile_meta",
                detail: format!("unknown kind {other:?}"),
            });
        }
    };
    let room_role = prop(meta, "room_role").filter(|value| !value.is_empty());
    if kind == ModuleKind::Room && room_role.is_none() {
        return Err(SourceError::RoomRoleMissing);
    }
    let id = prop(meta, "id").unwrap_or_else(|| {
        format!(
            "legacy/{}/{}/v{}",
            prototype.key.archetype, prototype.key.register, prototype.key.variant
        )
    });
    if id.trim().is_empty() {
        return Err(SourceError::InvalidProperty {
            entity: "tile_meta",
            detail: "id must not be empty".to_string(),
        });
    }
    let register_scope = prop(meta, "register_scope")
        .map(|scope| {
            scope
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|scope| !scope.is_empty())
        .unwrap_or_else(|| vec![prototype.key.register.clone()]);
    let rotation = match prop(meta, "rotation_policy").as_deref().unwrap_or("none") {
        "none" => RotationPolicy::None,
        "sixfold" => RotationPolicy::SixFold,
        other => {
            return Err(SourceError::InvalidProperty {
                entity: "tile_meta",
                detail: format!("unknown rotation_policy {other:?}"),
            });
        }
    };
    let weight = prop(meta, "weight")
        .unwrap_or_else(|| "1".to_string())
        .parse::<u16>()
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| SourceError::InvalidProperty {
            entity: "tile_meta",
            detail: "weight must be in 1..=1000".to_string(),
        })?;

    let mut footprint = Vec::new();
    let mut footprint_refs = BTreeSet::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_cell") {
            continue;
        }
        let cell_ref = ModuleCellRef {
            q: parse_i16(entity, "tile_cell", "q")?,
            r: parse_i16(entity, "tile_cell", "r")?,
            level: parse_i8(entity, "tile_cell", "level")?,
        };
        if !footprint_refs.insert(cell_ref) {
            return Err(SourceError::DuplicateCell(cell_ref));
        }
        let levels = prop(entity, "levels")
            .unwrap_or_else(|| "1".to_string())
            .parse::<u8>()
            .ok()
            .filter(|levels| *levels > 0)
            .ok_or_else(|| SourceError::InvalidProperty {
                entity: "tile_cell",
                detail: "levels must be a positive u8".to_string(),
            })?;
        let floor = match prop(entity, "floor").as_deref().unwrap_or("solid") {
            "solid" => FloorPolicy::Solid,
            "ramp" => FloorPolicy::Ramp,
            "open" => FloorPolicy::Open,
            other => {
                return Err(SourceError::InvalidProperty {
                    entity: "tile_cell",
                    detail: format!("unknown floor policy {other:?}"),
                });
            }
        };
        footprint.push(ModuleCell {
            q: cell_ref.q,
            r: cell_ref.r,
            level: cell_ref.level,
            levels,
            floor,
        });
    }
    if footprint.is_empty() {
        footprint.push(ModuleCell {
            q: 0,
            r: 0,
            level: 0,
            levels: prototype.levels,
            floor: FloorPolicy::Solid,
        });
    }
    footprint.sort();
    if kind == ModuleKind::Cell
        && (footprint.len() != 1 || footprint[0].q != 0 || footprint[0].r != 0)
    {
        return Err(SourceError::CellModuleFootprint);
    }

    let mut ports = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_port") {
            continue;
        }
        let face = face_from_name(&required(entity, "tile_port", "face")?)?;
        let class = class_from_name(&required(entity, "tile_port", "class")?)?;
        let origin = match prop(entity, "origin") {
            Some(value) => {
                Some(
                    parse_origin(&value).ok_or_else(|| SourceError::InvalidProperty {
                        entity: "tile_port",
                        detail: "origin must contain three numbers".to_string(),
                    })?,
                )
            }
            None => None,
        };
        ports.push(ModulePort {
            cell: cell_ref(entity, authoring_version >= 2 && kind == ModuleKind::Room)?,
            face,
            class,
            origin,
            name: prop(entity, "name").unwrap_or_default(),
        });
    }
    ports.sort_by_key(|port| (port.cell, port.face));

    let module = AuthoredModule {
        authoring_version,
        id,
        kind,
        archetype: prototype.key.archetype.clone(),
        room_role,
        register_scope,
        rotation,
        weight,
        footprint,
        ports,
        prototype,
    };
    validate_module(&module)?;
    Ok(module)
}

/// A compact diagnostic summary used by the CLI and tile lab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSummary {
    pub id: String,
    pub strict: bool,
    pub kind: ModuleKind,
    pub footprint_cells: usize,
    pub ports: usize,
    pub hulls: usize,
    pub lights: usize,
}

impl From<&AuthoredModule> for ModuleSummary {
    fn from(module: &AuthoredModule) -> Self {
        Self {
            id: module.id.clone(),
            strict: module.authoring_version >= 2,
            kind: module.kind,
            footprint_cells: expanded_cells(&module.footprint).len(),
            ports: module.ports.len(),
            hulls: module.prototype.hulls.len(),
            lights: module.prototype.lights.len(),
        }
    }
}

/// Return port counts by class for audit output without exposing editor data.
pub fn port_class_counts(module: &AuthoredModule) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for port in &module.ports {
        let name = match port.class {
            PortClass::Sealed => "sealed",
            PortClass::Door => "door",
            PortClass::RampOpen => "ramp_open",
            PortClass::ShaftOpen => "shaft_open",
        };
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strict_map(extra_meta: &str, cells: &str, ports: &str) -> String {
        format!(
            "{{\n\"classname\" \"worldspawn\"\n{}\n}}\n{{\n\"classname\" \"tile_meta\"\n\"authoring_version\" \"2\"\n\"id\" \"test/module\"\n\"kind\" \"cell\"\n\"archetype\" \"hall_cap\"\n\"register\" \"institutional\"\n\"register_scope\" \"all\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n\"weight\" \"1\"\n{extra_meta}}}\n{cells}{ports}",
            crate::tile_source::hex_slab_brush(0.0, 8.0)
        )
    }

    #[test]
    fn strict_cell_accepts_spatial_port_and_floor_contract() {
        let cells = "{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"floor\" \"solid\"\n}\n";
        let ports = "{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"face\" \"east\"\n\"class\" \"door\"\n\"origin\" \"112 0 48\"\n}\n";
        let module = parse_authored_module(&strict_map("", cells, ports)).expect("valid module");
        assert_eq!(module.id, "test/module");
        assert_eq!(module.register_scope, ["all"]);
        assert_eq!(ModuleSummary::from(&module).footprint_cells, 1);
    }

    #[test]
    fn center_only_floor_is_rejected_before_it_can_open_seam_holes() {
        let cells = "{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"floor\" \"solid\"\n}\n";
        let map = strict_map("", cells, "").replacen(
            &crate::tile_source::hex_slab_brush(0.0, 8.0),
            &crate::tile_source::box_brush_text([-32, -32, 0], [32, 32, 8]),
            1,
        );
        assert!(matches!(
            parse_authored_module(&map),
            Err(SourceError::FloorGap(_))
        ));
    }

    #[test]
    fn strict_port_must_be_on_the_exact_external_boundary() {
        let cells =
            "{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n}\n";
        let ports = "{\n\"classname\" \"tile_port\"\n\"face\" \"east\"\n\"class\" \"door\"\n\"origin\" \"0 0 48\"\n}\n";
        assert!(matches!(
            parse_authored_module(&strict_map("", cells, ports)),
            Err(SourceError::PortOriginMismatch { .. })
        ));
    }

    #[test]
    fn room_ports_cannot_point_into_its_own_footprint() {
        let mut map = strict_map(
            "",
            "{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n}\n{\n\"classname\" \"tile_cell\"\n\"q\" \"1\"\n\"r\" \"0\"\n\"level\" \"0\"\n}\n",
            "{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"face\" \"east\"\n\"class\" \"door\"\n\"origin\" \"112 0 48\"\n}\n",
        );
        map = map.replace(
            "\"kind\" \"cell\"",
            "\"kind\" \"room\"\n\"room_role\" \"decision\"",
        );
        assert!(matches!(
            parse_authored_module(&map),
            Err(SourceError::PortOnInternalFace { .. }) | Err(SourceError::FloorGap(_))
        ));
    }
}
