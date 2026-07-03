//! The running match's simulation-side resources: the live networked match brain, the
//! elimination series, the spectator bot's plan, the player's frame intents, and the
//! teleport place/body state. Presentation systems read these; only the input,
//! controller, and match-pump systems write them. Nothing here references rendering,
//! UI, audio, or asset types.

use bevy::prelude::*;
use observed_core::{RoomId, TeamId};
use observed_match::teamplay::TeamplayMatch;
use observed_progression::session::SessionLabWorld;
use observed_traversal::{FpsArena, FpsBody, FpsConfig};
use player_input::PlayerIntent;

use crate::teleport::{self, Place};

/// Menu-launched bot spectator mode. The bot drives the same first-person body and
/// threshold-crossing systems a player uses; the camera simply follows that body.
#[derive(Resource)]
pub struct SpectatorBot {
    pub seed: u64,
    pub focused_team: TeamId,
    pub focused_member: u8,
    pub teamplay: TeamplayMatch,
    pub last_teamplay_event: String,
    pub teamplay_frame_accum: u8,
    pub route_place: Option<Place>,
    pub route: Vec<Vec2>,
    pub waypoint: usize,
    pub blocked_ticks: u32,
    pub finished: bool,
}

impl Default for SpectatorBot {
    fn default() -> Self {
        Self::for_seed(crate::flow::MATCH_SEED)
    }
}

impl SpectatorBot {
    pub fn for_seed(seed: u64) -> Self {
        Self {
            seed,
            focused_team: crate::flow::LOCAL_TEAM,
            focused_member: 0,
            teamplay: TeamplayMatch::new(seed),
            last_teamplay_event: "Bot co-op team entered the procedural seed.".to_string(),
            teamplay_frame_accum: 0,
            route_place: None,
            route: Vec::new(),
            waypoint: 0,
            blocked_ticks: 0,
            finished: false,
        }
    }

    pub fn clear_route(&mut self) {
        self.route_place = None;
        self.route.clear();
        self.waypoint = 0;
    }
}

/// The player's first-person intent for the current frame, consumed by the
/// fixed-timestep controller.
#[derive(Resource, Default)]
pub struct MatchIntent(pub PlayerIntent);

/// One-frame item actions sampled from hardware input and consumed by the item systems.
/// Kept separate from [`PlayerIntent`] because these are game-local tool actions, not
/// movement/controller intent.
#[derive(Resource, Default)]
pub struct ItemIntent {
    pub(crate) torch_action: bool,
    pub(crate) pad_action: bool,
    pub(crate) activate_pad: bool,
}

#[derive(Resource, Default)]
pub struct MatchPaused(pub bool);

/// Latch resource to prevent infinite teleport pad loops
#[derive(Resource, Default, Debug)]
pub struct LastTeleportPad {
    pub last_used_pos: Option<(Place, Vec2)>,
}

/// The live teleport-place state: which discrete place the player occupies, the
/// controller body + its collision arena for that place, and what the renderer last
/// built (so it rebuilds only on a teleport).
#[derive(Resource)]
pub struct TeleportState {
    pub place: teleport::Place,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub arena: FpsArena,
    /// The current place's footprint + doorway gaps + interior (maze) walls. Cached so
    /// a labyrinth is generated once per teleport, not every fixed step.
    pub geom: teleport::PlaceGeom,
    pub prev_xz: Vec2,
    /// Latched once the body crosses a hallway's exit, until the round commits.
    pub crossed_exit: bool,
    /// The specific exit doorway crossed (held while `crossed_exit` is latched) so the
    /// seamless crossing remap can align the next room to the doorway actually used.
    pub pending_exit: Option<teleport::DoorGap>,
    /// For a room, the room it was entered *from* — its doorway stays an open `Entry`
    /// passage (not a sealed wall) so the way you came in matches the preview and you can
    /// step back out. `None` in a hallway or the start room.
    pub arrived_from: Option<RoomId>,
    /// The **frozen** destination of each passage doorway of the current place, captured the
    /// instant the place was entered (and re-used by both the doorway preview and the
    /// crossing). This realises "observed → frozen": while you can see a neighbour through an
    /// open threshold, what you see is exactly what you walk into, even if the brain rerolls
    /// that edge under you (it only "changes" once you look away and re-enter).
    pub gap_dests: Vec<FrozenDest>,
    /// The place the geometry currently reflects.
    pub rendered: Option<teleport::Place>,
}

/// A doorway's frozen destination: the resolved next [`teleport::Place`] (a hallway carries
/// its variation; a room carries the `conns`/`target` that shape it), snapshotted at
/// place-entry so the preview and the crossing can't diverge.
#[derive(Clone)]
pub struct FrozenDest {
    /// The gap centre (current place's local frame) used to match the crossed doorway.
    pub gap_center: Vec2,
    /// Explicit threshold identity used to match preview/crossing/arrival.
    pub threshold: teleport::ThresholdLink,
    pub place: teleport::Place,
    /// For a room destination, its frozen connection set (shape); empty for a hallway.
    pub conns: Vec<RoomId>,
    /// For a room destination, its frozen room-side threshold slots.
    pub connection_slots: Vec<teleport::RoomConnectionSlot>,
    /// For a room destination, its frozen collapse-sealed room-side threshold slots.
    pub sealed_slots: Vec<teleport::ThresholdSlotId>,
    /// For a hallway destination, the room slot at the entry side.
    pub hallway_entry_room_slot: Option<teleport::ThresholdSlotId>,
    /// For a hallway destination, the room slot at the exit side.
    pub hallway_exit_room_slot: Option<teleport::ThresholdSlotId>,
    /// For a room destination, its frozen spine target (which doorway stays forward).
    pub target: Option<RoomId>,
}

#[derive(Resource)]
pub struct LobbyRuntime {
    /// The formed session is retained for the duration of the lobby/match so the
    /// matchmaking state stays live; the screen renders from it at spawn time.
    #[allow(dead_code)]
    pub world: SessionLabWorld,
}
