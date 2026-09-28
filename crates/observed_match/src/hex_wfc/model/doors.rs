//! Doors on the facility's thresholds, as an Architect deploys them.
//!
//! The Ascent rules own every door: where it is, whether it is open, who may work it, and
//! when a rewritten or retracted cell consumes it (design section 4). This is where one
//! stands in the world. The rules hand the whole set over each tick
//! ([`HexWfcMatch::set_doors`]), and a closed door is a panel across its doorway: a
//! collider bodies, minors and a push cannot pass, and a line no Guardian hunts, catches
//! or is seen across. An open door is its frame and nothing else.
//!
//! A door fills the corpus's doorway, which every tile shares: 4.5 m wide and 4 m from the
//! floor to the lintel (`observed_authoring::forge::geometry`), at the middle of the face
//! the two cells share.

use glam::{Quat, Vec3};
use observed_core::PlayerId;
use observed_hex::{CORNERS, FLOOR_SLAB_TOP, HexCoord, HexFace, hex_origin};
use observed_traversal::{ColliderDelta, ColliderShape, ColliderSpec, StableColliderId};

use super::HexWfcMatch;

/// Half the width of a doorway, metres.
pub const DOOR_HALF_WIDTH: f32 = 2.25;
/// A doorway's clear height, floor to lintel, metres.
pub const DOOR_HEIGHT: f32 = 4.0;
/// A closed door's panel, metres through.
const DOOR_THICKNESS: f32 = 0.2;
/// How far from a door's middle, across the floor, a body works it.
pub const DOOR_REACH: f32 = 2.5;
/// How far above a door's floor a body's centre may be and still reach it.
const DOOR_REACH_HEIGHT: f32 = 2.5;
/// Door colliders' ids, clear of every cell's (`geometry::RESERVED_ID_BASE`).
const DOOR_COLLIDER_BASE: u32 = 0xF100_0000;

/// A deployed door.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HexDoor {
    /// The threshold, as the rules key it: `face` of `cell`.
    pub cell: HexCoord,
    pub face: HexFace,
    pub closed: bool,
    collider: StableColliderId,
}

/// Where a door on `face` of `cell` stands: the middle of its doorway on the floor, and
/// the unit direction along it.
#[must_use]
pub fn door_pose(cell: HexCoord, face: HexFace) -> (Vec3, Vec3) {
    let index = face.index();
    let corner = |i: usize| {
        let (x, z) = CORNERS[i % 6];
        Vec3::new(x as f32, 0.0, z as f32)
    };
    let (a, b) = (corner(index), corner(index + 1));
    let floor = Vec3::from_array(hex_origin(cell)) + Vec3::Y * FLOOR_SLAB_TOP;
    (floor + (a + b) * 0.5, (b - a).normalize())
}

impl HexDoor {
    /// The middle of its doorway on the floor, and the direction along it.
    #[must_use]
    pub fn pose(&self) -> (Vec3, Vec3) {
        door_pose(self.cell, self.face)
    }

    /// Whether it stands between cells `a` and `b`.
    fn joins(&self, a: HexCoord, b: HexCoord, grid: observed_hex::HexGridSize) -> bool {
        let other = grid.neighbor(self.cell, self.face);
        (a == self.cell && other == Some(b)) || (b == self.cell && other == Some(a))
    }

    fn panel(&self) -> ColliderSpec {
        let (floor, along) = self.pose();
        ColliderSpec {
            id: self.collider,
            center: floor + Vec3::Y * (DOOR_HEIGHT * 0.5),
            rotation: Quat::from_rotation_arc(Vec3::X, along).to_array(),
            shape: ColliderShape::Cuboid {
                half: Vec3::new(DOOR_HALF_WIDTH, DOOR_HEIGHT * 0.5, DOOR_THICKNESS * 0.5),
            },
            friction: 0.8,
        }
    }
}

impl HexWfcMatch {
    /// Every deployed door.
    pub fn doors(&self) -> impl Iterator<Item = &HexDoor> {
        self.doors.values()
    }

    /// Make the deployed doors exactly `wanted`: each threshold (`face` of `cell`, as the
    /// rules key it) and whether its door is closed. A closed door's panel is a collider; an
    /// open one's is not; a door no longer wanted is gone.
    pub fn set_doors(&mut self, wanted: impl IntoIterator<Item = ((HexCoord, HexFace), bool)>) {
        let wanted: std::collections::BTreeMap<_, _> = wanted.into_iter().collect();
        let mut delta = ColliderDelta::default();
        let gone: Vec<_> = self
            .doors
            .keys()
            .filter(|key| !wanted.contains_key(key))
            .copied()
            .collect();
        for key in gone {
            let door = self.doors.remove(&key).expect("listed");
            if door.closed {
                delta.removed.insert(door.collider);
            }
        }
        for (&(cell, face), &closed) in &wanted {
            let door = match self.doors.get(&(cell, face)) {
                Some(door) if door.closed == closed => continue,
                Some(door) => HexDoor { closed, ..*door },
                None => {
                    let collider = StableColliderId(DOOR_COLLIDER_BASE + self.next_door_collider);
                    self.next_door_collider += 1;
                    HexDoor {
                        cell,
                        face,
                        closed,
                        collider,
                    }
                }
            };
            if closed {
                delta.upserted.push(door.panel());
            } else if self.doors.get(&(cell, face)).is_some_and(|was| was.closed) {
                delta.removed.insert(door.collider);
            }
            self.doors.insert((cell, face), door);
        }
        if !delta.removed.is_empty() || !delta.upserted.is_empty() {
            self.physics
                .apply_collider_delta(&delta)
                .expect("a door's panel is a valid collider with an id of its own");
        }
    }

    /// Whether a closed door stands between neighbouring cells `a` and `b`.
    #[must_use]
    pub fn closed_door_between(&self, a: HexCoord, b: HexCoord) -> bool {
        closed_between(&self.doors, self.facility.config.grid(), a, b)
    }

    /// The deployed door nearest `player`'s body within [`DOOR_REACH`], if any.
    #[must_use]
    pub fn door_in_reach(&self, player: PlayerId) -> Option<&HexDoor> {
        let body = self
            .players
            .get(&player)
            .filter(|body| body.in_facility())?;
        self.doors
            .values()
            .filter_map(|door| {
                let (floor, _) = door.pose();
                let rise = body.position.y - floor.y;
                let across = (body.position - floor).with_y(0.0).length();
                ((0.0..=DOOR_REACH_HEIGHT).contains(&rise) && across <= DOOR_REACH)
                    .then_some((across, door))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, door)| door)
    }

    /// Whether `player`'s body is walking into a closed door within reach: one it faces.
    /// A bot opens such a door rather than stand against it.
    #[must_use]
    pub(super) fn walking_into_closed_door(&self, player: PlayerId) -> bool {
        let (Some(door), Some(body)) = (self.door_in_reach(player), self.bodies.get(&player))
        else {
            return false;
        };
        let (floor, _) = door.pose();
        let toward = (floor - body.position).with_y(0.0).normalize_or_zero();
        door.closed && toward.dot(body.look_dir().with_y(0.0).normalize_or_zero()) > 0.5
    }
}

/// Whether one of `doors` stands closed between neighbouring cells `a` and `b`: for callers
/// that hold the match's other fields mutably.
pub(super) fn closed_between(
    doors: &std::collections::BTreeMap<(HexCoord, HexFace), HexDoor>,
    grid: observed_hex::HexGridSize,
    a: HexCoord,
    b: HexCoord,
) -> bool {
    a != b
        && doors
            .values()
            .any(|door| door.closed && door.joins(a, b, grid))
}

#[cfg(test)]
mod tests;
