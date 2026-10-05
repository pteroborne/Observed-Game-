//! What the Architect's desk says: pure, so it can be tested without a window.

use observed_match::ascent::session::Refusal;
use observed_match::ascent::sim::{
    ArchitectLab, CardKind, CommandRefusal, MAX_SENSORS, RogueDirective, TileShape,
};

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
        CardKind::Station => "STATION",
        CardKind::Stair => "STAIR",
        CardKind::Cistern => "CISTERN",
        CardKind::Chargeworks => "CHARGEWORKS",
        CardKind::ArchiveWell => "ARCHIVE WELL",
        CardKind::RainCourt => "RAIN COURT",
        CardKind::JadeNave => "JADE NAVE",
        CardKind::LastPromenade => "LAST PROMENADE",
        CardKind::SwitchingConcourse => "CONCOURSE",
        CardKind::Directive => "DIRECTIVE",
        CardKind::Sensor => "SENSOR",
        CardKind::Surge => "SURGE",
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
        CardKind::Station => "recharges your team on a powered floor",
        CardKind::Stair => "climbs a floor",
        CardKind::Cistern => "3-hex reservoir, open spans & raised causeways",
        CardKind::Chargeworks => "3-hex factory & gantry",
        CardKind::ArchiveWell => "3-hex reading well & raised galleries",
        CardKind::RainCourt => "3-hex rain garden & sheltered veranda",
        CardKind::JadeNave => "3-hex raised nave",
        CardKind::LastPromenade => "3-hex sky bridges",
        CardKind::SwitchingConcourse => "3-hex transit hall",
        CardKind::Directive => "majors walk here",
        CardKind::Sensor => "sees 4 cells each way",
        CardKind::Surge => "raises floor pressure",
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

/// Where the Rogue has sent its Guardians, and for how long yet, at tick `now`.
#[must_use]
pub(super) fn directed(directive: RogueDirective, now: u64) -> String {
    let cell = directive.cell;
    format!(
        "Guardians sent to floor {:02}, cell {}, {}  ({} s left).",
        cell.level + 1,
        cell.q,
        cell.r,
        directive.until.saturating_sub(now).div_ceil(60)
    )
}

/// The Rogue's standing orders, for its board: where its Guardians were sent, and its
/// sensors - how many, and how many see someone now.
#[must_use]
pub(super) fn rogue_orders(rules: &ArchitectLab) -> String {
    let watching = rules
        .sensors
        .keys()
        .filter(|&&cell| rules.sensor_watching(cell))
        .count();
    let sensors = match (rules.sensors.len(), watching) {
        (0, _) => "No sensors.".to_owned(),
        (installed, 0) => format!("Sensors {installed} / {MAX_SENSORS}."),
        (installed, watching) => {
            format!("Sensors {installed} / {MAX_SENSORS}, {watching} seeing someone.")
        }
    };
    rules.directed.map_or(sensors.clone(), |directive| {
        format!("{}\n{sensors}", directed(directive, rules.tick))
    })
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
        (DeskButton::Look, false) => "LOOK THROUGH THEIR EYES   [V]",
        (DeskButton::Look, true) => "LOOK THROUGH THEIR EYES   [VIEW]",
    }
}

/// The line of controls under the hand.
#[must_use]
pub(super) const fn controls(pad: bool, rogue: bool) -> &'static str {
    match (pad, rogue) {
        (true, false) => {
            "LS point  >  A aim  >  LB/RB turn  >  A again play     B back   Y answer   D-pad < > card  ^ v floor   R3 requisition"
        }
        (false, false) => {
            "1-5 card  >  click a cell to aim  >  Q/E turn  >  Space play     Esc back   F answer   [ / ] floor   R requisition"
        }
        // Nobody asks the Rogue for help, and a requisition is a team's.
        (true, true) => {
            "LS point  >  A aim  >  LB/RB turn  >  A again play     B back   D-pad < > card  ^ v floor"
        }
        (false, true) => {
            "1-5 card  >  click a cell to aim  >  Q/E turn  >  Space play     Esc back   [ / ] floor"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_card_has_a_name_and_a_line() {
        for kind in TileShape::ALL.into_iter().map(CardKind::Tile).chain([
            CardKind::Door,
            CardKind::Station,
            CardKind::Stair,
            CardKind::Directive,
            CardKind::Sensor,
            CardKind::Surge,
        ]) {
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
            printed.push(controls(pad, false).to_owned());
            printed.push(controls(pad, true).to_owned());
            for action in [
                DeskButton::FloorDown,
                DeskButton::FloorUp,
                DeskButton::TurnLeft,
                DeskButton::TurnRight,
                DeskButton::Play,
                DeskButton::Cancel,
                DeskButton::Answer,
                DeskButton::Look,
            ] {
                printed.push(button_label(action, pad).to_owned());
            }
        }
        for kind in TileShape::ALL.into_iter().map(CardKind::Tile).chain([
            CardKind::Door,
            CardKind::Station,
            CardKind::Stair,
            CardKind::Cistern,
            CardKind::Chargeworks,
            CardKind::ArchiveWell,
            CardKind::Directive,
            CardKind::Sensor,
            CardKind::Surge,
        ]) {
            printed.push(card_name(kind).to_owned());
            printed.push(card_detail(kind).to_owned());
        }
        for text in printed {
            assert!(text.is_ascii(), "{text}");
        }
    }
}
