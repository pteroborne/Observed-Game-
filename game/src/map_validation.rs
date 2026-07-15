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
    self, DoorGap, GapKind, HallwayGeomEndpoints, Nav, Place, PlaceGeom, RoomConnectionSlot,
    ThresholdSlotId, WallSeg,
};

pub const DEFAULT_ITERATION_COUNT: usize = 24;
pub const DEFAULT_DECOHERE_VERSIONS: u32 = 4;
/// Phase 46's liminal scale pass intentionally allows the longest authored connector
/// templates to reach about 56 world units at their maximum deterministic stretch. Keep
/// this audit as a runaway-geometry guard, but size it for the liminal renderer frame
/// rather than the old dev-scale frame.
const MAX_EXPECTED_PLACE_HALF: f32 = 64.0;
const THRESHOLD_RECT_DEPTH: f32 = 0.35;
const THRESHOLD_APPROACH_DEPTH: f32 = teleport::MAZE_CELL * 0.5;
const THRESHOLD_OVERLAP_DEPTH: f32 = teleport::ENTRY_INSET;
const THRESHOLD_CLEARANCE_EPS: f32 = 0.02;
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
    let place = Place::legacy_hallway(from, to, variation);
    let geom = teleport::hallway_geom_with_slots_and_role(
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
        context.spec.corridor_role_between(from, to),
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

pub(crate) fn nav_for_spec_room(spec: &MapSpec, seed: u64, version: u32, room: RoomId) -> Nav {
    let connections = spec.neighbors(room);
    let target_room = target_for_room(spec, room, &connections);
    let connection_slots = connections
        .iter()
        .enumerate()
        .map(|(fallback, &target)| RoomConnectionSlot {
            target,
            slot: slot_for_connection(spec, room, target)
                .unwrap_or(ThresholdSlotId(fallback as u16)),
        })
        .collect();
    let corridor_roles = connections
        .iter()
        .filter_map(|&target| {
            spec.corridor_role_between(room, target)
                .map(|role| (target, role))
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
        corridor_roles,
        live_corridors: Vec::new(),
        seed,
        version,
        exit_locked: true,
        exit_room: spec.exit_room().unwrap_or(RoomId(0)),
        pinned_corridors: Vec::new(),
        map_spec: Some(spec.clone()),
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
    ThresholdSlotId(direction.index() as u16)
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
        let primitives = teleport::place_structural_primitives(geom, 0.0, 4.6);
        if bot::route_to_gap(geom, &primitives, config, Vec2::ZERO, forward).is_none() {
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
    audit_hallway_threshold_integrity(issues, report.clone(), geom);
    if geom.poly.is_some()
        && !(geom.is_wellshaft() && geom.poly.as_ref().is_some_and(|poly| poly.len() == 6))
    {
        push_issue(
            issues,
            report.clone(),
            "non-wellshaft hallway rendered as polygon room geometry",
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
    let primitives = teleport::place_structural_primitives(geom, 0.0, 4.6);
    let spawn = teleport::entry_spawn(geom, from);
    if bot::route_to_gap(geom, &primitives, config, spawn, exit).is_none() {
        push_issue(
            issues,
            report,
            "bot cannot route from hallway entry to exit",
        );
    }
}

fn audit_hallway_threshold_integrity(
    issues: &mut Vec<MapValidationIssue>,
    report: MapPlaceReport,
    geom: &PlaceGeom,
) {
    for gap in &geom.gaps {
        let threshold = threshold_rect(gap, THRESHOLD_RECT_DEPTH);
        for wall in &geom.interior {
            if rect_intersects_wall(&threshold, wall) {
                push_issue(
                    issues,
                    report.clone(),
                    format!(
                        "threshold {} intersects interior wall at ({:.2},{:.2}) half=({:.2},{:.2})",
                        threshold_name(gap),
                        wall.center.x,
                        wall.center.y,
                        wall.half.x,
                        wall.half.y
                    ),
                );
            }
        }

        for (label, approach) in threshold_approach_rects(gap) {
            for wall in &geom.interior {
                if rect_intersects_wall(&approach, wall) {
                    push_issue(
                        issues,
                        report.clone(),
                        format!(
                            "threshold {} {label} approach is blocked by interior wall at ({:.2},{:.2}) half=({:.2},{:.2})",
                            threshold_name(gap),
                            wall.center.x,
                            wall.center.y,
                            wall.half.x,
                            wall.half.y
                        ),
                    );
                }
            }
        }
    }

    for i in 0..geom.gaps.len() {
        for j in (i + 1)..geom.gaps.len() {
            if geom.gaps[i].threshold == geom.gaps[j].threshold {
                push_issue(
                    issues,
                    report.clone(),
                    format!(
                        "threshold {} reuses the same stable socket at two apertures",
                        threshold_name(&geom.gaps[i])
                    ),
                );
            }
            let a = threshold_rect(&geom.gaps[i], THRESHOLD_OVERLAP_DEPTH);
            let b = threshold_rect(&geom.gaps[j], THRESHOLD_OVERLAP_DEPTH);
            if rects_intersect(&a, &b) {
                push_issue(
                    issues,
                    report.clone(),
                    format!(
                        "threshold {} overlaps threshold {}",
                        threshold_name(&geom.gaps[i]),
                        threshold_name(&geom.gaps[j])
                    ),
                );
            }
        }
    }
}

fn audit_common(issues: &mut Vec<MapValidationIssue>, report: MapPlaceReport, geom: &PlaceGeom) {
    if !finite_vec2(geom.half) || geom.half.x <= 0.5 || geom.half.y <= 0.5 {
        push_issue(issues, report.clone(), "place bounds are invalid");
    }
    if geom.half.x.max(geom.half.y) > MAX_EXPECTED_PLACE_HALF {
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
    let boundary_storage;
    let boundary = if let Some(poly) = geom.poly.as_ref() {
        poly.as_slice()
    } else {
        let (hx, hz) = (geom.half.x, geom.half.y);
        boundary_storage = [
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ];
        boundary_storage.as_slice()
    };
    if let Err(error) = teleport::plan_boundary(
        boundary,
        &geom.gaps,
        teleport::structural_height(geom, 4.6),
        4.6,
    ) {
        push_issue(
            issues,
            report.clone(),
            format!("threshold aperture plan is invalid: {error:?}"),
        );
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
        if teleport::is_point_on_segment(center, a, b, 0.08) {
            return Some((a, b));
        }
    }
    None
}

#[derive(Clone, Copy)]
struct OrientedRect {
    center: Vec2,
    tangent: Vec2,
    normal: Vec2,
    half_tangent: f32,
    half_normal: f32,
}

fn threshold_rect(gap: &DoorGap, depth: f32) -> OrientedRect {
    let normal = gap.normal.normalize_or_zero();
    OrientedRect {
        center: gap.center,
        tangent: Vec2::new(-normal.y, normal.x),
        normal,
        half_tangent: gap.width * 0.5,
        half_normal: depth,
    }
}

fn threshold_approach_rects(gap: &DoorGap) -> [(&'static str, OrientedRect); 2] {
    let normal = gap.normal.normalize_or_zero();
    let tangent = Vec2::new(-normal.y, normal.x);
    let half_normal = THRESHOLD_APPROACH_DEPTH * 0.5;
    [
        (
            "interior",
            OrientedRect {
                center: gap.center - normal * half_normal,
                tangent,
                normal,
                half_tangent: gap.width * 0.5,
                half_normal,
            },
        ),
        (
            "exterior",
            OrientedRect {
                center: gap.center + normal * half_normal,
                tangent,
                normal,
                half_tangent: gap.width * 0.5,
                half_normal,
            },
        ),
    ]
}

fn rect_intersects_wall(rect: &OrientedRect, wall: &WallSeg) -> bool {
    let wall_rect = OrientedRect {
        center: wall.center,
        tangent: Vec2::X,
        normal: Vec2::Y,
        half_tangent: wall.half.x,
        half_normal: wall.half.y,
    };
    rects_intersect(rect, &wall_rect)
}

fn rects_intersect(a: &OrientedRect, b: &OrientedRect) -> bool {
    for axis in [a.tangent, a.normal, b.tangent, b.normal] {
        if separates(*a, *b, axis) {
            return false;
        }
    }
    true
}

fn separates(a: OrientedRect, b: OrientedRect, axis: Vec2) -> bool {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() < f32::EPSILON {
        return false;
    }
    let (a_min, a_max) = project_rect(a, axis);
    let (b_min, b_max) = project_rect(b, axis);
    a_max <= b_min + THRESHOLD_CLEARANCE_EPS || b_max <= a_min + THRESHOLD_CLEARANCE_EPS
}

fn project_rect(rect: OrientedRect, axis: Vec2) -> (f32, f32) {
    let center = rect.center.dot(axis);
    let radius = rect.half_tangent * rect.tangent.dot(axis).abs()
        + rect.half_normal * rect.normal.dot(axis).abs();
    (center - radius, center + radius)
}

fn threshold_name(gap: &DoorGap) -> String {
    format!(
        "R{}:S{} -> H{}:S{}",
        gap.threshold.room.room.0,
        gap.threshold.room.slot.0,
        gap.threshold.hall.corridor.0,
        gap.threshold.hall.slot.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::map_spec::CorridorRole;
    use observed_match::mutable::EXIT_ROOM;
    use std::collections::{BTreeMap, BTreeSet};
    use std::io;
    use std::path::Path;

    #[test]
    fn sector_relay_semantic_capture_sequence_has_representative_rooms() {
        let spec = sector_relay_v1();
        let rooms = semantic_capture_rooms(&spec);
        assert_eq!(rooms.len(), MAP_AUDIT_CAPTURE_ROLES.len());
        assert_eq!(rooms.first(), spec.start_room().as_ref());
        assert_eq!(rooms.last(), spec.exit_room().as_ref());
    }

    #[test]
    fn production_room_geometry_consumes_the_map_template_catalog() {
        let spec = sector_relay_v1();
        for room in &spec.rooms {
            let nav = nav_for_spec_room(&spec, MATCH_SEED, 0, room.id);
            let geom = teleport::geom_for(Place::Room(room.id), &nav);
            assert_eq!(
                geom.poly.as_ref().unwrap().len(),
                usize::from(room.template.shell_profile().sides),
                "room {:?} must use its stored architectural template",
                room.id
            );
        }
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

    #[test]
    fn hallway_threshold_integrity_holds_for_template_seed_corpus() {
        let mut issues = Vec::new();
        let config = FpsConfig::default();
        let from = RoomId(0);
        let to = RoomId(1);

        for (variation, template) in hallway::TEMPLATES.iter().enumerate() {
            let roles: &[Option<CorridorRole>] = if template.grid.is_some() {
                &[None, Some(CorridorRole::Mystery)]
            } else if template.flavor == hallway::HallwayFlavor::Straight {
                &[None, Some(CorridorRole::Vertical)]
            } else {
                &[None]
            };
            for &role in roles {
                for seed in 0..64_u64 {
                    let geom = teleport::hallway_geom_with_slots_and_role(
                        HallwayGeomEndpoints {
                            from,
                            to,
                            from_room_slot: ThresholdSlotId(0),
                            to_room_slot: ThresholdSlotId(0),
                            exit_room: RoomId(EXIT_ROOM),
                        },
                        template,
                        seed,
                        false,
                        role,
                    );
                    let report = MapPlaceReport {
                        map_name: "template_threshold_corpus",
                        seed,
                        version: u32::from(role == Some(CorridorRole::Mystery)),
                        place: Place::legacy_hallway(from, to, variation),
                        room: None,
                        role: None,
                        connections: vec![from, to],
                        bounds: geom.half,
                        gap_count: geom.gaps.len(),
                        spawn: teleport::entry_spawn(&geom, from),
                        screenshot_path: None,
                    };
                    audit_hallway_geom(&mut issues, report, &geom, from, to, &config);
                }
            }
        }

        assert!(
            issues.is_empty(),
            "hallway threshold integrity failed:\n{}",
            summarize_threshold_issues(&issues)
        );
    }

    #[test]
    fn capture_threshold_integrity_wfc_hallways_when_requested() {
        let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_THRESHOLD_INTEGRITY") else {
            return;
        };
        std::fs::create_dir_all(&dir).expect("capture dir can be created");
        let from = RoomId(0);
        let to = RoomId(1);
        for (variation, seed) in [(8usize, 0u64), (12, 0)] {
            let template = hallway::template(variation);
            let geom = teleport::hallway_geom_with_slots_and_role(
                HallwayGeomEndpoints {
                    from,
                    to,
                    from_room_slot: ThresholdSlotId(0),
                    to_room_slot: ThresholdSlotId(0),
                    exit_room: RoomId(EXIT_ROOM),
                },
                template,
                seed,
                false,
                Some(CorridorRole::Mystery),
            );
            let path = Path::new(&dir).join(format!(
                "phase64_threshold_integrity_v{variation}_seed{seed}.bmp"
            ));
            render_hallway_integrity_bmp(&geom, &path).expect("threshold BMP capture writes");
        }
    }

    fn summarize_threshold_issues(issues: &[MapValidationIssue]) -> String {
        let mut by_place: BTreeMap<(usize, u32), BTreeSet<u64>> = BTreeMap::new();
        for issue in issues {
            if let Place::Hallway { variation, .. } = issue.report.place {
                by_place
                    .entry((variation, issue.report.version))
                    .or_default()
                    .insert(issue.report.seed);
            }
        }
        let mut lines: Vec<String> = by_place
            .into_iter()
            .map(|((variation, version), seeds)| {
                let seeds = seeds
                    .into_iter()
                    .map(|seed| seed.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("variation={variation} version={version} seeds={seeds}")
            })
            .collect();
        lines.push("sample issues:".to_string());
        lines.extend(issues.iter().take(24).map(ToString::to_string));
        if issues.len() > 24 {
            lines.push(format!("... {} more issue(s)", issues.len() - 24));
        }
        lines.join("\n")
    }

    #[derive(Clone, Copy)]
    struct Rgb(u8, u8, u8);

    fn render_hallway_integrity_bmp(geom: &PlaceGeom, path: &Path) -> io::Result<()> {
        const WIDTH: usize = 960;
        const HEIGHT: usize = 720;
        const PAD: f32 = 42.0;
        let mut pixels = vec![Rgb(10, 12, 16); WIDTH * HEIGHT];
        let scale_x = (WIDTH as f32 - PAD * 2.0) / (geom.half.x * 2.0 + 2.0);
        let scale_y = (HEIGHT as f32 - PAD * 2.0) / (geom.half.y * 2.0 + 2.0);
        let scale = scale_x.min(scale_y);

        let to_pixel = |p: Vec2| -> Vec2 {
            Vec2::new(
                WIDTH as f32 * 0.5 + p.x * scale,
                HEIGHT as f32 * 0.5 - p.y * scale,
            )
        };
        let to_world = |x: usize, y: usize| -> Vec2 {
            Vec2::new(
                (x as f32 + 0.5 - WIDTH as f32 * 0.5) / scale,
                -(y as f32 + 0.5 - HEIGHT as f32 * 0.5) / scale,
            )
        };

        fill_axis_rect(
            &mut pixels,
            WIDTH,
            HEIGHT,
            to_pixel,
            -geom.half,
            geom.half,
            Rgb(24, 30, 38),
        );
        for gap in &geom.gaps {
            for (_, approach) in threshold_approach_rects(gap) {
                fill_oriented_rect(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    to_pixel,
                    to_world,
                    approach,
                    Rgb(32, 86, 63),
                );
            }
        }
        for wall in &geom.interior {
            fill_axis_rect(
                &mut pixels,
                WIDTH,
                HEIGHT,
                to_pixel,
                wall.center - wall.half,
                wall.center + wall.half,
                Rgb(180, 190, 205),
            );
        }
        for gap in &geom.gaps {
            fill_oriented_rect(
                &mut pixels,
                WIDTH,
                HEIGHT,
                to_pixel,
                to_world,
                threshold_rect(gap, THRESHOLD_RECT_DEPTH),
                Rgb(33, 220, 160),
            );
        }

        write_bmp(path, WIDTH, HEIGHT, &pixels)
    }

    fn fill_axis_rect(
        pixels: &mut [Rgb],
        width: usize,
        height: usize,
        to_pixel: impl Fn(Vec2) -> Vec2,
        min: Vec2,
        max: Vec2,
        color: Rgb,
    ) {
        let a = to_pixel(min);
        let b = to_pixel(max);
        let min_x = a.x.min(b.x).floor().max(0.0) as usize;
        let max_x = a.x.max(b.x).ceil().min(width as f32) as usize;
        let min_y = a.y.min(b.y).floor().max(0.0) as usize;
        let max_y = a.y.max(b.y).ceil().min(height as f32) as usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                pixels[y * width + x] = color;
            }
        }
    }

    fn fill_oriented_rect(
        pixels: &mut [Rgb],
        width: usize,
        height: usize,
        to_pixel: impl Fn(Vec2) -> Vec2,
        to_world: impl Fn(usize, usize) -> Vec2,
        rect: OrientedRect,
        color: Rgb,
    ) {
        let corners = [
            rect.center + rect.tangent * rect.half_tangent + rect.normal * rect.half_normal,
            rect.center + rect.tangent * rect.half_tangent - rect.normal * rect.half_normal,
            rect.center - rect.tangent * rect.half_tangent + rect.normal * rect.half_normal,
            rect.center - rect.tangent * rect.half_tangent - rect.normal * rect.half_normal,
        ];
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for corner in corners {
            let p = to_pixel(corner);
            min = min.min(p);
            max = max.max(p);
        }
        let min_x = min.x.floor().max(0.0) as usize;
        let max_x = max.x.ceil().min(width as f32) as usize;
        let min_y = min.y.floor().max(0.0) as usize;
        let max_y = max.y.ceil().min(height as f32) as usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let p = to_world(x, y);
                let d = p - rect.center;
                if d.dot(rect.tangent).abs() <= rect.half_tangent
                    && d.dot(rect.normal).abs() <= rect.half_normal
                {
                    pixels[y * width + x] = color;
                }
            }
        }
    }

    fn write_bmp(path: &Path, width: usize, height: usize, pixels: &[Rgb]) -> io::Result<()> {
        let row_stride = (width * 3).div_ceil(4) * 4;
        let pixel_bytes = row_stride * height;
        let file_size = 14 + 40 + pixel_bytes;
        let mut out = Vec::with_capacity(file_size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&(54u32).to_le_bytes());
        out.extend_from_slice(&(40u32).to_le_bytes());
        out.extend_from_slice(&(width as i32).to_le_bytes());
        out.extend_from_slice(&(height as i32).to_le_bytes());
        out.extend_from_slice(&(1u16).to_le_bytes());
        out.extend_from_slice(&(24u16).to_le_bytes());
        out.extend_from_slice(&(0u32).to_le_bytes());
        out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        out.extend_from_slice(&(2835i32).to_le_bytes());
        out.extend_from_slice(&(2835i32).to_le_bytes());
        out.extend_from_slice(&(0u32).to_le_bytes());
        out.extend_from_slice(&(0u32).to_le_bytes());
        for y in (0..height).rev() {
            let row_start = out.len();
            for x in 0..width {
                let Rgb(r, g, b) = pixels[y * width + x];
                out.extend_from_slice(&[b, g, r]);
            }
            while out.len() - row_start < row_stride {
                out.push(0);
            }
        }
        std::fs::write(path, out)
    }
}
