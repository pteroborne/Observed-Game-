use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use glam::Vec2;
use glam::Vec3;
use observed_core::{EquipmentId, PlayerId, TeamId};
use observed_facility::full_wfc::{
    CellCoord, FullWfcConfig, FullWfcError, FullWfcWorld, ObservationFrame, RelayoutCandidate,
    RelayoutWork,
};
use observed_facility::map_spec::RoomRole;
use observed_traversal::{FpsBody, FpsConfig, rapier_controller::RapierTraversalScene};
use player_input::PlayerIntent;

use super::equipment::{DeployableKind, EquipmentState};
use super::geometry::{FullWfcGeometrySnapshot, cell_origin};
use super::guardian::GuardianState;
use super::knowledge::TeamMapKnowledge;

mod movement;
mod mutation;
mod objectives;
mod snapshot;

const CLIMB_SPEED: f32 = 2.5;
const FIXED_DT: f32 = 1.0 / 60.0;
const MUTATION_WARNING_TICKS: u64 = 180;
const CALM_MUTATION_TICKS: u64 = 1_800;
const PRESSURED_MUTATION_TICKS: u64 = 600;
const FIRST_ESCAPE_COUNTDOWN_TICKS: u64 = 1_800;
const INTERACTION_RADIUS: f32 = 1.5;
pub const FULL_WFC_INPUT_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActionButtons {
    pub interact: bool,
    pub deploy_anchor: bool,
    pub deploy_pad: bool,
    pub recover: bool,
    pub activate_pad: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerCommand {
    pub intent: PlayerIntent,
    pub actions: ActionButtons,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputFrame {
    pub version: u16,
    pub tick: u64,
    pub commands: BTreeMap<PlayerId, PlayerCommand>,
}

impl Default for InputFrame {
    fn default() -> Self {
        Self {
            version: FULL_WFC_INPUT_VERSION,
            tick: 0,
            commands: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GameplayEventKind {
    MutationWarning,
    MutationCommitted,
    MutationCancelled,
    AnchorDeployed,
    AnchorRecovered,
    PadDeployed,
    PadRecovered,
    PadUsed,
    KeystoneCollected,
    DualStationProgress,
    DualStationCompleted,
    MonitorSurveyed,
    GuardianCatch,
    GuardianRedirected,
    TeamEscaped,
    MatchFinished,
}

impl GameplayEventKind {
    pub const ALL: [Self; 16] = [
        Self::MutationWarning,
        Self::MutationCommitted,
        Self::MutationCancelled,
        Self::AnchorDeployed,
        Self::AnchorRecovered,
        Self::PadDeployed,
        Self::PadRecovered,
        Self::PadUsed,
        Self::KeystoneCollected,
        Self::DualStationProgress,
        Self::DualStationCompleted,
        Self::MonitorSurveyed,
        Self::GuardianCatch,
        Self::GuardianRedirected,
        Self::TeamEscaped,
        Self::MatchFinished,
    ];
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameplayEvent {
    pub tick: u64,
    pub kind: GameplayEventKind,
    pub player: Option<PlayerId>,
    pub team: Option<TeamId>,
    pub source: Option<CellCoord>,
    pub destination: Option<CellCoord>,
}

impl GameplayEvent {
    pub(super) fn new(
        tick: u64,
        kind: GameplayEventKind,
        player: Option<PlayerId>,
        team: Option<TeamId>,
        source: Option<CellCoord>,
        destination: Option<CellCoord>,
    ) -> Self {
        Self {
            tick,
            kind,
            player,
            team,
            source,
            destination,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub id: PlayerId,
    pub team: TeamId,
    pub cell: CellCoord,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub climb_target: Option<CellCoord>,
    pub escaped: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamState {
    pub id: TeamId,
    pub members: [PlayerId; 2],
    pub keystones: u8,
    pub dual_station_ticks: u16,
    pub dual_station_complete: bool,
    pub escaped: bool,
    pub eliminated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchStatus {
    Running,
    Countdown { remaining_ticks: u64 },
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullWfcMatchConfig {
    pub teams: u8,
    pub members_per_team: u8,
    pub keystones_required: u8,
}

impl Default for FullWfcMatchConfig {
    fn default() -> Self {
        Self {
            teams: 4,
            members_per_team: 2,
            keystones_required: 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchError {
    InvalidRoster,
    Facility(FullWfcError),
}

impl From<FullWfcError> for MatchError {
    fn from(value: FullWfcError) -> Self {
        Self::Facility(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchSnapshot {
    pub input_version: u16,
    pub tick: u64,
    pub generation: u32,
    pub players: Vec<PlayerSnapshot>,
    pub teams: Vec<(TeamId, u8, bool, bool, bool)>,
    pub remaining_keystones: Vec<observed_core::RoomId>,
    pub deployed: Vec<(EquipmentId, TeamId, DeployableKind, CellCoord)>,
    pub guardian_cell: CellCoord,
    pub status: MatchStatus,
    pub digest: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerSnapshot {
    pub id: PlayerId,
    pub team: TeamId,
    pub cell: CellCoord,
    pub millimeters: [i32; 3],
    pub escaped: bool,
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct FullWfcMatch {
    pub seed: u64,
    pub tick: u64,
    pub facility: FullWfcWorld,
    pub geometry: FullWfcGeometrySnapshot,
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub teams: BTreeMap<TeamId, TeamState>,
    pub equipment: EquipmentState,
    pub guardian: GuardianState,
    pub map_knowledge: BTreeMap<TeamId, TeamMapKnowledge>,
    pub available_keystones: BTreeSet<observed_core::RoomId>,
    pub status: MatchStatus,
    pub escape_order: Vec<TeamId>,
    pub recent_events: Vec<GameplayEvent>,
    pub observation: ObservationFrame,
    bodies: BTreeMap<PlayerId, FpsBody>,
    physics: RapierTraversalScene,
    traversal_config: FpsConfig,
    surveyed_monitors: BTreeSet<(TeamId, observed_core::RoomId)>,
    pending_relayout: Option<PendingRelayout>,
    next_mutation_tick: u64,
    config: FullWfcMatchConfig,
}

#[derive(Clone, Debug)]
enum PendingRelayout {
    Solving(RelayoutWork),
    Ready(RelayoutCandidate),
}

impl FullWfcMatch {
    pub fn new(seed: u64, config: FullWfcMatchConfig) -> Result<Self, MatchError> {
        if config.teams != 4 || config.members_per_team != 2 || config.keystones_required != 2 {
            return Err(MatchError::InvalidRoster);
        }
        let facility = FullWfcWorld::new(seed, FullWfcConfig::default())?;
        let geometry = FullWfcGeometrySnapshot::project(&facility);
        let physics = geometry.rapier_scene();
        let mut traversal_config = FpsConfig::deliberate_rapier();
        traversal_config.look_step = 1.0;
        let spawn = facility.spawn();
        let spawn_yaw = initial_spawn_yaw(&facility, spawn);
        let mut players = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let mut teams = BTreeMap::new();
        for team_index in 0..config.teams {
            let team = TeamId(team_index);
            let members = [
                PlayerId(u16::from(team_index) * 2),
                PlayerId(u16::from(team_index) * 2 + 1),
            ];
            teams.insert(
                team,
                TeamState {
                    id: team,
                    members,
                    keystones: 0,
                    dual_station_ticks: 0,
                    dual_station_complete: false,
                    escaped: false,
                    eliminated: false,
                },
            );
            for id in members {
                let actor_index = usize::from(id.0);
                let angle = actor_index as f32 * std::f32::consts::TAU / 8.0;
                let position = cell_origin(spawn)
                    + Vec3::new(
                        angle.cos() * 3.0,
                        traversal_config.half_height,
                        angle.sin() * 3.0,
                    );
                players.insert(
                    id,
                    PlayerState {
                        id,
                        team,
                        cell: spawn,
                        position,
                        yaw: spawn_yaw,
                        pitch: 0.0,
                        climb_target: None,
                        escaped: false,
                    },
                );
                bodies.insert(id, FpsBody::spawned(position, spawn_yaw));
            }
        }
        let available_keystones = facility
            .rooms
            .values()
            .filter(|room| room.role == RoomRole::Keystone)
            .map(|room| room.id)
            .collect();
        let equipment = EquipmentState::new(teams.keys().copied());
        let guardian = GuardianState::new(&facility);
        let map_knowledge = teams
            .keys()
            .copied()
            .map(|team| (team, TeamMapKnowledge::default()))
            .collect();
        let mut game = Self {
            seed,
            tick: 0,
            facility,
            geometry,
            players,
            teams,
            equipment,
            guardian,
            map_knowledge,
            available_keystones,
            status: MatchStatus::Running,
            escape_order: Vec::new(),
            recent_events: Vec::new(),
            observation: ObservationFrame::default(),
            bodies,
            physics,
            traversal_config,
            surveyed_monitors: BTreeSet::new(),
            pending_relayout: None,
            next_mutation_tick: CALM_MUTATION_TICKS,
            config,
        };
        game.observation = game.build_observation();
        game.facility.update_observation(game.observation.clone());
        game.update_map_knowledge();
        Ok(game)
    }

    pub fn step(&mut self, frame: &InputFrame) -> &[GameplayEvent] {
        self.recent_events.clear();
        if self.status == MatchStatus::Finished || frame.version != FULL_WFC_INPUT_VERSION {
            return &self.recent_events;
        }
        self.tick = self.tick.wrapping_add(1);
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let command = frame.commands.get(&id).copied().unwrap_or_default();
            self.step_player(id, command);
        }
        self.resolve_dual_stations(frame);
        self.sync_teleports_to_bodies();
        self.observation = self.build_observation();
        self.facility.update_observation(self.observation.clone());
        self.update_map_knowledge();
        self.step_mutation();
        let anchor_cells = self
            .equipment
            .deployed
            .values()
            .filter(|item| item.kind == DeployableKind::Anchor)
            .map(|item| item.cell)
            .collect();
        self.guardian.step(
            self.tick,
            &self.facility,
            &anchor_cells,
            &mut self.players,
            &self.teams,
            &mut self.recent_events,
        );
        self.sync_teleports_to_bodies();
        self.resolve_team_escape();
        self.step_countdown();
        &self.recent_events
    }

    pub fn guardian_pressure(&self, player: PlayerId) -> f32 {
        self.players.get(&player).map_or(0.0, |player| {
            self.guardian.pressure_for(&self.facility, player)
        })
    }

    pub fn team_map(&self, team: TeamId) -> Option<&TeamMapKnowledge> {
        self.map_knowledge.get(&team)
    }

    /// Normalized warned-mutation phase used by presentation for its breathing cue.
    pub fn mutation_warning_progress(&self) -> f32 {
        if self.pending_relayout.is_none() {
            return 0.0;
        }
        let remaining = self.next_mutation_tick.saturating_sub(self.tick);
        (1.0 - remaining as f32 / MUTATION_WARNING_TICKS as f32).clamp(0.0, 1.0)
    }
}

fn initial_spawn_yaw(world: &FullWfcWorld, spawn: CellCoord) -> f32 {
    [
        (observed_facility::full_wfc::ModuleFace::North, 0.0),
        (
            observed_facility::full_wfc::ModuleFace::East,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            observed_facility::full_wfc::ModuleFace::West,
            -std::f32::consts::FRAC_PI_2,
        ),
        (
            observed_facility::full_wfc::ModuleFace::South,
            std::f32::consts::PI,
        ),
    ]
    .into_iter()
    .find_map(|(face, yaw)| world.placements[&spawn].is_open(face).then_some(yaw))
    .unwrap_or(0.0)
}

#[cfg(test)]
mod tests;
