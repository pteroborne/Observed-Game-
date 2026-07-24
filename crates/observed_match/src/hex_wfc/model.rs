//! Deterministic match layer over the hex facility: players on Rapier bodies,
//! physical stair/ramp walking, threshold-keyed door states, and the
//! observation frames that pin them — everything Phase 95's game shell drives.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_authoring::{RoomPrototype, TilePrototype};
use observed_core::{CorridorId, PlayerId, RoomId, TeamId};
use observed_facility::hex_wfc::{
    HexObservationFrame, HexRelayoutCandidate, HexRelayoutDelta, HexRelayoutWork, HexThresholdKey,
    HexWfcConfig, HexWfcError, HexWfcWorld, blueprint_for_role,
};
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_traversal::{FpsBody, FpsConfig, rapier_controller::RapierTraversalScene};
use player_input::PlayerIntent;

use super::geometry::{HexGeometryError, HexWfcGeometrySnapshot};

mod bot;
mod equipment;
mod guardian;
mod knowledge;
mod movement;
mod mutation;
mod snapshot;
#[cfg(test)]
mod tests;

pub use equipment::{HexDeployedLantern, HexLanternCache, HexLanternState};
pub use guardian::{HexGuardianState, HexGuardianStatus};
pub use knowledge::{HexMapCellKnowledge, HexMapDiscovery, HexPlayerMapKnowledge};
pub use snapshot::{HexMatchSnapshot, HexPlayerSnapshot};

pub(super) const FIXED_DT: f32 = 1.0 / 60.0;
/// Height of the authored floor surface above a cell's level origin.
///
/// Tile intake is standardized at 16 TrenchBroom units per metre and every
/// production cell uses an 8-unit floor slab. Spawn, recovery, and scripted
/// traversal must all agree on this surface or a capsule starts half embedded
/// in collision.
pub(super) const FLOOR_SLAB_TOP: f32 = 0.5;
pub const HEX_INPUT_VERSION: u16 = 4;

/// Ticks of forewarning between a [`HexMatchEventKind::MutationWarning`] and the
/// deterministic commit tick, so observation has a window to pin structure. Mirrors
/// the square lattice's `MUTATION_WARNING_TICKS`.
pub(super) const MUTATION_WARNING_TICKS: u64 = 120;
/// Deterministic local-breath interval range (8--12 seconds at 60 Hz).
pub(super) const MIN_MUTATION_TICKS: u64 = 480;
pub(super) const MAX_MUTATION_TICKS: u64 = 720;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HexActionButtons {
    pub interact: bool,
    pub deploy_lantern: bool,
    pub recover_lantern: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HexPlayerCommand {
    pub intent: PlayerIntent,
    pub actions: HexActionButtons,
}

/// One simulation input frame: sanitized commands keyed by stable player ID.
#[derive(Clone, Debug, PartialEq)]
pub struct HexInputFrame {
    pub version: u16,
    pub tick: u64,
    pub commands: BTreeMap<PlayerId, HexPlayerCommand>,
}

impl Default for HexInputFrame {
    fn default() -> Self {
        Self {
            version: HEX_INPUT_VERSION,
            tick: 0,
            commands: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexMatchStatus {
    Running,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexMatchEventKind {
    PlayerEscaped,
    /// A body left the world volume (NaN or below the arena floor) and was
    /// deterministically reset to its logical cell anchor. Ordinary drops —
    /// including full 8 m stairwell/ramp falls — are survivable-by-design and do
    /// not raise this.
    PlayerRecovered,
    /// A deterministic relayout warning fired: unobserved structure has begun to
    /// breathe and will re-collapse at the scheduled commit tick unless pinned.
    MutationWarning,
    /// A relayout re-collapse committed: `facility.generation` incremented and
    /// unobserved cells mutated. Presentation re-snapshots geometry on this.
    MutationCommitted,
    /// A pending relayout was rejected — observation held the structure, the
    /// re-collapse failed, or the candidate no longer projected.
    MutationCancelled,
    LanternCacheCollected,
    AnchorDeployed,
    AnchorRecovered,
    GuardianCatch,
    MatchFinished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexMatchEvent {
    pub tick: u64,
    pub kind: HexMatchEventKind,
    pub player: Option<PlayerId>,
    pub cell: Option<HexCoord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexPlayerState {
    pub id: PlayerId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub escaped: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexTeamState {
    pub id: TeamId,
    pub members: Vec<PlayerId>,
    pub escaped: bool,
    pub finish_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexMatchConfig {
    pub teams: u8,
    pub members_per_team: u8,
    pub wfc: HexWfcConfig,
}

impl Default for HexMatchConfig {
    fn default() -> Self {
        Self {
            teams: 2,
            members_per_team: 2,
            wfc: HexWfcConfig {
                levels: 4,
                ..HexWfcConfig::default()
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexMatchError {
    InvalidRoster,
    BlockedSpawn(PlayerId),
    Facility(HexWfcError),
    Geometry(HexGeometryError),
}

impl From<HexWfcError> for HexMatchError {
    fn from(value: HexWfcError) -> Self {
        Self::Facility(value)
    }
}

impl From<HexGeometryError> for HexMatchError {
    fn from(value: HexGeometryError) -> Self {
        Self::Geometry(value)
    }
}

/// The live state of one named blueprint door port. `key` is the stable
/// threshold identity that observation frames pin; `open` is driven entirely
/// by the facility's solved ports (boundary-stamped ports stay sealed).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexDoorState {
    pub key: HexThresholdKey,
    pub room: RoomId,
    pub room_cell: HexCoord,
    pub face: HexFace,
    /// The corridor attached through this port, when open and hall-adjacent.
    pub corridor: Option<CorridorId>,
    pub open: bool,
}

/// Deterministic fixed-step hex match: same inputs, same snapshots, headless
/// or interactive.
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct HexWfcMatch {
    pub seed: u64,
    /// Canonical authored simulation-content identity supplied by the host
    /// before tick zero. Snapshots/replays carry it so peers cannot silently
    /// simulate different collision catalogues with identical input frames.
    pub simulation_content_hash: [u8; 32],
    pub tick: u64,
    pub facility: HexWfcWorld,
    pub geometry: HexWfcGeometrySnapshot,
    pub players: BTreeMap<PlayerId, HexPlayerState>,
    pub teams: BTreeMap<TeamId, HexTeamState>,
    pub lanterns: HexLanternState,
    pub guardian: HexGuardianState,
    pub map_knowledge: BTreeMap<TeamId, HexPlayerMapKnowledge>,
    pub status: HexMatchStatus,
    pub escape_order: Vec<TeamId>,
    pub player_escape_order: Vec<PlayerId>,
    pub recent_events: Vec<HexMatchEvent>,
    /// Accepted bounded relayout from the current step, if any. Presentation
    /// and map-knowledge consumers can update exact cells without inferring a
    /// global rebuild from `facility.generation`.
    pub last_relayout_delta: Option<HexRelayoutDelta>,
    /// The latest simulation-frame observation projection (occupied cells and
    /// the looked-at blueprint thresholds). Relayout directors consume this to
    /// pin geometry exactly as on the square lattice.
    pub observation: HexObservationFrame,
    pub(super) bodies: BTreeMap<PlayerId, FpsBody>,
    pub(super) physics: RapierTraversalScene,
    pub(super) traversal_config: FpsConfig,
    /// Consecutive ticks each non-escaped player's body has failed to make net
    /// progress (wedged against geometry). The objective bot reads this to
    /// trigger a sideways unstick sweep; real net motion resets it. Measured
    /// against [`Self::progress_anchor`] so a body jittering
    /// in place still counts as stuck.
    pub(super) stuck_ticks: BTreeMap<PlayerId, u16>,
    /// The last position from which a player made clear net progress; the
    /// reference the stuck tracker measures displacement against.
    pub(super) progress_anchor: BTreeMap<PlayerId, Vec3>,
    /// Authored tile corpus retained so a committed relayout can re-project the
    /// collision geometry mid-match. Same prototypes the initial solve used.
    pub(super) prototypes: Vec<TilePrototype>,
    /// Validated whole-room modules, selected before the per-cell fallback kit.
    pub(super) room_prototypes: Vec<RoomPrototype>,
    /// In-flight observation-safe relayout, if any. `None` between cycles.
    pub(super) pending_relayout: Option<PendingRelayout>,
    /// The deterministic tick at which the next relayout commits. The paired
    /// warning fires [`MUTATION_WARNING_TICKS`] earlier.
    pub(super) next_mutation_tick: u64,
}

/// The two phases of one relayout cycle: an incremental solve, then a solved
/// candidate awaiting its commit tick. Mirrors the square `PendingRelayout`.
#[derive(Clone, Debug)]
pub(super) enum PendingRelayout {
    Solving(HexRelayoutWork),
    Ready(HexRelayoutCandidate),
}

impl HexWfcMatch {
    pub fn new(
        seed: u64,
        config: HexMatchConfig,
        prototypes: &[TilePrototype],
    ) -> Result<Self, HexMatchError> {
        Self::new_with_rooms(seed, config, prototypes, &[])
    }

    pub fn new_with_rooms(
        seed: u64,
        config: HexMatchConfig,
        prototypes: &[TilePrototype],
        room_prototypes: &[RoomPrototype],
    ) -> Result<Self, HexMatchError> {
        let player_count = config.teams.saturating_mul(config.members_per_team);
        if config.teams == 0
            || config.members_per_team == 0
            || player_count == 0
            || player_count > 8
        {
            return Err(HexMatchError::InvalidRoster);
        }
        let facility = HexWfcWorld::generate(seed, config.wfc)?;
        let geometry =
            HexWfcGeometrySnapshot::project_with_rooms(&facility, prototypes, room_prototypes)?;
        let physics = geometry.rapier_scene();
        let mut traversal_config = FpsConfig::deliberate_rapier();
        traversal_config.look_step = 1.0;
        let spawn = facility.config.spawn();
        let spawn_yaw = initial_spawn_yaw(&facility);
        let spawn_origin = Vec3::from_array(hex_origin(spawn));
        let mut players = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let mut teams = BTreeMap::new();
        for team_index in 0..config.teams {
            let team = TeamId(team_index);
            let first = u16::from(team_index) * u16::from(config.members_per_team);
            let members = (0..config.members_per_team)
                .map(|member| PlayerId(first + u16::from(member)))
                .collect();
            teams.insert(
                team,
                HexTeamState {
                    id: team,
                    members,
                    escaped: false,
                    finish_tick: None,
                },
            );
        }
        for index in 0..player_count {
            let id = PlayerId(u16::from(index));
            let team = TeamId(index / config.members_per_team);
            let position = clear_spawn_position(&physics, spawn_origin, &traversal_config, index)
                .ok_or(HexMatchError::BlockedSpawn(id))?;
            players.insert(
                id,
                HexPlayerState {
                    id,
                    team,
                    cell: spawn,
                    position,
                    yaw: spawn_yaw,
                    pitch: 0.0,
                    escaped: false,
                },
            );
            bodies.insert(id, FpsBody::spawned(position, spawn_yaw));
        }
        let lanterns = HexLanternState::new(players.keys().copied(), &facility);
        let guardian = HexGuardianState::new(&facility);
        let map_knowledge = teams
            .keys()
            .copied()
            .map(|team| (team, HexPlayerMapKnowledge::default()))
            .collect();
        let mut game = Self {
            seed,
            simulation_content_hash: [0; 32],
            tick: 0,
            facility,
            geometry,
            players,
            teams,
            lanterns,
            guardian,
            map_knowledge,
            status: HexMatchStatus::Running,
            escape_order: Vec::new(),
            player_escape_order: Vec::new(),
            recent_events: Vec::new(),
            last_relayout_delta: None,
            observation: HexObservationFrame::default(),
            bodies,
            physics,
            traversal_config,
            stuck_ticks: BTreeMap::new(),
            progress_anchor: BTreeMap::new(),
            prototypes: prototypes.to_vec(),
            room_prototypes: room_prototypes.to_vec(),
            pending_relayout: None,
            next_mutation_tick: mutation::scheduled_mutation_tick(seed, 0),
        };
        game.observation = game.build_observation();
        game.update_map_knowledge();
        Ok(game)
    }

    /// Bind the compiled authoring catalogue before the first input frame.
    /// Keeping filesystem discovery in the host preserves the pure simulation
    /// boundary while making content mismatch part of authoritative state.
    pub fn bind_simulation_content_hash(&mut self, hash: [u8; 32]) {
        assert_eq!(
            self.tick, 0,
            "simulation content must be bound before tick zero"
        );
        self.simulation_content_hash = hash;
    }

    pub fn step(&mut self, frame: &HexInputFrame) -> &[HexMatchEvent] {
        self.recent_events.clear();
        self.last_relayout_delta = None;
        if self.status == HexMatchStatus::Finished || frame.version != HEX_INPUT_VERSION {
            return &self.recent_events;
        }
        self.tick = self.tick.wrapping_add(1);
        self.sync_teleports_to_bodies();
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let command = frame.commands.get(&id).copied().unwrap_or_default();
            self.move_player(id, command.intent.sanitized());
            self.step_lantern_actions(id, command.actions);
        }
        self.update_stuck_ticks();
        self.recover_fallen_bodies();
        self.observation = self.build_observation();
        self.step_mutation();
        self.guardian.step(
            self.tick,
            &self.facility,
            &self.lanterns,
            &mut self.players,
            &mut self.recent_events,
        );
        self.sync_teleports_to_bodies();
        self.update_map_knowledge();
        self.resolve_escapes();
        &self.recent_events
    }

    /// The observation-frame projection of the current tick: occupied and
    /// visible cells plus the stable `HexThresholdKey`s of blueprint ports the
    /// occupants are looking at. Feeding this to the facility relayout API
    /// pins those thresholds exactly as `ThresholdKey { room, face }` does on
    /// the square lattice.
    #[must_use]
    pub fn build_observation(&self) -> HexObservationFrame {
        let mut frame = HexObservationFrame::default();
        for player in self.players.values().filter(|player| !player.escaped) {
            frame.visible_cells.insert(player.cell);
            frame.occupied_cells.insert(player.id, player.cell);
            if let Some(key) = self.looked_at_threshold(player) {
                frame.visible_thresholds.insert(key);
            }
        }
        self.lanterns.apply_mutation_pins(&mut frame);
        frame.landmark_cells.insert(self.guardian.cell);
        frame
    }

    pub(super) fn looked_at_threshold(&self, player: &HexPlayerState) -> Option<HexThresholdKey> {
        let blueprint = self
            .facility
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&player.cell))?;
        let face = movement::look_face(player.yaw, player.pitch);
        if !face.is_lateral() {
            return None;
        }
        let definition = blueprint_for_role(blueprint.role);
        let index = blueprint
            .cells
            .iter()
            .position(|&cell| cell == player.cell)?;
        let offset = definition.cells[index];
        let (name, _, _) = definition
            .named_ports
            .iter()
            .find(|&&(_, port_offset, port_face)| port_offset == offset && port_face == face)?;
        Some(HexThresholdKey {
            room_generation_key: blueprint.generation_key(),
            port: name,
        })
    }

    /// Door states for every named blueprint port, keyed by the stable
    /// threshold identity. Presentation drives door visuals from this list;
    /// simulation traversability is the underlying solved port itself.
    #[must_use]
    pub fn door_states(&self) -> Vec<HexDoorState> {
        let rooms: BTreeMap<u64, RoomId> = self
            .facility
            .rooms()
            .into_iter()
            .map(|room| (room.generation_key, room.id))
            .collect();
        let attachments = self.facility.threshold_attachments();
        let mut states = Vec::new();
        for blueprint in &self.facility.blueprints {
            let definition = blueprint_for_role(blueprint.role);
            let generation_key = blueprint.generation_key();
            for &(name, offset, face) in &definition.named_ports {
                let Some(index) = definition.cells.iter().position(|&cell| cell == offset) else {
                    continue;
                };
                let room_cell = blueprint.cells[index];
                let open = self.facility.placements[&room_cell].is_open(face);
                let corridor = attachments
                    .iter()
                    .find(|attachment| attachment.room_cell == room_cell && attachment.face == face)
                    .map(|attachment| attachment.corridor);
                states.push(HexDoorState {
                    key: HexThresholdKey {
                        room_generation_key: generation_key,
                        port: name,
                    },
                    room: rooms[&generation_key],
                    room_cell,
                    face,
                    corridor,
                    open,
                });
            }
        }
        states
    }

    #[must_use]
    pub fn lantern_proximity(&self, player: PlayerId) -> f32 {
        if self.lanterns.inventory(player) == 0 {
            return 0.0;
        }
        let Some(player) = self.players.get(&player) else {
            return 0.0;
        };
        let exit = self.facility.config.exit();
        let baseline = self
            .facility
            .route_between_cells(self.facility.config.spawn(), exit)
            .map_or(1, |route| route.cost_millis.max(1));
        let remaining = self
            .facility
            .route_between_cells(player.cell, exit)
            .map_or(baseline, |route| route.cost_millis);
        (1.0 - remaining as f32 / baseline as f32).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn guardian_pressure(&self, player: PlayerId) -> f32 {
        self.players.get(&player).map_or(0.0, |player| {
            self.guardian.pressure_for(&self.facility, player)
        })
    }

    #[must_use]
    pub fn player_map(&self, player: PlayerId) -> Option<&HexPlayerMapKnowledge> {
        self.players
            .get(&player)
            .and_then(|player| self.map_knowledge.get(&player.team))
    }

    #[must_use]
    pub fn team_map(&self, team: TeamId) -> Option<&HexPlayerMapKnowledge> {
        self.map_knowledge.get(&team)
    }

    fn update_map_knowledge(&mut self) {
        for (&team, state) in &self.teams {
            let players = state.members.iter().filter_map(|id| self.players.get(id));
            self.map_knowledge.entry(team).or_default().observe_team(
                team,
                &self.facility,
                players,
                &self.lanterns,
            );
        }
    }

    fn resolve_escapes(&mut self) {
        let exit = self.facility.config.exit();
        let mut escaped = Vec::new();
        for player in self.players.values() {
            if !player.escaped && player.cell == exit {
                escaped.push(player.id);
            }
        }
        for id in escaped {
            self.players.get_mut(&id).expect("player").escaped = true;
            self.player_escape_order.push(id);
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::PlayerEscaped,
                player: Some(id),
                cell: Some(exit),
            });
        }
        let finished_teams = self
            .teams
            .iter()
            .filter_map(|(&team, state)| {
                (!state.escaped
                    && state
                        .members
                        .iter()
                        .all(|player| self.players[player].escaped))
                .then_some(team)
            })
            .collect::<Vec<_>>();
        for team in finished_teams {
            let state = self.teams.get_mut(&team).expect("team");
            state.escaped = true;
            state.finish_tick = Some(self.tick);
            self.escape_order.push(team);
        }
        if self.status == HexMatchStatus::Running && self.teams.values().all(|team| team.escaped) {
            self.status = HexMatchStatus::Finished;
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::MatchFinished,
                player: None,
                cell: None,
            });
        }
    }
}

/// Pick a deterministic, collision-proven point on the spawn cell's walkable
/// floor. Authored rooms are allowed to occupy their centre with machinery or
/// landmarks, so a fixed-radius ring is not a valid spawn contract.
fn clear_spawn_position(
    physics: &RapierTraversalScene,
    origin: Vec3,
    config: &FpsConfig,
    player_index: u8,
) -> Option<Vec3> {
    const RADII: [f32; 4] = [4.0, 5.0, 3.0, 1.5];
    const ANGLE_STEPS: u8 = 16;
    const CLEARANCE: f32 = 0.02;

    let preferred = f32::from(player_index) * std::f32::consts::TAU / 8.0;
    let y = origin.y + FLOOR_SLAB_TOP + config.half_height + CLEARANCE;
    for radius in RADII {
        for step in 0..ANGLE_STEPS {
            let angle =
                preferred + f32::from(step) * std::f32::consts::TAU / f32::from(ANGLE_STEPS);
            let candidate = Vec3::new(
                origin.x + angle.cos() * radius,
                y,
                origin.z + angle.sin() * radius,
            );
            if physics.capsule_is_clear(candidate, config.radius, config.half_height) {
                return Some(candidate);
            }
        }
    }
    None
}

fn initial_spawn_yaw(world: &HexWfcWorld) -> f32 {
    let spawn = world.config.spawn();
    world
        .route_between(spawn, world.config.exit())
        .and_then(|route| route.get(1).copied())
        .map(|next| {
            let delta = Vec3::from_array(hex_origin(next)) - Vec3::from_array(hex_origin(spawn));
            delta.x.atan2(-delta.z)
        })
        .unwrap_or(0.0)
}
