//! The **authoritative facility**: four rooms divided by a cross of walls, joined
//! by four doors. This module is the single source of truth for connectivity. It
//! has no dependency on `vleue_navigator`; the navmesh ([`crate::nav`]) is a pure
//! *derived consumer* that reads this and never feeds back.
//!
//! Layout (floor-plane units, `y` increasing "south"):
//!
//! ```text
//!        x=0      14 16        30
//!   y=0  +---------#  #---------+
//!        |    A    #  #    B    |   A=RoomId(0)  B=RoomId(1)
//!        |        [AB gap]      |   doors are gaps in the wall cross
//!   14   #####[AC]## ##[BD]######
//!   16   #####    ## ##    ######
//!        |    C    #  #    D    |   C=RoomId(2)  D=RoomId(3)
//!        |        [CD gap]      |
//!   30   +---------#  #---------+
//! ```
//!
//! The room graph is a 4-cycle: `A-B`, `A-C`, `B-D`, `C-D`. There are two routes
//! from `A` to `D` (`A-B-D` and `A-C-D`), so closing one door forces the detour
//! and closing both of a room's doors isolates it — which is exactly what makes
//! "does navigation respect closed doors?" a meaningful question.

use std::collections::VecDeque;

use bevy::math::Vec2;
use bevy::prelude::Resource;
use observed_core::RoomId;

/// Lab-local door identifier. Kept local (not promoted to `observed_core`) until a
/// second consumer shares it, per the workspace ID rules.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DoorId(pub u8);

pub const ROOM_COUNT: usize = 4;
pub const DOOR_COUNT: usize = 4;

/// Facility extent in floor units.
pub const FACILITY_W: f32 = 30.0;
pub const FACILITY_H: f32 = 30.0;

/// Walls that meet the perimeter are extended this far past it so the constrained
/// triangulation never has to resolve a wall edge that is exactly collinear with
/// the outer boundary (which can produce degenerate zero-area slivers). Geometry
/// outside the boundary is simply clipped by the navmesh builder.
const EXT: f32 = 2.0;

/// An axis-aligned rectangle `[min, max]` in floor space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            min: Vec2::new(x0, y0),
            max: Vec2::new(x1, y1),
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// Inclusive containment so a point lying exactly on a shared room/divider edge
    /// still classifies into the room it borders.
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Counter-clockwise corner ring (open: first point not repeated), the form the
    /// navmesh builder expects for both outer edges and obstacle polygons.
    pub fn corners(&self) -> Vec<Vec2> {
        vec![
            Vec2::new(self.min.x, self.min.y),
            Vec2::new(self.max.x, self.min.y),
            Vec2::new(self.max.x, self.max.y),
            Vec2::new(self.min.x, self.max.y),
        ]
    }

    /// Clamp to the facility extent (for rendering walls that were extended past
    /// the perimeter for triangulation robustness).
    pub fn clamped(&self) -> Rect {
        Rect {
            min: self.min.max(Vec2::ZERO),
            max: self.max.min(Vec2::new(FACILITY_W, FACILITY_H)),
        }
    }
}

/// The four room label regions. These are used only to *place* centers/labels and
/// to *classify* a point into a room for cross-checking — never to build the
/// navmesh (which is boundary + obstacles).
pub fn room_rect(room: RoomId) -> Rect {
    match room.0 {
        0 => Rect::new(0.0, 0.0, 14.0, 14.0),   // A (NW)
        1 => Rect::new(16.0, 0.0, 30.0, 14.0),  // B (NE)
        2 => Rect::new(0.0, 16.0, 14.0, 30.0),  // C (SW)
        3 => Rect::new(16.0, 16.0, 30.0, 30.0), // D (SE)
        _ => panic!("room out of range: {room:?}"),
    }
}

pub fn room_center(room: RoomId) -> Vec2 {
    room_rect(room).center()
}

pub fn room_label(room: RoomId) -> char {
    (b'A' + room.0 as u8) as char
}

pub fn all_rooms() -> [RoomId; ROOM_COUNT] {
    [RoomId(0), RoomId(1), RoomId(2), RoomId(3)]
}

pub fn all_doors() -> [DoorId; DOOR_COUNT] {
    [DoorId(0), DoorId(1), DoorId(2), DoorId(3)]
}

/// The two rooms a door joins.
pub fn door_rooms(door: DoorId) -> (RoomId, RoomId) {
    match door.0 {
        0 => (RoomId(0), RoomId(1)), // AB
        1 => (RoomId(0), RoomId(2)), // AC
        2 => (RoomId(1), RoomId(3)), // BD
        3 => (RoomId(2), RoomId(3)), // CD
        _ => panic!("door out of range: {door:?}"),
    }
}

pub fn door_label(door: DoorId) -> &'static str {
    match door.0 {
        0 => "AB",
        1 => "AC",
        2 => "BD",
        3 => "CD",
        _ => panic!("door out of range: {door:?}"),
    }
}

/// The walkable opening a door occupies when open (and the exact plug added when it
/// is closed). It sits on the wall cross, between the two rooms.
pub fn door_gap(door: DoorId) -> Rect {
    match door.0 {
        0 => Rect::new(14.0, 5.0, 16.0, 9.0),   // AB: vertical divider
        1 => Rect::new(5.0, 14.0, 9.0, 16.0),   // AC: horizontal divider
        2 => Rect::new(21.0, 14.0, 25.0, 16.0), // BD: horizontal divider
        3 => Rect::new(14.0, 21.0, 16.0, 25.0), // CD: vertical divider
        _ => panic!("door out of range: {door:?}"),
    }
}

/// The permanent walls: the dividing cross minus the four door gaps. Walls that
/// touch the perimeter are extended past it (see [`EXT`]). These are always part
/// of the navmesh obstacle set regardless of door state.
pub fn base_walls() -> Vec<Rect> {
    vec![
        // Vertical divider x[14,16], split by the AB gap (y5..9) and CD gap (y21..25).
        Rect::new(14.0, -EXT, 16.0, 5.0),
        Rect::new(14.0, 9.0, 16.0, 21.0),
        Rect::new(14.0, 25.0, 16.0, FACILITY_H + EXT),
        // Horizontal divider y[14,16], split by the AC gap (x5..9) and BD gap (x21..25).
        Rect::new(-EXT, 14.0, 5.0, 16.0),
        Rect::new(9.0, 14.0, 21.0, 16.0),
        Rect::new(25.0, 14.0, FACILITY_W + EXT, 16.0),
    ]
}

/// CCW outer boundary of the facility (the perimeter wall).
pub fn outer_boundary() -> Vec<Vec2> {
    Rect::new(0.0, 0.0, FACILITY_W, FACILITY_H).corners()
}

/// Classify a floor point into the room that contains it, if any. Points on the
/// dividers/doorways belong to no room.
pub fn room_at(p: Vec2) -> Option<RoomId> {
    all_rooms().into_iter().find(|&r| room_rect(r).contains(p))
}

/// The authoritative facility state: which doors are open. Connectivity is decided
/// here, by graph search over open doors.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Facility {
    open: [bool; DOOR_COUNT],
}

impl Default for Facility {
    fn default() -> Self {
        Self::all_open()
    }
}

impl Facility {
    pub fn all_open() -> Self {
        Self {
            open: [true; DOOR_COUNT],
        }
    }

    pub fn is_open(&self, door: DoorId) -> bool {
        self.open[door.0 as usize]
    }

    pub fn set_open(&mut self, door: DoorId, open: bool) {
        self.open[door.0 as usize] = open;
    }

    pub fn toggle(&mut self, door: DoorId) {
        let i = door.0 as usize;
        self.open[i] = !self.open[i];
    }

    pub fn open_count(&self) -> usize {
        self.open.iter().filter(|o| **o).count()
    }

    /// The single door joining two rooms, if they are adjacent in the layout.
    pub fn door_between(&self, a: RoomId, b: RoomId) -> Option<DoorId> {
        all_doors().into_iter().find(|&d| {
            let (x, y) = door_rooms(d);
            (x == a && y == b) || (x == b && y == a)
        })
    }

    /// Rooms reachable in one step from `room` over *open* doors.
    pub fn neighbours(&self, room: RoomId) -> Vec<RoomId> {
        all_doors()
            .into_iter()
            .filter(|&d| self.is_open(d))
            .filter_map(|d| {
                let (a, b) = door_rooms(d);
                if a == room {
                    Some(b)
                } else if b == room {
                    Some(a)
                } else {
                    None
                }
            })
            .collect()
    }

    /// The authoritative answer: is `to` reachable from `from` over open doors?
    pub fn graph_reachable(&self, from: RoomId, to: RoomId) -> bool {
        self.graph_route(from, to).is_some()
    }

    /// Shortest room walk (in rooms) from `from` to `to` over open doors, or `None`
    /// if disconnected. Deterministic: neighbours are visited in `RoomId` order.
    pub fn graph_route(&self, from: RoomId, to: RoomId) -> Option<Vec<RoomId>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut prev: [Option<RoomId>; ROOM_COUNT] = [None; ROOM_COUNT];
        let mut seen = [false; ROOM_COUNT];
        let mut queue = VecDeque::new();
        seen[from.0 as usize] = true;
        queue.push_back(from);
        while let Some(room) = queue.pop_front() {
            if room == to {
                let mut path = vec![to];
                let mut cur = to;
                while cur != from {
                    cur = prev[cur.0 as usize].expect("BFS parent");
                    path.push(cur);
                }
                path.reverse();
                return Some(path);
            }
            let mut neighbours = self.neighbours(room);
            neighbours.sort();
            for next in neighbours {
                if !seen[next.0 as usize] {
                    seen[next.0 as usize] = true;
                    prev[next.0 as usize] = Some(room);
                    queue.push_back(next);
                }
            }
        }
        None
    }

    /// The obstacle set for the derived navmesh: the permanent walls plus a plug
    /// filling the gap of every *closed* door.
    pub fn obstacles(&self) -> Vec<Rect> {
        let mut obstacles = base_walls();
        for door in all_doors() {
            if !self.is_open(door) {
                obstacles.push(door_gap(door));
            }
        }
        obstacles
    }

    /// Validate a derived room walk against this authoritative graph: it must start
    /// at `from`, end at `to`, and only step between rooms joined by an *open*
    /// door. A walk that crossed a closed door (or a non-existent connection) is
    /// rejected — this is how the lab proves the navmesh respects door state.
    pub fn open_walk_valid(&self, rooms: &[RoomId], from: RoomId, to: RoomId) -> bool {
        if rooms.first() != Some(&from) || rooms.last() != Some(&to) {
            return false;
        }
        rooms.windows(2).all(|pair| {
            self.door_between(pair[0], pair[1])
                .is_some_and(|d| self.is_open(d))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_do_not_overlap_and_centers_classify_back() {
        for room in all_rooms() {
            assert_eq!(room_at(room_center(room)), Some(room));
        }
        for i in 0..ROOM_COUNT {
            for j in (i + 1)..ROOM_COUNT {
                let a = room_rect(RoomId(i as u32));
                let b = room_rect(RoomId(j as u32));
                let disjoint = a.max.x <= b.min.x
                    || b.max.x <= a.min.x
                    || a.max.y <= b.min.y
                    || b.max.y <= a.min.y;
                assert!(disjoint, "rooms {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn door_gaps_lie_between_their_two_rooms() {
        for door in all_doors() {
            let (a, b) = door_rooms(door);
            let gap = door_gap(door).center();
            // The gap sits on a divider, so it is in neither room interior, but it
            // is adjacent to both room rects (within one unit).
            let near = |r: RoomId| {
                let rect = room_rect(r);
                gap.x >= rect.min.x - 2.0
                    && gap.x <= rect.max.x + 2.0
                    && gap.y >= rect.min.y - 2.0
                    && gap.y <= rect.max.y + 2.0
            };
            assert!(
                near(a) && near(b),
                "{} gap not between rooms",
                door_label(door)
            );
        }
    }

    #[test]
    fn graph_is_a_four_cycle_when_all_open() {
        let f = Facility::all_open();
        // A connects to B and C, not D.
        assert!(f.graph_reachable(RoomId(0), RoomId(1)));
        assert!(f.graph_reachable(RoomId(0), RoomId(2)));
        assert!(f.graph_reachable(RoomId(0), RoomId(3)));
        // Direct A-D is two hops.
        let route = f.graph_route(RoomId(0), RoomId(3)).unwrap();
        assert_eq!(route.len(), 3);
        assert_eq!(route[0], RoomId(0));
        assert_eq!(route[2], RoomId(3));
    }

    #[test]
    fn closing_a_door_forces_the_detour() {
        let mut f = Facility::all_open();
        f.set_open(DoorId(0), false); // close AB
        // A can no longer reach B directly; only route to D is via C.
        let route = f.graph_route(RoomId(0), RoomId(3)).unwrap();
        assert_eq!(route, vec![RoomId(0), RoomId(2), RoomId(3)]);
        assert!(f.open_walk_valid(&route, RoomId(0), RoomId(3)));
    }

    #[test]
    fn isolating_a_room_disconnects_it() {
        let mut f = Facility::all_open();
        f.set_open(DoorId(0), false); // AB
        f.set_open(DoorId(1), false); // AC
        // A's only two doors are shut: it reaches nothing else.
        for other in [RoomId(1), RoomId(2), RoomId(3)] {
            assert!(!f.graph_reachable(RoomId(0), other));
        }
        assert!(f.graph_reachable(RoomId(0), RoomId(0)));
    }

    #[test]
    fn obstacle_count_grows_by_one_per_closed_door() {
        let mut f = Facility::all_open();
        assert_eq!(f.obstacles().len(), base_walls().len());
        f.toggle(DoorId(0));
        f.toggle(DoorId(3));
        assert_eq!(f.obstacles().len(), base_walls().len() + 2);
    }

    #[test]
    fn open_walk_rejects_a_closed_door_step() {
        let mut f = Facility::all_open();
        f.set_open(DoorId(0), false); // AB closed
        // A direct A-B walk is invalid now even though the rooms are adjacent.
        assert!(!f.open_walk_valid(&[RoomId(0), RoomId(1)], RoomId(0), RoomId(1)));
        // The detour through C is valid.
        assert!(f.open_walk_valid(&[RoomId(0), RoomId(2), RoomId(3)], RoomId(0), RoomId(3)));
    }
}
