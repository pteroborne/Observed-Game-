//! The **sighting ledger** writer: the single system that turns live rival truth into
//! team-local witnessed evidence (Design ruling: the tac-map reads fog-of-war sightings,
//! never live rival positions). Folds two sources every `Update` frame the player is in
//! a place:
//!
//! - `sim::nav::rival_signals` on the room whose thresholds are currently on screen —
//!   a neighbour with `presence` records [`SightingKind::Seen`]; a neighbour with
//!   `anchor` records [`SightingKind::AnchorSpotted`] (both may fire for the same
//!   neighbour; `RivalSightings::record` resolves which one sticks).
//! - `crate::rivals::rivals_in_room` on the room the local player physically stands in
//!   (a rival sharing your room is unambiguously `Seen`, regardless of `rival_signals`'
//!   neighbour-facing view).
//!
//! `screens::audio::bleed_rival_sound` is the ledger's only other writer, and it only
//! ever records [`SightingKind::Heard`] through the same [`RivalSightings::record`]
//! method — so there is exactly one *rule* (the method), even though two systems call
//! it for two different evidence sources. Presentation-only: this never touches the
//! deterministic brain, so replay/lockstep are untouched.

use bevy::prelude::*;

use crate::rivals;
use crate::sim::director::MatchDirector;
use crate::sim::nav::rival_signals;
use crate::sim::state::{RivalSightings, SightingKind, TeleportState};
use crate::teleport::Place;

pub(crate) fn record_rival_sightings(
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    mut sightings: ResMut<RivalSightings>,
) {
    let game = runtime.live.host_match();
    let commits = game.reroute_commits;
    let local_team = crate::flow::LOCAL_TEAM.0 as usize;

    let signal_room = match tp.place {
        Place::Room(room) => room,
        Place::Hallway { from, .. } => from,
    };
    for signal in rival_signals(game, local_team, signal_room) {
        if let Some(team) = signal.presence {
            sightings.record(team, signal.neighbor, SightingKind::Seen, commits);
        }
        if let Some(team) = signal.anchor {
            sightings.record(team, signal.neighbor, SightingKind::AnchorSpotted, commits);
        }
    }

    // A rival clump sharing the room the player physically stands in is Seen there,
    // regardless of the neighbour-facing `rival_signals` projection above.
    if let Place::Room(room) = tp.place {
        for team_index in rivals::rivals_in_room(&game.competitive, room) {
            let team = game.competitive.teams[team_index].id;
            sightings.record(team, room, SightingKind::Seen, commits);
        }
    }
}

// Integration coverage (building a full `App`) lives in `crate::tests` — this module
// must stay presentation-free per `arch_check::sim_never_imports_presentation`, and a
// headless test app pulls in `crate::view` for its asset plugin wiring. See
// `crate::tests::placing_a_rival_in_a_neighbour_and_rebuilding_records_a_seen_sighting`.
