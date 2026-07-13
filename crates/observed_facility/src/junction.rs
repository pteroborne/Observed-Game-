//! Stable room/corridor incidence topology.
//!
//! A corridor is a first-class place rather than an implicit room pair. Live
//! connectivity is a reciprocal matching between room threshold sockets and
//! corridor threshold sockets. This permits multi-exit junction corridors while
//! keeping every physical aperture stable and independently auditable.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_core::{CorridorId, PlaceId, RoomId, ThresholdId, ThresholdSlotId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThresholdAttachment {
    pub room: ThresholdId,
    pub corridor: ThresholdId,
}

impl ThresholdAttachment {
    pub fn new(a: ThresholdId, b: ThresholdId) -> Result<Self, JunctionError> {
        match (a.place, b.place) {
            (PlaceId::Room(_), PlaceId::Corridor(_)) => Ok(Self {
                room: a,
                corridor: b,
            }),
            (PlaceId::Corridor(_), PlaceId::Room(_)) => Ok(Self {
                room: b,
                corridor: a,
            }),
            _ => Err(JunctionError::NonBipartiteAttachment(a, b)),
        }
    }

    pub fn endpoints(self) -> [ThresholdId; 2] {
        [self.room, self.corridor]
    }

    pub fn partner(self, threshold: ThresholdId) -> Option<ThresholdId> {
        if threshold == self.room {
            Some(self.corridor)
        } else if threshold == self.corridor {
            Some(self.room)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorridorSpec {
    pub id: CorridorId,
    pub slots: Vec<ThresholdSlotId>,
}

impl CorridorSpec {
    pub fn with_slot_count(id: CorridorId, count: usize) -> Self {
        assert!(
            count <= u16::MAX as usize,
            "corridor slot count is u16-backed"
        );
        Self {
            id,
            slots: (0..count)
                .map(|slot| ThresholdSlotId(slot as u16))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JunctionError {
    DuplicateThreshold(ThresholdId),
    DuplicateCorridor(CorridorId),
    DuplicateCorridorSlot(CorridorId, ThresholdSlotId),
    UnknownCorridorSlot(ThresholdId),
    NonBipartiteAttachment(ThresholdId, ThresholdId),
    RoomAttachedTwice(ThresholdId),
    CorridorAttachedTwice(ThresholdId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JunctionTopology {
    corridors: BTreeMap<CorridorId, BTreeSet<ThresholdSlotId>>,
    partners: BTreeMap<ThresholdId, ThresholdId>,
}

impl JunctionTopology {
    pub fn new(
        corridors: impl IntoIterator<Item = CorridorSpec>,
        attachments: impl IntoIterator<Item = ThresholdAttachment>,
    ) -> Result<Self, Vec<JunctionError>> {
        let mut topology = Self::default();
        let mut errors = Vec::new();
        for corridor in corridors {
            if topology.corridors.contains_key(&corridor.id) {
                errors.push(JunctionError::DuplicateCorridor(corridor.id));
                continue;
            }
            let mut slots = BTreeSet::new();
            for slot in corridor.slots {
                if !slots.insert(slot) {
                    errors.push(JunctionError::DuplicateCorridorSlot(corridor.id, slot));
                }
            }
            topology.corridors.insert(corridor.id, slots);
        }
        for attachment in attachments {
            if let Err(error) = topology.attach(attachment) {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(topology)
        } else {
            Err(errors)
        }
    }

    pub fn attach(&mut self, attachment: ThresholdAttachment) -> Result<(), JunctionError> {
        let corridor_known = match attachment.corridor.place {
            PlaceId::Corridor(id) => self
                .corridors
                .get(&id)
                .is_some_and(|slots| slots.contains(&attachment.corridor.slot)),
            PlaceId::Room(_) => false,
        };
        if !corridor_known {
            return Err(JunctionError::UnknownCorridorSlot(attachment.corridor));
        }
        if self.partners.contains_key(&attachment.room) {
            return Err(JunctionError::RoomAttachedTwice(attachment.room));
        }
        if self.partners.contains_key(&attachment.corridor) {
            return Err(JunctionError::CorridorAttachedTwice(attachment.corridor));
        }
        self.partners.insert(attachment.room, attachment.corridor);
        self.partners.insert(attachment.corridor, attachment.room);
        Ok(())
    }

    pub fn partner(&self, threshold: ThresholdId) -> Option<ThresholdId> {
        self.partners.get(&threshold).copied()
    }

    pub fn attachment(&self, threshold: ThresholdId) -> Option<ThresholdAttachment> {
        ThresholdAttachment::new(threshold, self.partner(threshold)?).ok()
    }

    pub fn corridor_rooms(&self, corridor: CorridorId) -> Vec<RoomId> {
        let mut rooms = self
            .partners
            .iter()
            .filter_map(|(socket, partner)| {
                (socket.place == PlaceId::Corridor(corridor)).then_some(partner.place)
            })
            .filter_map(|place| match place {
                PlaceId::Room(room) => Some(room),
                PlaceId::Corridor(_) => None,
            })
            .collect::<Vec<_>>();
        rooms.sort_unstable();
        rooms.dedup();
        rooms
    }

    pub fn threshold_count(&self) -> usize {
        self.partners.len()
    }

    /// Every room reachable through a sequence of junction corridors.
    pub fn reachable_rooms(&self, start: RoomId) -> BTreeSet<RoomId> {
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(room) = queue.pop_front() {
            let room_place = PlaceId::Room(room);
            let corridors = self
                .partners
                .iter()
                .filter_map(|(socket, partner)| {
                    (socket.place == room_place).then_some(partner.place)
                })
                .filter_map(|place| match place {
                    PlaceId::Corridor(id) => Some(id),
                    PlaceId::Room(_) => None,
                })
                .collect::<Vec<_>>();
            for corridor in corridors {
                for next in self.corridor_rooms(corridor) {
                    if seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(room: u32, slot: u16) -> ThresholdId {
        ThresholdId::new(PlaceId::Room(RoomId(room)), slot)
    }

    fn corridor(corridor: u32, slot: u16) -> ThresholdId {
        ThresholdId::new(PlaceId::Corridor(CorridorId(corridor)), slot)
    }

    #[test]
    fn a_data_driven_junction_exposes_every_attached_room() {
        let attachments = (0..6)
            .map(|slot| ThresholdAttachment::new(room(slot, 0), corridor(7, slot as u16)).unwrap());
        let topology = JunctionTopology::new(
            [CorridorSpec::with_slot_count(CorridorId(7), 6)],
            attachments,
        )
        .unwrap();
        assert_eq!(
            topology.corridor_rooms(CorridorId(7)),
            (0..6).map(RoomId).collect::<Vec<_>>()
        );
        assert_eq!(topology.reachable_rooms(RoomId(0)).len(), 6);
    }

    #[test]
    fn attachments_are_reciprocal_and_a_socket_cannot_half_rewire() {
        let attachment = ThresholdAttachment::new(room(1, 2), corridor(3, 4)).unwrap();
        let mut topology = JunctionTopology::new(
            [CorridorSpec::with_slot_count(CorridorId(3), 5)],
            [attachment],
        )
        .unwrap();
        assert_eq!(topology.partner(attachment.room), Some(attachment.corridor));
        assert_eq!(topology.partner(attachment.corridor), Some(attachment.room));
        assert_eq!(
            topology.attach(ThresholdAttachment::new(room(9, 0), attachment.corridor).unwrap()),
            Err(JunctionError::CorridorAttachedTwice(attachment.corridor))
        );
    }

    #[test]
    fn room_to_room_and_corridor_to_corridor_links_are_rejected() {
        assert!(matches!(
            ThresholdAttachment::new(room(0, 0), room(1, 0)),
            Err(JunctionError::NonBipartiteAttachment(_, _))
        ));
        assert!(matches!(
            ThresholdAttachment::new(corridor(0, 0), corridor(1, 0)),
            Err(JunctionError::NonBipartiteAttachment(_, _))
        ));
    }
}
