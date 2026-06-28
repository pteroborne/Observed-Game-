//! Threshold continuity rules for the navigation probe.
//!
//! This is intentionally WFC-shaped but small: room templates expose fixed threshold
//! slots, the live facility assigns destinations to currently open slots, and a room
//! anchor collapses that room's visible assignment table. Other rooms may still vary,
//! but they cannot grow a new threshold into a locked room unless that locked room's
//! stored table already contains the relation.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use observed_core::RoomId;

use crate::facility::{self, DoorId, Facility, all_doors, all_rooms, door_rooms};

/// A deterministic threshold port on a room template. The slot exists even when no
/// current assignment points through it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SlotId {
    pub room: RoomId,
    pub ordinal: u8,
}

/// One collapsed assignment: `slot` currently leads through `door` to `target`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThresholdAssignment {
    pub slot: SlotId,
    pub door: DoorId,
    pub target: RoomId,
}

/// The room's current view: all possible slots, and the assigned thresholds that
/// are visible/passable under the live graph plus locks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomThresholdView {
    pub room: RoomId,
    pub slots: Vec<SlotId>,
    pub assignments: Vec<ThresholdAssignment>,
    pub locked: bool,
}

impl RoomThresholdView {
    pub fn assigned_targets(&self) -> Vec<RoomId> {
        self.assignments.iter().map(|a| a.target).collect()
    }
}

/// Preview/cross/arrival transcript for one threshold. This is the lab's equivalent of
/// "what you saw through the doorway is what crossing actually used."
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThresholdCrossing {
    pub preview: ThresholdAssignment,
    pub crossed_into: RoomId,
    pub arrival_has_return_threshold: bool,
}

/// The room-lock state. It is separate from the facility graph: the graph can keep
/// decohering, while locks constrain which assignments may be observed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Resource)]
pub struct ThresholdState {
    room_locks: BTreeMap<RoomId, Vec<ThresholdAssignment>>,
}

impl ThresholdState {
    pub fn clear(&mut self) {
        self.room_locks.clear();
    }

    pub fn is_room_locked(&self, room: RoomId) -> bool {
        self.room_locks.contains_key(&room)
    }

    pub fn locked_rooms(&self) -> Vec<RoomId> {
        self.room_locks.keys().copied().collect()
    }

    /// Collapse this room to the assignments visible right now.
    pub fn lock_room(&mut self, facility: &Facility, room: RoomId) -> RoomThresholdView {
        let live = self.unlocked_room_view(facility, room);
        self.room_locks.insert(room, live.assignments);
        self.room_view(facility, room)
    }

    pub fn unlock_room(&mut self, room: RoomId) {
        self.room_locks.remove(&room);
    }

    pub fn toggle_room_lock(&mut self, facility: &Facility, room: RoomId) -> bool {
        if self.is_room_locked(room) {
            self.unlock_room(room);
            false
        } else {
            self.lock_room(facility, room);
            true
        }
    }

    pub fn room_view(&self, facility: &Facility, room: RoomId) -> RoomThresholdView {
        if let Some(assignments) = self.room_locks.get(&room) {
            return RoomThresholdView {
                room,
                slots: slots_for_room(room),
                assignments: assignments.clone(),
                locked: true,
            };
        }
        self.unlocked_room_view(facility, room)
    }

    /// Whether locks allow the relation `a <-> b` to appear. A locked endpoint is an
    /// exclusive assignment table.
    pub fn relation_allowed_by_locks(&self, a: RoomId, b: RoomId) -> bool {
        self.endpoint_allows(a, b) && self.endpoint_allows(b, a)
    }

    pub fn relation_locked(&self, a: RoomId, b: RoomId) -> bool {
        self.locked_endpoint_contains(a, b) || self.locked_endpoint_contains(b, a)
    }

    pub fn preview(
        &self,
        facility: &Facility,
        room: RoomId,
        slot: SlotId,
    ) -> Option<ThresholdAssignment> {
        self.room_view(facility, room)
            .assignments
            .into_iter()
            .find(|assignment| assignment.slot == slot)
    }

    pub fn cross(&self, facility: &Facility, assignment: ThresholdAssignment) -> ThresholdCrossing {
        let arrival = self.room_view(facility, assignment.target);
        ThresholdCrossing {
            preview: assignment,
            crossed_into: assignment.target,
            arrival_has_return_threshold: arrival
                .assignments
                .iter()
                .any(|returning| returning.target == assignment.slot.room),
        }
    }

    /// Human-readable health report for the overlay and tests.
    pub fn audit(&self, facility: &Facility) -> RuleAudit {
        let mut issues = Vec::new();
        for room in all_rooms() {
            let view = self.room_view(facility, room);
            if view.slots.len() != slot_count(room) {
                issues.push(format!(
                    "room {} slot count changed from its template",
                    facility::room_label(room)
                ));
            }
            if view.assignments.len() > view.slots.len() {
                issues.push(format!(
                    "room {} has more assignments than slots",
                    facility::room_label(room)
                ));
            }
            for assignment in &view.assignments {
                if assignment.slot.room != room {
                    issues.push(format!(
                        "room {} owns foreign slot {:?}",
                        facility::room_label(room),
                        assignment.slot
                    ));
                }
                if !view.slots.contains(&assignment.slot) {
                    issues.push(format!(
                        "room {} assignment uses missing slot {:?}",
                        facility::room_label(room),
                        assignment.slot
                    ));
                }
                if !self.relation_allowed_by_locks(room, assignment.target) {
                    issues.push(format!(
                        "room {} shows forbidden relation to {}",
                        facility::room_label(room),
                        facility::room_label(assignment.target)
                    ));
                }
                let crossing = self.cross(facility, *assignment);
                if crossing.crossed_into != assignment.target {
                    issues.push(format!(
                        "room {} preview/cross mismatch through slot {}",
                        facility::room_label(room),
                        assignment.slot.ordinal
                    ));
                }
                if !crossing.arrival_has_return_threshold {
                    issues.push(format!(
                        "room {} threshold to {} has no reciprocal arrival threshold",
                        facility::room_label(room),
                        facility::room_label(assignment.target)
                    ));
                }
            }
        }
        RuleAudit { issues }
    }

    fn unlocked_room_view(&self, facility: &Facility, room: RoomId) -> RoomThresholdView {
        let mut assignments = live_assignments(facility, room);
        assignments.extend(self.pinned_assignments(room));
        assignments.retain(|assignment| self.relation_allowed_by_locks(room, assignment.target));
        assignments.sort_by_key(|assignment| (assignment.slot.ordinal, assignment.target.0));
        assignments.dedup_by_key(|assignment| assignment.slot);
        RoomThresholdView {
            room,
            slots: slots_for_room(room),
            assignments,
            locked: false,
        }
    }

    fn pinned_assignments(&self, room: RoomId) -> Vec<ThresholdAssignment> {
        let mut out = Vec::new();
        for (&locked_room, locked_assignments) in &self.room_locks {
            for assignment in locked_assignments {
                if assignment.target != room {
                    continue;
                }
                let Some(door) = door_between(room, locked_room) else {
                    continue;
                };
                out.push(ThresholdAssignment {
                    slot: slot_for_door(room, door),
                    door,
                    target: locked_room,
                });
            }
        }
        out
    }

    fn endpoint_allows(&self, room: RoomId, other: RoomId) -> bool {
        self.room_locks
            .get(&room)
            .is_none_or(|assignments| assignments.iter().any(|a| a.target == other))
    }

    fn locked_endpoint_contains(&self, room: RoomId, other: RoomId) -> bool {
        self.room_locks
            .get(&room)
            .is_some_and(|assignments| assignments.iter().any(|a| a.target == other))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleAudit {
    pub issues: Vec<String>,
}

impl RuleAudit {
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.passed() {
            "threshold rules pass".to_string()
        } else {
            format!("threshold rules FAIL: {}", self.issues.join("; "))
        }
    }
}

pub fn slots_for_room(room: RoomId) -> Vec<SlotId> {
    incident_doors(room)
        .into_iter()
        .enumerate()
        .map(|(ordinal, _)| SlotId {
            room,
            ordinal: ordinal as u8,
        })
        .collect()
}

pub fn slot_count(room: RoomId) -> usize {
    slots_for_room(room).len()
}

pub fn slot_for_door(room: RoomId, door: DoorId) -> SlotId {
    let ordinal = incident_doors(room)
        .into_iter()
        .position(|candidate| candidate == door)
        .unwrap_or_else(|| panic!("door {:?} is not incident to room {:?}", door, room));
    SlotId {
        room,
        ordinal: ordinal as u8,
    }
}

pub fn slot_position(slot: SlotId) -> bevy::math::Vec2 {
    let door = incident_doors(slot.room)[slot.ordinal as usize];
    facility::door_gap(door).center()
}

fn live_assignments(facility: &Facility, room: RoomId) -> Vec<ThresholdAssignment> {
    incident_doors(room)
        .into_iter()
        .filter(|&door| facility.is_open(door))
        .map(|door| {
            let (a, b) = door_rooms(door);
            let target = if a == room { b } else { a };
            ThresholdAssignment {
                slot: slot_for_door(room, door),
                door,
                target,
            }
        })
        .collect()
}

fn incident_doors(room: RoomId) -> Vec<DoorId> {
    all_doors()
        .into_iter()
        .filter(|&door| {
            let (a, b) = door_rooms(door);
            a == room || b == room
        })
        .collect()
}

fn door_between(a: RoomId, b: RoomId) -> Option<DoorId> {
    all_doors().into_iter().find(|&door| {
        let (x, y) = door_rooms(door);
        (x == a && y == b) || (x == b && y == a)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_count_is_fixed_by_room_template_not_open_doors() {
        let mut facility = Facility::all_open();
        let before: Vec<_> = all_rooms().into_iter().map(slot_count).collect();
        for door in all_doors() {
            facility.set_open(door, false);
        }
        let state = ThresholdState::default();
        for (room, expected) in all_rooms().into_iter().zip(before) {
            let view = state.room_view(&facility, room);
            assert_eq!(view.slots.len(), expected);
            assert!(view.assignments.is_empty());
        }
    }

    #[test]
    fn room_lock_freezes_visible_thresholds_and_rejects_new_inbound_edges() {
        let mut facility = Facility::all_open();
        facility.set_open(DoorId(1), false); // hide AC before locking A
        let mut state = ThresholdState::default();
        let locked = state.lock_room(&facility, RoomId(0));
        assert_eq!(locked.assigned_targets(), vec![RoomId(1)]);

        facility.set_open(DoorId(0), false); // live AB disappears
        facility.set_open(DoorId(1), true); // live AC appears

        let a = state.room_view(&facility, RoomId(0));
        assert_eq!(
            a.assigned_targets(),
            vec![RoomId(1)],
            "locked room keeps exactly its collapsed threshold table"
        );
        let b = state.room_view(&facility, RoomId(1));
        assert!(
            b.assigned_targets().contains(&RoomId(0)),
            "the pinned relation remains visible from the other endpoint"
        );
        let c = state.room_view(&facility, RoomId(2));
        assert!(
            !c.assigned_targets().contains(&RoomId(0)),
            "a new live edge cannot point into the locked room"
        );
        assert!(state.audit(&facility).passed());
    }

    #[test]
    fn preview_crossing_and_arrival_use_the_same_collapsed_assignment() {
        let facility = Facility::all_open();
        let mut state = ThresholdState::default();
        state.lock_room(&facility, RoomId(0));
        let slot = slot_for_door(RoomId(0), DoorId(0));
        let preview = state
            .preview(&facility, RoomId(0), slot)
            .expect("AB visible");
        let crossing = state.cross(&facility, preview);

        assert_eq!(crossing.preview, preview);
        assert_eq!(crossing.crossed_into, RoomId(1));
        assert!(
            crossing.arrival_has_return_threshold,
            "arrival room still has the reciprocal threshold back"
        );
    }

    #[test]
    fn audit_passes_for_every_door_configuration_with_each_single_room_lock() {
        for bits in 0u8..(1 << 4) {
            let mut facility = Facility::all_open();
            for door in 0..4u8 {
                facility.set_open(DoorId(door), bits & (1 << door) != 0);
            }
            assert!(ThresholdState::default().audit(&facility).passed());
            for room in all_rooms() {
                let mut state = ThresholdState::default();
                state.lock_room(&facility, room);
                assert!(
                    state.audit(&facility).passed(),
                    "audit failed for lock room {} with doors {bits:04b}: {:?}",
                    facility::room_label(room),
                    state.audit(&facility).issues
                );
            }
        }
    }
}
