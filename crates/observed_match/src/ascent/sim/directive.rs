//! The Rogue's directive to the major Guardians (design sections 7 and 10).
//!
//! A Rogue seat points at a cell and the major Guardians go there instead of hunting,
//! until one of them arrives or the directive runs out. It costs the seat's cooldown and
//! no card, so a directive is a play the Rogue did not make with its hand. Minor Guardians
//! belong to the facility and never take one.
//!
//! On a first-person facility the Guardians the directive moves are bodies: the host reads
//! [`ArchitectLab::directed`] and walks them (`hex_wfc::HexWfcMatch::direct_guardians`).
//! The rules keep only when it is spent.

use observed_hex::HexCoord;

use super::{
    ARCHITECT_COOLDOWN_TICKS, ArchitectCommand, ArchitectLab, CommandRefusal, FIXED_HZ,
    GuardianKind, LabEventKind,
};

/// How long a directive stands if no major Guardian reaches it: long enough to cross a
/// floor at a major's walk, short enough that a forgotten one does not hold them forever.
pub const DIRECTIVE_TICKS: u64 = 30 * FIXED_HZ as u64;

/// Where the Rogue has sent the major Guardians, and until when.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RogueDirective {
    pub cell: HexCoord,
    pub until: u64,
}

impl ArchitectLab {
    /// Why a directive to `target` would be refused under `cooldown`, if it would.
    pub(super) fn directive_refusal(
        &self,
        target: HexCoord,
        cooldown: u32,
    ) -> Option<CommandRefusal> {
        if cooldown > 0 {
            return Some(CommandRefusal::Cooldown);
        }
        if !self
            .world
            .placements
            .get(&target)
            .is_some_and(|placement| placement.space.built())
            || self.retracted.contains(&target)
        {
            return Some(CommandRefusal::VoidTarget);
        }
        if self.collapsed_floors.contains(&target.level) {
            return Some(CommandRefusal::CollapsedFloor);
        }
        // A Guardian waits at the prison's door, never inside it.
        if self.prison_core.contains(&target) || self.prison.cells.contains(&target) {
            return Some(CommandRefusal::PrisonCore);
        }
        if self
            .directed
            .is_some_and(|directive| directive.cell == target)
        {
            return Some(CommandRefusal::NoChange);
        }
        None
    }

    /// Give the directive legality has already admitted.
    pub(super) fn direct(&mut self, target: HexCoord) {
        self.cooldown = ARCHITECT_COOLDOWN_TICKS;
        self.directed = Some(RogueDirective {
            cell: target,
            until: self.tick + DIRECTIVE_TICKS,
        });
        // The lab's own Guardians obey the rules' directive too.
        self.rogue_directive = Some(target);
        self.command_log
            .push((self.tick, ArchitectCommand::Direct { target }));
        self.record_event(
            LabEventKind::Warning,
            Some(target),
            "The Rogue directs the Guardians here.",
        );
    }

    /// Spend the directive once a major Guardian stands on it or its time is up.
    pub(super) fn keep_directive(&mut self) {
        let Some(directive) = self.directed else {
            return;
        };
        let arrived = self.guardians.values().any(|guardian| {
            guardian.kind == GuardianKind::Major && guardian.cell == directive.cell
        });
        if arrived || self.tick >= directive.until {
            self.directed = None;
        }
    }
}
