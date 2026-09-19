//! Deterministic warning, retraction, repair, and floor closure.
use observed_facility::hex_wfc::HexSpace;
use observed_hex::{HexCoord, travel_distance};

use super::{ArchitectLab, District, DoorState, GuardianKind, threshold_touches};

pub const RETRACTION_TICKS: u64 = 180;

/// How long a condemned tile telegraphs before it commits, when an actor is
/// standing on it. Three beats at 60 ticks: long enough for the Observer tree to
/// reach `evade immediate danger` and walk off, short enough that standing in a
/// collapsing room is a decision rather than a formality.
pub const CONDEMNED_GRACE_TICKS: u64 = 180;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabEventKind {
    Played,
    Warning,
    Retracted,
    Repaired,
    Captured,
    FloorClosed,
    Requisition,
    Fell,
    Corrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabEvent {
    pub tick: u64,
    pub kind: LabEventKind,
    pub cell: Option<HexCoord>,
    pub message: String,
}

impl ArchitectLab {
    pub(crate) fn record_event(
        &mut self,
        kind: LabEventKind,
        cell: Option<HexCoord>,
        message: &str,
    ) {
        self.events.push_back(LabEvent {
            tick: self.tick,
            kind,
            cell,
            message: message.into(),
        });
        while self.events.len() > 24 {
            self.events.pop_front();
        }
    }

    /// Occupancy is deliberately **not** here. A cell somebody stands on used to
    /// be immune, which made an ordinary fall unreachable by construction rather
    /// than rare — see backlog #42, measured at 0 of 1,087 retractions. It is now
    /// condemned with a grace window instead. `CommandRefusal::Occupied` still
    /// forbids an Architect *playing* onto an actor; this is only the
    /// consequential path, and it warns first.
    pub fn retraction_protected(&self, cell: HexCoord) -> bool {
        self.prison_core.contains(&cell)
            || self.observed.contains(&cell)
            || self.anchored.contains(&cell)
            || self.doors.iter().any(|(&key, &state)| {
                state == DoorState::Open && threshold_touches(key, cell, &self.world)
            })
    }

    /// Closest exposed contradiction to the originating play; coordinate breaks ties.
    pub fn next_retraction(&self) -> Option<HexCoord> {
        self.contradictions
            .iter()
            .copied()
            .filter(|&cell| !self.retraction_protected(cell))
            .min_by_key(|&cell| {
                (
                    self.instability_origin
                        .map_or(0, |origin| travel_distance(origin, cell)),
                    cell,
                )
            })
    }

    pub(super) fn sync_retraction_clock(&mut self) {
        if self.contradictions.is_empty() {
            if self.next_retraction_tick.take().is_some() {
                self.record_event(
                    LabEventKind::Repaired,
                    self.instability_origin,
                    "Boundary repaired. Pending collapse cancelled.",
                );
            }
            self.instability_origin = None;
        } else if self.next_retraction_tick.is_none() {
            self.next_retraction_tick = Some(self.tick + RETRACTION_TICKS);
            self.record_event(
                LabEventKind::Warning,
                self.instability_origin,
                "Unmatched connections. First exposed tile retracts in 3 seconds.",
            );
        }
    }

    pub(crate) fn advance_retraction(&mut self) {
        if !self
            .next_retraction_tick
            .is_some_and(|deadline| self.tick >= deadline)
        {
            return;
        }
        // A tile with somebody on it is condemned rather than taken. The warning
        // is the whole point: a fall you could not have avoided is an ambush, and
        // one you walked away from is a decision you made.
        let Some(cell) = self.next_retraction() else {
            self.next_retraction_tick = Some(self.tick + RETRACTION_TICKS);
            self.sync_retraction_clock();
            return;
        };
        if self.occupied().contains(&cell) {
            match self.condemned {
                Some((condemned, due)) if condemned == cell && self.tick >= due => {}
                Some((condemned, _)) if condemned == cell => {
                    self.next_retraction_tick = Some(self.tick + RETRACTION_TICKS);
                    self.sync_retraction_clock();
                    return;
                }
                _ => {
                    self.condemned = Some((cell, self.tick + CONDEMNED_GRACE_TICKS));
                    self.record_event(
                        LabEventKind::Retracted,
                        Some(cell),
                        "This floor is going. Move.",
                    );
                    self.next_retraction_tick = Some(self.tick + RETRACTION_TICKS);
                    self.sync_retraction_clock();
                    return;
                }
            }
        }
        self.condemned = None;
        {
            let tile = self
                .world
                .placements
                .get_mut(&cell)
                .expect("contradiction is a tile");
            tile.space = HexSpace::Void;
            tile.archetype = observed_facility::hex_wfc::HexArchetype::Void;
            tile.up = observed_hex::PortClass::Sealed;
            tile.down = observed_hex::PortClass::Sealed;
            tile.doors = 0;
            self.retracted.insert(cell);
            *self.world.cell_revisions.entry(cell).or_default() += 1;
            self.doors
                .retain(|&key, _| !threshold_touches(key, cell, &self.world));
            self.economy.on_retraction_committed(cell.level);
            self.resolve_disturbance_waves(cell.level);
            self.record_event(
                LabEventKind::Retracted,
                Some(cell),
                "Tile retracted. Rebuild it or close the exposed connection to stop the spread.",
            );
            if self.economy.is_at_generator(cell) {
                self.cut_floor_power(cell.level);
            }
            let floor_empty = self.world.placements.iter().all(|(&other, tile)| {
                other.level != cell.level
                    || self.prison_core.contains(&other)
                    || tile.space == HexSpace::Void
            });
            if floor_empty && self.collapsed_floors.insert(cell.level) {
                self.cut_floor_power(cell.level);
                self.deck.retire_district(District::for_level(cell.level));
                self.guardians
                    .retain(|_, g| !(g.kind == GuardianKind::Minor && g.cell.level == cell.level));
                self.record_event(
                    LabEventKind::FloorClosed,
                    Some(cell),
                    "Floor lost. The prison survives; this floor cannot be rebuilt.",
                );
            }
            self.refresh_contradictions();
        }
        // Protection delays collapse; it never consumes the protected tile.
        self.next_retraction_tick = Some(self.tick + RETRACTION_TICKS);
        self.sync_retraction_clock();
    }
}
