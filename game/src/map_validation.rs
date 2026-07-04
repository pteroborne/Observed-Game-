//! Pure map/place validation for semantic map iterations.
//!
//! This audits the first-person teleport geometry that a [`MapSpec`] produces, without
//! launching Bevy rendering. Capture systems can use the same room sequence for visual
//! evidence, but geometry validity should fail in tests first.

use std::fmt;

use bevy::math::Vec2;
use observed_core::{Direction, RoomId};
use observed_facility::map_spec::{MapEdge, MapSpec, RoomRole, sector_relay_v1};
use observed_traversal::FpsConfig;

use crate::bot;
use crate::flow::MATCH_SEED;
use crate::hallway;
use crate::teleport::{
    self, GapKind, HallwayGeomEndpoints, Nav, Place, PlaceGeom, RoomConnectionSlot, ThresholdSlotId,
};

pub const DEFAULT_ITERATION_COUNT: usize = 24;
pub const DEFAULT_DECOHERE_VERSIONS: u32 = 4;
pub const MAP_AUDIT_CAPTURE_ROLES: [RoomRole; 6] = [
    RoomRole::Start,
    RoomRole::Keystone,
    RoomRole::AnchorCheckpoint,
    RoomRole::TeleportRelay,
    RoomRole::GuardianControl,
    RoomRole::Exit,
];

#[derive(Clone, Debug)]
pub struct MapPlaceReport {
    pub map_name: &'static str,
    pub seed: u64,
    pub version: u32,
    pub place: Place,
    pub room: Option<RoomId>,
    pub role: Option<RoomRole>,
    pub connections: Vec<RoomId>,
    pub bounds: Vec2,
    pub gap_count: usize,
    pub spawn: Vec2,
    pub screenshot_path: Option<String>,
}

impl fmt::Display for MapPlaceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} seed={} version={} place={:?} room={:?} role={} connections={:?} bounds=({:.2},{:.2}) gaps={} spawn=({:.2},{:.2})",
            self.map_name,
            self.seed,
            self.version,
            self.place,
            self.room,
            self.role.map(RoomRole::label).unwrap_or("n/a"),
            self.connections,
            self.bounds.x,
            self.bounds.y,
            self.gap_count,
            self.spawn.x,
            self.spawn.y
        )?;
        if let Some(path) = &self.screenshot_path {
            write!(f, " screenshot={path}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MapValidationIssue {
    pub report: MapPlaceReport,
    pub message: String,
}

impl fmt::Display for MapValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.message, self.report)
    }
}

pub fn semantic_capture_rooms(spec: &MapSpec) -> Vec<RoomId> {
    let mut rooms = Vec::new();
    for role in MAP_AUDIT_CAPTURE_ROLES {
        if let Some(room) = spec.rooms_with_role(role).first().copied()
            && !rooms.contains(&room)
        {
            rooms.push(room);
        }
    }
    rooms
}

pub fn audit_sector_relay_v1() -> Vec<MapValidationIssue> {
    audit_map_iterations(
        &sector_relay_v1(),
        MATCH_SEED,
        DEFAULT_ITERATION_COUNT,
        DEFAULT_DECOHERE_VERSIONS,
    )
}

pub fn audit_active_map() -> Vec<MapValidationIssue> {
    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    audit_map_iterations(
        &spec,
        MATCH_SEED,
        DEFAULT_ITERATION_COUNT,
        DEFAULT_DECOHERE_VERSIONS,
    )
}

pub fn audit_map_iterations(
    spec: &MapSpec,
    base_seed: u64,
    seed_count: usize,
    decohere_versions: u32,
) -> Vec<MapValidationIssue> {
    spec.validate_or_panic();
    let mut issues = Vec::new();
    let exit = spec.exit_room();
    let config = FpsConfig::default();

    for seed_offset in 0..seed_count {
        let seed = base_seed.wrapping_add(seed_offset as u64);
        for version in 0..decohere_versions {
            let edge_context = EdgeAuditContext {
                spec,
                seed,
                version,
                exit,
            };
            for room in &spec.rooms {
                let nav = nav_for_spec_room(spec, seed, version, room.id);
                let place = Place::Room(room.id);
                let geom = teleport::geom_for(place, &nav);
                let report = room_report(spec, seed, version, room.id, &nav, &geom, None);
                audit_room_geom(&mut issues, report, &geom, &nav, &config);
            }

            for edge in &spec.edges {
                audit_edge_direction(&mut issues, edge_context, edge, edge.a.room, edge.b.room);
                audit_edge_direction(&mut issues, edge_context, edge, edge.b.room, edge.a.room);
            }
        }
    }

    issues
}

pub fn room_report(
    spec: &MapSpec,
    seed: u64,
    version: u32,
    room: RoomId,
    nav: &Nav,
    geom: &PlaceGeom,
    screenshot_path: Option<String>,
) -> MapPlaceReport {
    MapPlaceReport {
        map_name: spec.name,
        seed,
        version,
        place: Place::Room(room),
        room: Some(room),
        role: spec.room(room).map(|room| room.role),
        connections: nav.connections.clone(),
        bounds: geom.half,
        gap_count: geom.gaps.len(),
        spawn: Vec2::ZERO,
        screenshot_path,
    }
}

pub fn capture_report_for_room(
    spec: &MapSpec,
    seed: u64,
    version: u32,
    room: RoomId,
    screenshot_path: String,
) -> MapPlaceReport {
    let nav = nav_for_spec_room(spec, seed, version, room);
    let geom = teleport::geom_for(Place::Room(room), &nav);
    room_report(
        spec,
        seed,
        version,
        room,
        &nav,
        &geom,
        Some(screenshot_path),
    )
}

#[derive(Clone, Copy)]
struct EdgeAuditContext<'a> {
    spec: &'a MapSpec,
    seed: u64,
    version: u32,
    exit: Option<RoomId>,
}

fn audit_edge_direction(
    issues: &mut Vec<MapValidationIssue>,
    context: EdgeAuditContext<'_>,
    edge: &MapEdge,
    from: RoomId,
    to: RoomId,
) {
    let Some(from_slot) = edge_slot(edge, from) else {
        return;
    };
    let Some(to_slot) = edge_slot(edge, to) else {
        return;
    };
    let variation = hallway::variation_for(from, to, context.seed, context.version);
    let place = Place::Hallway {
        from,
        to,
        variation,
    };
    let geom = teleport::hallway_geom_with_slots(
        HallwayGeomEndpoints {
            from,
            to,
            from_room_slot: from_slot,
            to_room_slot: to_slot,
            exit_room: context.exit.unwrap_or(to),
        },
        hallway::template(variation),
        hallway::layout_seed(from, to, variation),
        false,
    );
    let spawn = teleport::entry_spawn(&geom, from);
    let report = MapPlaceReport {
        map_name: context.spec.name,
        seed: context.seed,
        version: context.version,
        place,
        room: None,
        role: None,
        connections: vec![from, to],
        bounds: geom.half,
        gap_count: geom.gaps.len(),
        spawn,
        screenshot_path: None,
    };
    audit_hallway_geom(issues, report, &geom, from, to, &FpsConfig::default());
}

fn nav_for_spec_room(spec: &MapSpec, seed: u64, version: u32, room: RoomId) -> Nav {
    let connections = spec.neighbors(room);
    let target_room = target_for_room(spec, room, &connections);
    let connection_slots = connections
        .iter()
        .enumerate()
        .map(|(fallback, &target)| RoomConnectionSlot {
            target,
            slot: slot_for_connection(spec, room, target)
                .unwrap_or(ThresholdSlotId(fallback as u8)),
        })
        .collect();
    Nav {
        connections,
        connection_slots,
        sealed_slots: Vec::new(),
        hallway_entry_room_slot: None,
        hallway_exit_room_slot: None,
        target_room,
        room_role: spec.room(room).map(|room| room.role),
        seed,
        version,
        exit_locked: true,
        exit_room: spec.exit_room().unwrap_or(RoomId(0)),
        pins: Vec::new(),
    }
}

fn target_for_room(spec: &MapSpec, room: RoomId, connections: &[RoomId]) -> Option<RoomId> {
    let exit = spec.exit_room()?;
    if room == exit {
        return connections.first().copied();
    }
    spec.next_step_toward(room, exit)
        .filter(|target| connections.contains(target))
        .or_else(|| connections.first().copied())
}

fn slot_for_connection(spec: &MapSpec, room: RoomId, target: RoomId) -> Option<ThresholdSlotId> {
    spec.edges
        .iter()
        .find(|edge| {
            (edge.a.room == room && edge.b.room == target)
                || (edge.a.room == target && edge.b.room == room)
        })
        .and_then(|edge| edge_slot(edge, room))
}

fn edge_slot(edge: &MapEdge, room: RoomId) -> Option<ThresholdSlotId> {
    if edge.a.room == room {
        Some(slot_for_direction(edge.a.side))
    } else if edge.b.room == room {
        Some(slot_for_direction(edge.b.side))
    } else {
        None
    }
}

fn slot_for_direction(direction: Direction) -> ThresholdSlotId {
    ThresholdSlotId(direction.index() as u8)
}

fn audit_room_geom(
    issues: &mut Vec<MapValidationIssue>,
    report: MapPlaceReport,
    geom: &PlaceGeom,
    nav: &Nav,
    config: &FpsConfig,
) {
    audit_common(issues, report.clone(), geom);
    let Some(poly) = geom.poly.as_ref() else {
        push_issue(issues, report, "room rendered as non-polygon geometry");
        return;
    };
    if poly.len() < 4 {
        push_issue(issues, report.clone(), "room polygon has too few vertices");
    }
    if polygon_area(poly) <= 1.0 {
        push_issue(issues, report.clone(), "room polygon area collapsed");
    }
    if geom.gaps.len() != nav.connections.len() {
        push_issue(
            issues,
            report.clone(),
            format!(
                "room gap count {} does not match connection count {}",
                geom.gaps.len(),
                nav.connections.len()
            ),
        );
    }
    for gap in &geom.gaps {
        let Some((a, b)) = matching_edge(poly, gap.center) else {
            push_issue(
                issues,
                report.clone(),
                format!("gap to R{} is not centered on a polygon edge", gap.target.0),
            );
            continue;
        };
        let edge_len = (b - a).length();
        if gap.width > edge_len - 0.05 {
            push_issue(
                issues,
                report.clone(),
                format!(
                    "gap to R{} is too wide for its wall edge ({:.2} >= {:.2})",
                    gap.target.0, gap.width, edge_len
                ),
            );
        }
        let spawn = teleport::entry_spawn(geom, gap.target);
        if (teleport::contain(geom, spawn, config.radius) - spawn).length() > 0.08 {
            push_issue(
                issues,
                report.clone(),
                format!(
                    "entry spawn from R{} lies outside the room polygon",
                    gap.target.0
                ),
            );
        }
    }
    if let Some(forward) = geom.forward_gap() {
        let arena = teleport::place_arena(geom, 0.0, 4.6);
        if bot::route_to_gap(geom, &arena, config, Vec2::ZERO, forward).is_none() {
            push_issue(
                issues,
                report,
                "bot cannot route from room center to forward doorway",
            );
        }
    }
}

fn audit_hallway_geom(
    issues: &mut Vec<MapValidationIssue>,
    report: MapPlaceReport,
    geom: &PlaceGeom,
    from: RoomId,
    to: RoomId,
    config: &FpsConfig,
) {
    audit_common(issues, report.clone(), geom);
    if geom.poly.is_some() {
        push_issue(
            issues,
            report.clone(),
            "hallway rendered as polygon room geometry",
        );
    }
    if !geom
        .gaps
        .iter()
        .any(|gap| gap.kind == GapKind::Entry && gap.target == from)
    {
        push_issue(
            issues,
            report.clone(),
            "hallway has no entry gap for its source room",
        );
    }
    let Some(exit) = geom
        .gaps
        .iter()
        .filter(|gap| gap.kind == GapKind::Exit && gap.target == to)
        .min_by(|a, b| {
            a.floor_y
                .partial_cmp(&b.floor_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        push_issue(
            issues,
            report,
            "hallway has no open exit gap for its destination room",
        );
        return;
    };
    let arena = teleport::place_arena(geom, 0.0, 4.6);
    let spawn = teleport::entry_spawn(geom, from);
    if bot::route_to_gap(geom, &arena, config, spawn, exit).is_none() {
        push_issue(
            issues,
            report,
            "bot cannot route from hallway entry to exit",
        );
    }
}

fn audit_common(issues: &mut Vec<MapValidationIssue>, report: MapPlaceReport, geom: &PlaceGeom) {
    if !finite_vec2(geom.half) || geom.half.x <= 0.5 || geom.half.y <= 0.5 {
        push_issue(issues, report.clone(), "place bounds are invalid");
    }
    if geom.half.x.max(geom.half.y) > 48.0 {
        push_issue(
            issues,
            report.clone(),
            "place bounds exceed the expected renderer frame",
        );
    }
    for gap in &geom.gaps {
        if !finite_vec2(gap.center) || !finite_vec2(gap.normal) || !gap.width.is_finite() {
            push_issue(issues, report.clone(), "gap contains non-finite data");
        }
        if (gap.normal.length() - 1.0).abs() > 0.05 {
            push_issue(issues, report.clone(), "gap normal is not unit length");
        }
        if gap.width < 1.0 {
            push_issue(issues, report.clone(), "gap width is too narrow to read");
        }
        if gap.center.x.abs() > geom.half.x + 0.1 || gap.center.y.abs() > geom.half.y + 0.1 {
            push_issue(
                issues,
                report.clone(),
                "gap center lies outside place bounds",
            );
        }
    }
    for wall in &geom.interior {
        if !finite_vec2(wall.center)
            || !finite_vec2(wall.half)
            || wall.half.x < 0.0
            || wall.half.y < 0.0
        {
            push_issue(
                issues,
                report.clone(),
                "interior wall contains invalid bounds",
            );
        }
        if wall.center.x.abs() + wall.half.x > geom.half.x + 0.45
            || wall.center.y.abs() + wall.half.y > geom.half.y + 0.45
        {
            push_issue(
                issues,
                report.clone(),
                "interior wall extends outside place bounds",
            );
        }
    }
}

fn push_issue(
    issues: &mut Vec<MapValidationIssue>,
    report: MapPlaceReport,
    message: impl Into<String>,
) {
    issues.push(MapValidationIssue {
        report,
        message: message.into(),
    });
}

fn finite_vec2(v: Vec2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

fn polygon_area(poly: &[Vec2]) -> f32 {
    let mut area = 0.0;
    for pair in poly.windows(2) {
        area += pair[0].perp_dot(pair[1]);
    }
    if let (Some(first), Some(last)) = (poly.first(), poly.last()) {
        area += last.perp_dot(*first);
    }
    area.abs() * 0.5
}

fn matching_edge(poly: &[Vec2], center: Vec2) -> Option<(Vec2, Vec2)> {
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if ((a + b) * 0.5 - center).length() < 0.06 {
            return Some((a, b));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_relay_semantic_capture_sequence_has_representative_rooms() {
        let spec = sector_relay_v1();
        let rooms = semantic_capture_rooms(&spec);
        assert_eq!(rooms.len(), MAP_AUDIT_CAPTURE_ROLES.len());
        assert_eq!(rooms.first(), spec.start_room().as_ref());
        assert_eq!(rooms.last(), spec.exit_room().as_ref());
    }

    #[test]
    fn sector_relay_places_validate_across_map_iterations() {
        let issues = audit_map_iterations(&sector_relay_v1(), MATCH_SEED, 12, 3);
        assert!(
            issues.is_empty(),
            "semantic map geometry validation failed:\n{}",
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Phase 46: the generated default map (whatever the active catalog selection is,
    /// normally `liminal_wfc_v1`) must pass the same teleport-place geometry audit the
    /// authored map does — this is the game-side proof that generated rooms/hallways
    /// produce valid place geometry, not just a valid abstract `MapSpec` graph.
    #[test]
    fn active_map_places_validate_across_map_iterations() {
        let issues = audit_active_map();
        assert!(
            issues.is_empty(),
            "semantic map geometry validation failed on the active map:\n{}",
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
