use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_core::RoomId;
use observed_match::hybrid::LocalAction;

use crate::GameState;
use crate::flow::{self, Career};
use crate::map_validation;
use crate::screens::{DoorLeaf, GameCam, MatchRuntime, PlaceGeometry, TeleportState};
use crate::teleport::{self, Place};
use crate::{camera, hallway, items, keystones};
use std::path::PathBuf;

// --- Ceiling ---

#[derive(Resource)]
pub(super) struct CeilingCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
}

impl CeilingCaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self { path, phase: 0 }
    }
}

pub(super) fn capture_ceiling_progress(
    time: Res<Time>,
    mut request: ResMut<CeilingCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchRuntime>>,
    tp: Option<ResMut<TeleportState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp)) = (runtime, tp) {
                rt.done = true;
                rt.live.host.match_state.reroute_feedback_ticks = 0;
                tp.body.position = Vec3::new(0.0, tp.config.half_height, 0.0);
                tp.body.yaw = 0.0;
                tp.body.pitch = 1.22;
                request.phase = 2;
            }
        }
        2 if elapsed >= 0.8 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 3;
        }
        3 if elapsed >= 1.8 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

// --- Doorway ---

#[derive(Resource)]
pub(super) struct DoorwayCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
    pub(super) next_at: f32,
    pub(super) from_hallway: bool,
}

impl DoorwayCaptureRequest {
    pub(super) fn new(path: String, from_hallway: bool) -> Self {
        Self {
            path,
            phase: 0,
            next_at: 0.0,
            from_hallway,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn capture_doorway_progress(
    time: Res<Time>,
    mut request: ResMut<DoorwayCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchRuntime>>,
    tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut transforms: ParamSet<(
        Query<&mut Transform, With<GameCam>>,
        Query<(&DoorLeaf, &mut Transform)>,
    )>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                rt.live.host.match_state.reroute_feedback_ticks = 0;
                if request.from_hallway {
                    let (from, to) = {
                        let game = rt.live.host_match();
                        let from = game.local_room();
                        let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
                        (from, to)
                    };
                    let variation = hallway::variation_for(
                        from,
                        to,
                        flow::MATCH_SEED,
                        rt.live.host_match().reroute_commits,
                    );
                    crate::screens::debug_place_into(
                        &mut tp,
                        &rt,
                        Place::Hallway {
                            from,
                            to,
                            variation,
                        },
                        from,
                        &keys,
                        &item_state,
                    );
                }
                let aim = if request.from_hallway {
                    tp.geom
                        .gaps
                        .iter()
                        .find(|g| g.kind == teleport::GapKind::Exit)
                        .copied()
                } else {
                    tp.geom.forward_gap().copied()
                };
                if let Some(gap) = aim {
                    let back = if request.from_hallway {
                        teleport::ENTRY_INSET
                    } else {
                        1.6
                    };
                    let pitch = if request.from_hallway { -0.45 } else { -0.14 };
                    let y_offset = teleport::place_y_offset(tp.place);
                    let (position, yaw, pitch) = camera::doorway_body_pose(
                        &gap,
                        y_offset + tp.config.half_height,
                        back,
                        pitch,
                    );
                    tp.body.position = position;
                    tp.body.yaw = yaw;
                    tp.body.pitch = pitch;
                    if let Ok(mut transform) = transforms.p0().single_mut() {
                        camera::doorway_preview_view(
                            &gap,
                            y_offset + tp.config.eye_height,
                            back,
                            pitch,
                        )
                        .apply_to(&mut transform);
                    }
                }
                for (leaf, mut transform) in &mut transforms.p1() {
                    transform.translation.y = leaf.open_y;
                }
                request.phase = 2;
                request.next_at = elapsed + 0.4;
            }
        }
        2 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 3;
            request.next_at = elapsed + 1.0;
        }
        3 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

// --- Keystone ---

#[derive(Resource)]
pub(super) struct KeystoneCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
}

impl KeystoneCaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self { path, phase: 0 }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_keystone_progress(
    time: Res<Time>,
    mut request: ResMut<KeystoneCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchRuntime>>,
    tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                if let Some(&room) = keys.rooms.first() {
                    crate::screens::debug_place_into(
                        &mut tp,
                        &rt,
                        Place::Room(room),
                        room,
                        &keys,
                        &item_state,
                    );
                    tp.body.position = Vec3::new(0.0, tp.config.half_height, 5.0);
                }
                request.phase = 2;
            }
        }
        2 if elapsed >= 0.8 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 3;
        }
        3 if elapsed >= 1.8 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform =
            Transform::from_xyz(0.0, 1.7, 5.2).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    }
}

// --- Rivals ---

#[derive(Resource)]
pub(super) struct RivalCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
    pub(super) next_at: f32,
}

impl RivalCaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self {
            path,
            phase: 0,
            next_at: 0.0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_rivals_progress(
    time: Res<Time>,
    mut request: ResMut<RivalCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    mut runtime: Option<ResMut<MatchRuntime>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let Some(rt) = runtime.as_mut() {
                rt.done = true;
                request.phase = 2;
                request.next_at = elapsed + 1.2;
            }
        }
        2 if elapsed >= request.next_at => {
            for (entity, name) in &geometry {
                if name.as_str() == "Place ceiling" {
                    commands.entity(entity).despawn();
                }
            }
            request.phase = 3;
            request.next_at = elapsed + 0.4;
        }
        3 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 4;
            request.next_at = elapsed + 1.0;
        }
        4 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform =
            Transform::from_xyz(0.0, 9.0, 9.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
    }
}

// --- Menu Banner (CaptureRequest) ---

#[derive(Resource)]
pub(super) struct CaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
}

impl CaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self { path, phase: 0 }
    }
}

pub(super) fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        for _ in 0..4 {
            career.record(flow::play_match());
            career.award();
        }
        next.set(GameState::MainMenu);
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

// --- Match ---

#[derive(Resource)]
pub(super) struct MatchCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
}

impl MatchCaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self { path, phase: 0 }
    }
}

pub(super) fn capture_match_progress(
    time: Res<Time>,
    mut request: ResMut<MatchCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    mut runtime: Option<ResMut<MatchRuntime>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        next.set(GameState::Match);
        request.phase = 1;
    } else if request.phase == 1 {
        if let Some(runtime) = runtime.as_mut() {
            for _ in 0..5 {
                if runtime.live.finished() {
                    break;
                }
                let action = if runtime.live.local_active() {
                    LocalAction::Advance
                } else {
                    LocalAction::Wait
                };
                runtime.live.force_round(action);
                for _ in 0..400 {
                    if runtime.live.in_sync() {
                        break;
                    }
                    runtime.live.pump();
                }
            }
            runtime.done = true;
            runtime.live.host.match_state.reroute_feedback_ticks = 0;
            request.phase = 2;
        }
    } else if request.phase == 2 && elapsed >= 2.5 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 3;
    } else if request.phase == 3 && elapsed >= 3.5 {
        exit.write(AppExit::Success);
    }
}

// --- Semantic Map Audit ---

#[derive(Resource)]
pub(super) struct MapAuditCaptureRequest {
    pub(super) dir: String,
    pub(super) phase: u8,
    pub(super) next_at: f32,
    pub(super) index: usize,
    pub(super) rooms: Vec<RoomId>,
}

impl MapAuditCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        let spec = observed_facility::map_spec::sector_relay_v1();
        Self {
            dir,
            phase: 0,
            next_at: 0.0,
            index: 0,
            rooms: map_validation::semantic_capture_rooms(&spec),
        }
    }

    fn current_room(&self) -> Option<RoomId> {
        self.rooms.get(self.index).copied()
    }

    fn screenshot_path(&self, room: RoomId) -> String {
        let spec = observed_facility::map_spec::sector_relay_v1();
        let role = spec
            .room(room)
            .map(|room| room.role.label())
            .unwrap_or("unknown")
            .replace(' ', "_");
        PathBuf::from(&self.dir)
            .join(format!(
                "map_audit_{:02}_r{:02}_{role}.png",
                self.index, room.0
            ))
            .to_string_lossy()
            .into_owned()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_map_audit_progress(
    time: Res<Time>,
    mut request: ResMut<MapAuditCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    mut runtime: Option<ResMut<MatchRuntime>>,
    mut tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(rt), Some(tp), Some(keys), Some(item_state)) = (
                runtime.as_mut(),
                tp.as_mut(),
                keys.as_ref(),
                item_state.as_ref(),
            ) {
                rt.done = true;
                rt.live.host.match_state.reroute_feedback_ticks = 0;
                if let Some(room) = request.current_room() {
                    crate::screens::debug_place_into(
                        tp,
                        rt,
                        Place::Room(room),
                        room,
                        keys,
                        item_state,
                    );
                    let y_offset = teleport::place_y_offset(tp.place);
                    let frame = tp.geom.half.x.max(tp.geom.half.y).max(8.0);
                    tp.body.position = Vec3::new(0.0, y_offset + frame * 0.8, frame * 1.15);
                    tp.body.yaw = 0.0;
                    tp.body.pitch = -0.62;
                    request.phase = 2;
                    request.next_at = elapsed + 0.7;
                } else {
                    request.phase = 4;
                    request.next_at = elapsed + 0.2;
                }
            }
        }
        2 if elapsed >= request.next_at => {
            for (entity, name) in &geometry {
                if name.as_str() == "Place ceiling" {
                    commands.entity(entity).despawn();
                }
            }
            request.phase = 3;
            request.next_at = elapsed + 0.25;
        }
        3 if elapsed >= request.next_at => {
            if let Some(room) = request.current_room() {
                let path = request.screenshot_path(room);
                let report = map_validation::capture_report_for_room(
                    &observed_facility::map_spec::sector_relay_v1(),
                    flow::MATCH_SEED,
                    0,
                    room,
                    path.clone(),
                );
                info!("MAP_AUDIT_CAPTURE: {report}");
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                request.index += 1;
                request.phase = if request.index >= request.rooms.len() {
                    4
                } else {
                    1
                };
                request.next_at = elapsed + 0.8;
            } else {
                request.phase = 4;
                request.next_at = elapsed + 0.2;
            }
        }
        4 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }

    if request.phase >= 2
        && request.phase <= 3
        && let (Some(tp), Ok(mut transform)) = (tp.as_ref(), cam.single_mut())
    {
        let y_offset = teleport::place_y_offset(tp.place);
        let frame = tp.geom.half.x.max(tp.geom.half.y).max(8.0);
        *transform = Transform::from_xyz(0.0, y_offset + frame * 2.35, frame * 0.18)
            .looking_at(Vec3::new(0.0, y_offset + 0.8, 0.0), Vec3::Y);
    }
}

// --- Maze / Room ---

#[derive(Resource)]
pub(super) struct MazeCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
    pub(super) next_at: f32,
    pub(super) into_maze: bool,
}

impl MazeCaptureRequest {
    pub(super) fn new(path: String, into_maze: bool) -> Self {
        Self {
            path,
            phase: 0,
            next_at: 0.0,
            into_maze,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_maze_progress(
    time: Res<Time>,
    mut request: ResMut<MazeCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchRuntime>>,
    tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                if request.into_maze {
                    let (from, to) = {
                        let game = rt.live.host_match();
                        let from = game.local_room();
                        let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
                        (from, to)
                    };
                    let variation = hallway::TEMPLATES
                        .iter()
                        .position(|t| t.grid == Some((6, 7)))
                        .unwrap_or(0);
                    crate::screens::debug_place_into(
                        &mut tp,
                        &rt,
                        Place::Hallway {
                            from,
                            to,
                            variation,
                        },
                        from,
                        &keys,
                        &item_state,
                    );
                }
                if !request.into_maze {
                    for _ in 0..12 {
                        let room = rt.live.host_match().local_room();
                        crate::screens::debug_place_into(
                            &mut tp,
                            &rt,
                            Place::Room(room),
                            room,
                            &keys,
                            &item_state,
                        );
                        if tp.geom.poly.as_ref().map_or(0, |p| p.len()) >= 6 {
                            break;
                        }
                        let act = if rt.live.local_active() {
                            LocalAction::Advance
                        } else {
                            LocalAction::Wait
                        };
                        rt.live.force_round(act);
                        for _ in 0..400 {
                            if rt.live.in_sync() {
                                break;
                            }
                            rt.live.pump();
                        }
                    }
                }
                request.phase = 2;
                request.next_at = elapsed + 0.6;
            }
        }
        2 => {
            if elapsed >= request.next_at {
                for (entity, name) in &geometry {
                    if name.as_str() == "Place ceiling" {
                        commands.entity(entity).despawn();
                    }
                }
                request.phase = 3;
                request.next_at = elapsed + 0.4;
            }
        }
        3 => {
            if elapsed >= request.next_at {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(request.path.clone()));
                request.phase = 4;
                request.next_at = elapsed + 1.0;
            }
        }
        _ => {
            if elapsed >= request.next_at {
                exit.write(AppExit::Success);
            }
        }
    }
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform = Transform::from_xyz(0.0, 42.0, 0.1).looking_at(Vec3::ZERO, Vec3::NEG_Z);
    }
}
