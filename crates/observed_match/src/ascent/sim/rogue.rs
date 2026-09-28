//! The Rogue bot on a first-person facility.
//!
//! The lab's Rogue judges a play by making it on a copy of the whole rules, which on a
//! production facility - every cell of it the Rogue's to play on - is seconds a decision.
//! This one judges locally, as the loyal Architect does (`loyal`), from the other side:
//!
//! 1. Wait for the cooldown.
//! 2. **Close the hunt:** play what shortens a Guardian's way to an Observer the Rogue has
//!    detected - the Guardian's own search and the Observer's, joined through the cell a
//!    candidate would build.
//! 3. **Send the Guardians:** a directive card on the detected Observer nearest a major
//!    Guardian on its floor, while no directive stands.
//! 4. **Undermine:** in a detected Observer's path, play what leaves doorways meeting
//!    walls: a contradiction, which the rules retract out from under whoever walks into
//!    it unless a loyal Architect repairs it first. The cells beside an Observer are in
//!    its sight and cannot be played, so the path is as near as the Rogue can reach.
//! 5. Otherwise hold the card.
//!
//! With nobody detected it has nobody to play against, so it **watches the way up**: a
//! sensor card (`sensor`) at the foot of a stair or ramp, on the floor it watches least -
//! every climb passes one.
//!
//! It never reads where an undetected Observer is: every anchor and every target is a
//! detected Observer's cell or a Guardian's.

use std::collections::BTreeMap;

use observed_hex::{HexCoord, travel_distance};

use super::loyal::NO_WAY_UP;
use super::{ArchitectCommand, ArchitectLab, BehaviorTrace, ObserverState, command_key};

/// How far a Guardian's own search runs, in steps: the length of a hunt.
const HUNT_SEARCH_STEPS: usize = 40;
/// How far a detected Observer's own search runs: past every cell a candidate within
/// reach of it can join.
const PREY_SEARCH_STEPS: usize = super::loyal::REACH as usize + 3;

impl ArchitectLab {
    /// The Rogue bot's intent on a first-person facility, in the Rogue's own hand context.
    #[must_use]
    pub(crate) fn rogue_intent(&self) -> (Option<ArchitectCommand>, BehaviorTrace) {
        let mut trace = BehaviorTrace::default();
        if trace.test("wait for cooldown", self.cooldown > 0) {
            return (None, trace);
        }
        let detected = self.rogue_detected();
        let prey: Vec<HexCoord> = self
            .observers
            .values()
            .filter(|o| o.state == ObserverState::Active && detected.contains(&o.id))
            .map(|o| o.cell)
            .collect();
        if prey.is_empty() {
            let watch = self.watch_the_way_up();
            if trace.test("watch the way up", watch.is_some()) {
                return (watch, trace);
            }
            trace.test("nobody detected", true);
            return (None, trace);
        }
        let hunters: Vec<HexCoord> = self.guardians.values().map(|g| g.cell).collect();
        // A Rogue play matters beside its prey: every candidate is near a detected Observer.
        let candidates = self.candidates(&prey);
        if candidates.is_empty() {
            trace.test("hold card", true);
            return (None, trace);
        }

        // Close the hunt: each Guardian's search and each detected Observer's, joined
        // through a candidate's cell.
        let from_hunters: Vec<BTreeMap<HexCoord, usize>> = hunters
            .iter()
            .map(|&cell| self.distances_from(cell, HUNT_SEARCH_STEPS))
            .collect();
        // A candidate is within reach of its prey, so the prey's own search need only reach
        // past it; only the Guardians' runs the length of a hunt.
        let from_prey: Vec<BTreeMap<HexCoord, usize>> = prey
            .iter()
            .map(|&cell| self.distances_from(cell, PREY_SEARCH_STEPS))
            .collect();
        let now = from_hunters
            .iter()
            .flat_map(|reach| prey.iter().filter_map(|cell| reach.get(cell).copied()))
            .min()
            .unwrap_or(NO_WAY_UP);
        // The candidates are all beside prey, so each is judged against the prey it is
        // beside: a join to a far Observer's short search would be no join at all.
        let key = |command: &ArchitectCommand| command_key(*command);
        let closer = candidates
            .iter()
            .filter(|(_, changes)| changes.len() == 1)
            .map(|(command, changes)| {
                let route = from_hunters
                    .iter()
                    .flat_map(|hunter| {
                        from_prey.iter().map(|target| {
                            self.through(changes[0].coord, changes[0], hunter, target)
                        })
                    })
                    .min()
                    .unwrap_or(NO_WAY_UP);
                (route, command)
            })
            .filter(|(route, _)| *route < now)
            .min_by_key(|(route, command)| (*route, key(command)));
        if trace.test("close the hunt", closer.is_some()) {
            return (closer.map(|(_, command)| *command), trace);
        }

        // Send the Guardians: a major walks to the detected Observer nearest one.
        let sent = self.send_the_guardians(&prey);
        if trace.test("send the Guardians", sent.is_some()) {
            return (sent, trace);
        }

        // Undermine: in a detected Observer's path, the most doorways left meeting walls,
        // nearest first. Every candidate is already within reach of its prey, and the
        // cells beside it are in its sight, so no nearer bound would leave a play.
        let undermine = candidates
            .iter()
            .filter_map(|(command, changes)| {
                let ArchitectCommand::Play { target, .. } = *command else {
                    return None;
                };
                let near = prey
                    .iter()
                    .map(|&cell| travel_distance(cell, target))
                    .min()?;
                let (before, after) = self.mismatches_around(changes);
                (after > before).then(|| (after - before, near, command))
            })
            .max_by_key(|(broken, near, command)| {
                (
                    *broken,
                    std::cmp::Reverse(*near),
                    std::cmp::Reverse(key(command)),
                )
            });
        if trace.test("undermine", undermine.is_some()) {
            return (undermine.map(|(_, _, command)| *command), trace);
        }
        trace.test("hold card", true);
        (None, trace)
    }

    /// The id of a card of `kind` in the hand, if one is held.
    fn held(&self, kind: super::CardKind) -> Option<super::CardId> {
        self.deck
            .hand
            .iter()
            .find(|card| card.kind == kind)
            .map(|card| card.id)
    }

    /// A sensor card played at the foot of a climb no sensor watches yet, on the floor with
    /// fewest, while the Rogue holds one and has one to spare.
    fn watch_the_way_up(&self) -> Option<ArchitectCommand> {
        let card = self.held(super::CardKind::Sensor)?;
        if self.sensors.len() >= super::MAX_SENSORS {
            return None;
        }
        let watched: std::collections::BTreeSet<HexCoord> = self
            .sensors
            .keys()
            .flat_map(|&cell| self.sensor_sight(cell))
            .collect();
        let on_floor = |level: u8| self.sensors.keys().filter(|c| c.level == level).count();
        self.world
            .placements
            .iter()
            .filter(|(cell, placement)| {
                placement.up != observed_hex::PortClass::Sealed && !watched.contains(cell)
            })
            .map(|(&target, _)| target)
            .filter(|&target| {
                self.refusal(ArchitectCommand::Play {
                    card,
                    target,
                    rotation: 0,
                })
                .is_none()
            })
            .min_by_key(|&target| (on_floor(target.level), target.level, target))
            .map(|target| ArchitectCommand::Play {
                card,
                target,
                rotation: 0,
            })
    }

    /// A directive card played on the detected Observer nearest a major Guardian, while
    /// the Rogue holds one and no directive stands.
    fn send_the_guardians(&self, prey: &[HexCoord]) -> Option<ArchitectCommand> {
        let card = self.held(super::CardKind::Directive)?;
        if self.directed.is_some() {
            return None;
        }
        let majors: Vec<HexCoord> = self
            .guardians
            .values()
            .filter(|guardian| guardian.kind == super::GuardianKind::Major)
            .map(|guardian| guardian.cell)
            .collect();
        prey.iter()
            .copied()
            .filter_map(|target| {
                let near = majors
                    .iter()
                    .filter(|major| major.level == target.level && **major != target)
                    .map(|&major| travel_distance(major, target))
                    .min()?;
                let command = ArchitectCommand::Play {
                    card,
                    target,
                    rotation: 0,
                };
                self.refusal(command)
                    .is_none()
                    .then_some((near, target, command))
            })
            .min_by_key(|&(near, target, _)| (near, target))
            .map(|(_, _, command)| command)
    }
}
