//! Shared, stable domain identifiers for the Observed 2 prototypes.
//!
//! Bevy `Entity` values are not durable game identities: they are recycled and
//! are local to a single `World`. Gameplay logic instead refers to things by
//! these small newtype identifiers, which can be stored, looked up, persisted,
//! and matched against simulation state regardless of how — or whether — the
//! corresponding entity is currently rendered.
//!
//! `PlayerId` and `PlayerIntent` live in the focused `player_input` crate (the
//! input boundary) and are re-exported here so a lab can depend on a single
//! foundation crate for every shared type it needs.
//!
//! Identifiers are added here only once a system actually consumes them, so the
//! canonical set named in `AGENTS.md` lands incrementally as the labs need it.

pub use player_input::{PlayerId, PlayerIntent};

pub mod prng;
pub use prng::SplitMix;

pub mod direction;
pub use direction::Direction;

/// Common trait implemented by all domain newtype identifiers.
pub trait DomainId: Copy + Eq + Ord + std::hash::Hash {
    fn as_u32(self) -> u32;
    fn as_usize(self) -> usize;
}

#[macro_export]
macro_rules! domain_id {
    ($(#[$meta:meta])* $name:ident, $inner:ty) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub $inner);

        impl $name {
            #[inline]
            pub const fn as_u32(self) -> u32 {
                self.0 as u32
            }

            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0 as usize
            }
        }

        impl $crate::DomainId for $name {
            #[inline]
            fn as_u32(self) -> u32 {
                self.0 as u32
            }

            #[inline]
            fn as_usize(self) -> usize {
                self.0 as usize
            }
        }

        impl From<$inner> for $name {
            #[inline]
            fn from(val: $inner) -> Self {
                Self(val)
            }
        }
    };
}

domain_id!(
    /// Stable identifier for a logical room instance.
    RoomId,
    u32
);

domain_id!(
    /// Stable identifier for a logical corridor instance.
    CorridorId,
    u32
);

/// Stable identifier for a discrete playable place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlaceId {
    Room(RoomId),
    Corridor(CorridorId),
}

/// Stable identifier for an authored threshold socket. Slots are unique within
/// their owning place; a live attachment pairs two sockets without changing
/// either socket's identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThresholdId {
    pub place: PlaceId,
    pub slot: ThresholdSlotId,
}

impl ThresholdId {
    pub const fn new(place: PlaceId, slot: u16) -> Self {
        Self {
            place,
            slot: ThresholdSlotId(slot),
        }
    }
}

domain_id!(
    /// Place-local stable slot for a threshold aperture.
    ThresholdSlotId,
    u16
);

impl ThresholdSlotId {
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

domain_id!(
    /// Stable identifier for an authored connection point on a room (a port or socket).
    PortId,
    u32
);

domain_id!(
    /// Stable identifier for a persistent piece of equipment.
    EquipmentId,
    u32
);

domain_id!(
    /// Stable identifier for a team of cooperating players.
    TeamId,
    u8
);

impl TeamId {
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn label(self) -> String {
        format!("Team {}", self.0 + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn identifiers_are_distinct_ordered_map_keys() {
        let mut rooms = BTreeMap::new();
        rooms.insert(RoomId(2), "second");
        rooms.insert(RoomId(1), "first");
        assert_eq!(
            rooms.keys().copied().collect::<Vec<_>>(),
            vec![RoomId(1), RoomId(2)]
        );
        assert_ne!(PortId(0), PortId(1));
        assert_eq!(EquipmentId(7).0, 7);
        assert_ne!(CorridorId(0), CorridorId(1));
        assert_ne!(
            ThresholdId::new(PlaceId::Room(RoomId(1)), 0),
            ThresholdId::new(PlaceId::Corridor(CorridorId(1)), 0)
        );
    }

    #[test]
    fn player_identity_is_reexported_from_the_input_boundary() {
        assert_eq!(PlayerId(0).label(), "P1");
        assert!(PlayerIntent::default().is_neutral());
    }

    #[test]
    fn team_identity_labels_and_indexes() {
        assert_eq!(TeamId(0).label(), "Team 1");
        assert_eq!(TeamId(2).index(), 2);
        assert_ne!(TeamId(0), TeamId(1));
    }
}
