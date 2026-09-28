//! The Rogue's sensors (design section 7): "They see a loyal Observer only while that
//! Observer is detected by a Guardian or an explicit Rogue-controlled sensor."
//!
//! A Rogue seat installs one on a cell with [`ArchitectCommand::Sense`]. Like a directive
//! it costs the seat's cooldown and no card, and like any Rogue play it cannot be made
//! where an Observer is looking. A sensor watches its own cell and [`SENSOR_RANGE`] cells
//! along each open lateral line from it - a closed door or a wall ends the line, as it
//! ends a Guardian's - but only while its floor has power: sensors are the facility's
//! infrastructure, and a dark floor blinds them. What a sensor sees joins the Rogue's
//! knowledge (`rogue_detected`), never the Guardians' pursuit.
//!
//! The Rogue keeps at most [`MAX_SENSORS`]; a new one past that retires the oldest. A
//! loyal Observer beside one takes it down ([`ObserverAction::Dismantle`]), and a sensor
//! goes with its cell when the cell is emptied, retracted or its floor collapses.

use std::collections::BTreeSet;

use observed_hex::{HexCoord, HexFace, travel_distance};

use super::{
    ARCHITECT_COOLDOWN_TICKS, ArchitectCommand, ArchitectLab, CommandRefusal, LabEventKind,
    ObserverId, ObserverState,
};

/// How many sensors the Rogue keeps at once.
pub const MAX_SENSORS: usize = 4;
/// How many cells a sensor sees along each open line from its own: shorter than a
/// Guardian's six, since a sensor never sleeps.
pub const SENSOR_RANGE: u32 = 4;

impl ArchitectLab {
    /// Why a sensor on `target` would be refused under `cooldown`, if it would.
    pub(super) fn sense_refusal(&self, target: HexCoord, cooldown: u32) -> Option<CommandRefusal> {
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
        if self.prison_core.contains(&target) || self.prison.cells.contains(&target) {
            return Some(CommandRefusal::PrisonCore);
        }
        if self.sensors.contains_key(&target) {
            return Some(CommandRefusal::NoChange);
        }
        // Installed out of sight, as every Rogue play is made.
        if self.observed.contains(&target) {
            return Some(CommandRefusal::Observed);
        }
        None
    }

    /// Install the sensor legality has already admitted, retiring the oldest past the cap.
    pub(super) fn sense(&mut self, target: HexCoord) {
        self.cooldown = ARCHITECT_COOLDOWN_TICKS;
        self.sensors.insert(target, self.tick);
        while self.sensors.len() > MAX_SENSORS {
            let oldest = self
                .sensors
                .iter()
                .min_by_key(|&(&cell, &at)| (at, cell))
                .map(|(&cell, _)| cell)
                .expect("more than the cap");
            self.sensors.remove(&oldest);
        }
        self.command_log
            .push((self.tick, ArchitectCommand::Sense { target }));
        self.record_event(
            LabEventKind::Warning,
            Some(target),
            "The Rogue installs a sensor here.",
        );
    }

    /// Take down the sensors whose cells are gone: emptied, retracted or collapsed.
    pub(super) fn keep_sensors(&mut self) {
        let (placements, retracted, collapsed) = (
            &self.world.placements,
            &self.retracted,
            &self.collapsed_floors,
        );
        self.sensors.retain(|cell, _| {
            placements.get(cell).is_some_and(|p| p.space.built())
                && !retracted.contains(cell)
                && !collapsed.contains(&cell.level)
        });
    }

    /// Whether the sensor on `cell` watches now: there is one, and its floor has power.
    #[must_use]
    pub fn sensor_live(&self, cell: HexCoord) -> bool {
        self.sensors.contains_key(&cell) && self.economy.is_powered(cell.level)
    }

    /// The cells a sensor on `cell` watches while live: its own, and [`SENSOR_RANGE`] along
    /// each open lateral line.
    #[must_use]
    pub fn sensor_sight(&self, cell: HexCoord) -> BTreeSet<HexCoord> {
        let mut seen = BTreeSet::from([cell]);
        for face in HexFace::LATERAL {
            let mut at = cell;
            for _ in 0..SENSOR_RANGE {
                let Some(next) = self.step_through(at, face) else {
                    break;
                };
                seen.insert(next);
                at = next;
            }
        }
        seen
    }

    /// Whether the sensor on `cell` sees an active Observer now: what makes it pulse.
    #[must_use]
    pub fn sensor_watching(&self, cell: HexCoord) -> bool {
        self.sensor_live(cell) && {
            let sight = self.sensor_sight(cell);
            self.observers
                .values()
                .any(|o| o.state == ObserverState::Active && sight.contains(&o.cell))
        }
    }

    /// Active Observers a live sensor sees.
    #[must_use]
    pub fn sensed_observers(&self) -> BTreeSet<ObserverId> {
        let sight: BTreeSet<HexCoord> = self
            .sensors
            .keys()
            .filter(|&&cell| self.sensor_live(cell))
            .flat_map(|&cell| self.sensor_sight(cell))
            .collect();
        self.observers
            .values()
            .filter(|o| o.state == ObserverState::Active && sight.contains(&o.cell))
            .map(|o| o.id)
            .collect()
    }

    /// Active Observers the Rogue knows of: those a Guardian detects, and those a sensor
    /// sees.
    #[must_use]
    pub fn rogue_detected(&self) -> BTreeSet<ObserverId> {
        let mut detected = self.detected_observers();
        detected.extend(self.sensed_observers());
        detected
    }

    /// Whether Observer `id`, standing on `cell`, can reach the sensor on `sensor`: on it or
    /// beside it on its floor.
    pub(super) fn beside_sensor(&self, cell: HexCoord, sensor: HexCoord) -> bool {
        self.sensors.contains_key(&sensor)
            && cell.level == sensor.level
            && travel_distance(cell, sensor) <= 1
    }

    /// Take down the sensor on `cell`, which a loyal Observer beside it has dismantled.
    pub(super) fn dismantle(&mut self, cell: HexCoord) {
        if self.sensors.remove(&cell).is_some() {
            self.record_event(LabEventKind::Warning, Some(cell), "A sensor is dismantled.");
        }
    }
}
