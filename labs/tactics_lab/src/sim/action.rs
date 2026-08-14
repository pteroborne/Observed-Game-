//! What a unit can be told to do, and what it costs.
//!
//! Legality and cost live together on purpose. The shipped match learned this
//! the hard way in `HexWfcMatch::deployable_threshold`, whose doc comment says
//! it *is* the deploy precondition rather than a description of one — a separate
//! predicate eventually disagrees with the rule, and then the interface offers a
//! button that does nothing. Here [`TacticsGame::legal_actions`] is what the view
//! draws and what [`TacticsGame::apply`] enforces, so they cannot drift.

use observed_core::PlayerId;
use observed_hex::HexFace;

use crate::settings::ActionCosts;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticsAction {
    /// Step through an open port. Lateral faces need a door; vertical faces need
    /// a ramp or shaft opening on both sides.
    Move(HexFace),
    /// Whatever the room this unit stands in offers: a keystone, a station, a
    /// monitor survey, or an anchor cache.
    Interact,
    /// Pin this cell against relayout until it is recovered.
    DeployAnchor,
    /// Take the anchor in this cell back into inventory.
    RecoverAnchor,
    /// Set down a teleport plate. Two plates from the same squad form one link.
    DeployPad,
}

impl TacticsAction {
    #[must_use]
    pub const fn cost(self, costs: &ActionCosts) -> u8 {
        match self {
            TacticsAction::Move(_) => costs.move_step,
            TacticsAction::Interact => costs.interact,
            // Recovering is picking something up, not placing it: it costs a
            // plain action so a misplaced anchor is a recoverable mistake rather
            // than a turn thrown away.
            TacticsAction::DeployAnchor => costs.anchor,
            TacticsAction::RecoverAnchor => costs.interact,
            TacticsAction::DeployPad => costs.pad,
        }
    }
}

/// One entry in a match's action log: who did what.
///
/// A match replays from its [`crate::settings::MatchSettings`] plus this list,
/// which is what the determinism test asserts and what a save file would store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoggedAction {
    pub turn: u32,
    pub unit: PlayerId,
    pub action: TacticsAction,
}

/// Why an action was refused. Reported rather than silently dropped, so the HUD
/// can say what is wrong instead of a button appearing inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    NoSuchUnit,
    NotYourTurn,
    UnitEscaped,
    NotEnoughActionPoints,
    /// The face is sealed, leaves the grid, or leads into solid rock.
    Blocked,
    /// Nothing in this cell answers to `Interact`.
    NothingHere,
    /// Anchors or pads are switched off for this match, or the unit is empty.
    Unavailable,
}
