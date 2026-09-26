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
use std::collections::BTreeMap;

use observed_facility::hex_wfc::{HexCoord, HexPlacement};
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
    /// The tiles this Architect has built, and the tick each was built at. The team learns
    /// a cell only by seeing it, but the Architect knows what they built: the board draws
    /// these until the team has seen the cell since, when what it saw takes over.
    pub built: BTreeMap<HexCoord, (HexPlacement, u64)>,
}

impl ArchitectDesk {
    /// What this Architect believes stands at `cell`: what they built there, unless the
    /// team has seen the cell since, and otherwise what the team saw.
    pub(crate) fn believed(
        &self,
        cell: HexCoord,
        known: Option<&observed_match::ascent::sim::KnownCell>,
    ) -> Option<HexPlacement> {
        match (self.built.get(&cell), known) {
            (Some(&(built, at)), Some(known)) if known.seen_at < at => Some(built),
            (Some(&(built, _)), None) => Some(built),
            (_, known) => known.map(|known| known.placement),
        }
    }

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
            built: BTreeMap::new(),
            last_refusal: None,
        }
    }
}
mod board;
mod building;
pub(super) mod capture;
mod desk;
mod input;
mod pick;
mod stack;
mod words;

/// The desk's systems, in order: build, read the player, frame, draw.
pub(super) fn systems() -> impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (
        (board::setup, stack::setup, desk::spawn, init_building),
        stack::click,
        input::input,
        board::frame,
        stack::frame,
        building::clear,
        building::draw,
        board::draw_marks,
        board::draw_play,
        stack::draw,
        desk::sync,
    )
        .chain()
        .run_if(resource_exists::<ArchitectDesk>)
}

#[cfg(test)]
mod tests;

fn init_building(mut commands: Commands, building: Option<Res<building::Building>>) {
    if building.is_none() {
        commands.init_resource::<building::Building>();
    }
}
