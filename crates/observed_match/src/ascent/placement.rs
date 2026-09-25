//! A read-only survey of where the selected card can go, and why not elsewhere.
//!
//! Both interfaces show this. Legality stays in [`crate::ascent::sim`]: this module only asks
//! [`ArchitectLab::refusal`] about every known cell and every rotation, and decides
//! which of its answers to show a player when a cell has more than one.

use std::collections::BTreeSet;

use observed_hex::HexCoord;

use crate::ascent::sim::{ArchitectLab, CommandRefusal};

/// What the selected card can do at one known cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellVerdict {
    pub cell: HexCoord,
    /// Rotations the simulation would accept, in ascending order. Empty when refused.
    pub rotations: Vec<u8>,
    /// Why no rotation is accepted. `None` exactly when `rotations` is not empty.
    pub refusal: Option<CommandRefusal>,
}

impl CellVerdict {
    #[must_use]
    pub fn legal(&self) -> bool {
        !self.rotations.is_empty()
    }
}

/// Every known cell with a placement, answered for `card` as if the card were charged.
///
/// The recharge is deliberately ignored: it is already shown on the charge meter, and
/// "cooldown" on every tile would hide the reason that outlasts it. Submission still
/// checks the cooldown.
#[must_use]
pub fn survey(sim: &ArchitectLab, card: usize) -> Vec<CellVerdict> {
    let mut charged = sim.clone();
    charged.cooldown = 0;
    let targets: BTreeSet<_> = charged.mutable_targets().into_iter().collect();
    charged
        .known
        .iter()
        .copied()
        .filter(|cell| charged.world.placements.contains_key(cell))
        .filter_map(|cell| {
            let answers: Vec<_> = (0..6)
                .map(|rotation| {
                    charged
                        .selected_command(card, cell, rotation)
                        .map(|command| (rotation, charged.refusal(command)))
                })
                .collect::<Option<_>>()?;
            let rotations: Vec<u8> = if targets.contains(&cell) {
                answers
                    .iter()
                    .filter(|(_, refusal)| refusal.is_none())
                    .map(|&(rotation, _)| rotation)
                    .collect()
            } else {
                Vec::new()
            };
            let refusal = rotations
                .is_empty()
                .then(|| shown_refusal(answers.iter().filter_map(|&(_, refusal)| refusal)))
                .flatten();
            (!rotations.is_empty() || refusal.is_some()).then_some(CellVerdict {
                cell,
                rotations,
                refusal,
            })
        })
        .collect()
}

/// The one reason to show when rotations disagree about why a cell is refused.
///
/// Cell-wide reasons (observed, occupied, prison...) are the same for every rotation,
/// so disagreement only happens among the shape checks. There `NoChange` is the least
/// true summary: it names the one rotation that reproduces the current tile, and its
/// advice to rotate is wrong when no other rotation fits either.
fn shown_refusal(refusals: impl Iterator<Item = CommandRefusal>) -> Option<CommandRefusal> {
    let refusals: Vec<_> = refusals.collect();
    refusals
        .iter()
        .copied()
        .find(|&refusal| refusal != CommandRefusal::NoChange)
        .or_else(|| refusals.first().copied())
}

/// Legal targets per level, indexed by level.
#[must_use]
pub fn legal_by_level(verdicts: &[CellVerdict], levels: u8) -> Vec<usize> {
    let mut counts = vec![0; usize::from(levels)];
    for verdict in verdicts.iter().filter(|verdict| verdict.legal()) {
        if let Some(count) = counts.get_mut(usize::from(verdict.cell.level)) {
            *count += 1;
        }
    }
    counts
}

/// Refusal reasons on one level, most common first, for a one-line explanation of an
/// empty or nearly empty floor.
#[must_use]
pub fn refusals_on_level(verdicts: &[CellVerdict], level: u8) -> Vec<(CommandRefusal, usize)> {
    let mut counts: Vec<(CommandRefusal, usize)> = Vec::new();
    for refusal in verdicts
        .iter()
        .filter(|verdict| verdict.cell.level == level)
        .filter_map(|verdict| verdict.refusal)
    {
        match counts.iter_mut().find(|(seen, _)| *seen == refusal) {
            Some((_, count)) => *count += 1,
            None => counts.push((refusal, 1)),
        }
    }
    // Stable: equal counts keep first-seen order, so the line does not flicker.
    counts.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    counts
}

/// A short noun phrase for a refusal, for counts ("4 held in view").
#[must_use]
pub const fn refusal_tally(refusal: CommandRefusal) -> &'static str {
    match refusal {
        CommandRefusal::MatchFinished => "match over",
        CommandRefusal::Cooldown => "recharging",
        CommandRefusal::CardNotInHand => "no card",
        CommandRefusal::UnknownTarget => "unmapped",
        CommandRefusal::VoidTarget => "empty",
        CommandRefusal::CollapsedFloor => "collapsed",
        CommandRefusal::NoChange => "already this shape",
        CommandRefusal::WrongDistrict => "other district",
        CommandRefusal::Observed => "held in view",
        CommandRefusal::Occupied => "occupied",
        CommandRefusal::Anchored => "anchored",
        CommandRefusal::PrisonCore => "prison core",
        CommandRefusal::NoLocalAttachment => "would not connect",
        CommandRefusal::InvalidThreshold => "no open edge",
        CommandRefusal::DoorAlreadyPresent => "already doored",
        CommandRefusal::FixedStructure => "room or stair",
        CommandRefusal::Unbuildable => "no such tile",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascent::sim::ArchitectMode;

    #[test]
    fn the_survey_agrees_with_the_simulation_about_every_cell() {
        for mode in ArchitectMode::ALL {
            let sim = ArchitectLab::for_mode(mode).unwrap();
            for card in 0..sim.deck.hand.len() {
                let verdicts = survey(&sim, card);
                assert!(!verdicts.is_empty(), "{mode:?} card {card}");
                for verdict in &verdicts {
                    assert_eq!(verdict.legal(), verdict.refusal.is_none(), "{verdict:?}");
                    for rotation in 0..6 {
                        let command = sim.selected_command(card, verdict.cell, rotation).unwrap();
                        assert_eq!(
                            sim.refusal(command).is_none(),
                            verdict.rotations.contains(&rotation),
                            "{mode:?} card {card} {verdict:?} rotation {rotation}"
                        );
                    }
                }
                let legal = sim
                    .legal_commands()
                    .into_iter()
                    .filter(|command| {
                        matches!(command, crate::ascent::sim::ArchitectCommand::Play { card: id, .. }
                            if *id == sim.deck.hand[card].id)
                    })
                    .count();
                let surveyed: usize = verdicts.iter().map(|v| v.rotations.len()).sum();
                assert_eq!(surveyed, legal, "{mode:?} card {card}");
            }
        }
    }

    #[test]
    fn a_watched_cell_says_it_is_watched() {
        let sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let verdicts = survey(&sim, 0);
        for cell in sim.observed.iter().filter(|cell| sim.known.contains(cell)) {
            if let Some(verdict) = verdicts.iter().find(|v| v.cell == *cell) {
                assert_eq!(verdict.refusal, Some(CommandRefusal::Observed));
            }
        }
    }

    #[test]
    fn no_change_only_wins_when_it_is_the_whole_story() {
        use CommandRefusal::{NoChange, NoLocalAttachment};
        assert_eq!(
            shown_refusal([NoChange, NoLocalAttachment, NoLocalAttachment].into_iter()),
            Some(NoLocalAttachment)
        );
        assert_eq!(shown_refusal([NoChange].into_iter()), Some(NoChange));
        assert_eq!(shown_refusal(std::iter::empty()), None);
    }
}
