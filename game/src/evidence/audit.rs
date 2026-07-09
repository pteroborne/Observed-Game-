//! Opt-in visual diagnostics for the assembled game.
//!
//! `OBSERVED2_VIS_AUDIT=<dir>` drives the match through a small set of inspection
//! scenarios, captures a screenshot for each one, and writes a JSON snapshot with
//! machine-readable checks. The screenshots remain for human review; the JSON is
//! the agent-readable bridge for visual bugs.

use std::fs;
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use observed_core::RoomId;
use observed_diagnostics::{DiagnosticRun, DiagnosticSnapshotSummary};
use observed_facility::map_spec::RoomRole;

use super::snapshot::{
    ATLAS_Y, collect_snapshot, finish_audit, path_string, run_id, spawn_footprint_atlas,
    write_manifest, write_snapshot,
};
use super::tags::{DiagnosticTacMapVisual, DiagnosticThresholdVisual, freecam_enabled};
use crate::GameState;
use crate::camera;
use crate::guardian::{ActionLog, Guardian, GuardianModel, GuardianState};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::screens::place::{GuardianConsole, TetherCameraMonitor};
use crate::screens::{self};
use crate::sim::director::MatchDirector;
use crate::sim::state::{MapKnowledge, RivalSightings, TeleportState};
use crate::teleport::{self, Place};
use crate::view::components::{
    DroppedItemVisual, GameCam, KeystoneItem, RivalAvatar, TacMapElement, TacMapPanel, TacMapState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuditScenario {
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

    pub(super) fn label(self) -> &'static str {
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
    runtime: Option<ResMut<'w, MatchDirector>>,
    tp: Option<ResMut<'w, TeleportState>>,
    keys: Option<Res<'w, KeystoneState>>,
    items: Option<ResMut<'w, ItemsState>>,
    guardian: Option<ResMut<'w, Guardian>>,
    sightings: Option<Res<'w, RivalSightings>>,
    knowledge: Option<Res<'w, MapKnowledge>>,
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
    runtime: &'a mut MatchDirector,
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
    runtime: Option<ResMut<'w, MatchDirector>>,
    tp: Option<ResMut<'w, TeleportState>>,
    keys: Option<Res<'w, KeystoneState>>,
    items: Option<ResMut<'w, ItemsState>>,
    guardian: Option<ResMut<'w, Guardian>>,
    log: Option<ResMut<'w, ActionLog>>,
}

pub(super) fn configure_audit(app: &mut App) {
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
        "monitor" | "monitors" | "tether" | "tether_camera" | "tether-camera" | "camera"
        | "guardian_camera" | "guardian-camera" | "observation" => {
            default_role_room(RoomRole::Monitor)
        }
        "guardian" | "guardian_control" | "guardian-control" | "control" | "console" => {
            default_role_room(RoomRole::GuardianControl)
        }
        _ => token
            .strip_prefix("room")
            .or_else(|| token.strip_prefix('r'))
            .unwrap_or(&token)
            .parse::<u32>()
            .ok()
            .filter(|room| {
                crate::map_catalog::active_map_spec(crate::flow::MATCH_SEED)
                    .room(RoomId(*room))
                    .is_some()
            })
            .map(RoomId),
    }
}

fn parse_debug_room_list(value: &str) -> Vec<RoomId> {
    if value.trim().eq_ignore_ascii_case("all") {
        return crate::map_catalog::active_map_spec(crate::flow::MATCH_SEED)
            .rooms
            .into_iter()
            .map(|room| room.id)
            .collect();
    }
    let mut rooms: Vec<RoomId> = value
        .split([',', ';', ' '])
        .filter_map(parse_debug_room)
        .collect();
    rooms.sort_unstable_by_key(|room| room.0);
    rooms.dedup();
    rooms
}

fn default_role_room(role: RoomRole) -> Option<RoomId> {
    crate::map_catalog::active_map_spec(crate::flow::MATCH_SEED).role_room(role)
}

fn current_role_room(runtime: &MatchDirector, role: RoomRole) -> RoomId {
    runtime
        .live
        .host_match()
        .competitive
        .map_spec
        .as_ref()
        .and_then(|spec| spec.role_room(role))
        .unwrap_or_else(|| {
            panic!(
                "active map spec is missing a required {role:?} room; \
                 every catalog map must satisfy MapSpec::validate()"
            )
        })
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
        let connections = crate::sim::nav::connections_for(game, room);
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
            if let (
                Some(runtime),
                Some(tp),
                Some(keys),
                Some(items),
                Some(guardian),
                Some(sightings),
                Some(knowledge),
            ) = (
                params.runtime.as_ref(),
                params.tp.as_ref(),
                params.keys.as_ref(),
                params.items.as_ref(),
                params.guardian.as_ref(),
                params.sightings.as_ref(),
                params.knowledge.as_ref(),
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
                    sightings,
                    knowledge,
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
                super::driver::screenshot_to(&mut commands, path_string(&image_path));
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
    runtime.suppress_reroute_feedback();

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
            let monitor_room = current_role_room(runtime, RoomRole::Monitor);
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
                Place::Room(monitor_room),
                monitor_room,
                keys,
                items,
            );
        }
        AuditScenario::GuardianCameraRoom => {
            let monitor_room = current_role_room(runtime, RoomRole::Monitor);
            let game = runtime.live.host_match();
            let spec = game.competitive.map_spec.as_ref();
            guardian.room = spec
                .and_then(|spec| spec.neighbors(monitor_room).into_iter().min_by_key(|r| r.0))
                .unwrap_or(monitor_room);
            guardian.pos = Vec3::new(0.0, 0.76, 0.0);
            crate::screens::match_runtime::debug_place_into(
                tp,
                runtime,
                Place::Room(monitor_room),
                monitor_room,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Spec-driven: `parse_debug_room`'s aliases resolve via the active map spec's
    /// role rooms (`default_role_room`), not a pinned catalog map's literal ids, so
    /// this queries the same active spec the parser itself reads from.
    #[test]
    fn debug_room_parser_accepts_aliases_and_room_ids() {
        let spec = crate::map_catalog::active_map_spec(crate::flow::MATCH_SEED);
        let guardian_room = spec
            .role_room(RoomRole::GuardianControl)
            .expect("active map has a GuardianControl room");
        let monitor_room = spec
            .role_room(RoomRole::Monitor)
            .expect("active map has a Monitor room");

        assert_eq!(parse_debug_room("guardian"), Some(guardian_room));
        assert_eq!(parse_debug_room("guardian_camera"), Some(monitor_room));
        assert_eq!(parse_debug_room("tether"), Some(monitor_room));

        let some_room = spec.rooms.first().expect("active map has rooms").id;
        assert_eq!(
            parse_debug_room(&format!("r{}", some_room.0)),
            Some(some_room)
        );
        assert_eq!(
            parse_debug_room(&format!("room{}", some_room.0)),
            Some(some_room)
        );
        assert_eq!(
            parse_debug_room(&format!("room{}", spec.room_count())),
            None,
            "one past the last valid room index must be rejected"
        );
    }

    #[test]
    fn debug_coercion_reads_tethers_guardian_and_player_room() {
        let spec = crate::map_catalog::active_map_spec(crate::flow::MATCH_SEED);
        let guardian_room = spec
            .role_room(RoomRole::GuardianControl)
            .expect("active map has a GuardianControl room");
        let monitor_room = spec
            .role_room(RoomRole::Monitor)
            .expect("active map has a Monitor room");
        let mut sample_rooms: Vec<RoomId> = spec.rooms.iter().map(|room| room.id).take(3).collect();
        sample_rooms.sort_by_key(|room| room.0);
        let sample_list = sample_rooms
            .iter()
            .map(|room| format!("r{}", room.0))
            .collect::<Vec<_>>()
            .join(", ");

        let coercion = DebugMatchCoercion::from_values(
            Some(&format!("{sample_list}, nope")),
            Some("guardian_control"),
            Some("monitor"),
        )
        .expect("coercion should be enabled");
        assert_eq!(coercion.tether_rooms, sample_rooms);
        assert_eq!(coercion.guardian_room, Some(guardian_room));
        assert_eq!(coercion.player_room, Some(monitor_room));
        assert!(!coercion.applied);

        let all = parse_debug_room_list("all");
        let mut expected_all: Vec<RoomId> = spec.rooms.iter().map(|room| room.id).collect();
        expected_all.sort_by_key(|room| room.0);
        assert_eq!(
            all, expected_all,
            "\"all\" spans every room in the active spec"
        );
    }
}
