//! What the Architect's desk says: pure, so it can be tested without a window.

use observed_match::ascent::session::Refusal;
use observed_match::ascent::sim::{CardKind, CommandRefusal, TileShape};

use super::desk::DeskButton;

/// A card's name as the hand prints it.
#[must_use]
pub(super) const fn card_name(kind: CardKind) -> &'static str {
    match kind {
        CardKind::Tile(TileShape::DeadEnd) => "DEAD END",
        CardKind::Tile(TileShape::Corridor) => "CORRIDOR",
        CardKind::Tile(TileShape::Bend) => "TURN",
        CardKind::Tile(TileShape::Junction) => "JUNCTION",
        CardKind::Tile(TileShape::Hall) => "HALL",
        CardKind::Door => "DOOR",
    }
}

/// What a card does, in a line.
#[must_use]
pub(super) const fn card_detail(kind: CardKind) -> &'static str {
    match kind {
        CardKind::Tile(TileShape::DeadEnd) => "1 way: a pocket",
        CardKind::Tile(TileShape::Corridor) => "2 ways, straight",
        CardKind::Tile(TileShape::Bend) => "2 ways, turning",
        CardKind::Tile(TileShape::Junction) => "3 ways",
        CardKind::Tile(TileShape::Hall) => "4 ways",
        CardKind::Door => "on a doorway",
    }
}

/// The verdict on the cell aimed at, or under the cursor, for the card picked up.
#[must_use]
pub(super) fn verdict(refusal: Option<Refusal>) -> String {
    match refusal {
        None => "Ready to play.".to_owned(),
        Some(refusal) => format!("Not here: {}.", refusal_words(refusal)),
    }
}

/// Why the rules refused, as a player would say it.
#[must_use]
pub(super) fn refusal_words(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::Architect(CommandRefusal::UnknownTarget) => "your team has not mapped it",
        Refusal::Architect(reason) => reason.label(),
        Refusal::MatchFinished => "the match has ended",
        Refusal::WrongRole => "this seat cannot play that",
        _ => "the rules refused it",
    }
}

/// The cooldown as the panel prints it, from ticks left at 60 a second.
#[must_use]
pub(super) fn cooldown(ticks: u32) -> String {
    if ticks == 0 {
        "READY".to_owned()
    } else {
        format!("RECHARGING  {:.1} s", f64::from(ticks) / 60.0)
    }
}

/// A desk button's label, naming the key or the controller button that does the same.
#[must_use]
pub(super) const fn button_label(action: DeskButton, pad: bool) -> &'static str {
    match (action, pad) {
        (DeskButton::FloorDown, _) => "<   FLOOR",
        (DeskButton::FloorUp, _) => "FLOOR   >",
        (DeskButton::TurnLeft, false) => "Q   TURN",
        (DeskButton::TurnLeft, true) => "LB   TURN",
        (DeskButton::TurnRight, false) => "TURN   E",
        (DeskButton::TurnRight, true) => "TURN   RB",
        (DeskButton::Play, false) => "PLAY CARD   [SPACE]",
        (DeskButton::Play, true) => "PLAY CARD   [A]",
        (DeskButton::Cancel, false) => "CANCEL   [ESC]",
        (DeskButton::Cancel, true) => "CANCEL   [B]",
        (DeskButton::Answer, false) => "ANSWER   [F]",
        (DeskButton::Answer, true) => "ANSWER   [Y]",
    }
}

/// The line of controls under the hand.
#[must_use]
pub(super) const fn controls(pad: bool) -> &'static str {
    if pad {
        "LS point  >  A aim  >  LB/RB turn  >  A again play     B back   Y answer   D-pad < > card  ^ v floor   R3 requisition"
    } else {
        "1-5 card  >  click a cell to aim  >  Q/E turn  >  Space play     Esc back   F answer   [ / ] floor   R requisition"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_card_has_a_name_and_a_line() {
        for kind in TileShape::ALL
            .into_iter()
            .map(CardKind::Tile)
            .chain([CardKind::Door])
        {
            assert!(!card_name(kind).is_empty());
            assert!(!card_detail(kind).is_empty());
        }
    }

    #[test]
    fn a_refusal_says_why_and_a_legal_cell_says_so() {
        assert_eq!(verdict(None), "Ready to play.");
        assert_eq!(
            verdict(Some(Refusal::Architect(CommandRefusal::Observed))),
            "Not here: an Observer is holding that tile in view."
        );
        assert!(
            verdict(Some(Refusal::Architect(CommandRefusal::UnknownTarget))).contains("not mapped")
        );
    }

    #[test]
    fn the_cooldown_counts_down_in_seconds() {
        assert_eq!(cooldown(0), "READY");
        assert_eq!(cooldown(150), "RECHARGING  2.5 s");
    }

    #[test]
    fn everything_it_prints_is_in_the_shipped_font() {
        let mut printed = vec![verdict(None), cooldown(90)];
        for pad in [false, true] {
            printed.push(controls(pad).to_owned());
            for action in [
                DeskButton::FloorDown,
                DeskButton::FloorUp,
                DeskButton::TurnLeft,
                DeskButton::TurnRight,
                DeskButton::Play,
                DeskButton::Cancel,
                DeskButton::Answer,
            ] {
                printed.push(button_label(action, pad).to_owned());
            }
        }
        for kind in TileShape::ALL.into_iter().map(CardKind::Tile) {
            printed.push(card_name(kind).to_owned());
            printed.push(card_detail(kind).to_owned());
        }
        for text in printed {
            assert!(text.is_ascii(), "{text}");
        }
    }
}
