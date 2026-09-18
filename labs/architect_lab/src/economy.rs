//! Cell-level economy for the Architect Ascent: charge pools, powered stations,
//! power states, disturbance waves, and kinetic shove resolution.

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::{HexSpace, HexWfcWorld};
use observed_hex::HexCoord;

use super::sim::{Observer, ObserverId, ObserverState};

/// Maximum kinetic tool charge capacity per Observer.
pub const MAX_CHARGE: u32 = 100;

/// Charge consumed by one kinetic shove.
pub const SHOVE_COST: u32 = 25;

/// Charge restored per actor beat at a powered recharge station.
pub const RECHARGE_PER_BEAT: u32 = 25;

/// State for the cell-level economy on the facility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EconomyState {
    /// Finite charge pool per Observer.
    pub charges: BTreeMap<ObserverId, u32>,
    /// Station coordinates providing recharge when their floor has power.
    pub stations: BTreeSet<HexCoord>,
    /// Single contested generator coordinate per floor.
    pub generators: BTreeMap<u8, HexCoord>,
    /// Power state per floor level: `true` = powered, `false` = unpowered.
    pub power: BTreeMap<u8, bool>,
}

impl EconomyState {
    /// Initialize fresh economy state for a generated world and observer set.
    #[must_use]
    pub fn new(
        world: &HexWfcWorld,
        observers: &BTreeMap<ObserverId, Observer>,
        _seed: u64,
    ) -> Self {
        let mut charges = BTreeMap::new();
        for id in observers.keys() {
            charges.insert(*id, MAX_CHARGE);
        }

        let mut stations = BTreeSet::new();
        let mut generators = BTreeMap::new();
        let mut power = BTreeMap::new();

        for level in 0..world.config.levels {
            power.insert(level, true);

            let mut candidates: Vec<HexCoord> = world
                .placements
                .iter()
                .filter(|(coord, placement)| {
                    coord.level == level && placement.space != HexSpace::Void
                })
                .map(|(coord, _)| *coord)
                .collect();
            candidates.sort();

            if !candidates.is_empty() {
                generators.insert(level, candidates[0]);
                let station_idx = if candidates.len() > 1 {
                    candidates.len() / 2
                } else {
                    0
                };
                stations.insert(candidates[station_idx]);
            }
        }

        Self {
            charges,
            stations,
            generators,
            power,
        }
    }

    /// Check if a floor level is currently powered.
    #[must_use]
    pub fn is_powered(&self, level: u8) -> bool {
        self.power.get(&level).copied().unwrap_or(true)
    }

    /// Set power state on a given floor level.
    pub fn set_powered(&mut self, level: u8, powered: bool) {
        self.power.insert(level, powered);
    }

    /// Query the current charge for an Observer.
    #[must_use]
    pub fn charge(&self, id: ObserverId) -> u32 {
        self.charges.get(&id).copied().unwrap_or(0)
    }

    /// Set charge directly for an Observer, capped at [`MAX_CHARGE`].
    pub fn set_charge(&mut self, id: ObserverId, amount: u32) {
        self.charges.insert(id, amount.min(MAX_CHARGE));
    }

    /// Attempt to spend charge. Returns `true` and deducts if available; returns `false` otherwise.
    pub fn spend_charge(&mut self, id: ObserverId, amount: u32) -> bool {
        let current = self.charge(id);
        if current >= amount {
            self.charges.insert(id, current - amount);
            true
        } else {
            false
        }
    }

    /// Restore charge for an Observer, capped at [`MAX_CHARGE`].
    pub fn recharge_observer(&mut self, id: ObserverId, amount: u32) {
        let current = self.charge(id);
        self.charges.insert(id, (current + amount).min(MAX_CHARGE));
    }

    /// Check if a coordinate is at a recharge station whose floor currently has power.
    #[must_use]
    pub fn is_at_powered_station(&self, cell: HexCoord) -> bool {
        self.stations.contains(&cell) && self.is_powered(cell.level)
    }

    /// Process per-beat recharge: only active Observers at a powered station regain charge.
    pub fn tick_beat(&mut self, _world: &HexWfcWorld, observers: &BTreeMap<ObserverId, Observer>) {
        for (id, observer) in observers {
            if observer.state == ObserverState::Active && self.is_at_powered_station(observer.cell)
            {
                self.recharge_observer(*id, RECHARGE_PER_BEAT);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{ArchitectLab, ArchitectMode};

    #[test]
    fn observers_start_with_finite_max_charge() {
        let lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        assert!(!lab.observers.is_empty(), "pocket mode has observers");
        for id in lab.observers.keys() {
            assert_eq!(
                lab.economy.charge(*id),
                MAX_CHARGE,
                "observer {id:?} starts at MAX_CHARGE"
            );
        }
    }

    #[test]
    fn charge_restored_only_at_powered_station_and_never_passively() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let id = *lab.observers.keys().next().expect("an observer exists");

        // Deplete charge.
        lab.economy.set_charge(id, 0);
        assert_eq!(lab.economy.charge(id), 0);

        // Move away from any station.
        let station_cell = *lab.economy.stations.iter().next().expect("station exists");
        let non_station = lab
            .world
            .placements
            .keys()
            .copied()
            .find(|c| *c != station_cell && c.level == station_cell.level)
            .expect("non-station cell");

        lab.observers.get_mut(&id).unwrap().cell = non_station;

        // Run an actor beat while away from station.
        lab.step_beat();
        assert_eq!(
            lab.economy.charge(id),
            0,
            "charge must not regenerate away from a station (no passive recharge)"
        );

        // Hold at summit exit where observer holds.
        let summit = lab.world.config.exit();
        lab.economy.stations.insert(summit);
        lab.observers.get_mut(&id).unwrap().cell = summit;

        // Station is unpowered initially.
        lab.economy.set_powered(summit.level, false);
        lab.step_beat();
        assert_eq!(
            lab.economy.charge(id),
            0,
            "station on unpowered floor must supply 0 charge"
        );

        // Power the floor.
        lab.economy.set_powered(summit.level, true);
        lab.step_beat();
        assert_eq!(
            lab.economy.charge(id),
            RECHARGE_PER_BEAT,
            "powered station must restore charge per beat"
        );

        // Continue until full.
        for _ in 0..10 {
            lab.step_beat();
        }
        assert_eq!(
            lab.economy.charge(id),
            MAX_CHARGE,
            "charge must cap at MAX_CHARGE"
        );
    }

    #[test]
    fn card_play_alone_never_restores_charge() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let id = *lab.observers.keys().next().expect("an observer exists");

        lab.economy.set_charge(id, 40);

        // Place observer away from station.
        let non_station = lab
            .world
            .placements
            .keys()
            .copied()
            .find(|c| !lab.economy.stations.contains(c))
            .expect("non-station cell");
        lab.observers.get_mut(&id).unwrap().cell = non_station;

        // Play a legal card.
        let commands = lab.legal_commands();
        assert!(!commands.is_empty(), "legal commands exist");
        let command = commands[0];
        let result = lab.submit(command);
        assert!(result.is_ok(), "legal command succeeded");

        assert_eq!(
            lab.economy.charge(id),
            40,
            "card play alone must NEVER restore charge"
        );
    }

    #[test]
    fn per_observer_charges_are_independent() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("full ascent solves");
        let ids: Vec<_> = lab.observers.keys().copied().collect();
        assert!(ids.len() >= 2, "full ascent has multiple observers");

        lab.economy.set_charge(ids[0], 10);
        lab.economy.set_charge(ids[1], 80);

        assert_eq!(lab.economy.charge(ids[0]), 10);
        assert_eq!(lab.economy.charge(ids[1]), 80);

        assert!(lab.economy.spend_charge(ids[1], SHOVE_COST));
        assert_eq!(lab.economy.charge(ids[1]), 80 - SHOVE_COST);
        assert_eq!(
            lab.economy.charge(ids[0]),
            10,
            "spending charge on observer 1 must not alter observer 0"
        );
    }
}
