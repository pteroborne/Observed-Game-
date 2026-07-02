//! Opt-in visual diagnostics for the assembled game.
//!
//! `OBSERVED2_VIS_AUDIT=<dir>` drives the match through a small set of inspection
//! scenarios, captures a screenshot for each one, and writes a JSON snapshot with
//! machine-readable checks. The screenshots remain for human review; the JSON is
//! the agent-readable bridge for visual bugs.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_core::RoomId;
use observed_diagnostics::{
    DEFAULT_SIGNAL_MIN_LUMINANCE, DiagnosticFinding, DiagnosticRun, DiagnosticSnapshot,
    DiagnosticSnapshotSummary, FootprintSnapshot, GeometrySnapshot, LightSnapshot,
    MaterialSnapshot, MonitorSnapshot, PlaceSnapshot, TacMapSnapshot, ThresholdSnapshot,
};
use observed_style::{self as style, MarkerRole};

use crate::camera;
use crate::guardian::{ActionLog, Guardian, GuardianModel, GuardianState};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::screens::place::{GuardianConsole, TetherCameraMonitor};
use crate::screens::{self};
use crate::sim::state::{MatchRuntime, TeleportState};
use crate::teleport::{self, DoorGap, GapKind, Place, ThresholdLink};
use crate::view::components::{
    DroppedItemVisual, GameCam, KeystoneItem, RivalAvatar, TacMapElement, TacMapPanel, TacMapState,
};
use crate::{GameState, tacmap};

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticThresholdVisualKind {
    Frame,
    Leaf,
    FrameLight,
    Label,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticThresholdStatus {
    Passage,
    TetheredPassage,
    Locked,
    Sealed,
}

impl DiagnosticThresholdStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            DiagnosticThresholdStatus::Passage => "passage",
            DiagnosticThresholdStatus::TetheredPassage => "tethered_passage",
            DiagnosticThresholdStatus::Locked => "locked",
            DiagnosticThresholdStatus::Sealed => "sealed",
        }
    }
}

#[derive(Clone, Component, Debug)]
pub(crate) struct DiagnosticThresholdVisual {
    pub(crate) threshold: ThresholdLink,
    pub(crate) kind: DiagnosticThresholdVisualKind,
    pub(crate) status: DiagnosticThresholdStatus,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticTacMapRole {
    Route,
    Room,
    Exit,
    Keystone,
    Rival,
    Player,
}

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) struct DiagnosticTacMapVisual {
    pub(crate) role: DiagnosticTacMapRole,
    pub(crate) room: Option<RoomId>,
    pub(crate) other_room: Option<RoomId>,
}

impl DiagnosticTacMapVisual {
    pub(crate) fn route(a: RoomId, b: RoomId) -> Self {
        Self {
            role: DiagnosticTacMapRole::Route,
            room: Some(a),
            other_room: Some(b),
        }
    }

    pub(crate) fn room(room: RoomId) -> Self {
        Self {
            role: DiagnosticTacMapRole::Room,
            room: Some(room),
            other_room: None,
        }
    }

    pub(crate) fn one(role: DiagnosticTacMapRole, room: Option<RoomId>) -> Self {
        Self {
            role,
            room,
            other_room: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuditScenario {
    Geometry,
    Thresholds,
    Lighting,
    TacMap,
    TetherCameraRoom,
    GuardianCameraRoom,
    FootprintAtlas,
}

impl AuditScenario {
    const ALL: [AuditScenario; 7] = [
        AuditScenario::Geometry,
        AuditScenario::Thresholds,
        AuditScenario::Lighting,
        AuditScenario::TacMap,
        AuditScenario::TetherCameraRoom,
        AuditScenario::GuardianCameraRoom,
        AuditScenario::FootprintAtlas,
    ];

    fn parse(value: &str) -> Vec<Self> {
        match value {
            "geometry" => vec![Self::Geometry],
            "thresholds" => vec![Self::Thresholds],
            "lighting" => vec![Self::Lighting],
            "tacmap" | "tac-map" => vec![Self::TacMap],
            "camera_rooms" | "camera-rooms" => {
                vec![Self::TetherCameraRoom, Self::GuardianCameraRoom]
            }
            "atlas" | "footprints" | "footprint_atlas" | "footprint-atlas" | "topdown" => {
                vec![Self::FootprintAtlas]
            }
            _ => Self::ALL.to_vec(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Geometry => "geometry",
            Self::Thresholds => "thresholds",
            Self::Lighting => "lighting",
            Self::TacMap => "tacmap",
            Self::TetherCameraRoom => "tether_camera_room",
            Self::GuardianCameraRoom => "guardian_camera_room",
            Self::FootprintAtlas => "footprint_atlas",
        }
    }
}

#[derive(Resource)]
struct VisualAudit {
    dir: PathBuf,
    scenarios: Vec<AuditScenario>,
    index: usize,
    phase: u8,
    next_at: f32,
    frame_index: u32,
    run: DiagnosticRun,
}

#[derive(Resource)]
struct FreeCamState {
    initialized: bool,
    yaw: f32,
    pitch: f32,
    speed: f32,
}

impl Default for FreeCamState {
    fn default() -> Self {
        Self {
            initialized: false,
            yaw: 0.0,
            pitch: -1.35,
            speed: 14.0,
        }
    }
}

type ThresholdVisualQueryData = (
    &'static DiagnosticThresholdVisual,
    Option<&'static PointLight>,
    Option<&'static Name>,
);

type LightQueryData = (
    &'static PointLight,
    Option<&'static Name>,
    Option<&'static DiagnosticThresholdVisual>,
);

type MaterialDiagnosticQueryData = (
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static Name>,
    Option<&'static KeystoneItem>,
    Option<&'static DroppedItemVisual>,
    Option<&'static RivalAvatar>,
    Option<&'static GuardianModel>,
    Option<&'static GuardianConsole>,
);

type MonitorDiagnosticQueryData = (
    Option<&'static TetherCameraMonitor>,
    Option<&'static crate::screens::place::GuardianObservationMonitor>,
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static Name>,
);

type MonitorLabelSegmentDiagnosticQueryData =
    &'static crate::screens::place::ObservationMonitorLabelSegment;

impl VisualAudit {
    fn new(dir: PathBuf, scenarios: Vec<AuditScenario>) -> Self {
        let run_id = run_id();
        let run = DiagnosticRun::new(
            run_id,
            scenarios
                .iter()
                .map(|scenario| scenario.label().to_string())
                .collect(),
        );
        Self {
            dir,
            scenarios,
            index: 0,
            phase: 0,
            next_at: 0.0,
            frame_index: 0,
            run,
        }
    }

    fn current(&self) -> Option<AuditScenario> {
        self.scenarios.get(self.index).copied()
    }

    fn image_path(&self, scenario: AuditScenario) -> PathBuf {
        self.dir
            .join(format!("{:02}_{}.png", self.index, scenario.label()))
    }

    fn json_path(&self, scenario: AuditScenario) -> PathBuf {
        self.dir
            .join(format!("{:02}_{}.json", self.index, scenario.label()))
    }
}

#[derive(SystemParam)]
struct VisualAuditParams<'w, 's> {
    runtime: Option<ResMut<'w, MatchRuntime>>,
    tp: Option<ResMut<'w, TeleportState>>,
    keys: Option<Res<'w, KeystoneState>>,
    items: Option<ResMut<'w, ItemsState>>,
    guardian: Option<ResMut<'w, Guardian>>,
    camera: Query<'w, 's, &'static mut Transform, With<GameCam>>,
    fog: Query<'w, 's, &'static mut DistanceFog, With<GameCam>>,
    tac_state: Option<ResMut<'w, TacMapState>>,
    tac_panel: Query<'w, 's, &'static mut Visibility, With<TacMapPanel>>,
    threshold_visuals: Query<'w, 's, ThresholdVisualQueryData>,
    lights: Query<'w, 's, LightQueryData>,
    materials_query: Query<'w, 's, MaterialDiagnosticQueryData>,
    monitor_materials: Query<'w, 's, MonitorDiagnosticQueryData>,
    monitor_label_segments: Query<'w, 's, MonitorLabelSegmentDiagnosticQueryData>,
    tac_visuals: Query<'w, 's, &'static DiagnosticTacMapVisual, With<TacMapElement>>,
    materials: Res<'w, Assets<StandardMaterial>>,
}

struct ScenarioPrep<'a, 'w, 's> {
    runtime: &'a mut MatchRuntime,
    tp: &'a mut TeleportState,
    keys: &'a KeystoneState,
    items: &'a mut ItemsState,
    guardian: &'a mut Guardian,
    tac_state: Option<&'a mut TacMapState>,
    tac_panel: &'a mut Query<'w, 's, &'static mut Visibility, With<TacMapPanel>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Resource)]
struct DebugMatchCoercion {
    tether_rooms: Vec<RoomId>,
    guardian_room: Option<RoomId>,
    player_room: Option<RoomId>,
    applied: bool,
}

impl DebugMatchCoercion {
    fn from_env() -> Option<Self> {
        Self::from_values(
            std::env::var("OBSERVED2_DEBUG_TETHERS").ok().as_deref(),
            std::env::var("OBSERVED2_DEBUG_GUARDIAN").ok().as_deref(),
            std::env::var("OBSERVED2_DEBUG_ROOM").ok().as_deref(),
        )
    }

    fn from_values(
        tethers: Option<&str>,
        guardian: Option<&str>,
        player_room: Option<&str>,
    ) -> Option<Self> {
        let tether_rooms = tethers.map(parse_debug_room_list).unwrap_or_default();
        let guardian_room = guardian.and_then(parse_debug_room);
        let player_room = player_room.and_then(parse_debug_room);
        (!tether_rooms.is_empty() || guardian_room.is_some() || player_room.is_some()).then_some(
            Self {
                tether_rooms,
                guardian_room,
                player_room,
                applied: false,
            },
        )
    }
}

#[derive(SystemParam)]
struct DebugCoercionParams<'w> {
    runtime: Option<ResMut<'w, MatchRuntime>>,
    tp: Option<ResMut<'w, TeleportState>>,
    keys: Option<Res<'w, KeystoneState>>,
    items: Option<ResMut<'w, ItemsState>>,
    guardian: Option<ResMut<'w, Guardian>>,
    log: Option<ResMut<'w, ActionLog>>,
}

pub(crate) fn configure(app: &mut App) {
    if let Some(coercion) = DebugMatchCoercion::from_env() {
        app.insert_resource(coercion).add_systems(
            Update,
            apply_debug_match_coercion
                .before(crate::screens::place::rebuild_place)
                .run_if(in_state(GameState::Match)),
        );
    }

    if let Ok(dir) = std::env::var("OBSERVED2_VIS_AUDIT") {
        let scenarios = std::env::var("OBSERVED2_VIS_AUDIT_SCENARIO")
            .map(|value| AuditScenario::parse(&value))
            .unwrap_or_else(|_| AuditScenario::ALL.to_vec());
        let dir = PathBuf::from(dir);
        let _ = fs::create_dir_all(&dir);
        app.insert_resource(VisualAudit::new(dir, scenarios))
            .add_systems(
                Update,
                visual_audit_progress
                    .after(crate::screens::place::present_match_camera)
                    .after(screens::hud::draw_tac_map),
            );
    }

    if freecam_enabled() {
        app.init_resource::<FreeCamState>().add_systems(
            Update,
            freecam_control.after(crate::screens::place::present_match_camera),
        );
    }
}

fn parse_debug_room(value: &str) -> Option<RoomId> {
    let token = value.trim().to_ascii_lowercase();
    match token.as_str() {
        "tether" | "tether_camera" | "tether-camera" | "camera" => Some(RoomId(5)),
        "guardian" | "guardian_camera" | "guardian-camera" | "observation" => Some(RoomId(6)),
        _ => token
            .strip_prefix("room")
            .or_else(|| token.strip_prefix('r'))
            .unwrap_or(&token)
            .parse::<u32>()
            .ok()
            .filter(|room| *room < 9)
            .map(RoomId),
    }
}

fn parse_debug_room_list(value: &str) -> Vec<RoomId> {
    if value.trim().eq_ignore_ascii_case("all") {
        return (0..9).map(RoomId).collect();
    }
    let mut rooms: Vec<RoomId> = value
        .split([',', ';', ' '])
        .filter_map(parse_debug_room)
        .collect();
    rooms.sort_unstable_by_key(|room| room.0);
    rooms.dedup();
    rooms
}

fn apply_debug_match_coercion(
    mut coercion: ResMut<DebugMatchCoercion>,
    mut params: DebugCoercionParams,
) {
    if coercion.applied {
        return;
    }
    let (Some(runtime), Some(tp), Some(keys), Some(items), Some(guardian)) = (
        params.runtime.as_mut(),
        params.tp.as_mut(),
        params.keys.as_ref(),
        params.items.as_mut(),
        params.guardian.as_mut(),
    ) else {
        return;
    };

    let game = runtime.live.host_match();
    let version = game.reroute_commits;
    for &room in &coercion.tether_rooms {
        if items
            .placed
            .iter()
            .any(|item| item.kind == ItemKind::AnchorTorch && item.place == Place::Room(room))
        {
            continue;
        }
        items.torches = items.torches.saturating_add(1);
        let connections = crate::screens::match_runtime::connections_for(game, room);
        if items.drop_anchor_torch(Place::Room(room), Vec2::ZERO, version, &connections)
            && let Some(log) = params.log.as_mut()
        {
            log.add(format!("Debug tether spawned in Room {}.", room.0));
        }
    }

    if let Some(room) = coercion.guardian_room {
        guardian.room = room;
        guardian.pos = Vec3::new(0.0, 0.76, 0.0);
        guardian.anchor_timer = 30.0;
        guardian.state = GuardianState::Active;
        guardian.reassigned_target = None;
        if let Some(log) = params.log.as_mut() {
            log.add(format!("Debug guardian deployed to Room {}.", room.0));
        }
    }

    let target_place = coercion.player_room.map(Place::Room).unwrap_or(tp.place);
    let from = match target_place {
        Place::Room(room) => room,
        Place::Hallway { from, .. } => from,
    };
    crate::screens::match_runtime::debug_place_into(tp, runtime, target_place, from, keys, items);
    tp.rendered = None;
    if let Some(room) = coercion.player_room
        && let Some(log) = params.log.as_mut()
    {
        log.add(format!("Debug player placed in Room {}.", room.0));
    }

    coercion.applied = true;
}

pub(crate) fn visual_audit_enabled() -> bool {
    std::env::var("OBSERVED2_VIS_AUDIT").is_ok()
}

pub(crate) fn freecam_enabled() -> bool {
    std::env::var("OBSERVED2_FREECAM").is_ok()
}

pub(crate) fn threshold_label(threshold: &ThresholdLink) -> String {
    format!(
        "R{}:S{} -> H{}-{}:{}:S{}",
        threshold.room.room.0,
        threshold.room.slot.0,
        threshold.hall.hall.a.0,
        threshold.hall.hall.b.0,
        threshold.hall.side.0,
        threshold.hall.slot.0
    )
}

pub(crate) fn threshold_status(gap: &DoorGap, tethered: bool) -> DiagnosticThresholdStatus {
    if tethered && gap.kind.is_passage() {
        DiagnosticThresholdStatus::TetheredPassage
    } else if gap.kind == GapKind::LockedExit {
        DiagnosticThresholdStatus::Locked
    } else if gap.kind.is_passage() {
        DiagnosticThresholdStatus::Passage
    } else {
        DiagnosticThresholdStatus::Sealed
    }
}

fn visual_audit_progress(
    time: Res<Time>,
    mut audit: ResMut<VisualAudit>,
    mut next: ResMut<NextState<GameState>>,
    mut params: VisualAuditParams,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match audit.phase {
        0 => {
            write_manifest(&audit.dir, &audit.run);
            next.set(GameState::Match);
            audit.phase = 1;
        }
        1 => {
            let Some(scenario) = audit.current() else {
                finish_audit(&audit.dir, &audit.run);
                exit.write(AppExit::Success);
                audit.phase = 4;
                return;
            };
            if let (Some(runtime), Some(tp), Some(keys), Some(items), Some(guardian)) = (
                params.runtime.as_mut(),
                params.tp.as_mut(),
                params.keys.as_ref(),
                params.items.as_mut(),
                params.guardian.as_mut(),
            ) {
                if scenario == AuditScenario::FootprintAtlas {
                    spawn_footprint_atlas(&mut commands, runtime, keys, items);
                }
                prepare_scenario(
                    scenario,
                    ScenarioPrep {
                        runtime,
                        tp,
                        keys,
                        items,
                        guardian,
                        tac_state: params.tac_state.as_mut().map(|state| state.as_mut()),
                        tac_panel: &mut params.tac_panel,
                    },
                );
                audit.next_at = elapsed + 0.45;
                audit.phase = 2;
            }
        }
        2 if elapsed >= audit.next_at => {
            let Some(scenario) = audit.current() else {
                return;
            };
            if let (Some(runtime), Some(tp), Some(keys), Some(items), Some(guardian)) = (
                params.runtime.as_ref(),
                params.tp.as_ref(),
                params.keys.as_ref(),
                params.items.as_ref(),
                params.guardian.as_ref(),
            ) {
                if scenario == AuditScenario::FootprintAtlas {
                    relax_debug_fog(&mut params.fog);
                }
                frame_camera(scenario, tp, &mut params.camera);
                let mut snapshot = collect_snapshot(
                    &audit.run.run_id,
                    scenario,
                    audit.frame_index,
                    runtime,
                    tp,
                    keys,
                    items,
                    guardian,
                    &params.threshold_visuals,
                    &params.lights,
                    &params.materials_query,
                    &params.monitor_materials,
                    &params.monitor_label_segments,
                    &params.tac_visuals,
                    &params.materials,
                );
                snapshot.run_default_checks();
                let json_path = audit.json_path(scenario);
                let image_path = audit.image_path(scenario);
                write_snapshot(&json_path, &snapshot);
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path_string(&image_path)));
                audit.run.findings.extend(snapshot.findings.clone());
                audit.run.snapshots.push(DiagnosticSnapshotSummary {
                    scenario: scenario.label().to_string(),
                    image_path: path_string(&image_path),
                    json_path: path_string(&json_path),
                    finding_count: snapshot.findings.len(),
                });
                write_manifest(&audit.dir, &audit.run);
                audit.frame_index += 1;
                audit.next_at = elapsed + 0.9;
                audit.phase = 3;
            }
        }
        3 if elapsed >= audit.next_at => {
            audit.index += 1;
            if audit.index >= audit.scenarios.len() {
                finish_audit(&audit.dir, &audit.run);
                exit.write(AppExit::Success);
                audit.phase = 4;
            } else {
                audit.phase = 1;
            }
        }
        _ => {}
    }
}

fn prepare_scenario(scenario: AuditScenario, prep: ScenarioPrep) {
    let ScenarioPrep {
        runtime,
        tp,
        keys,
        items,
        guardian,
        tac_state,
        tac_panel,
    } = prep;

    runtime.done = true;
    runtime.live.host.match_state.reroute_feedback_ticks = 0;

    if let Some(tac_state) = tac_state {
        tac_state.0 = scenario == AuditScenario::TacMap;
    }
    if let Ok(mut visibility) = tac_panel.single_mut() {
        *visibility = if scenario == AuditScenario::TacMap {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    match scenario {
        AuditScenario::Geometry
        | AuditScenario::Thresholds
        | AuditScenario::Lighting
        | AuditScenario::FootprintAtlas => {
            let room = runtime.live.host_match().local_room();
            crate::screens::match_runtime::debug_place_into(
                tp,
                runtime,
                Place::Room(room),
                room,
                keys,
                items,
            );
        }
        AuditScenario::TacMap => {
            let room = runtime.live.host_match().local_room();
            crate::screens::match_runtime::debug_place_into(
                tp,
                runtime,
                Place::Room(room),
                room,
                keys,
                items,
            );
        }
        AuditScenario::TetherCameraRoom => {
            if !items
                .placed
                .iter()
                .any(|item| item.kind == ItemKind::AnchorTorch)
            {
                items.torches = items.torches.max(1);
                let version = runtime.live.host_match().reroute_commits;
                let _ = items.drop_anchor_torch(Place::Room(RoomId(0)), Vec2::ZERO, version, &[]);
            }
            crate::screens::match_runtime::debug_place_into(
                tp,
                runtime,
                Place::Room(RoomId(5)),
                RoomId(5),
                keys,
                items,
            );
        }
        AuditScenario::GuardianCameraRoom => {
            guardian.room = RoomId(4);
            guardian.pos = Vec3::new(0.0, 0.76, 0.0);
            crate::screens::match_runtime::debug_place_into(
                tp,
                runtime,
                Place::Room(RoomId(6)),
                RoomId(6),
                keys,
                items,
            );
        }
    }
    tp.rendered = None;
}

fn frame_camera(
    scenario: AuditScenario,
    tp: &TeleportState,
    camera: &mut Query<&mut Transform, With<GameCam>>,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let y = teleport::place_y_offset(tp.place);
    match scenario {
        AuditScenario::Geometry => {
            *transform = Transform::from_xyz(0.0, y + crate::layout::WALL_HEIGHT - 0.7, 0.1)
                .looking_at(Vec3::new(0.0, y + 0.05, 0.0), Vec3::NEG_Z);
        }
        AuditScenario::Thresholds => {
            if let Some(gap) = tp.geom.forward_gap() {
                let eye = Vec3::new(
                    gap.center.x - gap.normal.x * 4.0,
                    y + tp.config.eye_height,
                    gap.center.y - gap.normal.y * 4.0,
                );
                let target = Vec3::new(gap.center.x, y + tp.config.eye_height, gap.center.y);
                *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
            }
        }
        AuditScenario::Lighting | AuditScenario::TacMap => {
            camera::player_view(&tp.body, &tp.config).apply_to(&mut transform);
        }
        AuditScenario::TetherCameraRoom | AuditScenario::GuardianCameraRoom => {
            *transform = Transform::from_xyz(0.0, y + 2.2, 2.7)
                .looking_at(Vec3::new(0.0, y + 1.75, -5.0), Vec3::Y);
        }
        AuditScenario::FootprintAtlas => {
            *transform = Transform::from_xyz(0.0, ATLAS_Y + 145.0, 0.1)
                .looking_at(Vec3::new(0.0, ATLAS_Y, 0.0), Vec3::NEG_Z);
        }
    }
}

fn relax_debug_fog(fog: &mut Query<&mut DistanceFog, With<GameCam>>) {
    if let Ok(mut fog) = fog.single_mut() {
        fog.color = Color::srgb(0.0, 0.0, 0.0);
        fog.falloff = FogFalloff::Linear {
            start: 300.0,
            end: 360.0,
        };
    }
}

#[allow(clippy::too_many_arguments)]
fn freecam_control(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    tp: Option<Res<TeleportState>>,
    mut state: ResMut<FreeCamState>,
    mut camera: Query<&mut Transform, With<GameCam>>,
    mut fog: Query<&mut DistanceFog, With<GameCam>>,
) {
    let Some(tp) = tp else {
        return;
    };
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    relax_debug_fog(&mut fog);

    if !state.initialized || keyboard.just_pressed(KeyCode::KeyR) {
        let radius = tp.geom.half.x.max(tp.geom.half.y);
        transform.translation = Vec3::new(
            0.0,
            teleport::place_y_offset(tp.place)
                + (radius * 2.4).max(crate::layout::WALL_HEIGHT * 8.0),
            0.1,
        );
        state.yaw = 0.0;
        state.pitch = -1.35;
        state.initialized = true;
    }

    let dt = time.delta_secs();
    let look_key = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    state.yaw += look_key(KeyCode::ArrowLeft, KeyCode::ArrowRight) * dt * 1.7;
    state.pitch = (state.pitch + look_key(KeyCode::ArrowDown, KeyCode::ArrowUp) * dt * 1.5)
        .clamp(-1.50, 1.35);
    if mouse_buttons.pressed(MouseButton::Right) {
        let delta = mouse.map(|motion| motion.delta).unwrap_or(Vec2::ZERO);
        state.yaw -= delta.x * 0.003;
        state.pitch = (state.pitch - delta.y * 0.003).clamp(-1.50, 1.35);
    }

    let dir = freecam_direction(state.yaw, state.pitch);
    let forward = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::NEG_Z);
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let vertical = (keyboard.pressed(KeyCode::Space) as i32
        + keyboard.pressed(KeyCode::KeyE) as i32
        - keyboard.pressed(KeyCode::ControlLeft) as i32
        - keyboard.pressed(KeyCode::ControlRight) as i32
        - keyboard.pressed(KeyCode::KeyQ) as i32) as f32;
    let mut movement =
        right * axis(KeyCode::KeyA, KeyCode::KeyD) + forward * axis(KeyCode::KeyS, KeyCode::KeyW);
    movement += Vec3::Y * vertical;
    if movement.length_squared() > 0.0 {
        let speed = if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
        {
            state.speed * 3.0
        } else {
            state.speed
        };
        transform.translation += movement.normalize() * speed * dt;
    }

    *transform = Transform::from_translation(transform.translation).looking_to(dir, Vec3::Y);
}

fn freecam_direction(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(sy * cp, sp, -cy * cp).normalize_or(Vec3::NEG_Z)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn collect_snapshot(
    run_id: &str,
    scenario: AuditScenario,
    frame_index: u32,
    runtime: &MatchRuntime,
    tp: &TeleportState,
    keys: &KeystoneState,
    items: &ItemsState,
    guardian: &Guardian,
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
    snapshot.tac_map = Some(collect_tac_map(runtime, tp, keys, tac_visuals));
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
    runtime: &MatchRuntime,
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
        let nav = teleport::Nav {
            connections: dest.conns.clone(),
            connection_slots: dest.connection_slots.clone(),
            hallway_entry_room_slot: dest.hallway_entry_room_slot,
            hallway_exit_room_slot: dest.hallway_exit_room_slot,
            target_room: dest.target,
            seed: 0,
            version: 0,
            exit_locked: !keys.gate_open(),
            exit_room: keys.exit_room,
            pins: Vec::new(),
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
    runtime: &MatchRuntime,
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

const ATLAS_Y: f32 = 22.0;
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

fn atlas_room_center(room: RoomId) -> Vec2 {
    let grid = tacmap::grid_pos(room);
    Vec2::new(grid.x - 1.0, grid.y - 1.0)
}

fn atlas_layout(
    runtime: &MatchRuntime,
    keys: &KeystoneState,
    items: &ItemsState,
) -> Vec<AtlasEntry> {
    let game = runtime.live.host_match();
    let mut rooms = Vec::new();
    let mut hallways = Vec::new();

    for id in 0..9u32 {
        let room = RoomId(id);
        let nav = crate::screens::match_runtime::nav_for_place(
            crate::flow::MATCH_SEED,
            game,
            keys,
            items,
            Place::Room(room),
        );
        let geom = teleport::geom_for(Place::Room(room), &nav);
        rooms.push(AtlasEntry {
            subject: format!("atlas room {}", room.0),
            center: atlas_room_center(room),
            half: geom.half,
            kind: AtlasKind::Room,
        });
    }

    for route in &game.rendered {
        let (from, to) = route.rooms;
        let variation =
            crate::hallway::variation_for(from, to, crate::flow::MATCH_SEED, game.reroute_commits);
        let place = Place::Hallway {
            from,
            to,
            variation,
        };
        let nav = crate::screens::match_runtime::nav_for_place(
            crate::flow::MATCH_SEED,
            game,
            keys,
            items,
            place,
        );
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
    let room_right = room_step + room_max_half.x;
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

fn spawn_footprint_atlas(
    commands: &mut Commands,
    runtime: &MatchRuntime,
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
            DespawnOnExit(GameState::Match),
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

fn preview_aabb(
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
    runtime: &MatchRuntime,
    tp: &TeleportState,
    keys: &KeystoneState,
    items: &ItemsState,
    visuals: &Query<ThresholdVisualQueryData>,
) -> Vec<ThresholdSnapshot> {
    let game = runtime.live.host_match();
    let nav = crate::screens::match_runtime::nav_for_place(
        crate::flow::MATCH_SEED,
        game,
        keys,
        items,
        tp.place,
    );
    tp.geom
        .gaps
        .iter()
        .filter(|gap| gap.kind != GapKind::OneWayEntry)
        .map(|gap| {
            let (a, b) = match tp.place {
                Place::Room(room) => (room, gap.target),
                Place::Hallway { from, to, .. } => (from, to),
            };
            let tethered = nav.is_tethered(a, b);
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
    runtime: &MatchRuntime,
    tp: &TeleportState,
    keys: &KeystoneState,
    visuals: &Query<&DiagnosticTacMapVisual, With<TacMapElement>>,
) -> TacMapSnapshot {
    let model = tacmap::build_map(&runtime.live.host_match().competitive, keys, tp.place);
    let expected_routes = tacmap::spine().len().saturating_sub(1);
    let expected_rooms = 9;
    let expected_keystones = model.keystones.len();
    let expected_rivals = model.rivals.len();
    let expected_elements =
        expected_routes + expected_rooms + 1 + expected_keystones + expected_rivals + 1;

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
        _ => Color::srgb(0.45, 0.62, 0.78),
    }
}

fn place_label(place: Place) -> String {
    match place {
        Place::Room(room) => format!("room {}", room.0),
        Place::Hallway {
            from,
            to,
            variation,
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

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("observed-vis-audit-{millis}")
}

fn write_manifest(dir: &Path, run: &DiagnosticRun) {
    let path = dir.join("manifest.json");
    if let Ok(json) = serde_json::to_string_pretty(run) {
        let _ = fs::write(path, json);
    }
}

fn write_snapshot(path: &Path, snapshot: &DiagnosticSnapshot) {
    if let Ok(json) = serde_json::to_string_pretty(snapshot) {
        let _ = fs::write(path, json);
    }
}

fn finish_audit(dir: &Path, run: &DiagnosticRun) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_label_uses_stable_domain_ids() {
        let link = ThresholdLink {
            room: teleport::RoomThreshold {
                room: RoomId(3),
                slot: teleport::ThresholdSlotId(1),
            },
            hall: teleport::HallThreshold {
                hall: teleport::HallId::new(RoomId(3), RoomId(4)),
                side: RoomId(3),
                slot: teleport::ThresholdSlotId(0),
            },
            local_side: teleport::ThresholdLocalSide::Room,
        };
        assert_eq!(threshold_label(&link), "R3:S1 -> H3-4:3:S0");
    }

    #[test]
    fn scenario_parse_defaults_to_all_for_unknown_values() {
        assert_eq!(
            AuditScenario::parse("thresholds"),
            vec![AuditScenario::Thresholds]
        );
        assert_eq!(
            AuditScenario::parse("topdown"),
            vec![AuditScenario::FootprintAtlas]
        );
        assert_eq!(AuditScenario::parse("all").len(), AuditScenario::ALL.len());
    }

    #[test]
    fn debug_room_parser_accepts_aliases_and_room_ids() {
        assert_eq!(parse_debug_room("guardian"), Some(RoomId(6)));
        assert_eq!(parse_debug_room("tether"), Some(RoomId(5)));
        assert_eq!(parse_debug_room("r4"), Some(RoomId(4)));
        assert_eq!(parse_debug_room("room8"), Some(RoomId(8)));
        assert_eq!(parse_debug_room("room9"), None);
    }

    #[test]
    fn debug_coercion_reads_tethers_guardian_and_player_room() {
        let coercion = DebugMatchCoercion::from_values(
            Some("4, r0, room8, nope"),
            Some("guardian"),
            Some("tether"),
        )
        .expect("coercion should be enabled");
        assert_eq!(coercion.tether_rooms, vec![RoomId(0), RoomId(4), RoomId(8)]);
        assert_eq!(coercion.guardian_room, Some(RoomId(6)));
        assert_eq!(coercion.player_room, Some(RoomId(5)));
        assert!(!coercion.applied);

        let all = parse_debug_room_list("all");
        assert_eq!(all.len(), 9);
        assert_eq!(all[0], RoomId(0));
        assert_eq!(all[8], RoomId(8));
    }
}
