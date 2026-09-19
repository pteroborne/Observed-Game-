//! Cell-level economy for the Architect Ascent: charge pools, powered stations,
//! power states, disturbance waves, and kinetic shove resolution.

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::{HexSpace, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, ports_compatible, travel_distance};

use super::sim::{
    ArchitectLab, DoorState, Guardian, GuardianId, LabEventKind, Observer, ObserverId,
    ObserverState, PowerPolicy, ThresholdKey,
};

/// Maximum kinetic tool charge capacity per Observer.
pub const MAX_CHARGE: u32 = 100;

/// Charge consumed by one kinetic shove.
pub const SHOVE_COST: u32 = 25;

/// Charge restored per actor beat at a powered recharge station.
pub const RECHARGE_PER_BEAT: u32 = 25;

/// Disturbance threshold required to release a wave of minor Guardians.
pub const DISTURBANCE_THRESHOLD: u32 = 100;

/// Disturbance added by an accepted, locally consistent loyal placement.
pub const DISTURBANCE_CONSISTENT_LOYAL: u32 = 5;

/// Disturbance added by an accepted, locally consistent Rogue placement.
pub const DISTURBANCE_CONSISTENT_ROGUE: u32 = 10;

/// Disturbance added by a loyal placement that introduces a contradiction.
pub const DISTURBANCE_CONTRADICTION_LOYAL: u32 = 35;

/// Disturbance added by a Rogue placement that introduces a contradiction.
pub const DISTURBANCE_CONTRADICTION_ROGUE: u32 = 50;

/// Disturbance added each time an implicated tile commits retraction to void.
pub const DISTURBANCE_RETRACTION_COMMITTED: u32 = 25;

/// Disturbance decay per actor beat.
pub const DISTURBANCE_DECAY_PER_BEAT: u32 = 2;

/// Disturbance decay floor per floor level (level * DECAY_FLOOR_PER_LEVEL).
pub const DISTURBANCE_DECAY_FLOOR_PER_LEVEL: u32 = 15;

/// State for the cell-level economy on the facility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EconomyState {
    /// Finite charge pool per Observer.
    pub charges: BTreeMap<ObserverId, u32>,
    /// Station coordinates providing recharge when their floor has power.
    pub stations: BTreeSet<HexCoord>,
    /// Single contested generator coordinate per floor.
    pub generators: BTreeMap<u8, HexCoord>,
    /// Teleport and equipment pads per floor.
    pub pads: BTreeSet<HexCoord>,
    /// Power state per floor level: `true` = powered, `false` = unpowered.
    pub power: BTreeMap<u8, bool>,
    /// Disturbance meter per floor level.
    pub disturbance: BTreeMap<u8, u32>,
    /// Number of waves released per floor level.
    pub wave_counts: BTreeMap<u8, u32>,
    /// Counter for allocating unique Minor Guardian IDs deterministically.
    pub next_minor_guardian_id: u16,
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
        let mut pads = BTreeSet::new();
        let mut power = BTreeMap::new();
        let mut disturbance = BTreeMap::new();
        let mut wave_counts = BTreeMap::new();

        for level in 0..world.config.levels {
            power.insert(level, true);
            disturbance.insert(level, (level as u32) * DISTURBANCE_DECAY_FLOOR_PER_LEVEL);
            wave_counts.insert(level, 0);

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
                let pad_idx = candidates.len().saturating_sub(1);
                pads.insert(candidates[pad_idx]);
            }
        }

        Self {
            charges,
            stations,
            generators,
            pads,
            power,
            disturbance,
            wave_counts,
            next_minor_guardian_id: 1000,
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

    /// Check if a coordinate is at an active teleport pad whose floor has power.
    #[must_use]
    pub fn is_pad_active(&self, cell: HexCoord) -> bool {
        self.pads.contains(&cell) && self.is_powered(cell.level)
    }

    /// Check if ascent functions at the given coordinate (requires floor power).
    #[must_use]
    pub fn is_ascent_active(&self, cell: HexCoord) -> bool {
        self.is_powered(cell.level)
    }

    /// Toggle power on a floor level. Returns the new power state.
    pub fn toggle_power(&mut self, level: u8) -> bool {
        let current = self.is_powered(level);
        let next = !current;
        self.power.insert(level, next);
        next
    }

    /// Check if a coordinate is the generator for its floor.
    #[must_use]
    pub fn is_at_generator(&self, cell: HexCoord) -> bool {
        self.generators.get(&cell.level) == Some(&cell)
    }

    /// Query the disturbance meter on a given floor level.
    #[must_use]
    pub fn disturbance(&self, level: u8) -> u32 {
        self.disturbance.get(&level).copied().unwrap_or(0)
    }

    /// Set disturbance on a floor level directly.
    pub fn set_disturbance(&mut self, level: u8, amount: u32) {
        self.disturbance.insert(level, amount);
    }

    /// Add disturbance to a floor level.
    pub fn add_disturbance(&mut self, level: u8, amount: u32) {
        *self.disturbance.entry(level).or_default() += amount;
    }

    /// Query the number of waves released on a floor level.
    #[must_use]
    pub fn wave_count(&self, level: u8) -> u32 {
        self.wave_counts.get(&level).copied().unwrap_or(0)
    }

    /// Increment and return the new wave count on a floor level.
    pub fn increment_wave_count(&mut self, level: u8) -> u32 {
        let count = self.wave_counts.entry(level).or_default();
        *count += 1;
        *count
    }

    /// Consume one disturbance threshold unit from a floor level.
    pub fn consume_disturbance_threshold(&mut self, level: u8) {
        if let Some(dist) = self.disturbance.get_mut(&level) {
            *dist = dist.saturating_sub(DISTURBANCE_THRESHOLD);
        }
    }

    /// Allocate the next unique Minor Guardian ID deterministically.
    pub fn alloc_minor_id(&mut self) -> u16 {
        let id = self.next_minor_guardian_id;
        self.next_minor_guardian_id = self.next_minor_guardian_id.wrapping_add(1);
        id
    }

    /// Record disturbance contribution from a played card.
    pub fn on_card_played(&mut self, level: u8, is_contradiction: bool, is_rogue: bool) {
        let amount = match (is_contradiction, is_rogue) {
            (true, true) => DISTURBANCE_CONTRADICTION_ROGUE,
            (true, false) => DISTURBANCE_CONTRADICTION_LOYAL,
            (false, true) => DISTURBANCE_CONSISTENT_ROGUE,
            (false, false) => DISTURBANCE_CONSISTENT_LOYAL,
        };
        self.add_disturbance(level, amount);
    }

    /// Record disturbance contribution from a committed tile retraction.
    pub fn on_retraction_committed(&mut self, level: u8) {
        self.add_disturbance(level, DISTURBANCE_RETRACTION_COMMITTED);
    }

    /// Process per-beat recharge and disturbance decay down to height floors.
    pub fn tick_beat(&mut self, _world: &HexWfcWorld, observers: &BTreeMap<ObserverId, Observer>) {
        for (id, observer) in observers {
            if observer.state == ObserverState::Active && self.is_at_powered_station(observer.cell)
            {
                self.recharge_observer(*id, RECHARGE_PER_BEAT);
            }
        }
        for (level, dist) in &mut self.disturbance {
            let floor_min = (*level as u32) * DISTURBANCE_DECAY_FLOOR_PER_LEVEL;
            if *dist > floor_min {
                *dist = (*dist)
                    .saturating_sub(DISTURBANCE_DECAY_PER_BEAT)
                    .max(floor_min);
            }
        }
    }
}

/// Category of Guardian in the facility: Major (original frozen-by-observation entity)
/// or Minor (facility defense swarm spawned by disturbance, immune to freezing).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GuardianKind {
    Major,
    Minor,
}

/// Outcome of a kinetic shove directed at a Minor Guardian.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShoveOutcome {
    CommittedToVoid {
        target: GuardianId,
        from: HexCoord,
    },
    CommittedToRetraction {
        target: GuardianId,
        from: HexCoord,
        destination: HexCoord,
    },
    Displaced {
        target: GuardianId,
        from: HexCoord,
        destination: HexCoord,
    },
    BlockedByWall {
        target: GuardianId,
        cell: HexCoord,
    },
}

/// Reasons a shove cannot be performed or has no effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShoveError {
    InsufficientCharge,
    TargetNotFound,
    TargetNotMinor,
    TargetNotAdjacent,
    ObserverNotActive,
    TargetNotDetected,
    NoEffectOnObservers,
}

impl ArchitectLab {
    /// Verify whether an Observer at `from` can detect an adjacent actor at `to`.
    #[must_use]
    pub fn can_detect_adjacent(&self, from: HexCoord, to: HexCoord) -> bool {
        let Some(face) = HexFace::ALL
            .into_iter()
            .find(|&f| self.world.config.grid().neighbor(from, f) == Some(to))
        else {
            return false;
        };
        if self
            .threshold_key(from, face)
            .and_then(|k| self.doors.get(&k))
            == Some(&DoorState::Closed)
        {
            return false;
        }
        let Some(p_from) = self.world.placements.get(&from) else {
            return false;
        };
        let Some(p_to) = self.world.placements.get(&to) else {
            return false;
        };
        if p_from.space == HexSpace::Void || p_to.space == HexSpace::Void {
            return false;
        }
        if face.is_lateral() {
            p_from.is_open(face) && p_to.is_open(face.opposite())
        } else {
            ports_compatible(
                p_from.ports().port(face),
                p_to.ports().port(face.opposite()),
            )
        }
    }

    /// Check whether an Observer can legally shove a Minor Guardian.
    #[must_use]
    pub fn can_shove(&self, observer_id: ObserverId, target_id: GuardianId) -> bool {
        let Some(observer) = self.observers.get(&observer_id) else {
            return false;
        };
        if observer.state != ObserverState::Active {
            return false;
        }
        let Some(guardian) = self.guardians.get(&target_id) else {
            return false;
        };
        if guardian.kind != GuardianKind::Minor {
            return false;
        }
        let observer_cell = observer.cell;
        let guardian_cell = guardian.cell;
        if travel_distance(observer_cell, guardian_cell) != 1
            || observer_cell.level != guardian_cell.level
        {
            return false;
        }
        if !self.observed.contains(&guardian_cell)
            && !self.can_detect_adjacent(observer_cell, guardian_cell)
        {
            return false;
        }
        self.economy.charge(observer_id) >= SHOVE_COST
    }

    /// Check if a target coordinate is at or adjacent to the floor's generator room.
    #[must_use]
    pub fn is_contesting_generator(&self, target: HexCoord) -> bool {
        self.economy
            .generators
            .get(&target.level)
            .copied()
            .is_some_and(|gen_cell| travel_distance(target, gen_cell) <= 1)
    }

    /// Execute a fixed-tick kinetic shove from an Observer directed at a Minor Guardian.
    /// Deals no damage to any actor. Consumes [`SHOVE_COST`] charge on success.
    pub fn shove(
        &mut self,
        observer_id: ObserverId,
        target_id: GuardianId,
    ) -> Result<ShoveOutcome, ShoveError> {
        let observer = self
            .observers
            .get(&observer_id)
            .ok_or(ShoveError::ObserverNotActive)?;
        if observer.state != ObserverState::Active {
            return Err(ShoveError::ObserverNotActive);
        }

        let guardian = self
            .guardians
            .get(&target_id)
            .ok_or(ShoveError::TargetNotFound)?;
        if guardian.kind != GuardianKind::Minor {
            return Err(ShoveError::TargetNotMinor);
        }

        let observer_cell = observer.cell;
        let guardian_cell = guardian.cell;

        if travel_distance(observer_cell, guardian_cell) != 1
            || observer_cell.level != guardian_cell.level
        {
            return Err(ShoveError::TargetNotAdjacent);
        }

        if !self.observed.contains(&guardian_cell)
            && !self.can_detect_adjacent(observer_cell, guardian_cell)
        {
            return Err(ShoveError::TargetNotDetected);
        }

        if !self.economy.spend_charge(observer_id, SHOVE_COST) {
            return Err(ShoveError::InsufficientCharge);
        }

        let face = HexFace::LATERAL
            .into_iter()
            .find(|&f| self.world.config.grid().neighbor(observer_cell, f) == Some(guardian_cell))
            .expect("adjacent same level lateral");

        // Direction of shove impulse continues along the same face from the Guardian.
        let dest = self.world.config.grid().neighbor(guardian_cell, face);

        let guardian_tile = self.world.placements.get(&guardian_cell);
        let blocked = match guardian_tile {
            Some(tile) if tile.space != HexSpace::Void => {
                !tile.is_open(face)
                    || self
                        .threshold_key(guardian_cell, face)
                        .and_then(|k| self.doors.get(&k))
                        == Some(&DoorState::Closed)
            }
            _ => false,
        };

        if blocked {
            return Ok(ShoveOutcome::BlockedByWall {
                target: target_id,
                cell: guardian_cell,
            });
        }

        let is_void = match dest {
            None => true,
            Some(c) => match self.world.placements.get(&c) {
                None => true,
                Some(tile) => tile.space == HexSpace::Void,
            },
        };

        if is_void {
            self.guardians.remove(&target_id);
            self.record_event(
                LabEventKind::Retracted,
                dest,
                "Minor Guardian shoved into void.",
            );
            return Ok(ShoveOutcome::CommittedToVoid {
                target: target_id,
                from: guardian_cell,
            });
        }

        let dest_cell = dest.expect("dest is some");

        if self.retracted.contains(&dest_cell) || self.contradictions.contains(&dest_cell) {
            self.guardians.remove(&target_id);
            self.record_event(
                LabEventKind::Retracted,
                Some(dest_cell),
                "Minor Guardian shoved into retracting cell.",
            );
            return Ok(ShoveOutcome::CommittedToRetraction {
                target: target_id,
                from: guardian_cell,
                destination: dest_cell,
            });
        }

        if let Some(g) = self.guardians.get_mut(&target_id) {
            g.cell = dest_cell;
        }
        Ok(ShoveOutcome::Displaced {
            target: target_id,
            from: guardian_cell,
            destination: dest_cell,
        })
    }

    /// The kinetic tool has no effect on rival Observers and deals no damage.
    pub fn shove_observer(
        &mut self,
        _observer_id: ObserverId,
        _target: ObserverId,
    ) -> Result<(), ShoveError> {
        Err(ShoveError::NoEffectOnObservers)
    }

    /// Human or bot operator operates a deployed door. Refused if the door's floor lacks power.
    pub fn operate_door(&mut self, key: ThresholdKey, state: DoorState) -> bool {
        if !self.economy.is_powered(key.cell.level) {
            return false;
        }
        self.doors.insert(key, state);
        self.refresh_observation();
        true
    }

    /// Observer at generator room toggles floor power state.
    pub fn toggle_generator(&mut self, observer_id: ObserverId) -> Result<bool, &'static str> {
        let observer = self
            .observers
            .get(&observer_id)
            .ok_or("observer not active")?;
        if observer.state != ObserverState::Active {
            return Err("observer not active");
        }
        if !self.economy.is_at_generator(observer.cell) {
            return Err("observer not at generator");
        }
        let is_powered = self.economy.is_powered(observer.cell.level);
        if !is_powered && self.power_policy == PowerPolicy::OneWay {
            return Err("power cannot be restored under OneWay policy");
        }
        if is_powered && self.power_policy == PowerPolicy::AlwaysOn {
            return Err("power cannot be cut under AlwaysOn policy");
        }
        let new_state = self.economy.toggle_power(observer.cell.level);
        self.refresh_observation();
        Ok(new_state)
    }

    /// Cut floor power directly (e.g. via contested generator play or floor collapse).
    pub fn cut_floor_power(&mut self, level: u8) -> bool {
        if self.power_policy == PowerPolicy::AlwaysOn {
            return false;
        }
        if self.economy.is_powered(level) {
            self.economy.power.insert(level, false);
            self.refresh_observation();
            true
        } else {
            false
        }
    }

    /// Resolve any pending disturbance waves on a floor level.
    pub fn resolve_disturbance_waves(&mut self, level: u8) -> Vec<GuardianId> {
        if self.collapsed_floors.contains(&level) {
            return Vec::new();
        }
        let mut spawned = Vec::new();
        while self.economy.disturbance(level) >= DISTURBANCE_THRESHOLD {
            self.economy.consume_disturbance_threshold(level);
            let wave_num = self.economy.increment_wave_count(level);
            let wave_size = 1 + (level as usize);

            let mut candidates: Vec<HexCoord> = self
                .world
                .placements
                .iter()
                .filter(|(coord, placement)| {
                    coord.level == level
                        && placement.space != HexSpace::Void
                        && !self.prison_core.contains(coord)
                        && !self.guardians.values().any(|g| g.cell == **coord)
                })
                .map(|(coord, _)| *coord)
                .collect();
            candidates.sort();

            if candidates.is_empty() {
                candidates = self
                    .world
                    .placements
                    .iter()
                    .filter(|(coord, placement)| {
                        coord.level == level
                            && placement.space != HexSpace::Void
                            && !self.prison_core.contains(coord)
                    })
                    .map(|(coord, _)| *coord)
                    .collect();
                candidates.sort();
            }

            if candidates.is_empty() {
                break;
            }

            for i in 0..wave_size {
                let idx =
                    ((wave_num as usize).wrapping_mul(17) + i.wrapping_mul(7)) % candidates.len();
                let spawn_cell = candidates[idx];
                let minor_id = GuardianId(self.economy.alloc_minor_id());
                self.guardians.insert(
                    minor_id,
                    Guardian {
                        id: minor_id,
                        cell: spawn_cell,
                        last_detection: None,
                        kind: GuardianKind::Minor,
                    },
                );
                self.record_event(
                    LabEventKind::Warning,
                    Some(spawn_cell),
                    "Disturbance threshold crossed: minor Guardian wave released.",
                );
                spawned.push(minor_id);
            }
        }
        if !spawned.is_empty() {
            self.refresh_observation();
        }
        spawned
    }

    /// Add disturbance to a floor level and resolve any resulting waves.
    pub fn add_disturbance(&mut self, level: u8, amount: u32) -> Vec<GuardianId> {
        self.economy.add_disturbance(level, amount);
        self.resolve_disturbance_waves(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{
        ArchitectLab, ArchitectMode, Guardian, GuardianId, GuardianIntent, ObserverId,
    };

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
        if let Some(other) = lab.observers.get_mut(&ObserverId(1)) {
            other.cell = non_station;
        }

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

    #[test]
    fn minor_guardians_never_frozen_by_observation_while_major_are() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let observer_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&observer_id].cell;
        let observed_cell = lab
            .observed
            .iter()
            .copied()
            .find(|&c| c != obs_cell)
            .expect("has observed adjacent cell");

        // Major Guardian placed here: must freeze.
        let major_id = GuardianId(101);
        lab.guardians.insert(
            major_id,
            Guardian {
                id: major_id,
                cell: observed_cell,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        );
        let (major_intent, major_trace) = lab.guardian_intent(major_id);
        assert_eq!(major_intent, GuardianIntent::Hold);
        assert_eq!(major_trace.selected, Some("frozen while observed"));

        // Minor Guardian placed in the exact same spot: must NOT freeze.
        let minor_id = GuardianId(102);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: observed_cell,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );
        let (minor_intent, minor_trace) = lab.guardian_intent(minor_id);
        assert_ne!(minor_trace.selected, Some("frozen while observed"));
        assert_ne!(minor_intent, GuardianIntent::Hold);
    }

    #[test]
    fn minor_guardians_never_accept_rogue_directive() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let (start_cell, target_cell) = lab
            .world
            .placements
            .keys()
            .copied()
            .find_map(|c| {
                let exits = lab.exits(c);
                exits.first().map(|&next| (c, next))
            })
            .expect("two connected cells exist");
        lab.rogue_directive = Some(target_cell);

        let major_id = GuardianId(201);
        let minor_id = GuardianId(202);

        lab.guardians.insert(
            major_id,
            Guardian {
                id: major_id,
                cell: start_cell,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        );
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: start_cell,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );

        let (_, major_trace) = lab.guardian_intent(major_id);
        let (_, minor_trace) = lab.guardian_intent(minor_id);

        assert_eq!(
            major_trace.selected,
            Some("obey Rogue directive"),
            "Major Guardian obeys Rogue directive"
        );
        assert_ne!(
            minor_trace.selected,
            Some("obey Rogue directive"),
            "Minor Guardian must never obey Rogue directive"
        );
    }

    #[test]
    fn minor_guardians_never_enter_prison_core() {
        let lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        assert!(!lab.prison_core.is_empty(), "prison core exists");

        // Check exits_minor from any cell neighboring prison_core
        for &prison_cell in &lab.prison_core {
            for neighbor in lab.exits(prison_cell) {
                let minor_exits = lab.exits_minor(neighbor);
                assert!(
                    !minor_exits.contains(&prison_cell),
                    "minor exits must never contain prison core cell"
                );
            }
        }

        // Check route_minor to any prison core cell returns None
        for &prison_cell in &lab.prison_core {
            let outside = lab
                .world
                .placements
                .keys()
                .copied()
                .find(|c| {
                    !lab.prison_core.contains(c) && lab.world.placements[c].space != HexSpace::Void
                })
                .expect("cell outside prison");
            assert_eq!(
                lab.route_minor(outside, prison_cell),
                None,
                "Minor Guardian pathfinding to prison core must be refused"
            );
        }
    }

    #[test]
    fn minor_guardians_removed_with_permanently_collapsed_floor() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
        let command = lab
            .legal_commands()
            .into_iter()
            .find(|&cmd| {
                let mut candidate = lab.clone();
                candidate.submit(cmd).unwrap();
                !candidate.contradictions.is_empty() && candidate.next_retraction().is_some()
            })
            .expect("scenario admits a locally fitting, globally contradicted card");
        lab.submit(command).unwrap();

        let target = lab.next_retraction().unwrap();
        for (&cell, tile) in &mut lab.world.placements {
            if cell.level == target.level && cell != target && !lab.prison_core.contains(&cell) {
                tile.space = HexSpace::Void;
                tile.doors = 0;
            }
        }
        lab.observed.clear();
        lab.anchored.clear();
        lab.guardians.clear();
        lab.observers.clear();
        lab.doors.clear();
        lab.contradictions = BTreeSet::from([target]);

        let floor_cell = lab
            .prison_core
            .iter()
            .copied()
            .find(|c| c.level == target.level)
            .unwrap_or(target);

        let minor_id = GuardianId(301);
        let major_id = GuardianId(302);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: floor_cell,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );
        lab.guardians.insert(
            major_id,
            Guardian {
                id: major_id,
                cell: floor_cell,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        );

        lab.tick = crate::sim::RETRACTION_TICKS;
        lab.advance_retraction();

        assert!(
            lab.collapsed_floors.contains(&target.level),
            "floor should be permanently collapsed"
        );
        assert!(
            !lab.guardians.contains_key(&minor_id),
            "minor guardian must be removed when floor permanently collapses"
        );
        assert!(
            lab.guardians.contains_key(&major_id),
            "major guardian remains when floor permanently collapses"
        );
    }

    #[test]
    fn shove_commits_adjacent_minor_to_void() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&obs_id].cell;

        let mut found = None;
        for face in HexFace::LATERAL {
            if let Some(neighbor) = lab.world.config.grid().neighbor(obs_cell, face)
                && lab.can_detect_adjacent(obs_cell, neighbor)
                && let Some(dest) = lab.world.config.grid().neighbor(neighbor, face)
                && let Some(tile) = lab.world.placements.get(&dest)
                && tile.space != HexSpace::Void
            {
                found = Some((neighbor, dest));
                break;
            }
        }

        let (minor_cell, dest_cell) = found.expect("found 3-in-a-row tiles in pocket mode");
        // Void the destination tile to create an open hole in the architecture.
        lab.world.placements.get_mut(&dest_cell).unwrap().space = HexSpace::Void;

        let minor_id = GuardianId(401);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: minor_cell,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );

        let initial_charge = lab.economy.charge(obs_id);
        assert_eq!(initial_charge, MAX_CHARGE);

        let outcome = lab.shove(obs_id, minor_id).expect("shove succeeds");
        assert_eq!(
            outcome,
            ShoveOutcome::CommittedToVoid {
                target: minor_id,
                from: minor_cell
            }
        );
        assert!(
            !lab.guardians.contains_key(&minor_id),
            "Minor Guardian committed to void is destroyed"
        );
        assert_eq!(
            lab.economy.charge(obs_id),
            initial_charge - SHOVE_COST,
            "Shove spent exactly SHOVE_COST"
        );
    }

    #[test]
    fn shove_commits_adjacent_minor_to_retracting_cell() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&obs_id].cell;

        let mut found = None;
        for face in HexFace::LATERAL {
            if let Some(neighbor) = lab.world.config.grid().neighbor(obs_cell, face)
                && lab.can_detect_adjacent(obs_cell, neighbor)
                && let Some(dest) = lab.world.config.grid().neighbor(neighbor, face)
                && let Some(tile) = lab.world.placements.get(&dest)
                && tile.space != HexSpace::Void
            {
                found = Some((neighbor, dest));
                break;
            }
        }

        let (minor_cell, dest_cell) = found.expect("found 3-in-a-row tiles in pocket mode");
        lab.contradictions.insert(dest_cell);

        let minor_id = GuardianId(501);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: minor_cell,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );

        let initial_charge = lab.economy.charge(obs_id);
        let outcome = lab.shove(obs_id, minor_id).expect("shove succeeds");
        assert_eq!(
            outcome,
            ShoveOutcome::CommittedToRetraction {
                target: minor_id,
                from: minor_cell,
                destination: dest_cell,
            }
        );
        assert!(
            !lab.guardians.contains_key(&minor_id),
            "Minor Guardian committed to retracting cell is destroyed"
        );
        assert_eq!(lab.economy.charge(obs_id), initial_charge - SHOVE_COST);
    }

    #[test]
    fn shove_refused_for_insufficient_charge_or_invalid_targets() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&obs_id].cell;

        let neighbor = lab
            .exits(obs_cell)
            .into_iter()
            .next()
            .expect("has adjacent exit");

        // Major Guardian: cannot be shoved
        let major_id = GuardianId(601);
        lab.guardians.insert(
            major_id,
            Guardian {
                id: major_id,
                cell: neighbor,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        );
        assert_eq!(
            lab.shove(obs_id, major_id),
            Err(ShoveError::TargetNotMinor),
            "Major Guardian cannot be shoved"
        );

        // Minor Guardian with insufficient charge
        let minor_id = GuardianId(602);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: neighbor,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );
        lab.economy.set_charge(obs_id, SHOVE_COST - 1);
        assert_eq!(
            lab.shove(obs_id, minor_id),
            Err(ShoveError::InsufficientCharge),
            "Insufficient charge refuses shove"
        );
        assert_eq!(
            lab.economy.charge(obs_id),
            SHOVE_COST - 1,
            "Refused shove spends zero charge"
        );
        assert!(
            lab.guardians.contains_key(&minor_id),
            "Guardian remains in place when shove is refused"
        );

        // Rival Observer: shove has no effect and deals no damage
        let rival_id = ObserverId(999);
        assert_eq!(
            lab.shove_observer(obs_id, rival_id),
            Err(ShoveError::NoEffectOnObservers),
            "Shoving rival Observer has no effect"
        );
    }

    #[test]
    fn shove_is_deterministic_and_reproducible() {
        let mut lab_a = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab_a.observers.keys().next().expect("observer exists");
        let obs_cell = lab_a.observers[&obs_id].cell;

        let neighbor = lab_a
            .exits(obs_cell)
            .into_iter()
            .next()
            .expect("adjacent neighbor");
        let minor_id = GuardianId(701);
        lab_a.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: neighbor,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );

        let mut lab_b = lab_a.clone();

        let outcome_a = lab_a.shove(obs_id, minor_id);
        let outcome_b = lab_b.shove(obs_id, minor_id);

        assert_eq!(
            outcome_a, outcome_b,
            "both shoves produce identical outcomes"
        );
        assert_eq!(
            lab_a.guardians, lab_b.guardians,
            "both labs have identical guardians after shove"
        );
        assert_eq!(
            lab_a.economy, lab_b.economy,
            "both labs have identical economy after shove"
        );
        assert_eq!(
            lab_a.observers, lab_b.observers,
            "both labs have identical observers after shove"
        );
    }

    #[test]
    fn floor_power_gates_recharge() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let station_cell = *lab.economy.stations.iter().next().expect("station exists");
        let floor = station_cell.level;

        // Move observer to station with 0 charge.
        lab.observers.get_mut(&obs_id).unwrap().cell = station_cell;
        lab.economy.set_charge(obs_id, 0);
        assert_eq!(lab.economy.charge(obs_id), 0);

        // When floor is powered, tick_beat recharges.
        lab.economy.set_powered(floor, true);
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(
            lab.economy.charge(obs_id),
            RECHARGE_PER_BEAT,
            "Powered floor recharges observer at station"
        );

        // When floor is unpowered, tick_beat supplies 0 charge.
        lab.economy.set_powered(floor, false);
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(
            lab.economy.charge(obs_id),
            RECHARGE_PER_BEAT,
            "Unpowered floor does not restore charge at station"
        );
    }

    #[test]
    fn floor_power_gates_door_operation() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let cell = *lab.world.placements.keys().next().expect("cell exists");
        let face = HexFace::East;
        let key = ThresholdKey { cell, face };

        lab.doors.insert(key, DoorState::Closed);
        let floor = cell.level;

        // When unpowered, door operation is refused and state is unchanged.
        lab.economy.set_powered(floor, false);
        let res = lab.operate_door(key, DoorState::Open);
        assert!(!res, "Door operation refused on unpowered floor");
        assert_eq!(
            lab.doors.get(&key),
            Some(&DoorState::Closed),
            "Door remains frozen in current state"
        );

        // When powered, door operation succeeds.
        lab.economy.set_powered(floor, true);
        let res = lab.operate_door(key, DoorState::Open);
        assert!(res, "Door operation succeeds on powered floor");
        assert_eq!(
            lab.doors.get(&key),
            Some(&DoorState::Open),
            "Door state transitions when powered"
        );
    }

    #[test]
    fn floor_power_gates_ascent_and_pads() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        // Verify vertical ascent passage is gated by floor power
        let (from_cell, to_cell) = lab
            .world
            .placements
            .keys()
            .find_map(|&c| {
                let exits = lab.exits(c);
                exits
                    .into_iter()
                    .find(|&next| next.level != c.level)
                    .map(|next| (c, next))
            })
            .expect("vertical exit exists in QuickClimb");

        assert!(
            lab.exits(from_cell).contains(&to_cell),
            "Vertical exit present when both floors powered"
        );

        // Cut power on from_cell's floor: ascent is inert
        lab.economy.set_powered(from_cell.level, false);
        assert!(
            !lab.exits(from_cell).contains(&to_cell),
            "Vertical exit disabled when origin floor is unpowered"
        );

        // Restore origin floor, cut destination floor: ascent is inert
        lab.economy.set_powered(from_cell.level, true);
        lab.economy.set_powered(to_cell.level, false);
        assert!(
            !lab.exits(from_cell).contains(&to_cell),
            "Vertical exit disabled when destination floor is unpowered"
        );

        // Restore both: ascent functions again
        lab.economy.set_powered(to_cell.level, true);
        assert!(
            lab.exits(from_cell).contains(&to_cell),
            "Vertical exit restored when both floors are powered"
        );

        // Test pad and ascent gating helper methods
        let pad_cell = *lab.economy.pads.iter().next().expect("pad exists");
        lab.economy.set_powered(pad_cell.level, true);
        assert!(lab.economy.is_pad_active(pad_cell));
        assert!(lab.economy.is_ascent_active(pad_cell));

        lab.economy.set_powered(pad_cell.level, false);
        assert!(
            !lab.economy.is_pad_active(pad_cell),
            "Pad inert on unpowered floor"
        );
        assert!(
            !lab.economy.is_ascent_active(pad_cell),
            "Ascent inert on unpowered floor"
        );
    }

    #[test]
    fn floor_power_gates_observation_range() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&obs_id].cell;
        let floor = obs_cell.level;

        let facing_neighbor = lab
            .exits(obs_cell)
            .into_iter()
            .find_map(|next| {
                let face = HexFace::ALL
                    .into_iter()
                    .find(|&f| lab.world.config.grid().neighbor(obs_cell, f) == Some(next))?;
                Some((face, next))
            })
            .expect("reachable neighbor in some facing");

        lab.observers.get_mut(&obs_id).unwrap().facing = facing_neighbor.0;
        let target_cell = facing_neighbor.1;

        // When powered: observation covers both observer's cell and target cell at range
        lab.economy.set_powered(floor, true);
        lab.refresh_observation();
        assert!(lab.observed.contains(&obs_cell));
        assert!(
            lab.observed.contains(&target_cell),
            "Observation extends at range when powered"
        );

        // When unpowered: observation fails at range, but observer's self-occupancy holds
        lab.economy.set_powered(floor, false);
        lab.refresh_observation();
        assert!(
            lab.observed.contains(&obs_cell),
            "Observer cell self-observed under unpowered darkness"
        );
        assert!(
            !lab.observed.contains(&target_cell),
            "Observation fails at range on unpowered floor"
        );
    }

    #[test]
    fn major_guardians_remain_frozen_under_every_power_state_permitting_sight() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let obs_cell = lab.observers[&obs_id].cell;
        let floor = obs_cell.level;

        let (facing, target_cell) = lab
            .exits(obs_cell)
            .into_iter()
            .find_map(|next| {
                let face = HexFace::ALL
                    .into_iter()
                    .find(|&f| lab.world.config.grid().neighbor(obs_cell, f) == Some(next))?;
                Some((face, next))
            })
            .expect("reachable neighbor");
        lab.observers.get_mut(&obs_id).unwrap().facing = facing;

        let major_id = GuardianId(801);
        lab.guardians.insert(
            major_id,
            Guardian {
                id: major_id,
                cell: target_cell,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        );

        // Case 1: Powered floor -> target is in range -> Major Guardian freezes
        lab.economy.set_powered(floor, true);
        lab.refresh_observation();
        let (intent_powered, trace_powered) = lab.guardian_intent(major_id);
        assert_eq!(intent_powered, GuardianIntent::Hold);
        assert_eq!(trace_powered.selected, Some("frozen while observed"));

        // Case 2: Unpowered floor -> target is out of range -> Major Guardian does NOT freeze
        lab.economy.set_powered(floor, false);
        lab.refresh_observation();
        let (_intent_dark, trace_dark) = lab.guardian_intent(major_id);
        assert_ne!(
            trace_dark.selected,
            Some("frozen while observed"),
            "Major Guardian not frozen when outside observation range in darkness"
        );

        // Case 3: Major Guardian on the observer's cell (within sight under darkness)
        lab.guardians.get_mut(&major_id).unwrap().cell = obs_cell;
        lab.refresh_observation();
        let (intent_sight, trace_sight) = lab.guardian_intent(major_id);
        assert_eq!(
            intent_sight,
            GuardianIntent::Hold,
            "Major Guardian remains frozen under darkness when sight still reaches them"
        );
        assert_eq!(
            trace_sight.selected,
            Some("frozen while observed"),
            "Major Guardian frozen while observed in darkness"
        );
    }

    #[test]
    fn generator_toggles_power_and_refreshes_observation() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let floor = 0;
        let gen_cell = lab.economy.generators[&floor];

        // Placing observer away from generator: toggle returns error
        let non_gen = lab
            .world
            .placements
            .keys()
            .copied()
            .find(|&c| c != gen_cell && lab.world.placements[&c].space != HexSpace::Void)
            .expect("non-generator cell");
        lab.observers.get_mut(&obs_id).unwrap().cell = non_gen;
        assert!(
            lab.toggle_generator(obs_id).is_err(),
            "Observer not at generator cannot toggle power"
        );

        // Placing observer at generator: toggle flips power
        lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
        assert!(lab.economy.is_powered(floor), "Initially powered");

        let new_state = lab.toggle_generator(obs_id).expect("toggle succeeds");
        assert!(!new_state, "Toggled off");
        assert!(!lab.economy.is_powered(floor), "Floor is now unpowered");

        let toggled_again = lab.toggle_generator(obs_id).expect("toggle succeeds");
        assert!(toggled_again, "Toggled back on");
        assert!(lab.economy.is_powered(floor), "Floor is re-powered");
    }

    #[test]
    fn power_policy_enforcement_and_reset_invariance() {
        // 1. AlwaysOn policy: cut_floor_power is ignored and toggle_generator refuses to cut power
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
            .expect("pocket solves")
            .with_power_policy(PowerPolicy::AlwaysOn);
        let obs_id = *lab.observers.keys().next().expect("observer exists");
        let gen_cell = lab.economy.generators[&0];
        lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
        assert!(!lab.cut_floor_power(0));
        assert!(lab.economy.is_powered(0));
        assert!(lab.toggle_generator(obs_id).is_err());
        assert!(lab.economy.is_powered(0));

        // 2. OneWay policy: power can be cut, but toggle_generator cannot restore it
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
            .expect("pocket solves")
            .with_power_policy(PowerPolicy::OneWay);
        lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
        assert!(lab.cut_floor_power(0));
        assert!(!lab.economy.is_powered(0));
        assert!(lab.toggle_generator(obs_id).is_err());
        assert!(!lab.economy.is_powered(0));
        let (intent, trace) = lab.observer_intent(obs_id);
        assert_ne!(intent, crate::sim::ObserverIntent::ToggleGenerator);
        assert_ne!(trace.selected, Some("restore floor power at generator"));

        // 3. Restorable policy: power can be cut and restored
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
            .expect("pocket solves")
            .with_power_policy(PowerPolicy::Restorable);
        lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
        assert!(lab.cut_floor_power(0));
        assert!(!lab.economy.is_powered(0));
        lab.guardians.clear();
        let (intent, trace) = lab.observer_intent(obs_id);
        assert_eq!(intent, crate::sim::ObserverIntent::ToggleGenerator);
        assert_eq!(trace.selected, Some("restore floor power at generator"));
        assert!(lab.toggle_generator(obs_id).is_ok());
        assert!(lab.economy.is_powered(0));

        // Reset Path 1: desktop.rs (LabSession::reset and cycle_mode)
        #[cfg(feature = "desktop")]
        {
            let mut session = crate::desktop::LabSession::default();
            session.set_power_policy(PowerPolicy::OneWay);
            session.reset();
            assert_eq!(session.sim.power_policy, PowerPolicy::OneWay);
        }

        // Reset Path 2: view.rs (MapCameraState::reset_for_mode)
        #[cfg(feature = "desktop")]
        {
            let mut camera = crate::view::MapCameraState::default();
            camera.zoom = 2.5;
            camera.reset_for_mode(ArchitectMode::Pocket);
            assert!((camera.zoom - 0.62).abs() < f32::EPSILON);
        }

        // Reset Path 3: web.rs (RogueGame::reset)
        #[cfg(feature = "web")]
        {
            let mut game = crate::web::RogueGame::new(0).unwrap();
            game.set_power_policy("one_way").unwrap();
            game.reset(0).unwrap();
            assert_eq!(game.power_policy(), "One-Way");
        }
    }

    #[test]
    fn disturbance_rises_with_uneven_contributions_dominated_by_contradictions_and_retractions() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let floor = 0;
        assert_eq!(lab.economy.disturbance(floor), 0);

        // 1. Consistent loyal play: +5
        lab.economy.on_card_played(floor, false, false);
        assert_eq!(lab.economy.disturbance(floor), DISTURBANCE_CONSISTENT_LOYAL);

        // 2. Consistent rogue play: +10
        lab.economy.on_card_played(floor, false, true);
        assert_eq!(
            lab.economy.disturbance(floor),
            DISTURBANCE_CONSISTENT_LOYAL + DISTURBANCE_CONSISTENT_ROGUE
        );

        // 3. Contradiction loyal play: +35 (sharp spike)
        let before_contra = lab.economy.disturbance(floor);
        lab.economy.on_card_played(floor, true, false);
        assert_eq!(
            lab.economy.disturbance(floor) - before_contra,
            DISTURBANCE_CONTRADICTION_LOYAL
        );

        // 4. Contradiction rogue play: +50 (heaviest play spike)
        let before_rogue = lab.economy.disturbance(floor);
        lab.economy.on_card_played(floor, true, true);
        assert_eq!(
            lab.economy.disturbance(floor) - before_rogue,
            DISTURBANCE_CONTRADICTION_ROGUE
        );

        // 5. Committed retraction: +25 (sharp spike)
        let before_retract = lab.economy.disturbance(floor);
        lab.economy.on_retraction_committed(floor);
        assert_eq!(
            lab.economy.disturbance(floor) - before_retract,
            DISTURBANCE_RETRACTION_COMMITTED
        );

        // Verify domination: contradiction and retraction far outweigh consistent
        const {
            assert!(DISTURBANCE_CONTRADICTION_LOYAL >= DISTURBANCE_CONSISTENT_LOYAL * 7);
            assert!(DISTURBANCE_RETRACTION_COMMITTED >= DISTURBANCE_CONSISTENT_LOYAL * 5);
        }
    }

    #[test]
    fn disturbance_decays_per_beat_down_to_height_scaled_floor() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        // QuickClimb has floor 0 and floor 1
        assert_eq!(lab.economy.disturbance(0), 0);
        assert_eq!(
            lab.economy.disturbance(1),
            DISTURBANCE_DECAY_FLOOR_PER_LEVEL,
            "Floor 1 decay floor is 15"
        );

        lab.economy.set_disturbance(0, 6);
        lab.economy.set_disturbance(1, 21);

        // Beat 1: floor 0 drops 6 -> 4, floor 1 drops 21 -> 19
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(lab.economy.disturbance(0), 4);
        assert_eq!(lab.economy.disturbance(1), 19);

        // Beat 2: floor 0 drops 4 -> 2, floor 1 drops 19 -> 17
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(lab.economy.disturbance(0), 2);
        assert_eq!(lab.economy.disturbance(1), 17);

        // Beat 3: floor 0 drops 2 -> 0, floor 1 drops 17 -> 15 (hitting floor minimum)
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(lab.economy.disturbance(0), 0);
        assert_eq!(lab.economy.disturbance(1), 15);

        // Beat 4: decay does NOT reduce below floor minimum
        lab.economy.tick_beat(&lab.world, &lab.observers);
        assert_eq!(lab.economy.disturbance(0), 0);
        assert_eq!(lab.economy.disturbance(1), 15);
    }

    #[test]
    fn disturbance_threshold_release_and_subtraction() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let floor = 0;
        let initial_guardians = lab.guardians.len();

        // Adding disturbance below threshold releases no waves
        let spawned = lab.add_disturbance(floor, 95);
        assert!(spawned.is_empty());
        assert_eq!(lab.guardians.len(), initial_guardians);
        assert_eq!(lab.economy.disturbance(floor), 95);
        assert_eq!(lab.economy.wave_count(floor), 0);

        // Adding 10 crosses 100 threshold (95 + 10 = 105)
        let spawned = lab.add_disturbance(floor, 10);
        assert_eq!(spawned.len(), 1, "Floor 0 wave size is 1");
        assert_eq!(lab.economy.disturbance(floor), 5, "105 - 100 = 5");
        assert_eq!(lab.economy.wave_count(floor), 1);
        let minor = &lab.guardians[&spawned[0]];
        assert_eq!(minor.kind, GuardianKind::Minor);

        // Adding 200 crosses threshold twice (5 + 200 = 205 -> releases 2 waves)
        let spawned_multi = lab.add_disturbance(floor, 200);
        assert_eq!(
            spawned_multi.len(),
            2,
            "Releases 2 waves of 1 minor Guardian each"
        );
        assert_eq!(lab.economy.disturbance(floor), 5, "205 - 200 = 5");
        assert_eq!(lab.economy.wave_count(floor), 3);
    }

    #[test]
    fn disturbance_waves_are_deterministic_and_reproducible() {
        let mut lab_a =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        let mut lab_b = lab_a.clone();

        let spawned_a_0 = lab_a.add_disturbance(0, 150);
        let spawned_a_1 = lab_a.add_disturbance(1, 120);

        let spawned_b_0 = lab_b.add_disturbance(0, 150);
        let spawned_b_1 = lab_b.add_disturbance(1, 120);

        assert_eq!(spawned_a_0, spawned_b_0, "spawned IDs match on floor 0");
        assert_eq!(spawned_a_1, spawned_b_1, "spawned IDs match on floor 1");
        assert_eq!(
            lab_a.guardians, lab_b.guardians,
            "guardians map matches exactly"
        );
        assert_eq!(
            lab_a.economy, lab_b.economy,
            "economy state matches exactly"
        );
    }

    #[test]
    fn disturbance_waves_respect_collapsed_floors_and_never_enter_prison() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        let floor = 0;

        let spawned = lab.add_disturbance(floor, 150);
        for id in spawned {
            let guardian = &lab.guardians[&id];
            assert!(
                !lab.prison_core.contains(&guardian.cell),
                "Minor Guardian never spawns in prison core"
            );
        }

        // Collapse floor 0
        lab.collapsed_floors.insert(floor);
        let spawned_after_collapse = lab.add_disturbance(floor, 200);
        assert!(
            spawned_after_collapse.is_empty(),
            "No waves release on collapsed floor"
        );
    }

    #[test]
    fn bot_observer_shoves_adjacent_minor_guardian() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let id = ObserverId(0);
        let obs_cell = lab.observers[&id].cell;

        // Place a Minor Guardian on an adjacent neighbor
        let neighbor = lab
            .exits(obs_cell)
            .into_iter()
            .next()
            .expect("has neighbor exit");
        let minor_id = GuardianId(888);
        lab.guardians.insert(
            minor_id,
            Guardian {
                id: minor_id,
                cell: neighbor,
                last_detection: None,
                kind: GuardianKind::Minor,
            },
        );

        let (intent, trace) = lab.observer_intent(id);
        assert_eq!(
            intent,
            crate::sim::ObserverIntent::Shove(minor_id),
            "Bot observer must choose shove intent when adjacent to Minor Guardian"
        );
        assert_eq!(trace.selected, Some("shove adjacent Minor Guardian"));

        let before_charge = lab.economy.charge(id);
        lab.apply_observer_intent(id, intent);
        assert_eq!(
            lab.economy.charge(id),
            before_charge - SHOVE_COST,
            "Applying shove intent deducts SHOVE_COST"
        );
    }

    #[test]
    fn bot_observer_seeks_recharge_when_charge_depleted_and_recharges() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        let id = ObserverId(0);
        lab.guardians.clear(); // remove danger so observer seeks recharge

        let station = *lab.economy.stations.iter().next().expect("station exists");
        let non_station = lab
            .exits(station)
            .into_iter()
            .find(|&c| c != station)
            .expect("neighbor of station exists");

        // Observer is not at station and has 0 charge
        lab.observers.get_mut(&id).unwrap().cell = non_station;
        lab.economy.set_charge(id, 0);

        let (intent, trace) = lab.observer_intent(id);
        assert_eq!(
            trace.selected,
            Some("seek recharge station"),
            "Observer with depleted charge routes to powered station"
        );
        let crate::sim::ObserverIntent::Step(next_step) = intent else {
            panic!("Expected Step intent toward station, got {intent:?}");
        };
        assert_eq!(next_step, station, "Step should lead toward station");

        // Move to station and verify Hold to recharge
        lab.observers.get_mut(&id).unwrap().cell = station;
        let (intent_at_station, trace_at_station) = lab.observer_intent(id);
        assert_eq!(intent_at_station, crate::sim::ObserverIntent::Hold);
        assert_eq!(trace_at_station.selected, Some("recharge at station"));

        // Step beat to recharge
        lab.step_beat();
        assert_eq!(
            lab.economy.charge(id),
            RECHARGE_PER_BEAT,
            "Recharge beat at station increments charge"
        );
    }

    #[test]
    fn bot_observer_seeks_generator_and_toggles_power_when_dark() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        let id = ObserverId(0);
        lab.guardians.clear();
        let (non_gen, gen_cell, next_hop) = lab
            .world
            .placements
            .keys()
            .copied()
            .filter(|&c| c.level == 0)
            .find_map(|c| {
                for next in lab.exits(c) {
                    if next.level == 0 {
                        return Some((c, next, next));
                    }
                }
                None
            })
            .expect("connected cells on floor 0");

        lab.economy.generators.insert(0, gen_cell);

        // Power off floor 0
        lab.economy.set_powered(0, false);
        assert!(!lab.economy.is_powered(0));

        lab.observers.get_mut(&id).unwrap().cell = non_gen;
        let (intent, trace) = lab.observer_intent(id);
        assert_eq!(
            trace.selected,
            Some("seek generator to restore power"),
            "Observer seeks generator when floor is unpowered"
        );
        let crate::sim::ObserverIntent::Step(step) = intent else {
            panic!("Expected step toward generator, got {intent:?}");
        };
        assert_eq!(step, next_hop);

        // Stand at generator
        lab.observers.get_mut(&id).unwrap().cell = gen_cell;
        let (intent_at_gen, trace_at_gen) = lab.observer_intent(id);
        assert_eq!(intent_at_gen, crate::sim::ObserverIntent::ToggleGenerator);
        assert_eq!(
            trace_at_gen.selected,
            Some("restore floor power at generator")
        );

        lab.apply_observer_intent(id, intent_at_gen);
        assert!(
            lab.economy.is_powered(0),
            "Toggling generator restores floor power"
        );
    }

    #[test]
    fn rogue_architect_tree_prioritizes_disturbance_wave_release() {
        let mut lab =
            ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        lab.bot_architect = true;
        let floor = 0;

        // Set disturbance right at 90 (threshold is 100)
        lab.economy.set_disturbance(floor, 90);
        assert_eq!(lab.economy.wave_count(floor), 0);

        // When architect plays a card, disturbance crosses 100 and wave releases
        let (cmd, trace) = lab.architect_intent();
        assert!(cmd.is_some(), "Architect has a playable command");
        assert_eq!(
            trace.selected,
            Some("release disturbance wave"),
            "Rogue architect prioritizes crossing disturbance threshold to release waves"
        );

        lab.submit(cmd.unwrap())
            .expect("command submits successfully");
        assert!(
            lab.economy.wave_count(floor) >= 1,
            "Wave count incremented after wave release play"
        );
        assert!(
            lab.guardians
                .values()
                .any(|g| g.kind == GuardianKind::Minor),
            "Minor Guardians spawned from wave release"
        );
    }

    #[test]
    fn rogue_architect_tree_contests_generator() {
        let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
        lab.bot_architect = true;
        let floor = 0;

        // Isolate so guardian cannot shorten route, and disturbance starts at 0
        lab.guardians.clear();
        lab.economy.set_disturbance(floor, 0);

        // Put a door card in hand
        lab.deck.hand.clear();
        lab.deck.hand.push(crate::sim::Card {
            id: crate::sim::CardId(999),
            kind: crate::sim::CardKind::Door,
            district: None,
        });

        // Find a candidate door target from legal commands and place generator at/adjacent to it
        let candidate_target = lab
            .legal_commands()
            .into_iter()
            .find_map(|cmd| match cmd {
                crate::sim::ArchitectCommand::Play { target, .. } => Some(target),
                _ => None,
            })
            .expect("must have at least one legal door placement");

        let gen_cell = candidate_target;
        lab.economy.generators.insert(floor, gen_cell);

        let (cmd, trace) = lab.architect_intent();
        let cmd = cmd.expect("architect produces a command contesting generator");
        let crate::sim::ArchitectCommand::Play { target, .. } = cmd else {
            panic!("expected Play command, got {cmd:?}");
        };
        assert!(
            observed_hex::travel_distance(target, gen_cell) <= 1,
            "Target {target:?} must be adjacent to or at generator {gen_cell:?}"
        );
        assert_eq!(trace.selected, Some("contest generator"));
    }

    #[test]
    fn bot_observer_and_architect_soak_with_all_economy_features() {
        let mut a = ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        let mut b = ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves");
        a.bot_architect = true;
        b.bot_architect = true;

        for _ in 0..40 {
            a.step_beat();
            b.step_beat();
        }

        assert_eq!(a.world, b.world);
        assert_eq!(a.observers, b.observers);
        assert_eq!(a.guardians, b.guardians);
        assert_eq!(a.economy, b.economy);
        assert_eq!(a.command_log, b.command_log);
        assert_eq!(a.traces, b.traces);
    }
}
