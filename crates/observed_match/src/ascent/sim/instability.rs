//! A Rogue instability surge uses the facility's disturbance meter and existing
//! contradiction clock. It does not create geometry or bypass retraction protections.

use observed_hex::HexCoord;

use super::{ArchitectLab, LabEventKind};

/// A surge is as disruptive as a Rogue play that creates a contradiction.
const SURGE_DISTURBANCE: u32 = crate::ascent::economy::DISTURBANCE_CONTRADICTION_ROGUE;
/// A surge can pull an exposed retraction's deadline to this many ticks from now.
const SURGE_WARNING_TICKS: u64 = 90;

impl ArchitectLab {
    /// Apply a legal surge to one floor. Only the next unprotected contradiction on
    /// that floor can have its clock shortened; the usual grace for an occupied tile
    /// still follows when retraction is attempted.
    pub(super) fn surge(&mut self, target: HexCoord) {
        let hastened = self
            .next_retraction()
            .is_some_and(|cell| cell.level == target.level)
            && self.next_retraction_tick.is_some();
        if hastened {
            self.next_retraction_tick = self
                .next_retraction_tick
                .map(|due| due.min(self.tick + SURGE_WARNING_TICKS));
        }
        self.economy
            .add_disturbance(target.level, SURGE_DISTURBANCE);
        self.resolve_disturbance_waves(target.level);
        self.record_event(
            LabEventKind::Warning,
            Some(target),
            if hastened {
                "Instability surge. Floor pressure rises; exposed tile retracts sooner."
            } else {
                "Instability surge. Floor pressure rises."
            },
        );
    }
}
