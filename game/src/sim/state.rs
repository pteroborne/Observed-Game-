//! The running match's simulation-side resources: the live networked match brain, the
//! elimination series, the spectator bot's plan, the player's frame intents, and the
//! teleport place/body state. Presentation systems read these; only the input,
//! controller, and match-pump systems write them. Nothing here references rendering,
//! UI, audio, or asset types.

use std::collections::BTreeMap;

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

/// How a rival trace was witnessed. `Seen` (a rival clump physically sharing your room
/// or standing in a neighbour you can see through an open threshold) outranks `Heard`
/// (sound bleed only). `AnchorSpotted` (a rival's anchor torch witnessed through a
/// threshold or preview) is kept over a mere `Seen` at the same room when both would
/// apply — an anchor is a durable claim, so once you've clocked it, a passer-through
/// `Seen` at the same room should not downgrade what you know about that room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SightingKind {
    Heard,
    Seen,
    AnchorSpotted,
}

/// One recorded sighting of a rival team's trace in a room: what kind of evidence it
/// was, and the `reroute_commits` value at the moment it was recorded (staleness is
/// `game.reroute_commits - commits_seen`, computed by the reader, not stored).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sighting {
    pub kind: SightingKind,
    pub commits_seen: u32,
}

/// The team-local **sighting ledger**: fog of war over the live rival truth. The tac-map
/// projects rival pips from this, not from live team-room positions — a rival only
/// appears on your map where and when *you* actually witnessed a trace of them, and that
/// sighting goes stale (fades) rather than tracking them live. Keyed by rival team index
/// (`0..TEAM_COUNT`, excluding the local team) then by the room the trace was witnessed
/// in. Your own team and the facility structure stay live elsewhere; this ledger only
/// demotes *rival* presence.
#[derive(Resource, Default, Debug)]
pub struct RivalSightings {
    pub teams: BTreeMap<u8, BTreeMap<RoomId, Sighting>>,
}

impl RivalSightings {
    /// Record a sighting of `team` at `room`, replacing any existing entry for that
    /// room unless the existing entry is a higher-information kind recorded at least as
    /// recently is retained (an `AnchorSpotted` is never downgraded to a same-room
    /// `Seen`/`Heard`; a `Seen` is never downgraded to a `Heard`). A strictly newer
    /// sighting of the *same or higher* kind always replaces the old one so staleness
    /// resets.
    pub fn record(&mut self, team: TeamId, room: RoomId, kind: SightingKind, commits: u32) {
        let rooms = self.teams.entry(team.0).or_default();
        match rooms.get(&room) {
            Some(existing) if existing.kind > kind => {
                // A higher-information kind already on record for this room stays,
                // but only refresh its recency if this new (lower-kind) witnessing is
                // at least as recent — otherwise stale data would look fresher.
                if commits > existing.commits_seen {
                    rooms.insert(
                        room,
                        Sighting {
                            kind: existing.kind,
                            commits_seen: commits,
                        },
                    );
                }
            }
            _ => {
                rooms.insert(
                    room,
                    Sighting {
                        kind,
                        commits_seen: commits,
                    },
                );
            }
        }
    }

    /// The last-witnessed sighting of `team` in `room`, if any.
    pub fn get(&self, team: TeamId, room: RoomId) -> Option<Sighting> {
        self.teams.get(&team.0)?.get(&room).copied()
    }
}

#[cfg(test)]
mod sighting_tests {
    use super::*;

    #[test]
    fn a_newer_sighting_of_the_same_kind_replaces_the_older_one() {
        let mut ledger = RivalSightings::default();
        ledger.record(TeamId(1), RoomId(2), SightingKind::Seen, 0);
        ledger.record(TeamId(1), RoomId(2), SightingKind::Seen, 3);
        assert_eq!(
            ledger.get(TeamId(1), RoomId(2)),
            Some(Sighting {
                kind: SightingKind::Seen,
                commits_seen: 3
            })
        );
    }

    #[test]
    fn seen_outranks_heard_and_is_not_downgraded() {
        let mut ledger = RivalSightings::default();
        ledger.record(TeamId(1), RoomId(2), SightingKind::Seen, 1);
        ledger.record(TeamId(1), RoomId(2), SightingKind::Heard, 2);
        assert_eq!(
            ledger.get(TeamId(1), RoomId(2)).unwrap().kind,
            SightingKind::Seen,
            "a later Heard must not downgrade an existing Seen"
        );
    }

    #[test]
    fn anchor_spotted_outranks_seen_and_is_not_downgraded() {
        let mut ledger = RivalSightings::default();
        ledger.record(TeamId(1), RoomId(2), SightingKind::AnchorSpotted, 1);
        ledger.record(TeamId(1), RoomId(2), SightingKind::Seen, 2);
        assert_eq!(
            ledger.get(TeamId(1), RoomId(2)).unwrap().kind,
            SightingKind::AnchorSpotted,
            "a later Seen must not downgrade an existing AnchorSpotted"
        );
    }

    #[test]
    fn a_different_room_does_not_affect_an_existing_sighting() {
        let mut ledger = RivalSightings::default();
        ledger.record(TeamId(1), RoomId(2), SightingKind::Seen, 1);
        ledger.record(TeamId(1), RoomId(5), SightingKind::Heard, 2);
        assert_eq!(
            ledger.get(TeamId(1), RoomId(2)).unwrap().kind,
            SightingKind::Seen
        );
        assert_eq!(
            ledger.get(TeamId(1), RoomId(5)).unwrap().kind,
            SightingKind::Heard
        );
    }
}
