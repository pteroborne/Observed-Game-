//! Guardian and tether observation monitors: the wall-mounted screens that let players
//! see anchor/guardian state from a distance, the interactive Guardian Control Console,
//! and the systems that keep their glowing materials and room-id glyph labels in sync
//! with live game state.
//!
//! **Role-driven, not room-id-driven** (Arc D stage D1 fix): both monitor banks and the
//! console spawn from a [`MapSpec`]'s [`RoomRole::Monitor`] / [`RoomRole::GuardianControl`]
//! rooms — never a literal room id — so any map spec wires up correctly. [`monitor_pages`]
//! partitions the map's rooms across a Monitor room's panels (9 per room, in room-id
//! order); each panel then renders its assigned room's *current* structural state as a
//! miniature via [`super::preview::spawn_room_miniature`] (shell + door states + a
//! room-id label), never the old static "R{n}\nSTATUS" text-only glow. Monitors are
//! presentation-only: they read live state but never freeze, decohere, or otherwise
//! mutate anything the deterministic brain owns.

use bevy::prelude::*;
use observed_core::RoomId;
use observed_facility::map_spec::{MapSpec, RoomRole};
use observed_match::hybrid::HybridMatch;

use crate::GameState;
use crate::items::{ItemKind, ItemsState};
use crate::layout::WALL_HEIGHT;
use crate::rivals;
use crate::screens::match_runtime::district_for_place;
use crate::sim::state::{RivalSightings, SightingKind, TeleportState};
use crate::teleport::{self, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;

use super::preview::{RoomMiniatureOverlays, spawn_room_miniature};

const MONITOR_COUNT: usize = 9;
const MONITOR_FACE_OFFSET: f32 = 0.28;
const MONITOR_BORDER_RAISE: f32 = 0.08;
const MONITOR_DEPTH: f32 = 0.1;
const MONITOR_BORDER_DEPTH: f32 = 0.14;
const MONITOR_BORDER_THICKNESS: f32 = 0.12;
const MONITOR_HORIZONTAL_MARGIN: f32 = 0.7;
const MONITOR_VERTICAL_MARGIN: f32 = 0.45;
const MONITOR_GLYPH_FACE_OFFSET: f32 = 0.2;
const MONITOR_GLYPH_DEPTH: f32 = 0.035;

/// Partition every room of `spec` (sorted by [`RoomId`]) across the map's
/// [`RoomRole::Monitor`] rooms (also in [`RoomId`] order): [`MONITOR_COUNT`] panels per
/// monitor room, each map room appearing on at most one panel across the whole map.
/// Extra panels (a monitor room past the room count, or a map with more rooms than
/// `9 * monitor_room_count` can show) simply stay dark — callers render a page's `Vec`
/// shorter than [`MONITOR_COUNT`] by leaving the remaining wall mounts idle.
pub(crate) fn monitor_pages(spec: &MapSpec) -> Vec<Vec<RoomId>> {
    let mut rooms: Vec<RoomId> = spec.rooms.iter().map(|room| room.id).collect();
    rooms.sort_unstable_by_key(|room| room.0);
    let monitor_rooms = spec.rooms_with_role(RoomRole::Monitor);

    rooms
        .chunks(MONITOR_COUNT)
        .map(<[RoomId]>::to_vec)
        .chain(std::iter::repeat(Vec::new()))
        .take(monitor_rooms.len())
        .collect()
}

/// The panel page for one specific [`RoomRole::Monitor`] room (its position among the
/// map's monitor rooms in [`RoomId`] order selects its [`monitor_pages`] entry). `None`
/// if `room` does not hold the `Monitor` role.
pub(crate) fn monitor_page_for(spec: &MapSpec, room: RoomId) -> Option<Vec<RoomId>> {
    let mut monitor_rooms = spec.rooms_with_role(RoomRole::Monitor);
    monitor_rooms.sort_unstable_by_key(|r| r.0);
    let index = monitor_rooms.iter().position(|&r| r == room)?;
    monitor_pages(spec).into_iter().nth(index)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationMonitorKind {
    Tether,
    Guardian,
}

#[derive(Clone, Copy)]
struct WallMonitorMount {
    center: Vec2,
    inward: Vec2,
    yaw: f32,
    width: f32,
}

fn monitor_dark_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.03, 0.07, 0.09, 1.0),
        emissive: LinearRgba::new(0.02, 0.16, 0.2, 1.0),
        perceptual_roughness: 0.1,
        metallic: 0.9,
        ..default()
    }
}

fn wall_inward_normal(a: Vec2, b: Vec2) -> Vec2 {
    let d = (b - a).normalize_or_zero();
    let inward = Vec2::new(-d.y, d.x);
    if inward.dot((a + b) * 0.5) > 0.0 {
        -inward
    } else {
        inward
    }
}

fn wall_monitor_mounts(geom: &teleport::PlaceGeom) -> Vec<WallMonitorMount> {
    let Some(poly) = geom.poly.as_ref() else {
        return Vec::new();
    };
    let mut mounts = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let mid = (a + b) * 0.5;
        if geom
            .gaps
            .iter()
            .any(|gap| (gap.center - mid).length() < 0.05)
        {
            continue;
        }
        let d = b - a;
        let len = d.length();
        if len < MONITOR_HORIZONTAL_MARGIN * 2.0 + 1.0 {
            continue;
        }
        let inward = wall_inward_normal(a, b);
        mounts.push(WallMonitorMount {
            center: mid + inward * MONITOR_FACE_OFFSET,
            inward,
            yaw: inward.x.atan2(inward.y),
            width: len - MONITOR_HORIZONTAL_MARGIN * 2.0,
        });
    }
    mounts.sort_by(|a, b| {
        a.center
            .y
            .total_cmp(&b.center.y)
            .then_with(|| a.center.x.total_cmp(&b.center.x))
    });
    mounts
}

/// Per-panel entity budget (Arc D stage D1): a monitor panel's miniature must stay cheap
/// — the screen quad, its border (4), the two-glyph room-id label (up to ~12 segments),
/// plus a handful of shell/overlay entities from [`spawn_room_miniature`] (floor, ceiling,
/// per-edge walls, one light, a couple of surface-detail strips, and at most one rival
/// avatar since a monitor renders shell + a same-room presence marker only, never a
/// nested monitor's own contents). Tests assert the real spawn stays under this.
#[cfg(test)]
pub(crate) const MONITOR_PANEL_ENTITY_BUDGET: usize = 60;

/// Half-size (world units) a monitor panel's room miniature is scaled to, so a full room
/// footprint reads as a small diorama on the wall rather than a life-size room punched
/// into it.
const MONITOR_MINIATURE_SCALE: f32 = 0.05;

/// Spawn the tether-camera bank on a Monitor room's wall mounts starting at mount index
/// `mount_offset` (so a Guardian bank sharing the same room can start after it — see
/// [`spawn_guardian_observation_monitors`]), one panel per room in `page`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_tether_camera_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed: u64,
    room: RoomId,
    mount_offset: usize,
    page: &[RoomId],
    game: &HybridMatch,
    klaxon_active: bool,
    sightings: &mut RivalSightings,
) {
    let border_material = assets.district_accent_materials
        [district_for_place(seed, Place::Room(room)).index()]
    .clone();
    spawn_wall_monitors(
        commands,
        assets,
        meshes,
        materials,
        geom,
        y_offset,
        seed,
        border_material,
        ObservationMonitorKind::Tether,
        mount_offset,
        page,
        game,
        klaxon_active,
        sightings,
    );
}

/// Spawn the guardian-observation bank on a Monitor room's wall mounts starting at mount
/// index `mount_offset`, one panel per room in `page`. Coexists with
/// [`spawn_tether_camera_monitors`] in the same room by claiming a disjoint mount range.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_guardian_observation_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed: u64,
    room: RoomId,
    mount_offset: usize,
    page: &[RoomId],
    game: &HybridMatch,
    klaxon_active: bool,
    sightings: &mut RivalSightings,
) {
    let border_material = assets.district_accent_materials
        [district_for_place(seed, Place::Room(room)).index()]
    .clone();
    spawn_wall_monitors(
        commands,
        assets,
        meshes,
        materials,
        geom,
        y_offset,
        seed,
        border_material,
        ObservationMonitorKind::Guardian,
        mount_offset,
        page,
        game,
        klaxon_active,
        sightings,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_wall_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed: u64,
    border_material: Handle<StandardMaterial>,
    kind: ObservationMonitorKind,
    mount_offset: usize,
    page: &[RoomId],
    game: &HybridMatch,
    klaxon_active: bool,
    sightings: &mut RivalSightings,
) {
    for (mount, &room) in wall_monitor_mounts(geom)
        .into_iter()
        .skip(mount_offset)
        .take(MONITOR_COUNT)
        .zip(page.iter())
    {
        spawn_wall_monitor(
            commands,
            assets,
            meshes,
            materials,
            border_material.clone(),
            mount,
            y_offset,
            seed,
            room,
            kind,
            game,
            klaxon_active,
        );

        // Phase 39/42 payoff: a monitor panel that currently shows a rival occupying its
        // target room is itself an observation, exactly like a threshold or an audio
        // bleed — so it writes into the same one-writer sighting ledger
        // (`RivalSightings::record`, see `sim::sightings`'s module doc).
        let commits = game.reroute_commits;
        for team_index in rivals::rivals_in_room(&game.competitive, room) {
            let team = game.competitive.teams[team_index].id;
            sightings.record(team, room, SightingKind::Seen, commits);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_wall_monitor(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    border_material: Handle<StandardMaterial>,
    mount: WallMonitorMount,
    y_offset: f32,
    seed: u64,
    room: RoomId,
    kind: ObservationMonitorKind,
    game: &HybridMatch,
    klaxon_active: bool,
) {
    let height = WALL_HEIGHT - MONITOR_VERTICAL_MARGIN * 2.0;
    let center_y = y_offset + WALL_HEIGHT * 0.5;
    let screen_material = materials.add(monitor_dark_material());
    let screen_transform = Transform::from_xyz(mount.center.x, center_y, mount.center.y)
        .with_rotation(Quat::from_rotation_y(mount.yaw))
        .with_scale(Vec3::new(mount.width, height, MONITOR_DEPTH));
    let mut entity = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(screen_material),
        screen_transform,
        Name::new(match kind {
            ObservationMonitorKind::Tether => format!("Tether Monitor Room {}", room.0),
            ObservationMonitorKind::Guardian => format!("Guardian Monitor Room {}", room.0),
        }),
    ));
    match kind {
        ObservationMonitorKind::Tether => {
            entity.insert(TetherCameraMonitor { room });
        }
        ObservationMonitorKind::Guardian => {
            entity.insert(GuardianObservationMonitor { room });
        }
    }

    let border_center = mount.center + mount.inward * MONITOR_BORDER_RAISE;
    let base = Transform::from_xyz(border_center.x, center_y, border_center.y)
        .with_rotation(Quat::from_rotation_y(mount.yaw));
    let horizontal = Vec3::new(
        mount.width + MONITOR_BORDER_THICKNESS * 2.0,
        MONITOR_BORDER_THICKNESS,
        MONITOR_BORDER_DEPTH,
    );
    let vertical = Vec3::new(
        MONITOR_BORDER_THICKNESS,
        height + MONITOR_BORDER_THICKNESS * 2.0,
        MONITOR_BORDER_DEPTH,
    );
    for (offset, scale) in [
        (Vec3::new(0.0, height * 0.5, 0.0), horizontal),
        (Vec3::new(0.0, -height * 0.5, 0.0), horizontal),
        (Vec3::new(mount.width * 0.5, 0.0, 0.0), vertical),
        (Vec3::new(-mount.width * 0.5, 0.0, 0.0), vertical),
    ] {
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(border_material.clone()),
            base.mul_transform(Transform::from_translation(offset).with_scale(scale)),
            Name::new("Observation monitor border"),
        ));
    }

    let glyph_center = mount.center + mount.inward * MONITOR_GLYPH_FACE_OFFSET;
    let glyph_base = Transform::from_xyz(glyph_center.x, center_y, glyph_center.y)
        .with_rotation(Quat::from_rotation_y(mount.yaw));
    let glyph_material = materials.add(monitor_label_segment_material(kind, false));
    spawn_monitor_room_id_glyph(commands, assets, glyph_material, glyph_base, room, kind);

    // The panel's actual payoff (Arc D stage D1 fix): the target room's *current*
    // structural state as a real miniature, via the same technique the threshold
    // previews use (`preview::spawn_room_miniature`) — never the flat status-only glow
    // the old wall-monitor scheme rendered. Presentation-only: a monitor never freezes
    // or decoheres anything, it only reads and displays live state.
    //
    // A monitor room's own panel shows the target room's shell only, never that room's
    // own nested monitor contents — this never recurses into `spawn_wall_monitor` for the
    // target room, regardless of its role.
    let role = game
        .competitive
        .map_spec
        .as_ref()
        .and_then(|spec| spec.room(room).map(|room| room.role));
    let dest = teleport::room_preview_geom(room, &[], &[], &[], None, role, seed);
    if let Some(poly) = dest.poly.clone() {
        let inward_yaw = mount.yaw + std::f32::consts::PI;
        let miniature_center = mount.center + mount.inward * (MONITOR_DEPTH * 0.5 + 0.02);
        let parent = Transform::from_xyz(miniature_center.x, center_y, miniature_center.y)
            .with_rotation(Quat::from_rotation_y(inward_yaw))
            .with_scale(Vec3::splat(MONITOR_MINIATURE_SCALE));

        // A named, tagged marker at the miniature's root so tests (and any future
        // inspection tooling) can find "the miniature for panel/room X" without having to
        // pattern-match the shared helper's generic shell entity names.
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            MonitorMiniature { room },
            parent,
            Name::new("Monitor miniature"),
        ));

        spawn_room_miniature(
            commands,
            assets,
            meshes,
            materials,
            &dest,
            &poly,
            room,
            parent,
            seed,
            game,
            klaxon_active,
            RoomMiniatureOverlays {
                open_edge_target: None,
                rival_lane_origin: Vec3::new(0.0, 0.82, 0.0),
                anchor_torch_root: Vec3::new(dest.half.x * 0.5 + 0.3, 0.55, 0.6),
            },
        );
    }
}

/// Marks a monitor panel's miniature root (an invisible anchor entity, no mesh) so tests
/// and diagnostics can find "the miniature currently showing room X" directly, without
/// pattern-matching the shared preview helper's generic shell entity names.
#[derive(Component)]
pub(crate) struct MonitorMiniature {
    // Read by tests only today (e.g. `game::tests::monitor_room_renders_miniatures_...`);
    // kept a real field rather than a unit marker since it's the natural hook for a future
    // visual-audit scenario (`evidence::audit`) to report "panel N shows room Y".
    #[allow(dead_code)]
    pub(crate) room: RoomId,
}

pub(crate) fn monitor_label(kind: ObservationMonitorKind, room: RoomId, active: bool) -> String {
    let status = match (kind, active) {
        (ObservationMonitorKind::Tether, true) => "ANCHOR LOCKED",
        (ObservationMonitorKind::Tether, false) => "NO ANCHOR",
        (ObservationMonitorKind::Guardian, true) => "GUARDIAN",
        (ObservationMonitorKind::Guardian, false) => "CLEAR",
    };
    format!("R{}\n{status}", room.0)
}

fn monitor_label_segment_material(kind: ObservationMonitorKind, active: bool) -> StandardMaterial {
    let (base_color, emissive) = match (kind, active) {
        (ObservationMonitorKind::Tether, true) => (
            Color::srgb(0.55, 1.0, 1.0),
            LinearRgba::new(1.2, 6.0, 6.0, 1.0),
        ),
        (ObservationMonitorKind::Guardian, true) => (
            Color::srgb(1.0, 0.62, 0.52),
            LinearRgba::new(7.0, 2.0, 1.2, 1.0),
        ),
        _ => (
            Color::srgb(0.44, 0.64, 0.72),
            LinearRgba::new(0.3, 1.1, 1.5, 1.0),
        ),
    };
    StandardMaterial {
        base_color,
        emissive,
        perceptual_roughness: 0.28,
        metallic: 0.0,
        ..default()
    }
}

fn spawn_monitor_room_id_glyph(
    commands: &mut Commands,
    assets: &MatchAssets,
    material: Handle<StandardMaterial>,
    base: Transform,
    room: RoomId,
    kind: ObservationMonitorKind,
) {
    let glyph_height = 1.35;
    let glyph_width = 0.72;
    let thickness = 0.12;
    let spacing = 0.28;
    let start_x = -(glyph_width + spacing) * 0.5;
    spawn_monitor_letter_r(
        commands,
        assets,
        material.clone(),
        base,
        room,
        kind,
        Vec2::new(start_x, 0.0),
        glyph_width,
        glyph_height,
        thickness,
    );
    spawn_monitor_digit(
        commands,
        assets,
        material,
        base,
        room,
        kind,
        Vec2::new(start_x + glyph_width + spacing, 0.0),
        glyph_width,
        glyph_height,
        thickness,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_monitor_letter_r(
    commands: &mut Commands,
    assets: &MatchAssets,
    material: Handle<StandardMaterial>,
    base: Transform,
    room: RoomId,
    kind: ObservationMonitorKind,
    origin: Vec2,
    width: f32,
    height: f32,
    thickness: f32,
) {
    let half_h = height * 0.5;
    let half_w = width * 0.5;
    let vertical_h = height * 0.48;
    for (x, y, w, h, rot) in [
        (
            origin.x - half_w,
            origin.y + height * 0.24,
            thickness,
            vertical_h,
            0.0,
        ),
        (
            origin.x - half_w,
            origin.y - height * 0.24,
            thickness,
            vertical_h,
            0.0,
        ),
        (origin.x, origin.y + half_h, width, thickness, 0.0),
        (origin.x, origin.y, width, thickness, 0.0),
        (
            origin.x + half_w,
            origin.y + height * 0.25,
            thickness,
            vertical_h * 0.95,
            0.0,
        ),
        (
            origin.x + width * 0.18,
            origin.y - height * 0.26,
            thickness,
            height * 0.72,
            -std::f32::consts::FRAC_PI_4,
        ),
    ] {
        spawn_monitor_glyph_segment(
            commands,
            assets,
            material.clone(),
            base,
            room,
            kind,
            Vec2::new(x, y),
            Vec2::new(w, h),
            rot,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_monitor_digit(
    commands: &mut Commands,
    assets: &MatchAssets,
    material: Handle<StandardMaterial>,
    base: Transform,
    room: RoomId,
    kind: ObservationMonitorKind,
    origin: Vec2,
    width: f32,
    height: f32,
    thickness: f32,
) {
    let digit = room.0.min(9) as usize;
    for segment in digit_segments(digit) {
        let (center, size) = segment_rect(*segment, origin, width, height, thickness);
        spawn_monitor_glyph_segment(
            commands,
            assets,
            material.clone(),
            base,
            room,
            kind,
            center,
            size,
            0.0,
        );
    }
}

fn digit_segments(digit: usize) -> &'static [usize] {
    match digit {
        0 => &[0, 1, 2, 3, 4, 5],
        1 => &[1, 2],
        2 => &[0, 1, 6, 4, 3],
        3 => &[0, 1, 6, 2, 3],
        4 => &[5, 6, 1, 2],
        5 => &[0, 5, 6, 2, 3],
        6 => &[0, 5, 6, 4, 2, 3],
        7 => &[0, 1, 2],
        8 => &[0, 1, 2, 3, 4, 5, 6],
        9 => &[0, 1, 2, 3, 5, 6],
        _ => &[],
    }
}

fn segment_rect(
    segment: usize,
    origin: Vec2,
    width: f32,
    height: f32,
    thickness: f32,
) -> (Vec2, Vec2) {
    let half_h = height * 0.5;
    let half_w = width * 0.5;
    let vertical_h = (height - thickness * 3.0) * 0.5;
    match segment {
        0 => (
            Vec2::new(origin.x, origin.y + half_h),
            Vec2::new(width, thickness),
        ),
        1 => (
            Vec2::new(origin.x + half_w, origin.y + height * 0.25),
            Vec2::new(thickness, vertical_h),
        ),
        2 => (
            Vec2::new(origin.x + half_w, origin.y - height * 0.25),
            Vec2::new(thickness, vertical_h),
        ),
        3 => (
            Vec2::new(origin.x, origin.y - half_h),
            Vec2::new(width, thickness),
        ),
        4 => (
            Vec2::new(origin.x - half_w, origin.y - height * 0.25),
            Vec2::new(thickness, vertical_h),
        ),
        5 => (
            Vec2::new(origin.x - half_w, origin.y + height * 0.25),
            Vec2::new(thickness, vertical_h),
        ),
        _ => (origin, Vec2::new(width, thickness)),
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_monitor_glyph_segment(
    commands: &mut Commands,
    assets: &MatchAssets,
    material: Handle<StandardMaterial>,
    base: Transform,
    room: RoomId,
    kind: ObservationMonitorKind,
    center: Vec2,
    size: Vec2,
    rotation: f32,
) {
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        ObservationMonitorLabelSegment { room, kind },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        base.mul_transform(
            Transform::from_xyz(center.x, center.y, 0.0)
                .with_rotation(Quat::from_rotation_z(rotation))
                .with_scale(Vec3::new(size.x, size.y, MONITOR_GLYPH_DEPTH)),
        ),
        Name::new("Observation monitor room-id segment"),
    ));
}

#[derive(Component)]
pub(crate) struct TetherCameraMonitor {
    pub(crate) room: RoomId,
}

#[derive(Component)]
pub(crate) struct GuardianObservationMonitor {
    pub(crate) room: RoomId,
}

#[derive(Component)]
pub(crate) struct ObservationMonitorLabelSegment {
    pub(crate) room: RoomId,
    pub(crate) kind: ObservationMonitorKind,
}

#[derive(Component)]
pub(crate) struct GuardianConsole;

pub(crate) fn update_tether_monitors(
    items: Res<ItemsState>,
    monitors: Query<(&TetherCameraMonitor, &MeshMaterial3d<StandardMaterial>)>,
    label_segments: Query<(
        &ObservationMonitorLabelSegment,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (monitor, mat_handle) in &monitors {
        let is_tethered = items.placed.iter().any(|item| {
            item.kind == ItemKind::AnchorTorch && item.place == Place::Room(monitor.room)
        });

        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            if is_tethered {
                mat.base_color = Color::srgb(0.0, 0.8, 1.0);
                mat.emissive = LinearRgba::new(0.0, 4.0, 5.0, 1.0);
            } else {
                mat.base_color = Color::srgba(0.03, 0.07, 0.09, 1.0);
                mat.emissive = LinearRgba::new(0.02, 0.16, 0.2, 1.0);
            }
        }
    }

    for (segment, mat_handle) in &label_segments {
        if segment.kind != ObservationMonitorKind::Tether {
            continue;
        }
        let is_tethered = items.placed.iter().any(|item| {
            item.kind == ItemKind::AnchorTorch && item.place == Place::Room(segment.room)
        });
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            *mat = monitor_label_segment_material(segment.kind, is_tethered);
        }
    }
}

pub(crate) fn update_guardian_monitors(
    time: Res<Time>,
    guardian: Option<Res<crate::guardian::Guardian>>,
    monitors: Query<(
        &GuardianObservationMonitor,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    label_segments: Query<(
        &ObservationMonitorLabelSegment,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(guardian) = guardian else {
        return;
    };
    let t = time.elapsed_secs();
    for (monitor, mat_handle) in &monitors {
        let is_guardian_here = guardian.room == monitor.room;

        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            if is_guardian_here {
                // Pulsating warning red emission
                let pulse = 10.0 + (t * 8.0).sin() * 3.0;
                mat.base_color = Color::srgb(1.0, 0.1, 0.05);
                mat.emissive = LinearRgba::new(pulse, 0.8, 0.3, 1.0);
            } else {
                mat.base_color = Color::srgba(0.03, 0.07, 0.09, 1.0);
                mat.emissive = LinearRgba::new(0.02, 0.16, 0.2, 1.0);
            }
        }
    }

    for (segment, mat_handle) in &label_segments {
        if segment.kind != ObservationMonitorKind::Guardian {
            continue;
        }
        let is_guardian_here = guardian.room == segment.room;
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            *mat = monitor_label_segment_material(segment.kind, is_guardian_here);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn interact_guardian_console(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&bevy::input::gamepad::Gamepad>,
    tp: Res<TeleportState>,
    runtime: Res<crate::sim::director::MatchDirector>,
    guardian: Option<ResMut<crate::guardian::Guardian>>,
    log: Option<ResMut<crate::guardian::ActionLog>>,
    consoles: Query<&MeshMaterial3d<StandardMaterial>, With<GuardianConsole>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut guardian) = guardian else {
        return;
    };
    let Some(mut log) = log else {
        return;
    };
    let console_room = runtime
        .live
        .host_match()
        .competitive
        .map_spec
        .as_ref()
        .and_then(|spec| spec.role_room(RoomRole::GuardianControl));
    if console_room.is_none_or(|room| tp.place != Place::Room(room)) {
        return;
    }

    // Check distance to console (at Vec2::ZERO in room space)
    let pos = Vec2::new(tp.body.position.x, tp.body.position.z);
    if pos.length() > 2.0 {
        return;
    }

    let interact_pressed = keyboard.just_pressed(KeyCode::KeyE)
        || gamepads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::West));

    if interact_pressed {
        // Cycle target: None (Local Player) -> Some(1) -> Some(2) -> Some(3) -> None
        let next_target = match guardian.reassigned_target {
            None => Some(1),
            Some(1) => Some(2),
            Some(2) => Some(3),
            Some(3) => None,
            Some(_) => None,
        };
        guardian.reassigned_target = next_target;

        let msg = match next_target {
            None => "Guardian reassigned to target: Local Player".to_string(),
            Some(t) => format!("Guardian reassigned to target: Rival Team {t}"),
        };
        log.add(msg);
    }

    // Visual feedback: Console active/inactive glow
    let target_active = guardian.reassigned_target.is_some();
    for mat_handle in &consoles {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            if target_active {
                mat.base_color = Color::srgb(0.1, 1.0, 0.2);
                mat.emissive = LinearRgba::new(0.5, 5.0, 1.0, 1.0);
            } else {
                mat.base_color = Color::srgb(0.2, 0.2, 0.2);
                mat.emissive = LinearRgba::new(0.2, 0.2, 0.2, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::map_spec::sector_relay_v1;

    #[test]
    fn monitor_labels_encode_room_and_state() {
        assert_eq!(
            monitor_label(ObservationMonitorKind::Tether, RoomId(4), true),
            "R4\nANCHOR LOCKED"
        );
        assert_eq!(
            monitor_label(ObservationMonitorKind::Guardian, RoomId(4), true),
            "R4\nGUARDIAN"
        );
        assert_eq!(
            monitor_label(ObservationMonitorKind::Guardian, RoomId(4), false),
            "R4\nCLEAR"
        );
    }

    #[test]
    fn monitor_yaw_points_the_front_face_inward() {
        let inward = Vec2::new(-0.6, 0.8).normalize();
        let yaw = inward.x.atan2(inward.y);
        let front = Quat::from_rotation_y(yaw) * Vec3::Z;
        assert!((front.x - inward.x).abs() < 0.0001);
        assert!((front.z - inward.y).abs() < 0.0001);
    }

    #[test]
    fn monitor_pages_partition_every_room_at_most_once() {
        for spec in [sector_relay_v1(), crate::map_catalog::default_map_spec(0)] {
            let pages = monitor_pages(&spec);
            let monitor_room_count = spec.rooms_with_role(RoomRole::Monitor).len();
            assert_eq!(
                pages.len(),
                monitor_room_count,
                "one page per Monitor-role room ({})",
                spec.name
            );

            let mut seen = std::collections::BTreeSet::new();
            let mut total = 0;
            for page in &pages {
                assert!(
                    page.len() <= MONITOR_COUNT,
                    "a page never exceeds the physical panel count ({})",
                    spec.name
                );
                for &room in page {
                    assert!(
                        seen.insert(room),
                        "{room:?} must appear on at most one panel across every monitor room ({})",
                        spec.name
                    );
                    total += 1;
                }
            }
            assert!(
                total <= spec.room_count(),
                "never invents rooms ({})",
                spec.name
            );
        }
    }

    #[test]
    fn monitor_pages_are_deterministic() {
        let spec = sector_relay_v1();
        assert_eq!(monitor_pages(&spec), monitor_pages(&spec));
    }

    #[test]
    fn monitor_page_for_matches_the_room_s_slot_in_monitor_pages() {
        let spec = sector_relay_v1();
        let monitor_room = spec
            .rooms_with_role(RoomRole::Monitor)
            .first()
            .copied()
            .expect("sector_relay_v1 has a Monitor room");
        let page = monitor_page_for(&spec, monitor_room).expect("the Monitor room has a page");
        assert_eq!(page, monitor_pages(&spec)[0]);
    }

    #[test]
    fn monitor_page_for_a_non_monitor_room_is_none() {
        let spec = sector_relay_v1();
        let non_monitor = spec
            .rooms
            .iter()
            .find(|room| room.role != RoomRole::Monitor)
            .map(|room| room.id)
            .expect("sector_relay_v1 has non-monitor rooms");
        assert_eq!(monitor_page_for(&spec, non_monitor), None);
    }
}
