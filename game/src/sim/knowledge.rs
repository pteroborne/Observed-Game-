//! The **map knowledge ledger** writer: the single system that turns the player's
//! first-person experience into the fog-of-war structure the tac-map is allowed to draw
//! (Design ruling: the map shows only what the local player has personally witnessed —
//! the facility must be explored, never read off a complete schematic). Folds two
//! sources every `Update` frame the player is in a place:
//!
//! - Standing in a room *visits* it, and every doorway gap currently on screen
//!   *glimpses* its destination room and records the connection — the same
//!   "observed → frozen" doorways the preview renders, so the map can never know more
//!   than the player's own eyes.
//! - Walking a hallway records the edge being traversed and glimpses the far room.
//!
//! Presentation-only: this never touches the deterministic brain, so replay/lockstep
//! are untouched. The tac-map projection (`tacmap::build_map`) filters the live spec
//! against this ledger, so an edge a reroute removed simply drops off the map.

use bevy::prelude::*;

use crate::sim::state::{MapKnowledge, TeleportState};
use crate::teleport::Place;

pub(crate) fn record_map_knowledge(tp: Res<TeleportState>, mut knowledge: ResMut<MapKnowledge>) {
    match tp.place {
        Place::Room(room) => {
            knowledge.visit(room);
            for gap in &tp.geom.gaps {
                knowledge.glimpse(gap.target);
                knowledge.connect(room, gap.target);
            }
        }
        Place::Hallway { from, to, .. } => {
            knowledge.glimpse(to);
            knowledge.connect(from, to);
        }
    }
}

// Integration coverage (building a full `App`) lives in `crate::tests` — this module
// must stay presentation-free per `arch_check::sim_never_imports_presentation`. See
// `crate::tests::the_tac_map_only_shows_what_the_player_has_witnessed`.
