use bevy::app::AppExit;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use observed_core::RoomId;
use observed_facility::map_spec::MapSpec;
use observed_match::hybrid::LocalAction;

use crate::GameState;
use crate::flow::{self, Career};
use crate::map_validation;
use crate::sim::director::MatchDirector;
use crate::sim::replay::ReplayTape;
use crate::sim::state::{MatchPaused, TeleportState};
use crate::teleport::{self, Place};
use crate::view::components::{DoorLeaf, GameCam, PlaceGeometry};
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
    runtime: Option<ResMut<MatchDirector>>,
    tp: Option<ResMut<TeleportState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (Some(mut rt), Some(mut tp)) = (runtime, tp) {
                rt.done = true;
                rt.suppress_reroute_feedback();
                tp.body.position = Vec3::new(0.0, tp.config.half_height, 0.0);
                tp.body.yaw = 0.0;
                tp.body.pitch = 1.22;
                request.phase = 2;
            }
        }
        2 if elapsed >= 0.8 => {
            crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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
    runtime: Option<ResMut<MatchDirector>>,
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
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                rt.suppress_reroute_feedback();
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
                    crate::screens::match_runtime::debug_place_into(
                        &mut tp,
                        &rt,
                        Place::legacy_hallway(from, to, variation),
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
                // The manifest-selected glTF gate expands asynchronously after the place
                // rebuild. Give it time to become visible so this evidence captures the
                // canonical threshold, not only its procedural semantic accents.
                request.next_at = elapsed + 1.2;
            }
        }
        2 if elapsed >= request.next_at => {
            crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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
    runtime: Option<ResMut<MatchDirector>>,
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
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                if let Some(&room) = keys.rooms.first() {
                    crate::screens::match_runtime::debug_place_into(
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
            crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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
    mut runtime: Option<ResMut<MatchDirector>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
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
            crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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

// --- Pause-settings rebind overlay (Phase 63) ---

#[derive(Resource)]
pub(super) struct RebindCaptureRequest {
    pub(super) path: String,
    pub(super) phase: u8,
    pub(super) next_at: f32,
}

impl RebindCaptureRequest {
    pub(super) fn new(path: String) -> Self {
        Self {
            path,
            phase: 0,
            next_at: 0.0,
        }
    }
}

/// Drive the game into a paused Match with the pause-settings overlay open and a
/// rebind capture armed on the Jump row, then screenshot it — the falsifiable
/// evidence for Phase 63 (the "press a key" prompt and the binding-conflict warning
/// must be visible in the image, not just asserted by tests). The deliberate
/// jump/move-left conflict is in-memory only (never saved), so the run leaves the
/// player's settings file untouched.
#[allow(clippy::too_many_arguments)]
pub(super) fn capture_rebind_progress(
    time: Res<Time>,
    mut request: ResMut<RebindCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchDirector>>,
    paused: Option<ResMut<MatchPaused>>,
    open: Option<ResMut<crate::screens::match_runtime::pause_settings::PauseSettingsOpen>>,
    cursor: Option<ResMut<crate::screens::match_runtime::pause_settings::PauseSettingsCursor>>,
    rebind: Option<ResMut<crate::screens::match_runtime::pause_settings::PauseSettingsRebind>>,
    mut settings: ResMut<crate::settings::Settings>,
    mut panel: Query<&mut Visibility, With<crate::view::components::PauseSettingsPanel>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    use crate::screens::settings::SettingsRow;
    use crate::settings::BindingSlot;

    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (
                Some(mut rt),
                Some(mut paused),
                Some(mut open),
                Some(mut cursor),
                Some(mut rebind),
            ) = (runtime, paused, open, cursor, rebind)
            {
                rt.done = true;
                rt.suppress_reroute_feedback();
                paused.0 = true;
                open.0 = true;
                if let Ok(mut visibility) = panel.single_mut() {
                    *visibility = Visibility::Visible;
                }
                // Show the conflict warning alongside the capture prompt: bind Jump
                // onto Move-left's key for this run only (never persisted).
                let clash = BindingSlot::MoveLeft.get(&settings.bindings);
                BindingSlot::Jump.set(&mut settings.bindings, clash);
                cursor.0 = SettingsRow::all()
                    .iter()
                    .position(|row| matches!(row, SettingsRow::Binding(BindingSlot::Jump)))
                    .unwrap_or(0);
                rebind.0.begin_armed(BindingSlot::Jump);
                request.phase = 2;
                request.next_at = elapsed + 0.8;
            }
        }
        2 if elapsed >= request.next_at => {
            crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
            request.phase = 3;
            request.next_at = elapsed + 1.0;
        }
        3 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

// --- Menu Banner (CaptureRequest) ---

#[derive(Resource)]
pub(super) struct ResultsCaptureRequest {
    dir: PathBuf,
    index: usize,
    end_index: usize,
    phase: u8,
    next_at: f32,
}

impl ResultsCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        let selected = std::env::var("OBSERVED2_RESULTS_SHAPE")
            .ok()
            .and_then(|shape| match shape.trim().to_ascii_lowercase().as_str() {
                "victory" | "win" | "0" => Some(0),
                "placed" | "place" | "1" => Some(1),
                "absorbed" | "loss" | "2" => Some(2),
                "solo" | "3" => Some(3),
                _ => None,
            });
        let index = selected.unwrap_or(0);
        Self {
            dir: PathBuf::from(dir),
            index,
            end_index: selected.map_or(RESULTS_CAPTURE_CASES, |index| index + 1),
            phase: 0,
            next_at: 0.0,
        }
    }
}

pub(super) fn capture_results_progress(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut request: ResMut<ResultsCaptureRequest>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            let (name, result, solo, tape) = staged_results_case(request.index);
            *career = Career::default();
            career.bot_rival_teams = !solo;
            career.record(result);
            let _ = career.award();
            career.last_unlocks.clear();
            commands.insert_resource(tape);
            next.set(GameState::Results);
            request.next_at = elapsed + 1.5;
            request.phase = 1;
            info!("RESULTS_CAPTURE staged={name}");
        }
        1 if *state.get() == GameState::Results && elapsed >= request.next_at => {
            let (name, _, _, _) = staged_results_case(request.index);
            let path = request.dir.join(format!("{name}.png"));
            crate::evidence::driver::screenshot_to(
                &mut commands,
                path.to_string_lossy().into_owned(),
            );
            request.next_at = elapsed + 1.0;
            request.phase = 2;
        }
        1 if *state.get() != GameState::Results => {
            next.set(GameState::Results);
        }
        2 if elapsed >= request.next_at => {
            request.index += 1;
            if request.index >= request.end_index {
                if std::env::var("OBSERVED2_RESULTS_HOLD").is_ok() {
                    request.phase = 4;
                } else {
                    exit.write(AppExit::Success);
                }
            } else {
                next.set(GameState::MainMenu);
                request.next_at = elapsed + 0.8;
                request.phase = 3;
            }
        }
        3 if *state.get() == GameState::MainMenu && elapsed >= request.next_at => {
            request.phase = 0;
        }
        _ => {}
    }
}

const RESULTS_CAPTURE_CASES: usize = 4;

fn staged_results_case(index: usize) -> (&'static str, flow::MatchResult, bool, ReplayTape) {
    use observed_core::TeamId;

    let cases = [
        (
            "00_victory",
            flow::MatchResult {
                placement: Some(1),
                escaped: 2,
                absorbed: 2,
                winner: Some(TeamId(0)),
                local_won: true,
            },
            false,
        ),
        (
            "01_placed",
            flow::MatchResult {
                placement: Some(2),
                escaped: 2,
                absorbed: 2,
                winner: Some(TeamId(1)),
                local_won: false,
            },
            false,
        ),
        (
            "02_absorbed",
            flow::MatchResult {
                placement: None,
                escaped: 1,
                absorbed: 3,
                winner: Some(TeamId(3)),
                local_won: false,
            },
            false,
        ),
        (
            "03_solo",
            flow::MatchResult {
                placement: Some(1),
                escaped: 1,
                absorbed: 0,
                winner: Some(TeamId(0)),
                local_won: true,
            },
            true,
        ),
    ];
    let (name, result, solo) = cases[index].clone();
    let spec = crate::map_catalog::default_map_spec(6_600 + index as u64);
    let mut tape = ReplayTape::new(6_600 + index as u64, &spec);
    for round in 0..6 {
        tape.push_sample(round, 1, Vec::new());
    }
    tape.visited_rooms = vec![RoomId(0), RoomId(3), RoomId(7), RoomId(11), RoomId(18)];
    tape.collapsed_rooms = if solo {
        Vec::new()
    } else {
        vec![RoomId(0), RoomId(3), RoomId(7)]
    };
    tape.escape_order = if solo {
        vec![TeamId(0)]
    } else if result.local_won {
        vec![TeamId(0), TeamId(1)]
    } else {
        vec![result.winner.unwrap_or(TeamId(1)), TeamId(0)]
    };
    tape.keystones_collected = 3;
    tape.keystones_required = 3;
    tape.anchor_uses = if solo { 2 } else { 1 };
    tape.result = Some(result.clone());
    (name, result, solo, tape)
}

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
        crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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
    mut runtime: Option<ResMut<MatchDirector>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        if runtime.is_some() {
            request.phase = 1;
        } else {
            next.set(GameState::Match);
        }
    } else if request.phase == 1 {
        if let Some(runtime) = runtime.as_mut() {
            runtime.force_scripted_rounds(5);
            runtime.done = true;
            runtime.suppress_reroute_feedback();
            request.phase = 2;
        }
    } else if request.phase == 2 && elapsed >= 2.5 {
        crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
        request.phase = 3;
    } else if request.phase == 3 && elapsed >= 3.5 {
        exit.write(AppExit::Success);
    }
}

// --- Semantic Map Audit ---

#[derive(Resource)]
pub(super) struct MapAuditCaptureRequest {
    pub(super) dir: String,
    pub(super) spec: MapSpec,
    pub(super) phase: u8,
    pub(super) next_at: f32,
    pub(super) index: usize,
    pub(super) rooms: Vec<RoomId>,
}

impl MapAuditCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        let spec = crate::map_catalog::active_map_spec(flow::MATCH_SEED);
        let rooms = map_validation::semantic_capture_rooms(&spec);
        Self {
            dir,
            spec,
            phase: 0,
            next_at: 0.0,
            index: 0,
            rooms,
        }
    }

    fn current_room(&self) -> Option<RoomId> {
        self.rooms.get(self.index).copied()
    }

    fn screenshot_path(&self, room: RoomId) -> String {
        let role = self
            .spec
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
    mut runtime: Option<ResMut<MatchDirector>>,
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
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (Some(rt), Some(tp), Some(keys), Some(item_state)) = (
                runtime.as_mut(),
                tp.as_mut(),
                keys.as_ref(),
                item_state.as_ref(),
            ) {
                rt.done = true;
                rt.suppress_reroute_feedback();
                if let Some(room) = request.current_room() {
                    crate::screens::match_runtime::debug_place_into(
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
                    &request.spec,
                    flow::MATCH_SEED,
                    0,
                    room,
                    path.clone(),
                );
                info!("MAP_AUDIT_CAPTURE: {report}");
                crate::evidence::driver::screenshot_to(&mut commands, path);
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

// --- Wellshaft ---------------------------------------------------------------

#[derive(Resource)]
pub(super) struct WellshaftCaptureRequest {
    dir: PathBuf,
    phase: u8,
    next_at: f32,
}

impl WellshaftCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        Self {
            dir: PathBuf::from(dir),
            phase: 0,
            next_at: 0.0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_wellshaft_progress(
    time: Res<Time>,
    mut request: ResMut<WellshaftCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchDirector>>,
    tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut fog: Query<&mut DistanceFog, (With<GameCam>, Without<PlaceGeometry>)>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    let framing_phase = request.phase;
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                let (from, to) = {
                    let game = rt.live.host_match();
                    let from = game.local_room();
                    (from, game.local_target().unwrap_or(RoomId(from.0 + 1)))
                };
                let variation = hallway::TEMPLATES
                    .iter()
                    .position(|template| template.flavor == hallway::HallwayFlavor::Wellshaft)
                    .expect("wellshaft capture template");
                crate::screens::match_runtime::debug_place_into(
                    &mut tp,
                    &rt,
                    Place::legacy_hallway(from, to, variation),
                    from,
                    &keys,
                    &item_state,
                );
                request.phase = 2;
                // The wellshaft snaps the global ambient down toward near-black
                // (apply_place_atmosphere eases at DISTRICT_BLEND_RATE); wait for
                // it to settle so the capture shows the register at rest, not the
                // half-lit transient the previous place's ambient bleeds into.
                request.next_at = elapsed + 3.0;
            }
        }
        2 if elapsed >= request.next_at => {
            crate::evidence::driver::screenshot_to(
                &mut commands,
                request
                    .dir
                    .join("wellshaft_top.png")
                    .to_string_lossy()
                    .to_string(),
            );
            request.phase = 3;
            request.next_at = elapsed + 1.2;
        }
        3 if elapsed >= request.next_at => {
            for (entity, name) in &geometry {
                if name.as_str() == "Place ceiling" {
                    commands.entity(entity).despawn();
                }
            }
            request.phase = 4;
            request.next_at = elapsed + 0.5;
        }
        4 if elapsed >= request.next_at => {
            crate::evidence::driver::screenshot_to(
                &mut commands,
                request
                    .dir
                    .join("wellshaft_bottom.png")
                    .to_string_lossy()
                    .to_string(),
            );
            request.phase = 5;
            request.next_at = elapsed + 1.2;
        }
        5 if elapsed >= request.next_at => {
            crate::evidence::driver::screenshot_to(
                &mut commands,
                request
                    .dir
                    .join("wellshaft_birdseye.png")
                    .to_string_lossy()
                    .to_string(),
            );
            request.phase = 6;
            request.next_at = elapsed + 1.2;
        }
        6 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }

    if framing_phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        let y = teleport::place_y_offset(Place::legacy_hallway(RoomId(0), RoomId(1), 0));
        let top = y + hallway::WELL_SHAFT_HEIGHT;
        let first_upper = hallway::wellshaft_landing_center(1);
        let top_landing = hallway::wellshaft_landing_center(hallway::WELL_SHAFT_LEVELS - 1);
        let next_landing = hallway::wellshaft_landing_center(hallway::WELL_SHAFT_LEVELS - 2);
        *transform = match framing_phase {
            // Eye-level descent from inside the elevated entry: the live bridge,
            // pillar face, first flight, and receiving landing share the frame.
            2 => Transform::from_xyz(top_landing.0, top + 1.65, top_landing.1).looking_at(
                Vec3::new(
                    next_landing.0,
                    top - hallway::WELL_SHAFT_LEVEL_HEIGHT + 0.45,
                    next_landing.1,
                ),
                Vec3::Y,
            ),
            // Ground bridge looking directly up the first flight toward level one.
            4 => Transform::from_xyz(hallway::WELL_SHAFT_BRIDGE_END_RADIUS - 0.8, y + 1.7, 0.0)
                .looking_at(
                    Vec3::new(
                        first_upper.0,
                        y + hallway::WELL_SHAFT_LEVEL_HEIGHT + 1.2,
                        first_upper.1,
                    ),
                    Vec3::Y,
                ),
            // Plan view straight down the well.
            _ => Transform::from_xyz(0.1, top + 36.0, 0.1).looking_at(
                Vec3::new(0.0, y + hallway::WELL_SHAFT_HEIGHT * 0.45, 0.0),
                Vec3::NEG_Z,
            ),
        };
    }
    if framing_phase >= 5
        && let Ok(mut fog) = fog.single_mut()
    {
        fog.falloff = FogFalloff::Linear {
            start: 100.0,
            end: 140.0,
        };
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_maze_progress(
    time: Res<Time>,
    mut request: ResMut<MazeCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchDirector>>,
    tp: Option<ResMut<TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut fog: Query<&mut DistanceFog, (With<GameCam>, Without<PlaceGeometry>)>,
    geometry: Query<(Entity, &Name), With<PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
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
                    crate::screens::match_runtime::debug_place_into(
                        &mut tp,
                        &rt,
                        Place::legacy_hallway(from, to, variation),
                        from,
                        &keys,
                        &item_state,
                    );
                }
                if !request.into_maze {
                    for _ in 0..12 {
                        let room = rt.live.host_match().local_room();
                        crate::screens::match_runtime::debug_place_into(
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
                crate::evidence::driver::screenshot_to(&mut commands, request.path.clone());
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
    if request.phase >= 2 {
        if let Ok(mut transform) = cam.single_mut() {
            *transform = Transform::from_xyz(0.0, 42.0, 0.1).looking_at(Vec3::ZERO, Vec3::NEG_Z);
        }
        // The bird's-eye vantage sits far beyond every district's fog_end; without
        // relaxing the fog this diagnostic photographs nothing but fog (the black
        // "drained room" evidence that shipped with Phase 62 was exactly this).
        if let Ok(mut fog) = fog.single_mut() {
            fog.falloff = FogFalloff::Linear {
                start: 300.0,
                end: 360.0,
            };
        }
    }
}
