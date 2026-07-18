//! Deterministic match layer over the hex facility: players on Rapier bodies,
//! shaft climbing, ramp walking, threshold-keyed door states, and the
//! observation frames that pin them — everything Phase 95's game shell drives.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_authoring::TilePrototype;
use observed_core::{CorridorId, PlayerId, RoomId};
use observed_facility::hex_wfc::{
    HexObservationFrame, HexThresholdKey, HexWfcConfig, HexWfcError, HexWfcWorld,
    blueprint_for_role,
};
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_traversal::{FpsBody, FpsConfig, rapier_controller::RapierTraversalScene};
use player_input::PlayerIntent;

use super::geometry::{HexGeometryError, HexWfcGeometrySnapshot};

mod bot;
mod movement;
mod snapshot;
#[cfg(test)]
mod tests;

pub use snapshot::{HexMatchSnapshot, HexPlayerSnapshot};

pub(super) const FIXED_DT: f32 = 1.0 / 60.0;
/// Shafts keep the climb-style vertical traversal; ramps are plain walking.
pub(super) const CLIMB_SPEED: f32 = 2.5;
pub const HEX_INPUT_VERSION: u16 = 1;

/// One simulation input frame: sanitized intents keyed by stable player ID.
#[derive(Clone, Debug, PartialEq)]
pub struct HexInputFrame {
    pub version: u16,
    pub tick: u64,
    pub commands: BTreeMap<PlayerId, PlayerIntent>,
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
    /// including full 8 m shaft/ramp falls — are survivable-by-design and do
    /// not raise this.
    PlayerRecovered,
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
    pub cell: HexCoord,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Active shaft climb destination (one vertical neighbor at a time).
    pub climb_target: Option<HexCoord>,
    /// Active scripted lateral shaft exit: the lateral neighbor a shaft-transit
    /// glide is currently crossing the landing toward. Ledges and the central
    /// well make naive center-seeking physics tangle, so the lateral step off a
    /// shaft landing is scripted just like the vertical climb.
    pub transit_target: Option<HexCoord>,
    pub escaped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexMatchConfig {
    pub players: u8,
    pub wfc: HexWfcConfig,
}

impl Default for HexMatchConfig {
    fn default() -> Self {
        Self {
            players: 4,
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
    pub tick: u64,
    pub facility: HexWfcWorld,
    pub geometry: HexWfcGeometrySnapshot,
    pub players: BTreeMap<PlayerId, HexPlayerState>,
    pub status: HexMatchStatus,
    pub escape_order: Vec<PlayerId>,
    pub recent_events: Vec<HexMatchEvent>,
    /// The latest simulation-frame observation projection (occupied cells and
    /// the looked-at blueprint thresholds). Relayout directors consume this to
    /// pin geometry exactly as on the square lattice.
    pub observation: HexObservationFrame,
    pub(super) bodies: BTreeMap<PlayerId, FpsBody>,
    pub(super) physics: RapierTraversalScene,
    pub(super) traversal_config: FpsConfig,
    /// Consecutive ticks each non-escaped player's body has failed to make net
    /// progress (wedged against geometry). The objective bot reads this to
    /// trigger a sideways unstick sweep; real net motion or a scripted transit
    /// resets it. Measured against [`Self::progress_anchor`] so a body jittering
    /// in place still counts as stuck.
    pub(super) stuck_ticks: BTreeMap<PlayerId, u16>,
    /// The last position from which a player made clear net progress; the
    /// reference the stuck tracker measures displacement against.
    pub(super) progress_anchor: BTreeMap<PlayerId, Vec3>,
}

impl HexWfcMatch {
    pub fn new(
        seed: u64,
        config: HexMatchConfig,
        prototypes: &[TilePrototype],
    ) -> Result<Self, HexMatchError> {
        if config.players == 0 || config.players > 8 {
            return Err(HexMatchError::InvalidRoster);
        }
        let facility = HexWfcWorld::generate(seed, config.wfc)?;
        let geometry = HexWfcGeometrySnapshot::project(&facility, prototypes)?;
        let physics = geometry.rapier_scene();
        let mut traversal_config = FpsConfig::deliberate_rapier();
        traversal_config.look_step = 1.0;
        let spawn = facility.config.spawn();
        let spawn_yaw = initial_spawn_yaw(&facility);
        let mut players = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        for index in 0..config.players {
            let id = PlayerId(u16::from(index));
            let angle = f32::from(index) * std::f32::consts::TAU / 8.0;
            let position = Vec3::from_array(hex_origin(spawn))
                + Vec3::new(
                    angle.cos() * 2.5,
                    traversal_config.half_height + 0.25,
                    angle.sin() * 2.5,
                );
            players.insert(
                id,
                HexPlayerState {
                    id,
                    cell: spawn,
                    position,
                    yaw: spawn_yaw,
                    pitch: 0.0,
                    climb_target: None,
                    transit_target: None,
                    escaped: false,
                },
            );
            bodies.insert(id, FpsBody::spawned(position, spawn_yaw));
        }
        let mut game = Self {
            seed,
            tick: 0,
            facility,
            geometry,
            players,
            status: HexMatchStatus::Running,
            escape_order: Vec::new(),
            recent_events: Vec::new(),
            observation: HexObservationFrame::default(),
            bodies,
            physics,
            traversal_config,
            stuck_ticks: BTreeMap::new(),
            progress_anchor: BTreeMap::new(),
        };
        game.observation = game.build_observation();
        Ok(game)
    }

    pub fn step(&mut self, frame: &HexInputFrame) -> &[HexMatchEvent] {
        self.recent_events.clear();
        if self.status == HexMatchStatus::Finished || frame.version != HEX_INPUT_VERSION {
            return &self.recent_events;
        }
        self.tick = self.tick.wrapping_add(1);
        self.sync_teleports_to_bodies();
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let intent = frame.commands.get(&id).copied().unwrap_or_default();
            self.move_player(id, intent.sanitized());
        }
        self.update_stuck_ticks();
        self.recover_fallen_bodies();
        self.observation = self.build_observation();
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
        frame
    }

    fn looked_at_threshold(&self, player: &HexPlayerState) -> Option<HexThresholdKey> {
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
            self.escape_order.push(id);
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::PlayerEscaped,
                player: Some(id),
                cell: Some(exit),
            });
        }
        if self.status == HexMatchStatus::Running
            && self.players.values().all(|player| player.escaped)
        {
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
