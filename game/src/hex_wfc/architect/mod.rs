//! The Architect's seat: no body, a map of what the team knows, and a hand of cards.
//!
//! A local player who takes their team's Architect seat in Architect Ascent plays from
//! here. Their team's bodies are bots. The desk ([`ArchitectDesk`]) holds what the
//! player is doing - the card picked up, its rotation, the cell under the cursor, the
//! floor in view - and the play they have committed, which the Ascent step hands to the
//! rules as that seat's command. Legality is never decided here: every verdict shown is
//! the rules' own answer to the same question the play will ask.

use bevy::prelude::*;
use observed_core::PlayerId;
use observed_facility::hex_wfc::HexCoord;
use observed_match::ascent::session::Refusal;
use observed_match::ascent::sim::{ArchitectCommand, TeamId};

/// What the local Architect is doing.
#[derive(Resource, Debug)]
pub(crate) struct ArchitectDesk {
    /// The seat the rules know this player by.
    pub seat: PlayerId,
    pub team: TeamId,
    /// The card in hand picked up, by its place in the hand.
    pub selected: Option<usize>,
    /// Sixths of a turn clockwise.
    pub rotation: u8,
    /// The cell under the cursor, on the floor in view.
    pub hovered: Option<HexCoord>,
    /// The floor the board shows.
    pub floor: u8,
    /// A play committed and not yet stepped.
    pub pending: Option<ArchitectCommand>,
    /// The rules' answer to the last play stepped, when they refused it.
    pub last_refusal: Option<Refusal>,
}

impl ArchitectDesk {
    /// A desk for `team`'s Architect, looking at `floor`, where the team starts.
    pub(crate) fn new(seat: PlayerId, team: TeamId, floor: u8) -> Self {
        Self {
            seat,
            team,
            selected: None,
            rotation: 0,
            hovered: None,
            floor,
            pending: None,
            last_refusal: None,
        }
    }
}
mod board;
pub(super) mod capture;
mod desk;
mod input;
mod pick;
mod stack;
mod words;

/// The desk's systems, in order: build, read the player, frame, draw.
pub(super) fn systems() -> impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (
        board::setup,
        stack::setup,
        desk::spawn,
        stack::click,
        input::input,
        board::frame,
        stack::frame,
        board::draw_floor,
        board::draw_play,
        stack::draw,
        desk::sync,
    )
        .chain()
        .run_if(resource_exists::<ArchitectDesk>)
}

#[cfg(test)]
mod tests;
