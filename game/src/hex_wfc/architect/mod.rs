//! The Architect's seat: no body, a map of what the team knows, and a hand of cards.
//!
//! A local player who takes their team's Architect seat in Architect Ascent plays from
//! here. Their team's bodies are bots. A player whose body fell into true void plays from
//! here too, at the Rogue board (`ArchitectDesk::rogue`): the Rogue's shared hand, and the
//! facility's truth in place of a team's memory of it (`ArchitectLab::rogue_view`). The desk ([`ArchitectDesk`]) holds what the
//! player is doing - the card picked up, its rotation, the cell under the cursor, the
//! floor in view - and the play they have committed, which the Ascent step hands to the
//! rules as that seat's command. Legality is never decided here: every verdict shown is
//! the rules' own answer to the same question the play will ask.

use bevy::prelude::*;
use observed_core::PlayerId;
use std::collections::BTreeMap;

use observed_facility::hex_wfc::{HexCoord, HexPlacement};
use observed_match::ascent::session::{AscentSession, Refusal, TeamRequest};
use observed_match::ascent::sim::{
    ArchitectCommand, ArchitectLab, Deck, Observer, TeamId, TeamKnowledge,
};

/// What the local Architect is doing.
#[derive(Resource, Debug)]
pub(crate) struct ArchitectDesk {
    /// The seat the rules know this player by.
    pub seat: PlayerId,
    pub team: TeamId,
    /// The Rogue board: a corrupted player's, with the Rogue's shared hand and the
    /// facility's truth rather than a team's hand and memory.
    pub rogue: bool,
    /// What the Rogue board shows, and the rules' tick it was read at (`refresh_rogue`).
    pub rogue_sight: Option<(u64, TeamKnowledge)>,
    /// The card in hand picked up, by its place in the hand.
    pub selected: Option<usize>,
    /// Sixths of a turn clockwise.
    pub rotation: u8,
    /// The cell under the cursor, on the floor in view.
    pub hovered: Option<HexCoord>,
    /// The cell the card picked up is aimed at: a click aims, and the play waits there,
    /// its ghost standing, until it is confirmed or cancelled.
    pub aimed: Option<HexCoord>,
    /// The floor the board shows.
    pub floor: u8,
    /// Where a controller's cursor stands on the floor in view, in plan metres, while a
    /// controller is pointing rather than the mouse.
    pub pad_cursor: Option<Vec2>,
    /// Whether the last hand on the desk was on a controller, which is what the desk's
    /// prompts name.
    pub pad: bool,
    /// The body whose eyes the Architect is looking through, when off the desk (`eyes`).
    pub eyes: Option<PlayerId>,
    /// A play committed and not yet stepped.
    pub pending: Option<ArchitectCommand>,
    /// A team request answered at the desk and not yet stepped: its author and when it
    /// was made, which is what the rules acknowledge it by.
    pub pending_answer: Option<(PlayerId, u64)>,
    /// The rules' answer to the last play stepped, when they refused it.
    pub last_refusal: Option<Refusal>,
    /// The tiles this Architect has built, and the tick each was built at. The team learns
    /// a cell only by seeing it, but the Architect knows what they built: the board draws
    /// these until the team has seen the cell since, when what it saw takes over.
    pub built: BTreeMap<HexCoord, (HexPlacement, u64)>,
}

/// A hand as the desk reads it: a team's, or the Rogue's.
#[derive(Clone, Copy)]
pub(crate) struct HandView<'a> {
    pub deck: &'a Deck,
    pub cooldown: u32,
}

impl ArchitectDesk {
    /// What the board is drawn from: the team's knowledge, or at the Rogue board the view
    /// the rules allow the Rogue.
    #[must_use]
    pub(crate) fn knowledge<'a>(&'a self, rules: &'a ArchitectLab) -> Option<&'a TeamKnowledge> {
        if self.rogue {
            self.rogue_sight.as_ref().map(|(_, sight)| sight)
        } else {
            rules.team_knowledge.get(&self.team)
        }
    }

    /// The hand at this desk: the team's, or the Rogue's shared one.
    #[must_use]
    pub(crate) fn hand<'a>(&self, session: &'a AscentSession) -> Option<HandView<'a>> {
        if self.rogue {
            Some(HandView {
                deck: &session.sim.deck,
                cooldown: session.sim.cooldown,
            })
        } else {
            session.hands.get(&self.team).map(|hand| HandView {
                deck: &hand.deck,
                cooldown: hand.cooldown,
            })
        }
    }

    /// Whether the board shows `observer`: the team's own, or at the Rogue board those the
    /// Rogue may know of (`knowledge`).
    #[must_use]
    pub(crate) fn shows(&self, observer: &Observer, knowledge: &TeamKnowledge) -> bool {
        if self.rogue {
            knowledge.known_observers.contains_key(&observer.id)
        } else {
            observer.team == self.team
        }
    }

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

    /// The cell the play is about: the one aimed at, or else the one under the cursor.
    #[must_use]
    pub(crate) fn focus(&self) -> Option<HexCoord> {
        self.aimed.or(self.hovered)
    }

    /// Pick up card `index` of `held`, or put it down if it is the card in hand. The aim
    /// stays with another card, so cards can be compared on one cell.
    pub(crate) fn pick_up(&mut self, index: usize, held: usize) {
        if index >= held {
            return;
        }
        if self.selected == Some(index) {
            self.put_down();
        } else {
            self.selected = Some(index);
        }
    }

    /// Pick up the card `step` places along the hand of `held` from the one in hand,
    /// wrapping; from none, the first card forward or the last one back.
    pub(crate) fn cycle(&mut self, step: i8, held: usize) {
        if held == 0 {
            return;
        }
        let held_i = i64::try_from(held).unwrap_or(i64::MAX);
        let next = match self.selected {
            None if step >= 0 => 0,
            None => held_i - 1,
            Some(index) => (i64::try_from(index).unwrap_or(0) + i64::from(step)).rem_euclid(held_i),
        };
        self.selected = usize::try_from(next).ok();
    }

    /// Put the card down, and the aim with it.
    pub(crate) fn put_down(&mut self) {
        self.selected = None;
        self.aimed = None;
    }

    /// Step back once: from an aim to the card in hand, and from the card to none.
    pub(crate) fn cancel(&mut self) {
        if self.aimed.take().is_none() {
            self.selected = None;
        }
    }

    /// A click on `cell` of the board. With a card in hand it aims there; a click on the
    /// cell already aimed at is the confirmation, and says so.
    pub(crate) fn click_cell(&mut self, cell: HexCoord) -> bool {
        if self.selected.is_none() {
            return false;
        }
        if self.aimed == Some(cell) {
            return true;
        }
        self.aimed = Some(cell);
        self.last_refusal = None;
        false
    }

    /// Settle a confirmed `play` by the rules' own inspection of it: sent, with the card
    /// put down, when they would take it; kept in hand, with their reason, when not.
    pub(crate) fn settle(&mut self, play: ArchitectCommand, refusal: Option<Refusal>) {
        match refusal {
            None => {
                self.pending = Some(play);
                self.put_down();
                self.last_refusal = None;
            }
            Some(refusal) => self.last_refusal = Some(refusal),
        }
    }

    /// Answer `request`: acknowledge it to the team on the next step, and look at the
    /// floor it is on.
    pub(crate) fn answer(&mut self, request: &TeamRequest) {
        self.pending_answer = Some((request.author, request.created_at));
        self.look_at(request.target.level);
    }

    /// Look at `floor`, which drops an aim on the floor left.
    pub(crate) fn look_at(&mut self, floor: u8) {
        if floor != self.floor {
            self.floor = floor;
            self.aimed = None;
            self.hovered = None;
            self.pad_cursor = None;
        }
    }

    /// The Rogue board, for the corrupted player in `seat`, once of `team`, looking at
    /// `floor`, where they fell.
    pub(crate) fn rogue(seat: PlayerId, team: TeamId, floor: u8) -> Self {
        Self {
            rogue: true,
            ..Self::new(seat, team, floor)
        }
    }

    /// A desk for `team`'s Architect, looking at `floor`, where the team starts.
    pub(crate) fn new(seat: PlayerId, team: TeamId, floor: u8) -> Self {
        Self {
            seat,
            team,
            rogue: false,
            rogue_sight: None,
            selected: None,
            rotation: 0,
            hovered: None,
            aimed: None,
            floor,
            pad_cursor: None,
            pad: false,
            eyes: None,
            pending: None,
            pending_answer: None,
            built: BTreeMap::new(),
            last_refusal: None,
        }
    }
}
mod board;
mod building;
pub(super) mod capture;
mod cards;
mod desk;
mod eyes;
mod feedback;
mod input;
mod overlay;
mod pad;
mod pick;
mod readout;
mod requests;
mod stack;
mod words;

/// The desk's systems, in order: build, read the player, frame, draw.
pub(super) fn systems() -> impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (
        (
            board::setup,
            stack::setup,
            cards::setup,
            feedback::setup,
            init_building,
        ),
        (desk::spawn, eyes::spawn, refresh_rogue),
        stack::click,
        input::input,
        pad::input,
        // After the desk's own hands, which it silences while looking.
        eyes::input,
        eyes::sync,
        board::frame,
        stack::frame,
        building::clear,
        building::draw,
        board::draw_marks,
        overlay::draw,
        feedback::build_in,
        feedback::pulse,
        feedback::sounds,
        stack::draw,
        cards::sync,
        readout::sync,
        readout::prompts,
    )
        .chain()
        .run_if(resource_exists::<ArchitectDesk>)
}

#[cfg(test)]
mod tests;

/// Read the Rogue board's view again when the rules have ticked.
fn refresh_rogue(
    mut desk: ResMut<ArchitectDesk>,
    runtime: Res<crate::hex_wfc::sim::HexWfcRuntime>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let tick = ascent.rules().tick;
    if desk.rogue && desk.rogue_sight.as_ref().is_none_or(|(at, _)| *at != tick) {
        desk.rogue_sight = Some((tick, ascent.rules().rogue_view()));
    }
}

fn init_building(mut commands: Commands, building: Option<Res<building::Building>>) {
    if building.is_none() {
        commands.init_resource::<building::Building>();
    }
}
