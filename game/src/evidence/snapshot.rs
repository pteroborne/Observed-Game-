//! World -> JSON snapshot collectors for the `OBSERVED2_VIS_AUDIT` visual audit.
//! `evidence::audit` drives the scenario phase machine and calls into
//! [`collect_snapshot`]; everything here just reads component/resource state and
//! builds the `observed_diagnostics` snapshot types (or, for the footprint atlas,
//! the debug overlay entities themselves).

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use observed_core::RoomId;
use observed_diagnostics::{
    DEFAULT_SIGNAL_MIN_LUMINANCE, DiagnosticFinding, DiagnosticRun, DiagnosticSnapshot,
    FootprintSnapshot, GeometrySnapshot, LightSnapshot, MaterialSnapshot, MonitorSnapshot,
    PlaceSnapshot, TacMapSnapshot, ThresholdSnapshot,
};
use observed_style::{self as style, MarkerRole};

use crate::evidence::audit::AuditScenario;
use crate::evidence::tags::{
    DiagnosticTacMapRole, DiagnosticTacMapVisual, DiagnosticThresholdStatus,
    DiagnosticThresholdVisual, DiagnosticThresholdVisualKind, threshold_label, threshold_status,
};
use crate::guardian::Guardian;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{MapKnowledge, RivalSightings, TeleportState};
use crate::tacmap;
use crate::teleport::{self, DoorGap, GapKind, Place};
use crate::view::components::{DroppedItemVisual, KeystoneItem, RivalAvatar, TacMapElement};

pub(super) type ThresholdVisualQueryData = (
    &'static DiagnosticThresholdVisual,
    Option<&'static PointLight>,
    Option<&'static Name>,
);

pub(super) type LightQueryData = (
    &'static PointLight,
    Option<&'static Name>,
    Option<&'static DiagnosticThresholdVisual>,
);

pub(super) type MaterialDiagnosticQueryData = (
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static Name>,
    Option<&'static KeystoneItem>,
    Option<&'static DroppedItemVisual>,
    Option<&'static RivalAvatar>,
    Option<&'static crate::guardian::GuardianModel>,
    Option<&'static crate::screens::place::GuardianConsole>,
);

pub(super) type MonitorDiagnosticQueryData = (
    Option<&'static crate::screens::place::TetherCameraMonitor>,
    Option<&'static crate::screens::place::GuardianObservationMonitor>,
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static Name>,
);

pub(super) type MonitorLabelSegmentDiagnosticQueryData =
    &'static crate::screens::place::ObservationMonitorLabelSegment;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn collect_snapshot(
    run_id: &str,
    scenario: AuditScenario,
    frame_index: u32,
    runtime: &MatchDirector,
    tp: &TeleportState,
    keys: &KeystoneState,
    items: &ItemsState,
    guardian: &Guardian,
    sightings: &RivalSightings,
    knowledge: &MapKnowledge,
    threshold_visuals: &Query<ThresholdVisualQueryData>,
    lights: &Query<LightQueryData>,
    materials_query: &Query<MaterialDiagnosticQueryData>,
    monitor_materials: &Query<MonitorDiagnosticQueryData>,
    monitor_label_segments: &Query<MonitorLabelSegmentDiagnosticQueryData>,
    tac_visuals: &Query<&DiagnosticTacMapVisual, With<TacMapElement>>,
    materials: &Assets<StandardMaterial>,
) -> DiagnosticSnapshot {
    let mut snapshot = DiagnosticSnapshot::new(run_id, scenario.label(), frame_index);
    snapshot.place = Some(PlaceSnapshot {
        label: place_label(tp.place),
        player_position: vec3_array(tp.body.position),
        player_yaw: tp.body.yaw,
        player_pitch: tp.body.pitch,
    });
    snapshot.geometry = collect_geometry(runtime, tp, keys, items, scenario);
    snapshot.thresholds = collect_thresholds(runtime, tp, keys, items, threshold_visuals);
    snapshot.lights = collect_lights(lights);
    snapshot.materials = collect_materials(materials_query, materials);
    snapshot.tac_map = Some(collect_tac_map(
        runtime,
        tp,
        keys,
        sightings,
        knowledge,
        tac_visuals,
    ));
    snapshot.monitors = collect_monitors(
        items,
        guardian,
        monitor_materials,
        monitor_label_segments,
        materials,
    );
    snapshot
}

fn collect_geometry(
    runtime: &MatchDirector,
    tp: &TeleportState,
    keys: &KeystoneState,
    items: &ItemsState,
    scenario: AuditScenario,
) -> GeometrySnapshot {
    if scenario == AuditScenario::FootprintAtlas {
        return collect_atlas_geometry(runtime, keys, items);
    }

    let mut footprints = vec![FootprintSnapshot {
        subject: format!("current {}", place_label(tp.place)),
        center: [0.0, 0.0],
        half: [tp.geom.half.x, tp.geom.half.y],
        allow_overlap: true,
    }];

    for gap in tp.geom.gaps.iter().filter(|gap| gap.kind.is_passage()) {
        let Some(dest) = tp
            .gap_dests
            .iter()
            .find(|dest| dest.threshold == gap.threshold)
        else {
            continue;
        };
        // `geom_for`'s Hallway arm looks up `Nav::corridor_role_for(to)`, so the
        // already-resolved frozen edge role (see `crossing::compute_gap_dests`) only
        // needs keying by the hallway's own `to` — the only neighbour a hallway
        // `Place` is ever queried about.
        let corridor_roles = match (dest.place, dest.corridor_role) {
            (Place::Hallway { to, .. }, Some(role)) => vec![(to, role)],
            _ => Vec::new(),
        };
        let nav = teleport::Nav {
            connections: dest.conns.clone(),
            connection_slots: dest.connection_slots.clone(),
            sealed_slots: dest.sealed_slots.clone(),
            hallway_entry_room_slot: dest.hallway_entry_room_slot,
            hallway_exit_room_slot: dest.hallway_exit_room_slot,
            target_room: dest.target,
            room_role: dest.room_role,
            corridor_roles,
            seed: 0,
            version: 0,
            exit_locked: !keys.gate_open(),
            exit_room: keys.exit_room,
            pinned_corridors: Vec::new(),
            map_spec: runtime.live.map_spec.clone(),
        };
        let geom = teleport::geom_for(dest.place, &nav);
        let Some((center, half)) = preview_aabb(tp.place, gap, dest.place, &geom) else {
            continue;
        };
        footprints.push(FootprintSnapshot {
            subject: format!("preview {}", place_label(dest.place)),
            center: [center.x, center.y],
            half: [half.x, half.y],
            allow_overlap: false,
        });
    }

    GeometrySnapshot { footprints }
}

fn collect_atlas_geometry(
    runtime: &MatchDirector,
    keys: &KeystoneState,
    items: &ItemsState,
) -> GeometrySnapshot {
    GeometrySnapshot {
        footprints: atlas_layout(runtime, keys, items)
            .into_iter()
            .map(|entry| FootprintSnapshot {
                subject: entry.subject,
                center: [entry.center.x, entry.center.y],
                half: [entry.half.x, entry.half.y],
                allow_overlap: false,
            })
            .collect(),
    }
}

pub(super) const ATLAS_Y: f32 = 22.0;
const ATLAS_PANEL_LEFT: f32 = 700.0;
const ATLAS_PANEL_TOP: f32 = 110.0;
const ATLAS_PANEL_SIZE: f32 = 560.0;
const ATLAS_LAYOUT_MARGIN: f32 = 14.0;
const ATLAS_GROUP_GAP: f32 = 36.0;
const ATLAS_UI_PADDING: f32 = 28.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AtlasKind {
    Room,
    SpineHallway,
    SideHallway,
}

#[derive(Clone, Debug)]
struct AtlasEntry {
    subject: String,
    center: Vec2,
    half: Vec2,
    kind: AtlasKind,
}

impl AtlasEntry {
    fn color(&self) -> Color {
        match self.kind {
            AtlasKind::Room => Color::srgb(0.04, 0.42, 1.0),
            AtlasKind::SpineHallway => Color::srgb(1.0, 0.80, 0.10),
            AtlasKind::SideHallway => Color::srgb(0.06, 1.0, 0.78),
        }
    }
}

fn atlas_layout(
    runtime: &MatchDirector,
    keys: &KeystoneState,
    items: &ItemsState,
) -> Vec<AtlasEntry> {
    let game = runtime.live.host_match();
    let mut rooms = Vec::new();
    let mut hallways = Vec::new();
    let room_entries: Vec<(RoomId, Vec2)> = game
        .competitive
        .map_spec
        .as_ref()
        .map(|spec| {
            spec.rooms
                .iter()
                .map(|room| (room.id, room.schematic))
                .collect()
        })
        .unwrap_or_else(|| {
            (0..9u32)
                .map(|id| {
                    let room = RoomId(id);
                    let grid = tacmap::grid_pos(room);
                    (room, Vec2::new(grid.x - 1.0, grid.y - 1.0))
                })
                .collect()
        });

    for (room, schematic) in room_entries {
        let nav = crate::sim::nav::nav_for_place(
            crate::flow::MATCH_SEED,
            game,
            keys,
            items,
            Place::Room(room),
        );
        let geom = teleport::geom_for(Place::Room(room), &nav);
        rooms.push(AtlasEntry {
            subject: format!("atlas room {}", room.0),
            center: schematic,
            half: geom.half,
            kind: AtlasKind::Room,
        });
    }

    for route in &game.rendered {
        let (from, to) = route.rooms;
        let variation =
            crate::hallway::variation_for(from, to, crate::flow::MATCH_SEED, game.reroute_commits);
        let place = Place::legacy_hallway(from, to, variation);
        let nav = crate::sim::nav::nav_for_place(crate::flow::MATCH_SEED, game, keys, items, place);
        let geom = teleport::geom_for(place, &nav);
        hallways.push(AtlasEntry {
            subject: format!("atlas hallway {} -> {} v{}", from.0, to.0, variation),
            center: Vec2::ZERO,
            half: geom.half,
            kind: if route.spine {
                AtlasKind::SpineHallway
            } else {
                AtlasKind::SideHallway
            },
        });
    }

    let room_max_half = max_half(&rooms);
    let room_step = (room_max_half.x.max(room_max_half.y) * 2.0 + ATLAS_LAYOUT_MARGIN).max(1.0);
    for room in &mut rooms {
        room.center *= room_step;
    }

    let hall_max_half = max_half(&hallways);
    let hall_step_x = (hall_max_half.x * 2.0 + ATLAS_LAYOUT_MARGIN).max(1.0);
    let hall_step_y = (hall_max_half.y * 2.0 + ATLAS_LAYOUT_MARGIN).max(1.0);
    let room_right = rooms
        .iter()
        .map(|entry| entry.center.x + entry.half.x)
        .fold(room_step + room_max_half.x, f32::max);
    let first_hall_center_x = room_right + ATLAS_GROUP_GAP + hall_max_half.x;
    let hall_cols = 4usize;
    for (index, hallway) in hallways.iter_mut().enumerate() {
        let col = index % hall_cols;
        let row = index / hall_cols;
        hallway.center = Vec2::new(
            first_hall_center_x + col as f32 * hall_step_x,
            (row as f32 - 1.0) * hall_step_y,
        );
    }

    rooms.extend(hallways);
    rooms
}

fn max_half(entries: &[AtlasEntry]) -> Vec2 {
    entries
        .iter()
        .fold(Vec2::ZERO, |max, entry| max.max(entry.half))
}

fn atlas_bounds(entries: &[AtlasEntry]) -> Option<(Vec2, Vec2)> {
    let first = entries.first()?;
    let mut min = first.center - first.half;
    let mut max = first.center + first.half;
    for entry in entries.iter().skip(1) {
        min = min.min(entry.center - entry.half);
        max = max.max(entry.center + entry.half);
    }
    Some((min, max))
}

pub(super) fn spawn_footprint_atlas(
    commands: &mut Commands,
    runtime: &MatchDirector,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let entries = atlas_layout(runtime, keys, items);
    let Some((min, max)) = atlas_bounds(&entries) else {
        return;
    };
    let bounds_size = (max - min).max(Vec2::splat(1.0));
    let scale = ((ATLAS_PANEL_SIZE - ATLAS_UI_PADDING * 2.0) / bounds_size.x)
        .min((ATLAS_PANEL_SIZE - ATLAS_UI_PADDING * 2.0) / bounds_size.y);

    commands
        .spawn((
            DespawnOnExit(crate::GameState::Match),
            Node {
                position_type: PositionType::Absolute,
                left: px(ATLAS_PANEL_LEFT),
                top: px(ATLAS_PANEL_TOP),
                width: px(ATLAS_PANEL_SIZE),
                height: px(ATLAS_PANEL_SIZE),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            BorderColor::all(Color::srgb(0.16, 0.95, 1.0)),
            Name::new("Footprint atlas panel"),
        ))
        .with_children(|parent| {
            for entry in entries.iter().filter(|entry| entry.kind != AtlasKind::Room) {
                spawn_atlas_rect(parent, entry, min, scale);
            }
            for entry in entries.iter().filter(|entry| entry.kind == AtlasKind::Room) {
                spawn_atlas_rect(parent, entry, min, scale);
            }
        });
}

fn spawn_atlas_rect(
    parent: &mut ChildSpawnerCommands,
    entry: &AtlasEntry,
    bounds_min: Vec2,
    scale: f32,
) {
    let c = Vec2::splat(ATLAS_UI_PADDING) + (entry.center - bounds_min) * scale;
    let s = (entry.half * 2.0 * scale).max(Vec2::splat(3.0));
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(c.x - s.x * 0.5),
            top: px(c.y - s.y * 0.5),
            width: px(s.x),
            height: px(s.y),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(entry.color()),
        BorderColor::all(Color::WHITE),
        Name::new(entry.subject.clone()),
    ));
}

pub(super) fn preview_aabb(
    place: Place,
    gap: &DoorGap,
    dest_place: Place,
    geom: &teleport::PlaceGeom,
) -> Option<(Vec2, Vec2)> {
    let align = match dest_place {
        Place::Hallway { .. } => teleport::hallway_alignment(gap, geom)?,
        Place::Room(dest_room) => {
            let back = match place {
                Place::Hallway { from, to, .. } => {
                    if dest_room == to {
                        from
                    } else {
                        to
                    }
                }
                Place::Room(src_room) => src_room,
            };
            let src = geom
                .gaps
                .iter()
                .find(|candidate| candidate.target == back)?;
            teleport::room_alignment(gap, src)
        }
    };
    let points = if let Some(poly) = geom.poly.as_ref() {
        poly.clone()
    } else {
        let half = geom.half;
        vec![
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ]
    };
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for point in points {
        let p = align.apply(point);
        min = min.min(p);
        max = max.max(p);
    }
    Some(((min + max) * 0.5, (max - min) * 0.5))
}

fn collect_thresholds(
    runtime: &MatchDirector,
    tp: &TeleportState,
    keys: &KeystoneState,
    items: &ItemsState,
    visuals: &Query<ThresholdVisualQueryData>,
) -> Vec<ThresholdSnapshot> {
    let game = runtime.live.host_match();
    let nav = crate::sim::nav::nav_for_place(crate::flow::MATCH_SEED, game, keys, items, tp.place);
    tp.geom
        .gaps
        .iter()
        .filter(|gap| gap.kind != GapKind::OneWayEntry)
        .map(|gap| {
            let (a, b) = match tp.place {
                Place::Room(room) => (room, gap.target),
                Place::Hallway { from, to, .. } => (from, to),
            };
            let tethered = nav.is_tethered_corridor(crate::teleport::corridor_id_for(a, b));
            let status = threshold_status(gap, tethered);
            let mut frame_count = 0;
            let mut leaf_count = 0;
            let mut frame_light_count = 0;
            let mut matching_status_light_count = 0;
            let mut label_count = 0;
            let expected_light = expected_threshold_light(status);
            for (visual, light, _name) in visuals {
                if visual.threshold != gap.threshold {
                    continue;
                }
                match visual.kind {
                    DiagnosticThresholdVisualKind::Frame => frame_count += 1,
                    DiagnosticThresholdVisualKind::Leaf => leaf_count += 1,
                    DiagnosticThresholdVisualKind::Label => label_count += 1,
                    DiagnosticThresholdVisualKind::FrameLight => {
                        frame_light_count += 1;
                        if let Some(light) = light
                            && visual.status == status
                            && color_distance(light.color, expected_light) <= 0.08
                            && light.intensity > 0.1
                        {
                            matching_status_light_count += 1;
                        }
                    }
                }
            }
            ThresholdSnapshot {
                subject: threshold_label(&gap.threshold),
                label: threshold_label(&gap.threshold),
                status: status.label().to_string(),
                target_room: gap.target.0,
                center: [
                    gap.center.x,
                    teleport::place_y_offset(tp.place),
                    gap.center.y,
                ],
                width: gap.width,
                passage: gap.kind.is_passage(),
                locked: gap.kind == GapKind::LockedExit,
                tethered,
                frame_count,
                leaf_count,
                frame_light_count,
                matching_status_light_count,
                label_count,
            }
        })
        .collect()
}

fn collect_lights(lights: &Query<LightQueryData>) -> Vec<LightSnapshot> {
    lights
        .iter()
        .map(|(light, name, threshold)| {
            let subject = threshold
                .map(|visual| format!("threshold {}", threshold_label(&visual.threshold)))
                .or_else(|| name.map(|name| name.as_str().to_string()))
                .unwrap_or_else(|| "unnamed light".to_string());
            LightSnapshot {
                subject,
                intensity: light.intensity,
                range: light.range,
                color_rgb: color_rgb(light.color),
                expected_min_intensity: 1.0,
            }
        })
        .collect()
}

fn collect_materials(
    query: &Query<MaterialDiagnosticQueryData>,
    materials: &Assets<StandardMaterial>,
) -> Vec<MaterialSnapshot> {
    query
        .iter()
        .filter_map(|(handle, name, key, dropped, rival, guardian, console)| {
            let material = materials.get(&handle.0)?;
            let signal = key.is_some()
                || dropped.is_some()
                || rival.is_some()
                || guardian.is_some()
                || console.is_some();
            if !signal && observed_diagnostics::luminance(linear_rgb(material.emissive)) <= 0.01 {
                return None;
            }
            Some(MaterialSnapshot {
                subject: name
                    .map(|name| name.as_str().to_string())
                    .unwrap_or_else(|| "unnamed material".to_string()),
                signal,
                base_rgb: color_rgb(material.base_color),
                emissive_rgb: linear_rgb(material.emissive),
                emissive_luminance: observed_diagnostics::luminance(linear_rgb(material.emissive)),
                min_luminance: DEFAULT_SIGNAL_MIN_LUMINANCE,
            })
        })
        .collect()
}

fn collect_tac_map(
    runtime: &MatchDirector,
    tp: &TeleportState,
    keys: &KeystoneState,
    sightings: &RivalSightings,
    knowledge: &MapKnowledge,
    visuals: &Query<&DiagnosticTacMapVisual, With<TacMapElement>>,
) -> TacMapSnapshot {
    let game = runtime.live.host_match();
    let model = tacmap::build_map(
        &game.competitive,
        keys,
        sightings,
        knowledge,
        game.reroute_commits,
        tp.place,
    );
    let expected_routes = tacmap::route_segment_count(&model);
    // Visited rooms draw filled squares; glimpsed rooms draw hollow outlines — both are
    // tagged `Room` (Phase 50 fog of war).
    let expected_rooms = model.rooms.len() + model.glimpsed.len();
    let expected_keystones = model.keystones.len();
    // Rival team labels only exist in spectator mode (Phase 50); the audit always runs
    // the live race, so each pip is exactly one tagged node.
    let expected_rivals = model.rivals.len();
    // The exit outline only draws once the player has actually found the exit room.
    let expected_exit = usize::from(model.exit_known);
    let expected_elements =
        expected_routes + expected_rooms + expected_exit + expected_keystones + expected_rivals + 1;

    let mut rendered_routes = 0;
    let mut rendered_rooms = 0;
    let mut rendered_keystones = 0;
    let mut rendered_rivals = 0;
    let mut player_marker_count = 0;
    let mut rendered_elements = 0;
    for visual in visuals {
        rendered_elements += 1;
        match visual.role {
            DiagnosticTacMapRole::Route => rendered_routes += 1,
            DiagnosticTacMapRole::Room => rendered_rooms += 1,
            DiagnosticTacMapRole::Keystone => rendered_keystones += 1,
            DiagnosticTacMapRole::Rival => rendered_rivals += 1,
            DiagnosticTacMapRole::Player => player_marker_count += 1,
            DiagnosticTacMapRole::Exit => {}
        }
    }

    TacMapSnapshot {
        visible: rendered_elements > 0,
        expected_elements,
        rendered_elements,
        expected_rooms,
        rendered_rooms,
        expected_routes,
        rendered_routes,
        expected_keystones,
        rendered_keystones,
        expected_rivals,
        rendered_rivals,
        player_marker_count,
        player_model: match model.player {
            tacmap::PlayerMark::Room(room) => format!("room {}", room.0),
            tacmap::PlayerMark::Between(a, b) => format!("between {} {}", a.0, b.0),
        },
    }
}

fn collect_monitors(
    items: &ItemsState,
    guardian: &Guardian,
    monitors: &Query<MonitorDiagnosticQueryData>,
    monitor_label_segments: &Query<MonitorLabelSegmentDiagnosticQueryData>,
    materials: &Assets<StandardMaterial>,
) -> Vec<MonitorSnapshot> {
    let mut segment_counts: HashMap<(u8, u32), usize> = HashMap::new();
    for segment in monitor_label_segments.iter() {
        *segment_counts
            .entry((monitor_kind_key(segment.kind), segment.room.0))
            .or_default() += 1;
    }

    monitors
        .iter()
        .filter_map(|(tether, guardian_monitor, handle, _name)| {
            let material = materials.get(&handle.0)?;
            let (kind, monitor_kind, kind_key, room, active) = if let Some(monitor) = tether {
                (
                    "tether",
                    crate::screens::place::ObservationMonitorKind::Tether,
                    monitor_kind_key(crate::screens::place::ObservationMonitorKind::Tether),
                    monitor.room,
                    items.placed.iter().any(|item| {
                        item.kind == ItemKind::AnchorTorch
                            && item.place == Place::Room(monitor.room)
                    }),
                )
            } else if let Some(monitor) = guardian_monitor {
                (
                    "guardian",
                    crate::screens::place::ObservationMonitorKind::Guardian,
                    monitor_kind_key(crate::screens::place::ObservationMonitorKind::Guardian),
                    monitor.room,
                    guardian.room == monitor.room,
                )
            } else {
                return None;
            };
            Some(MonitorSnapshot {
                subject: format!("{kind} monitor room {}", room.0),
                room: room.0,
                active,
                visible: true,
                label: crate::screens::place::monitor_label(monitor_kind, room, active),
                label_segment_count: segment_counts
                    .get(&(kind_key, room.0))
                    .copied()
                    .unwrap_or_default(),
                base_rgb: color_rgb(material.base_color),
                emissive_rgb: linear_rgb(material.emissive),
                emissive_luminance: observed_diagnostics::luminance(linear_rgb(material.emissive)),
                min_luminance: DEFAULT_SIGNAL_MIN_LUMINANCE,
            })
        })
        .collect()
}

fn monitor_kind_key(kind: crate::screens::place::ObservationMonitorKind) -> u8 {
    match kind {
        crate::screens::place::ObservationMonitorKind::Tether => 0,
        crate::screens::place::ObservationMonitorKind::Guardian => 1,
    }
}

fn expected_threshold_light(status: DiagnosticThresholdStatus) -> Color {
    match status {
        DiagnosticThresholdStatus::TetheredPassage => style::marker(MarkerRole::Control).base_color,
        DiagnosticThresholdStatus::Collapsed => style::surface(style::SurfaceRole::Rubble)
            .edge
            .unwrap_or(style::marker(MarkerRole::Collapse).base_color),
        _ => Color::srgb(0.45, 0.62, 0.78),
    }
}

pub(super) fn place_label(place: Place) -> String {
    match place {
        Place::Room(room) => format!("room {}", room.0),
        Place::Hallway {
            from,
            to,
            variation,
            ..
        } => format!("hallway {} -> {} v{}", from.0, to.0, variation),
    }
}

fn color_rgb(color: Color) -> [f32; 3] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue]
}

fn linear_rgb(color: LinearRgba) -> [f32; 3] {
    [color.red, color.green, color.blue]
}

fn color_distance(a: Color, b: Color) -> f32 {
    let a = a.to_srgba();
    let b = b.to_srgba();
    ((a.red - b.red).powi(2) + (a.green - b.green).powi(2) + (a.blue - b.blue).powi(2)).sqrt()
}

fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("observed-vis-audit-{millis}")
}

pub(super) fn write_manifest(dir: &Path, run: &DiagnosticRun) {
    let path = dir.join("manifest.json");
    if let Ok(json) = serde_json::to_string_pretty(run) {
        let _ = fs::write(path, json);
    }
}

pub(super) fn write_snapshot(path: &Path, snapshot: &DiagnosticSnapshot) {
    if let Ok(json) = serde_json::to_string_pretty(snapshot) {
        let _ = fs::write(path, json);
    }
}

pub(super) fn finish_audit(dir: &Path, run: &DiagnosticRun) {
    write_manifest(dir, run);
    let path = dir.join("findings.ndjson");
    let mut out = String::new();
    for finding in &run.findings {
        if let Ok(line) = serde_json::to_string::<DiagnosticFinding>(finding) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    let _ = fs::write(path, out);
}
