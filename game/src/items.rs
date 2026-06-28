//! Droppable single-player items for the teleport facility.
//!
//! Two tools, both *presentation-layer* (they never touch the deterministic/networked
//! match brain):
//! - the **anchor torch** — dropped in a room, it *locks that room's current threshold
//!   set*: every visible threshold keeps its destination and no new thresholds appear
//!   while the torch remains. Dropped in a hallway, it pins just that hallway edge. Pick
//!   it up and the shifting resumes.
//! - the **teleport pad** — drop two and they form a *reusable link*: step onto either and
//!   activate to travel to the other. They persist until picked up.
//!
//! This module is the pure inventory/placement state and the rules the presentation and
//! nav read (which edges are pinned, where a pad link leads). No Bevy systems, no
//! rendering — unit-testable on its own.

use bevy::math::Vec2;
use bevy::prelude::Resource;
use observed_core::RoomId;

use crate::teleport::{PinnedEdge, Place};

/// The two droppable item kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    AnchorTorch,
    TeleportPad,
}

/// An item resting in the world, bound to the place it was dropped in.
#[derive(Clone, Debug)]
pub struct PlacedItem {
    pub kind: ItemKind,
    pub place: Place,
    pub pos: Vec2,
    /// For an anchor torch: the decohere version frozen at drop time (so its corridors
    /// keep the shape they had when you anchored them).
    pub pin_version: u32,
    /// For an anchor torch: the exact room-pair relations frozen at drop time. This is
    /// the "foreign key" of the tether; it does not follow later graph reroutes.
    pub pin_edges: Vec<(RoomId, RoomId)>,
}

/// The single-player loadout plus everything dropped in the world.
#[derive(Clone, Debug, Resource)]
pub struct ItemsState {
    /// Carried (un-dropped) anchor torches.
    pub torches: u8,
    /// Carried (un-dropped) teleport pads.
    pub pads: u8,
    pub placed: Vec<PlacedItem>,
}

/// A canonical key so a place matches regardless of a hallway's re-rolled variation (an
/// item bound to an edge stays in that edge even if its corridor shape changed).
fn place_key(p: Place) -> (u8, u32, u32) {
    match p {
        Place::Room(r) => (0, r.0, r.0),
        Place::Hallway { from, to, .. } => {
            let (lo, hi) = if from.0 <= to.0 {
                (from.0, to.0)
            } else {
                (to.0, from.0)
            };
            (1, lo, hi)
        }
    }
}

fn same_place(a: Place, b: Place) -> bool {
    place_key(a) == place_key(b)
}

impl ItemsState {
    /// The single-player loadout: one anchor torch, two teleport pads.
    pub fn single_player() -> Self {
        Self {
            torches: 1,
            pads: 2,
            placed: Vec::new(),
        }
    }

    /// How many of `kind` are still carried.
    pub fn carried(&self, kind: ItemKind) -> u8 {
        match kind {
            ItemKind::AnchorTorch => self.torches,
            ItemKind::TeleportPad => self.pads,
        }
    }

    fn carried_mut(&mut self, kind: ItemKind) -> &mut u8 {
        match kind {
            ItemKind::AnchorTorch => &mut self.torches,
            ItemKind::TeleportPad => &mut self.pads,
        }
    }

    fn drop_with_edges(
        &mut self,
        kind: ItemKind,
        place: Place,
        pos: Vec2,
        version: u32,
        pin_edges: Vec<(RoomId, RoomId)>,
    ) -> bool {
        if self.carried(kind) == 0 {
            return false;
        }
        *self.carried_mut(kind) -= 1;
        self.placed.push(PlacedItem {
            kind,
            place,
            pos,
            pin_version: version,
            pin_edges,
        });
        true
    }

    /// Drop a carried item of `kind` at `place`/`pos` (`version` freezes an anchor's
    /// pin). Returns true if one was carried and dropped.
    ///
    /// Room anchor torches need the room's current connections to freeze a relation; use
    /// [`Self::drop_anchor_torch`] for gameplay. This generic helper still pins a torch
    /// dropped inside a hallway, because the hallway already names its edge.
    pub fn drop(&mut self, kind: ItemKind, place: Place, pos: Vec2, version: u32) -> bool {
        let pin_edges = match (kind, place) {
            (
                ItemKind::AnchorTorch,
                Place::Hallway {
                    from,
                    to,
                    variation: _,
                },
            ) => vec![(from, to)],
            _ => Vec::new(),
        };
        self.drop_with_edges(kind, place, pos, version, pin_edges)
    }

    /// Drop an anchor torch and freeze the exact relations it touches. A room anchor pins
    /// each incident relation supplied by the caller; a hallway anchor pins just that edge.
    pub fn drop_anchor_torch(
        &mut self,
        place: Place,
        pos: Vec2,
        version: u32,
        room_connections: &[RoomId],
    ) -> bool {
        let pin_edges = match place {
            Place::Room(room) => room_connections.iter().map(|&to| (room, to)).collect(),
            Place::Hallway { from, to, .. } => vec![(from, to)],
        };
        self.drop_with_edges(ItemKind::AnchorTorch, place, pos, version, pin_edges)
    }

    /// Pick up the nearest placed item of `kind` in `place` within `radius` of `pos`.
    /// Returns true if one was picked up (it returns to the carried count).
    pub fn pickup(&mut self, kind: ItemKind, place: Place, pos: Vec2, radius: f32) -> bool {
        let nearest = self
            .placed
            .iter()
            .enumerate()
            .filter(|(_, it)| it.kind == kind && same_place(it.place, place))
            .map(|(i, it)| (i, it.pos.distance(pos)))
            .filter(|(_, d)| *d <= radius)
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let Some((idx, _)) = nearest else {
            return false;
        };
        let item = self.placed.remove(idx);
        *self.carried_mut(item.kind) += 1;
        true
    }

    /// The placed items currently resting in `place` (for rendering).
    pub fn placed_in(&self, place: Place) -> Vec<PlacedItem> {
        self.placed
            .iter()
            .filter(|&it| same_place(it.place, place))
            .cloned()
            .collect()
    }

    /// Relations frozen by dropped anchor torches, expressed as pinned edges with their
    /// drop-time decohere version. These edges are stored at drop time and do not follow
    /// later graph reroutes.
    pub fn pins(&self) -> Vec<PinnedEdge> {
        let mut out = Vec::new();
        for it in &self.placed {
            if it.kind != ItemKind::AnchorTorch {
                continue;
            }
            for &(a, b) in &it.pin_edges {
                out.push(PinnedEdge {
                    a,
                    b,
                    version: it.pin_version,
                });
            }
        }
        out
    }

    /// Rooms that remain connected to `room` because an anchor froze that relation.
    pub fn pinned_connections(&self, room: RoomId) -> Vec<RoomId> {
        let mut out = Vec::new();
        for pin in self.pins() {
            if pin.a == room {
                out.push(pin.b);
            } else if pin.b == room {
                out.push(pin.a);
            }
        }
        out.sort_by_key(|room| room.0);
        out.dedup();
        out
    }

    /// If `room` contains an anchor torch, return the exact threshold set frozen when
    /// the room was tethered. This is stricter than [`Self::pinned_connections`]: a
    /// room-level lock is an exclusive table of thresholds, so live graph additions do
    /// not create new doorways until the room anchor is picked up.
    pub fn locked_room_connections(&self, room: RoomId) -> Option<Vec<RoomId>> {
        let mut locked = false;
        let mut out = Vec::new();
        for it in &self.placed {
            if it.kind != ItemKind::AnchorTorch || !matches!(it.place, Place::Room(r) if r == room)
            {
                continue;
            }
            locked = true;
            for &(a, b) in &it.pin_edges {
                if a == room {
                    out.push(b);
                } else if b == room {
                    out.push(a);
                }
            }
        }
        if !locked {
            return None;
        }
        out.sort_by_key(|room| room.0);
        out.dedup();
        Some(out)
    }

    /// Whether the current room-lock tables permit the relation `a <-> b`. If either
    /// endpoint room is locked, that endpoint must have frozen the other room in its
    /// threshold table; otherwise new live edges cannot point into a locked room from
    /// the outside.
    pub fn relation_allowed_by_room_locks(&self, a: RoomId, b: RoomId) -> bool {
        let allows = |room: RoomId, other: RoomId| {
            self.locked_room_connections(room)
                .is_none_or(|connections| connections.contains(&other))
        };
        allows(a, b) && allows(b, a)
    }

    /// If the player at `place`/`pos` is standing on a placed teleport pad (within
    /// `radius`) and a *second* pad is placed elsewhere, returns the place + position of
    /// that other pad — where activating the link sends you. `None` if not on a pad or
    /// fewer than two are down.
    pub fn pad_link_target(&self, place: Place, pos: Vec2, radius: f32) -> Option<(Place, Vec2)> {
        let pads: Vec<&PlacedItem> = self
            .placed
            .iter()
            .filter(|it| it.kind == ItemKind::TeleportPad)
            .collect();
        if pads.len() < 2 {
            return None;
        }
        let on = pads
            .iter()
            .position(|it| same_place(it.place, place) && it.pos.distance(pos) <= radius)?;
        let other = pads
            .iter()
            .enumerate()
            .find(|(i, _)| *i != on)
            .map(|(_, it)| (*it).clone())?;
        Some((other.place, other.pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(id: u32) -> Place {
        Place::Room(RoomId(id))
    }

    #[test]
    fn single_player_loadout_is_one_torch_two_pads() {
        let s = ItemsState::single_player();
        assert_eq!(s.carried(ItemKind::AnchorTorch), 1);
        assert_eq!(s.carried(ItemKind::TeleportPad), 2);
        assert!(s.placed.is_empty());
    }

    #[test]
    fn drop_then_pick_up_round_trips_the_inventory() {
        let mut s = ItemsState::single_player();
        let p = room(2);
        assert!(s.drop(ItemKind::AnchorTorch, p, Vec2::new(1.0, 0.0), 4));
        assert_eq!(s.carried(ItemKind::AnchorTorch), 0);
        assert_eq!(s.placed_in(p).len(), 1);
        // Can't drop a second torch (only had one).
        assert!(!s.drop(ItemKind::AnchorTorch, p, Vec2::ZERO, 4));
        // Too far to pick up.
        assert!(!s.pickup(ItemKind::AnchorTorch, p, Vec2::new(9.0, 0.0), 1.5));
        // Close enough -> back in hand.
        assert!(s.pickup(ItemKind::AnchorTorch, p, Vec2::new(1.2, 0.0), 1.5));
        assert_eq!(s.carried(ItemKind::AnchorTorch), 1);
        assert!(s.placed_in(p).is_empty());
    }

    #[test]
    fn an_item_stays_in_its_place_only() {
        let mut s = ItemsState::single_player();
        s.drop(ItemKind::TeleportPad, room(1), Vec2::ZERO, 0);
        assert_eq!(s.placed_in(room(1)).len(), 1);
        assert_eq!(s.placed_in(room(5)).len(), 0, "not visible in another room");
    }

    #[test]
    fn a_hallway_item_ignores_the_re_rolled_variation() {
        let mut s = ItemsState::single_player();
        let dropped = Place::Hallway {
            from: RoomId(1),
            to: RoomId(4),
            variation: 2,
        };
        s.drop(ItemKind::TeleportPad, dropped, Vec2::ZERO, 0);
        // The same edge with a different (re-rolled) variation still holds the item.
        let rerolled = Place::Hallway {
            from: RoomId(1),
            to: RoomId(4),
            variation: 7,
        };
        assert_eq!(s.placed_in(rerolled).len(), 1);
    }

    #[test]
    fn an_anchor_torch_in_a_room_pins_its_incident_edges() {
        let mut s = ItemsState::single_player();
        s.drop_anchor_torch(room(2), Vec2::ZERO, 5, &[RoomId(1), RoomId(3)]);
        let pins = s.pins();
        assert_eq!(pins.len(), 2);
        assert!(pins.iter().all(|p| p.version == 5 && (p.a == RoomId(2))));
        assert_eq!(s.pinned_connections(RoomId(2)), vec![RoomId(1), RoomId(3)]);
        assert_eq!(s.pinned_connections(RoomId(1)), vec![RoomId(2)]);
        // A teleport pad pins nothing.
        let mut s2 = ItemsState::single_player();
        s2.drop(ItemKind::TeleportPad, room(2), Vec2::ZERO, 5);
        assert!(s2.pins().is_empty());
    }

    #[test]
    fn a_room_anchor_keeps_its_original_relation_after_the_graph_changes() {
        let mut s = ItemsState::single_player();
        s.drop_anchor_torch(room(2), Vec2::ZERO, 5, &[RoomId(1), RoomId(3)]);
        // The live room graph may later say room 2 connects somewhere else, but the
        // anchor remains a stored relation, not a live query.
        assert_eq!(s.pinned_connections(RoomId(2)), vec![RoomId(1), RoomId(3)]);
        assert_eq!(
            s.locked_room_connections(RoomId(2)),
            Some(vec![RoomId(1), RoomId(3)])
        );
        let pins = s.pins();
        assert!(pins.iter().any(|p| p.a == RoomId(2) && p.b == RoomId(1)));
        assert!(pins.iter().any(|p| p.a == RoomId(2) && p.b == RoomId(3)));
        assert!(!pins.iter().any(|p| p.a == RoomId(2) && p.b == RoomId(8)));
    }

    #[test]
    fn only_a_room_anchor_locks_the_room_threshold_set() {
        let mut s = ItemsState::single_player();
        s.drop(
            ItemKind::AnchorTorch,
            Place::Hallway {
                from: RoomId(1),
                to: RoomId(4),
                variation: 0,
            },
            Vec2::ZERO,
            3,
        );
        assert_eq!(s.pinned_connections(RoomId(1)), vec![RoomId(4)]);
        assert_eq!(s.locked_room_connections(RoomId(1)), None);
    }

    #[test]
    fn a_locked_room_rejects_new_inbound_relations_too() {
        let mut s = ItemsState::single_player();
        s.drop_anchor_torch(room(2), Vec2::ZERO, 5, &[RoomId(1), RoomId(3)]);

        assert!(s.relation_allowed_by_room_locks(RoomId(1), RoomId(2)));
        assert!(s.relation_allowed_by_room_locks(RoomId(3), RoomId(2)));
        assert!(
            !s.relation_allowed_by_room_locks(RoomId(8), RoomId(2)),
            "an outside room cannot grow a new threshold into the locked room"
        );
    }

    #[test]
    fn an_anchor_torch_in_a_hallway_pins_just_that_edge() {
        let mut s = ItemsState::single_player();
        s.drop(
            ItemKind::AnchorTorch,
            Place::Hallway {
                from: RoomId(1),
                to: RoomId(4),
                variation: 0,
            },
            Vec2::ZERO,
            3,
        );
        let pins = s.pins();
        assert_eq!(pins.len(), 1);
        assert_eq!(
            (pins[0].a, pins[0].b, pins[0].version),
            (RoomId(1), RoomId(4), 3)
        );
    }

    #[test]
    fn two_placed_pads_link_when_you_stand_on_one() {
        let mut s = ItemsState::single_player();
        s.drop(ItemKind::TeleportPad, room(1), Vec2::new(0.0, 0.0), 0);
        // One pad down is not enough to link.
        assert!(s.pad_link_target(room(1), Vec2::ZERO, 1.2).is_none());
        s.drop(ItemKind::TeleportPad, room(5), Vec2::new(2.0, -1.0), 0);
        // Standing on the room-1 pad links to the room-5 pad.
        let target = s.pad_link_target(room(1), Vec2::new(0.2, 0.0), 1.2);
        assert_eq!(target, Some((room(5), Vec2::new(2.0, -1.0))));
        // Standing on neither pad -> no link.
        assert!(
            s.pad_link_target(room(1), Vec2::new(8.0, 0.0), 1.2)
                .is_none()
        );
        // Standing on the room-5 pad links back (reusable, bidirectional).
        let back = s.pad_link_target(room(5), Vec2::new(2.0, -1.0), 1.2);
        assert_eq!(back, Some((room(1), Vec2::new(0.0, 0.0))));
    }
}
