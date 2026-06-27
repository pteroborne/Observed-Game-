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

/// Stable identifier for a logical room instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RoomId(pub u32);

/// Stable identifier for an authored connection point on a room (a port or
/// socket). Unique within the room that owns it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortId(pub u32);

/// Stable identifier for a persistent piece of equipment. Equipment keeps this
/// identity while it is carried, deployed, socketed, or temporarily despawned.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EquipmentId(pub u32);

/// Stable identifier for a team of cooperating players. A team groups one or
/// more `PlayerId`s; shared resources may be contended or owned per team.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TeamId(pub u8);

impl TeamId {
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
