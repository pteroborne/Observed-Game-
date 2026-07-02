//! Phase 21: **continuous line-of-sight observation**.
//!
//! Phase 19 (`fps_observation_lab`) observed at room granularity — face an open
//! doorway and the whole room beyond froze. This promotes observation to a
//! *continuous* field: a real **frustum + occlusion** test over a fixed
//! architecture of walls with doorway gaps. Sub-room **cells** are seen or unseen
//! individually, so a room can be *partially* observed; and a doorway freezes only
//! when you can actually see it. Decoherence then re-pairs only the doorways whose
//! ends are both unseen — "freeze exactly what you see," at sub-room granularity.
//!
//! The topology (which doorway links to which) is reused from
//! [`observed_observation`]'s graph; the *architecture* (walls + gaps) is fixed and
//! defines occlusion. Everything here is pure 2D floor-plane geometry, so the
//! observed set is a deterministic function of the camera pose — the lab projects
//! it into a first-person `Camera3d`.

use bevy::math::Vec2;
use bevy::prelude::Resource;
use observed_core::{RoomId, SplitMix};
use observed_observation::{DOOR_COUNT, DoorId, ObservationWorld, ROOM_COUNT, Side};
use player_input::PlayerIntent;

pub const GRID: u32 = 3;
pub const ROOM_SIZE: f32 = 8.0;
pub const HALF: f32 = ROOM_SIZE * 0.5;
/// Cells per room axis — the sub-room resolution of the observed field.
pub const CELLS: usize = 5;
pub const CELL_COUNT: usize = ROOM_COUNT * CELLS * CELLS;
/// Half-width of the doorway gap in a wall.
pub const GAP_HALF: f32 = 1.3;
pub const FOV_HALF_DEG: f32 = 35.0;
pub const VIEW_RANGE: f32 = 2.6 * ROOM_SIZE;
pub const EYE_HEIGHT: f32 = 1.7;
pub const TURN_SPEED: f32 = 1.7;
pub const MOVE_SPEED: f32 = 3.2;

/// A wall segment on the floor plane (an occluder).
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub a: Vec2,
    pub b: Vec2,
}

/// Room centre on the floor plane (x, z). Row 0 is north (`-z`).
pub fn room_center(room: RoomId) -> Vec2 {
    let r = (room.0 / GRID) as f32;
    let c = (room.0 % GRID) as f32;
    Vec2::new((c - 1.0) * ROOM_SIZE, (r - 1.0) * ROOM_SIZE)
}

pub fn side_dir(side: Side) -> Vec2 {
    match side {
        Side::North => Vec2::new(0.0, -1.0),
        Side::East => Vec2::new(1.0, 0.0),
        Side::South => Vec2::new(0.0, 1.0),
        Side::West => Vec2::new(-1.0, 0.0),
    }
}

fn tangent(side: Side) -> Vec2 {
    let d = side_dir(side);
    Vec2::new(-d.y, d.x)
}

pub fn door_pos(room: RoomId, side: Side) -> Vec2 {
    room_center(room) + side_dir(side) * HALF
}

/// Does `room` have a neighbour across `side` (i.e. an interior doorway)?
pub fn is_interior(room: RoomId, side: Side) -> bool {
    let r = room.0 / GRID;
    let c = room.0 % GRID;
    match side {
        Side::North => r > 0,
        Side::South => r + 1 < GRID,
        Side::West => c > 0,
        Side::East => c + 1 < GRID,
    }
}

fn cell_offset(i: usize) -> f32 {
    (i as f32 + 0.5) * (ROOM_SIZE / CELLS as f32) - HALF
}

pub fn cell_center(room: RoomId, ci: usize, cj: usize) -> Vec2 {
    room_center(room) + Vec2::new(cell_offset(ci), cell_offset(cj))
}

pub fn cell_index(room: RoomId, ci: usize, cj: usize) -> usize {
    room.0 as usize * CELLS * CELLS + cj * CELLS + ci
}

pub fn forward(yaw: f32) -> Vec2 {
    Vec2::new(yaw.sin(), -yaw.cos())
}

/// The fixed wall architecture: every room side is a wall, with a central gap on
/// interior sides (doorways) and solid on the boundary.
pub fn walls() -> Vec<Seg> {
    let mut out = Vec::new();
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        for side in Side::ALL {
            let c = door_pos(room, side);
            let t = tangent(side);
            let a = c - t * HALF;
            let b = c + t * HALF;
            if is_interior(room, side) {
                out.push(Seg {
                    a,
                    b: c - t * GAP_HALF,
                });
                out.push(Seg {
                    a: c + t * GAP_HALF,
                    b,
                });
            } else {
                out.push(Seg { a, b });
            }
        }
    }
    out
}

/// Do open segments `p1p2` and `p3p4` cross (strictly, not just touch endpoints)?
fn segments_cross(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    let r = p2 - p1;
    let s = p4 - p3;
    let denom = r.perp_dot(s);
    if denom.abs() < 1e-6 {
        return false;
    }
    let t = (p3 - p1).perp_dot(s) / denom;
    let u = (p3 - p1).perp_dot(r) / denom;
    (1e-4..1.0 - 1e-4).contains(&t) && (1e-4..1.0 - 1e-4).contains(&u)
}

/// Is point `p` within the camera's frustum (range + FOV) and not occluded?
pub fn visible(eye: Vec2, fwd: Vec2, p: Vec2, walls: &[Seg]) -> bool {
    let to = p - eye;
    let dist = to.length();
    if dist < 1e-5 {
        return true;
    }
    if dist > VIEW_RANGE {
        return false;
    }
    let cos_half = FOV_HALF_DEG.to_radians().cos();
    if (to / dist).dot(fwd) < cos_half {
        return false;
    }
    walls.iter().all(|w| !segments_cross(eye, p, w.a, w.b))
}

// Replaced duplicate SplitMix with shared observed_core::SplitMix

#[derive(Resource, Clone, Debug)]
pub struct VisionField {
    /// Reused topology: which doorway currently links to which.
    pub graph: ObservationWorld,
    pub eye: Vec2,
    pub yaw: f32,
    pub seed: u64,
    pub decohere_count: u32,
    /// Per-cell visibility (length `CELL_COUNT`).
    pub seen_cells: Vec<bool>,
    /// Per-door direct visibility (length `DOOR_COUNT`).
    pub seen_doors: Vec<bool>,
    pub last_event: String,
}

impl VisionField {
    pub fn authored() -> Self {
        let mut field = Self {
            graph: ObservationWorld::authored(),
            eye: room_center(RoomId(4)),
            yaw: std::f32::consts::FRAC_PI_2, // look east, into room 5
            seed: 0x0B5E_47ED_5EE0_1234,
            decohere_count: 0,
            seen_cells: vec![false; CELL_COUNT],
            seen_doors: vec![false; DOOR_COUNT],
            last_event: "Only what you can see freezes; the rest rewires.".to_string(),
        };
        field.recompute();
        field
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    /// Recompute the continuous observed field from the current camera pose.
    pub fn recompute(&mut self) {
        let walls = walls();
        let fwd = forward(self.yaw);

        for index in 0..ROOM_COUNT {
            let room = RoomId(index as u32);
            for cj in 0..CELLS {
                for ci in 0..CELLS {
                    let seen = visible(self.eye, fwd, cell_center(room, ci, cj), &walls);
                    self.seen_cells[cell_index(room, ci, cj)] = seen;
                }
            }
        }

        for index in 0..DOOR_COUNT {
            let door = DoorId(index as u16);
            let d = self.graph.door(door);
            self.seen_doors[index] = is_interior(d.room, d.side)
                && visible(self.eye, fwd, door_pos(d.room, d.side), &walls);
        }
    }

    /// Advance the observer deterministically from an abstract input intent.
    pub fn advance_camera(&mut self, intent: PlayerIntent, dt: f32) {
        self.yaw += intent.look.x * TURN_SPEED * dt;
        let fwd = forward(self.yaw);
        let right = Vec2::new(-fwd.y, fwd.x);
        self.eye += (right * intent.movement.x + fwd * intent.movement.y) * MOVE_SPEED * dt;

        // Phase 21 isolates sight rather than collision. Keep the observer within
        // the centre room so walls and doorway gaps remain the visibility variables.
        let centre = room_center(RoomId(4));
        let margin = 0.35;
        self.eye.x = self
            .eye
            .x
            .clamp(centre.x - HALF + margin, centre.x + HALF - margin);
        self.eye.y = self
            .eye
            .y
            .clamp(centre.y - HALF + margin, centre.y + HALF - margin);
        self.recompute();
    }

    /// A doorway is frozen when either of its ends is seen (observing one end
    /// collapses the connection).
    pub fn frozen(&self, door: DoorId) -> bool {
        self.seen_doors[door.0 as usize] || self.seen_doors[self.graph.partner(door).0 as usize]
    }

    pub fn seen_cell_count(&self) -> usize {
        self.seen_cells.iter().filter(|s| **s).count()
    }

    pub fn seen_door_count(&self) -> usize {
        (0..DOOR_COUNT).filter(|i| self.seen_doors[*i]).count()
    }

    pub fn room_seen_cell_count(&self, room: RoomId) -> usize {
        let base = room.0 as usize * CELLS * CELLS;
        self.seen_cells[base..base + CELLS * CELLS]
            .iter()
            .filter(|seen| **seen)
            .count()
    }

    pub fn room_coverage(&self, room: RoomId) -> f32 {
        self.room_seen_cell_count(room) as f32 / (CELLS * CELLS) as f32
    }

    /// Rooms that are observed in part but not whole — the sub-room signature.
    pub fn partially_seen_rooms(&self) -> usize {
        (0..ROOM_COUNT)
            .filter(|index| {
                let seen = self.room_seen_cell_count(RoomId(*index as u32));
                seen > 0 && seen < CELLS * CELLS
            })
            .count()
    }

    pub fn interior_openings() -> Vec<DoorId> {
        (0..DOOR_COUNT)
            .map(|i| DoorId(i as u16))
            .filter(|d| {
                let room = RoomId((d.0 as u32) / 4);
                let side = Side::ALL[(d.0 as usize) % 4];
                is_interior(room, side)
            })
            .collect()
    }

    /// Re-pair only the doorways whose ends are both unseen; seen doorways and all
    /// boundary walls are left exactly as they are.
    pub fn decohere(&mut self) {
        self.decohere_count += 1;
        let before = self.graph.links.clone();

        let mut free: Vec<DoorId> = Self::interior_openings()
            .into_iter()
            .filter(|d| !self.frozen(*d))
            .collect();

        let mut rng =
            SplitMix(self.seed ^ (self.decohere_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for i in (1..free.len()).rev() {
            free.swap(i, rng.below(i + 1));
        }

        for pair in free.chunks_exact(2) {
            let (a, b) = (pair[0], pair[1]);
            self.graph.links[a.0 as usize] = b;
            self.graph.links[b.0 as usize] = a;
        }

        let rewired = (0..self.graph.links.len())
            .filter(|i| self.graph.links[*i] != before[*i])
            .count();
        self.last_event = format!(
            "Decohered: {rewired} doorways re-paired; {} seen doorways held.",
            self.seen_door_count()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn looking(eye: RoomId, yaw: f32) -> VisionField {
        let mut f = VisionField::authored();
        f.eye = room_center(eye);
        f.yaw = yaw;
        f.recompute();
        f
    }

    const EAST: f32 = std::f32::consts::FRAC_PI_2;
    const WEST: f32 = std::f32::consts::PI + std::f32::consts::FRAC_PI_2;

    #[test]
    fn the_observation_is_sub_room_and_partial() {
        // Standing in room 4 looking east: room 4 is largely seen, and room 5 (east)
        // is seen only in part through the doorway — not the whole room.
        let f = looking(RoomId(4), EAST);
        let room5_seen = (0..CELLS)
            .flat_map(|cj| (0..CELLS).map(move |ci| (ci, cj)))
            .filter(|(ci, cj)| f.seen_cells[cell_index(RoomId(5), *ci, *cj)])
            .count();
        assert!(
            room5_seen > 0,
            "some of the next room is visible through the gap"
        );
        assert!(
            room5_seen < CELLS * CELLS,
            "but not the whole room — partial"
        );
        assert!(f.partially_seen_rooms() >= 1);
    }

    #[test]
    fn occlusion_hides_what_a_wall_blocks() {
        // Room 3 (west of room 4) is behind room 4's west wall; looking east, its
        // cells must be unseen.
        let f = looking(RoomId(4), EAST);
        let any_room3 = (0..CELLS)
            .flat_map(|cj| (0..CELLS).map(move |ci| (ci, cj)))
            .any(|(ci, cj)| f.seen_cells[cell_index(RoomId(3), ci, cj)]);
        assert!(!any_room3, "a room behind the camera/wall is not observed");
    }

    #[test]
    fn looking_away_changes_the_observed_set() {
        let east = looking(RoomId(4), EAST).seen_cell_count();
        let west = looking(RoomId(4), WEST).seen_cell_count();
        // The two views observe different cells (the set tracks the camera).
        let e = looking(RoomId(4), EAST);
        let w = looking(RoomId(4), WEST);
        assert_ne!(e.seen_cells, w.seen_cells, "turning changes what is seen");
        assert!(east > 0 && west > 0);
    }

    #[test]
    fn nearby_camera_angles_update_incrementally() {
        let mut previous = looking(RoomId(4), EAST);
        for step in 1..=12 {
            let current = looking(RoomId(4), EAST + step as f32 * 0.01);
            let changed = current
                .seen_cells
                .iter()
                .zip(&previous.seen_cells)
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                changed <= CELLS * CELLS,
                "a small turn should update the sampled field incrementally"
            );
            previous = current;
        }
    }

    #[test]
    fn the_same_camera_intents_reproduce_the_same_visibility_sequence() {
        let tape = [
            PlayerIntent {
                look: Vec2::new(0.4, 0.0),
                movement: Vec2::new(0.0, 0.7),
                ..Default::default()
            },
            PlayerIntent {
                look: Vec2::new(-0.2, 0.0),
                movement: Vec2::new(0.5, 0.2),
                ..Default::default()
            },
        ];
        let mut a = VisionField::authored();
        let mut b = VisionField::authored();
        for _ in 0..90 {
            for intent in tape {
                a.advance_camera(intent, 1.0 / 60.0);
                b.advance_camera(intent, 1.0 / 60.0);
                assert_eq!(a.eye, b.eye);
                assert_eq!(a.yaw, b.yaw);
                assert_eq!(a.seen_cells, b.seen_cells);
                assert_eq!(a.seen_doors, b.seen_doors);
            }
        }
    }

    #[test]
    fn a_door_freezes_only_when_it_can_be_seen() {
        let f = looking(RoomId(4), EAST);
        // Room 4's east doorway is faced → seen/frozen. Its west doorway is behind →
        // not seen.
        let east_door = f.graph.door_id(RoomId(4), Side::East);
        let west_door = f.graph.door_id(RoomId(4), Side::West);
        assert!(f.seen_doors[east_door.0 as usize], "faced doorway is seen");
        assert!(
            !f.seen_doors[west_door.0 as usize],
            "doorway behind is not seen"
        );
    }

    #[test]
    fn decoherence_only_repairs_unseen_doorways() {
        let mut f = looking(RoomId(4), EAST);
        // Record the partners of every currently-frozen doorway.
        let frozen: Vec<(DoorId, DoorId)> = (0..DOOR_COUNT)
            .map(|i| DoorId(i as u16))
            .filter(|d| f.frozen(*d))
            .map(|d| (d, f.graph.partner(d)))
            .collect();
        assert!(!frozen.is_empty(), "something is seen and thus frozen");

        let before = f.graph.links.clone();
        let mut changed = false;
        for _ in 0..4 {
            f.decohere();
            if f.graph.links != before {
                changed = true;
            }
            // Frozen doorways keep their partner every time.
            for (door, partner) in &frozen {
                assert_eq!(f.graph.partner(*door), *partner, "seen doorway must hold");
            }
        }
        assert!(changed, "unseen doorways must rewire");

        // The architecture is intact: boundary walls sealed, interior still open,
        // matching still a valid involution.
        for index in 0..DOOR_COUNT {
            let door = DoorId(index as u16);
            let d = f.graph.door(door);
            if is_interior(d.room, d.side) {
                assert!(!f.graph.is_sealed(door), "interior doorway stays open");
            } else {
                assert!(f.graph.is_sealed(door), "boundary wall stays sealed");
            }
            assert_eq!(
                f.graph.partner(f.graph.partner(door)),
                door,
                "valid matching"
            );
        }
    }

    #[test]
    fn decoherence_is_deterministic() {
        let mut a = looking(RoomId(4), EAST);
        let mut b = looking(RoomId(4), EAST);
        for _ in 0..6 {
            a.decohere();
            b.decohere();
        }
        assert_eq!(a.graph.links, b.graph.links);
    }

    #[test]
    fn visibility_is_deterministic() {
        let a = looking(RoomId(0), EAST);
        let b = looking(RoomId(0), EAST);
        assert_eq!(a.seen_cells, b.seen_cells);
        assert_eq!(a.seen_doors, b.seen_doors);
    }

    #[test]
    fn reset_restores_the_authored_field() {
        let mut f = looking(RoomId(0), WEST);
        for _ in 0..5 {
            f.decohere();
        }
        f.reset();
        assert_eq!(f.decohere_count, 0);
        assert_eq!(f.eye, room_center(RoomId(4)));
    }
}
