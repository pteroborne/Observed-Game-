//! The first-person place renderer: turn the current place (room or hallway) into
//! neon-noir geometry in its own local frame — floor/ceiling, walls straight from the
//! collision arena, neon doorway frames + animated leaves, a preview of the place beyond
//! each open doorway, and keystone items — plus the shared camera and the walking rival
//! avatars. Rebuilds only when the player teleports.

pub(crate) mod factory;
pub(crate) mod items;
pub(crate) mod lighting;
pub(crate) mod mesh;
pub(crate) mod preview;
pub(crate) mod strategies;

use bevy::prelude::*;
use observed_style::{self as style, MarkerRole};

use super::*;
use crate::GameState;
use crate::camera;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::rivals;
use crate::teleport::{self, GapKind, Place};

/// Place the shared camera at the player's first-person eye in the current place's
/// local frame (each place is centred at the origin). No shake — a reroute is felt
/// through the world's flickering lights ([`super::flicker_lights`]), not the camera.
pub(crate) fn present_match_camera(
    tp: Res<TeleportState>,
    mut camera: Query<&mut Transform, With<GameCam>>,
) {
    if let Ok(mut transform) = camera.single_mut() {
        camera::player_view(&tp.body, &tp.config).apply_to(&mut transform);
    }
}

#[derive(Resource, Default)]
pub(crate) struct LastRenderedSignature(pub(crate) Option<(Place, u64, usize)>);

/// Rebuild the place presentation geometry (floors, walls, ceiling, previews, lights, items).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_place(
    assets: Res<MatchAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Res<crate::guardian::Guardian>,
    runtime: Res<MatchRuntime>,
    existing: Query<Entity, With<PlaceGeometry>>,
    last_sig: Option<ResMut<LastRenderedSignature>>,
    mut commands: Commands,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let tp = tp.into_inner();
    let game = runtime.live.host_match();
    let seed_val = seed.map(|s| s.0).unwrap_or(crate::flow::MATCH_SEED);
    let nav = match_runtime::nav_for_place(seed_val, game, &keys, &items, tp.place);

    // Signature to detect if the place needs rebuilding (e.g. dropped items or tethers changed)
    let signature = {
        let mut tethers_hash = 0u64;
        for &conn in &nav.connections {
            if nav.is_tethered(game.local_room(), conn) {
                tethers_hash += 1;
            }
        }
        let item_count = items.placed_in(tp.place).len();
        (tp.place, tethers_hash, item_count)
    };

    let mut rebuild = true;
    if let Some(mut sig) = last_sig {
        if tp.rendered == Some(tp.place) && sig.0 == Some(signature) {
            rebuild = false;
        } else {
            sig.0 = Some(signature);
        }
    } else {
        commands.insert_resource(LastRenderedSignature(Some(signature)));
    }

    if tp.rendered == Some(tp.place) && !rebuild {
        return;
    }
    tp.rendered = Some(tp.place);

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let geom = tp.geom.clone();
    let y_offset = teleport::place_y_offset(tp.place);

    // ==========================================
    // STRATEGY PATTERN: Spawn floor, ceiling, and walls
    // ==========================================
    let strategy: Box<dyn strategies::SpawningStrategy> = match tp.place {
        Place::Room(_) => Box::new(strategies::RoomSpawningStrategy),
        Place::Hallway { .. } => Box::new(strategies::HallwaySpawningStrategy),
    };

    let floor_material = match tp.place {
        Place::Room(room) => factory::RoomMaterialFactory::create_floor_material(
            room,
            &assets.floor_material,
            &mut materials,
        ),
        Place::Hallway { .. } => assets.floor_material.clone(),
    };

    strategy.spawn_geometry(
        &mut commands,
        &assets,
        &mut meshes,
        &geom,
        floor_material,
        &tp.arena.solids,
        y_offset,
    );

    // ==========================================
    // TEMPLATE METHOD PATTERN: Spawn threshold gateways
    // ==========================================
    for gap in &geom.gaps {
        if gap.kind == teleport::GapKind::OneWayEntry {
            continue;
        }
        let (ea, eb) = match tp.place {
            Place::Room(room) => (room, gap.target),
            Place::Hallway { from, to, .. } => (from, to),
        };
        let tethered = nav.is_tethered(ea, eb);

        if gap.kind.is_passage() {
            let dest = tp
                .gap_dests
                .iter()
                .find(|d| d.threshold == gap.threshold)
                .or_else(|| {
                    tp.gap_dests
                        .iter()
                        .find(|d| (d.gap_center - gap.center).length() < 0.05)
                })
                .cloned()
                .unwrap_or_else(|| preview::fallback_dest(tp.place, gap, &nav, game));

            // A maze hallway can expose multiple apertures to the same room-side
            // threshold. Keep one full preview for the canonical aperture and use short
            // stubs for secondary apertures instead of drawing overlapping room copies.
            let multi_aperture_room_preview = matches!(tp.place, Place::Hallway { .. })
                && matches!(dest.place, Place::Room(_))
                && gap.threshold.hall.slot.0 != 0
                && geom
                    .gaps
                    .iter()
                    .filter(|other| {
                        other.kind.is_passage()
                            && other.target == gap.target
                            && other.normal.dot(gap.normal) > 0.99
                    })
                    .count()
                    > 1;
            if multi_aperture_room_preview {
                preview::spawn_passage_stub(&mut commands, &assets, gap, y_offset);
            } else {
                preview::spawn_passage_preview(
                    &mut commands,
                    &assets,
                    &mut meshes,
                    &mut materials,
                    gap,
                    tp.place,
                    &dest,
                    &nav,
                    game,
                );
            }

            strategies::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                strategies::PassagePolicy { tethered },
                y_offset,
            );
        } else if gap.kind == GapKind::LockedExit {
            strategies::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                strategies::LockedExitPolicy { tethered },
                y_offset,
            );
        } else {
            strategies::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                strategies::SideDoorPolicy,
                y_offset,
            );
        }
    }

    // Spawn specialized room visual elements (monitors, interactive console)
    if let Place::Room(room) = tp.place {
        if room.0 == 5 {
            spawn_tether_camera_monitors(
                &mut commands,
                &assets,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
            );
        } else if room.0 == 6 {
            spawn_guardian_observation_monitors(
                &mut commands,
                &assets,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
            );
        } else if room.0 == 3 {
            // Guardian Control Room: central interactive console
            let console_inactive = materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.2, 0.2),
                emissive: LinearRgba::new(0.2, 0.2, 0.2, 1.0),
                ..default()
            });
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                GuardianConsole,
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(console_inactive),
                Transform::from_xyz(0.0, 1.0 + y_offset, 0.0).with_scale(Vec3::new(1.0, 2.0, 1.0)),
                Name::new("Guardian Control Console"),
            ));
        }
    }

    // Spawn keystone item
    if let Place::Room(room) = tp.place
        && keys.has_uncollected(room)
    {
        items::spawn_keystone_item(&mut commands, &assets, room, y_offset);
    }

    // Spawn dropped items
    for item in items.placed_in(tp.place) {
        items::spawn_dropped_item(&mut commands, &assets, item, y_offset);
    }

    // Lighting and surface details
    let district = match_runtime::district_for_place(seed_val, tp.place);
    let light_color = match tp.place {
        Place::Room(room) => factory::RoomMaterialFactory::create_light_color(room),
        Place::Hallway { .. } => style::district(district).light_color,
    };

    let place_transform = Transform::from_xyz(0.0, y_offset, 0.0);
    lighting::spawn_place_lighting(
        &mut commands,
        &assets,
        &geom,
        light_color,
        place_transform,
        false,
    );

    let accent = assets.district_accent_materials[district.index()].clone();
    lighting::spawn_surface_detail(
        &mut commands,
        &assets,
        &geom,
        accent,
        place_transform,
        false,
    );

    // Spawn guardian
    if let Place::Room(room) = tp.place
        && room == guardian.room
    {
        spawn_guardian_model(&mut commands, &assets, guardian.pos);
    }
}

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
const WALL_MONITOR_ROOM_ORDER: [u32; MONITOR_COUNT] = [0, 1, 4, 2, 3, 5, 6, 7, 8];

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

fn spawn_tether_camera_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed: u64,
    room: RoomId,
) {
    let border_material = assets.district_accent_materials
        [district_for_place(seed, Place::Room(room)).index()]
    .clone();
    spawn_wall_monitors(
        commands,
        assets,
        materials,
        geom,
        y_offset,
        border_material,
        ObservationMonitorKind::Tether,
    );
}

fn spawn_guardian_observation_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    seed: u64,
    room: RoomId,
) {
    let border_material = assets.district_accent_materials
        [district_for_place(seed, Place::Room(room)).index()]
    .clone();
    spawn_wall_monitors(
        commands,
        assets,
        materials,
        geom,
        y_offset,
        border_material,
        ObservationMonitorKind::Guardian,
    );
}

fn spawn_wall_monitors(
    commands: &mut Commands,
    assets: &MatchAssets,
    materials: &mut Assets<StandardMaterial>,
    geom: &teleport::PlaceGeom,
    y_offset: f32,
    border_material: Handle<StandardMaterial>,
    kind: ObservationMonitorKind,
) {
    for (slot, mount) in wall_monitor_mounts(geom)
        .into_iter()
        .take(MONITOR_COUNT)
        .enumerate()
    {
        let room = RoomId(WALL_MONITOR_ROOM_ORDER[slot]);
        spawn_wall_monitor(
            commands,
            assets,
            materials,
            border_material.clone(),
            mount,
            y_offset,
            room,
            kind,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_wall_monitor(
    commands: &mut Commands,
    assets: &MatchAssets,
    materials: &mut Assets<StandardMaterial>,
    border_material: Handle<StandardMaterial>,
    mount: WallMonitorMount,
    y_offset: f32,
    room: RoomId,
    kind: ObservationMonitorKind,
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

fn gap_yaw(normal: Vec2) -> f32 {
    let along = Vec2::new(-normal.y, normal.x);
    (-along.y).atan2(along.x)
}

/// A neon doorway frame (two posts + a lintel) at a gap, built from primitives and
/// rotated to the gap's wall so it matches the opening at any angle.
pub(super) fn spawn_place_frame(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    tethered: bool,
    y_offset: f32,
) {
    let material = assets.doorframe_material.clone();
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let half = gap.width * 0.5;
    let status = crate::diagnostics::threshold_status(gap, tethered);
    for offset in [half, -half] {
        let p = gap.center + along * offset;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::diagnostics::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::diagnostics::DiagnosticThresholdVisualKind::Frame,
                status,
            },
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(p.x, y_offset + WALL_HEIGHT * 0.5, p.y).with_scale(Vec3::new(
                0.24,
                WALL_HEIGHT,
                0.24,
            )),
            Name::new("Doorframe post"),
        ));
    }
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::Frame,
            status,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT - 0.2, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.3), 0.34, 0.24)),
        Name::new("Doorframe lintel"),
    ));

    let tether_color = if tethered {
        style::marker(MarkerRole::Control).base_color
    } else {
        Color::srgb(0.45, 0.62, 0.78)
    };
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::FrameLight,
            status,
        },
        PointLight {
            color: tether_color,
            intensity: 1_400.0,
            range: 5.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT - 0.35, gap.center.y),
        Name::new("Doorframe tether light"),
    ));

    if crate::diagnostics::visual_audit_enabled() {
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::diagnostics::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::diagnostics::DiagnosticThresholdVisualKind::Label,
                status,
            },
            Text2d::new(crate::diagnostics::threshold_label(&gap.threshold)),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT + 0.35, gap.center.y)
                .with_rotation(rot)
                .with_scale(Vec3::splat(0.035)),
            Name::new("Threshold debug label"),
        ));
    }
}

/// A door leaf filling a sealed doorway gap, flush with the wall.
pub(super) fn spawn_leaf(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &teleport::DoorGap,
    openable: bool,
    material: Handle<StandardMaterial>,
    y_offset: f32,
) {
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let leaf_h = WALL_HEIGHT - DOOR_LINTEL_H;
    let closed_y = y_offset + leaf_h * 0.5;
    let open_y = closed_y + leaf_h;
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::Leaf,
            status: crate::diagnostics::threshold_status(gap, false),
        },
        DoorLeaf {
            center: gap.center,
            closed_y,
            open_y,
            openable,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, closed_y, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), leaf_h, DOOR_LEAF_D)),
        Name::new(if openable { "Door leaf" } else { "Closed door" }),
    ));
}

const DOOR_OPEN_RADIUS: f32 = 4.6;
const DOOR_SLIDE_SPEED: f32 = 6.0;

/// Slide any future openable sealed leaf between shut and tucked-into-the-lintel by the
/// player's proximity.
pub(crate) fn animate_doors(
    time: Res<Time>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
) {
    if paused.0 {
        return;
    }
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    let max_step = DOOR_SLIDE_SPEED * time.delta_secs();
    for (leaf, mut transform) in &mut leaves {
        let target = if leaf.openable && body.distance(leaf.center) <= DOOR_OPEN_RADIUS {
            leaf.open_y
        } else {
            leaf.closed_y
        };
        let y = transform.translation.y;
        transform.translation.y = y + (target - y).clamp(-max_step, max_step);
    }
}

pub(crate) fn sync_rival_avatars(
    time: Res<Time>,
    runtime: Res<MatchRuntime>,
    tp: Res<TeleportState>,
    assets: Res<MatchAssets>,
    paused: Res<MatchPaused>,
    mut avatars: Query<(Entity, &RivalAvatar, &mut Transform)>,
    mut commands: Commands,
) {
    let game = runtime.live.host_match();
    let present: Vec<usize> = match tp.place {
        Place::Room(room) => rivals::rivals_in_room(&game.competitive, room),
        Place::Hallway { .. } => Vec::new(),
    };

    let (a, b) = rivals::pace_segment(&tp.geom);
    let along = b - a;
    let tangent = Vec2::new(-along.y, along.x).normalize_or_zero();
    let n = present.len();

    let mut have: Vec<usize> = Vec::new();
    for (entity, avatar, mut transform) in &mut avatars {
        let Some(slot) = present.iter().position(|&t| t == avatar.team) else {
            commands.entity(entity).despawn();
            continue;
        };
        have.push(avatar.team);
        if paused.0 {
            continue;
        }
        let phase = avatar.team as f32 * 0.7;
        let theta = time.elapsed_secs() * rivals::RIVAL_PACE_SPEED + phase;
        let u = rivals::triangle_wave(theta);
        let lane = (slot as f32 - (n as f32 - 1.0) * 0.5) * 1.3;
        let foot = a + along * u + tangent * lane;
        let bob = (theta * 6.0).sin() * 0.06;
        transform.translation = Vec3::new(foot.x, 0.82 + bob, foot.y);
    }

    for &team in &present {
        if !have.contains(&team) {
            commands.spawn((
                RivalAvatar { team },
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.rival_body_mesh.clone()),
                MeshMaterial3d(assets.rival_material.clone()),
                Transform::from_xyz(a.x, 0.82, a.y),
                Name::new(format!("Rival team {team}")),
            ));
        }
    }
}

pub(crate) fn spawn_guardian_model(commands: &mut Commands, assets: &MatchAssets, pos: Vec3) {
    commands
        .spawn((
            crate::guardian::GuardianModel,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.rival_body_mesh.clone()),
            MeshMaterial3d(assets.trap_active_material.clone()),
            Transform::from_translation(pos),
            Name::new("Guardian"),
        ))
        .with_children(|g| {
            g.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.05, 0.05),
                    intensity: 3000.0,
                    range: 9.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
        });
}

pub(crate) fn animate_teleport_pad_glow(
    time: Res<Time>,
    mut glow_q: Query<
        (
            &mut Transform,
            &GlobalTransform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<TeleportPadGlow>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: Option<Res<TeleportState>>,
) {
    let elapsed = time.elapsed_secs();
    let player_pos = tp.map(|t| t.body.position).unwrap_or(Vec3::ZERO);

    for (mut transform, global_transform, material_handle) in &mut glow_q {
        let pad_pos = global_transform.translation();
        let dist = Vec2::new(pad_pos.x, pad_pos.z).distance(Vec2::new(player_pos.x, player_pos.z));
        let stepped_on = dist < 1.2;

        let scale_y = if stepped_on {
            0.33 + (elapsed * 3.5).sin() * 0.03
        } else {
            0.01
        };
        let scale_xz = if stepped_on {
            5.0 + (elapsed * 2.0).cos() * 0.4
        } else {
            5.0
        };

        transform.scale = Vec3::new(scale_xz, scale_y, scale_xz);
        transform.rotate_local_y(time.delta_secs() * 0.5);
        transform.translation.y = scale_y * 0.5 + 0.05;

        if let Some(mat) = materials.get_mut(material_handle.as_ref()) {
            let pad = style::marker(MarkerRole::You);
            let intensity = if stepped_on {
                0.8 + (elapsed * 3.0).sin() * 0.2
            } else {
                0.05
            };
            let mut col = LinearRgba::from(pad.base_color);
            col.alpha *= intensity;
            mat.base_color = Color::from(col);
        }
    }
}

#[derive(Component)]
pub(crate) struct CarriedTorchLight;

pub(crate) fn update_carried_torch_light(
    items: Option<Res<ItemsState>>,
    camera: Query<Entity, With<GameCam>>,
    mut commands: Commands,
    lights: Query<Entity, With<CarriedTorchLight>>,
) {
    let carrying_torch = items
        .map(|it| it.carried(crate::items::ItemKind::AnchorTorch) > 0)
        .unwrap_or(false);
    let has_light = !lights.is_empty();

    if carrying_torch && !has_light {
        if let Some(cam_ent) = camera.iter().next() {
            commands.entity(cam_ent).with_children(|parent| {
                parent.spawn((
                    CarriedTorchLight,
                    PointLight {
                        color: style::marker(MarkerRole::Control).base_color,
                        intensity: 2_200.0,
                        range: 8.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    Name::new("Carried torch light"),
                ));
            });
        }
    } else if !carrying_torch && has_light {
        for entity in &lights {
            commands.entity(entity).despawn();
        }
    }
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
    guardian: Res<crate::guardian::Guardian>,
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

pub(crate) fn interact_guardian_console(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&bevy::input::gamepad::Gamepad>,
    tp: Res<TeleportState>,
    mut guardian: ResMut<crate::guardian::Guardian>,
    mut log: ResMut<crate::guardian::ActionLog>,
    consoles: Query<&MeshMaterial3d<StandardMaterial>, With<GuardianConsole>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let in_room_3 = matches!(tp.place, Place::Room(RoomId(3)));
    if !in_room_3 {
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
}
