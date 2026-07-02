use bevy::prelude::Resource;
use observed_core::RoomId;
use observed_observation::{ObservationWorld, ROOM_COUNT, Side};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianState {
    Active,
    FrozenByPlayer,
    FrozenByAnchor,
}

#[derive(Resource, Clone, Debug)]
pub struct Guardian {
    pub room: RoomId,
    pub target_player: usize,
    pub anchor_timer: f32, // ticks down from 30.0 when in an anchor room
}

impl Default for Guardian {
    fn default() -> Self {
        Self {
            room: RoomId(8), // Starts in room 8 (bottom right in 3x3)
            target_player: 0,
            anchor_timer: 30.0,
        }
    }
}

/// A simple deterministic PRNG (SplitMix64) to choose a random room for banishment.
#[derive(Resource)]
pub struct SimpleRng(u64);

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_room(&mut self, exclude: RoomId) -> RoomId {
        loop {
            let r = (self.next_u64() % ROOM_COUNT as u64) as u32;
            if r != exclude.0 {
                return RoomId(r);
            }
        }
    }
}

/// Compute the visible rooms from a player's room and facing direction.
/// Follows active (non-sealed) connections up to 2 hops.
pub fn visible_rooms_from_view(
    world: &ObservationWorld,
    player_room: RoomId,
    facing: Side,
) -> HashSet<RoomId> {
    let mut visible = HashSet::new();
    visible.insert(player_room);

    let mut queue = VecDeque::new();
    queue.push_back((player_room, 0u32));

    while let Some((curr, depth)) = queue.pop_front() {
        if depth >= 2 {
            continue;
        }

        if depth == 0 {
            // First hop: only look in the facing direction
            let door = world.door_id(curr, facing);
            if !world.is_sealed(door) {
                let partner = world.partner(door);
                let dest = world.door(partner).room;
                if visible.insert(dest) {
                    queue.push_back((dest, 1));
                }
            }
        } else {
            // Second hop: look in all directions
            for side in Side::ALL {
                let door = world.door_id(curr, side);
                if !world.is_sealed(door) {
                    let partner = world.partner(door);
                    let dest = world.door(partner).room;
                    if visible.insert(dest) {
                        queue.push_back((dest, depth + 1));
                    }
                }
            }
        }
    }

    visible
}

/// BFS pathfinding to compute the shortest path of rooms from start to target.
pub fn find_shortest_path(
    world: &ObservationWorld,
    start: RoomId,
    target: RoomId,
) -> Option<Vec<RoomId>> {
    if start == target {
        return Some(vec![start]);
    }

    let mut visited = [false; ROOM_COUNT];
    let mut parent = [None; ROOM_COUNT];
    let mut queue = VecDeque::new();

    visited[start.0 as usize] = true;
    queue.push_back(start);

    let mut found = false;
    while let Some(curr) = queue.pop_front() {
        if curr == target {
            found = true;
            break;
        }

        // Neighbors on active matching
        for side in Side::ALL {
            let door = world.door_id(curr, side);
            if !world.is_sealed(door) {
                let partner = world.partner(door);
                let nbr = world.door(partner).room;
                if !visited[nbr.0 as usize] {
                    visited[nbr.0 as usize] = true;
                    parent[nbr.0 as usize] = Some(curr);
                    queue.push_back(nbr);
                }
            }
        }
    }

    if !found {
        return None;
    }

    // Reconstruct path
    let mut path = Vec::new();
    let mut curr = target;
    path.push(curr);
    while let Some(p) = parent[curr.0 as usize] {
        path.push(p);
        curr = p;
    }
    path.reverse();
    Some(path)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub id: usize,
    pub room: RoomId,
    pub facing: Side,
    pub is_bot: bool,
    pub touch_count: u32,
    pub is_teleported: bool,
}

/// Tick logic for the guardian. Updates freezing states, ticks anchor timers,
/// steps towards the nearest active actor when unfrozen, and resolves collisions.
/// Returns the state, optional random teleport destination of the guardian (if banished),
/// and optional actor index if an actor was caught/teleported.
pub fn tick_guardian(
    world: &ObservationWorld,
    guardian: &mut Guardian,
    actors: &mut [Actor],
    anchors: &HashSet<RoomId>,
    rng: &mut SimpleRng,
    dt: f32,
    step_tick: bool,
) -> (GuardianState, Option<RoomId>, Option<usize>) {
    // 1. Check direct player line of sight (observable through thresholds by any active actor)
    let mut seen_by_player = false;
    for actor in actors.iter() {
        let seen_rooms = visible_rooms_from_view(world, actor.room, actor.facing);
        if seen_rooms.contains(&guardian.room) {
            seen_by_player = true;
            break;
        }
    }

    // 2. Check if in an anchored room
    let seen_by_anchor = anchors.contains(&guardian.room);

    // Determine state
    let state = if seen_by_player {
        GuardianState::FrozenByPlayer
    } else if seen_by_anchor {
        GuardianState::FrozenByAnchor
    } else {
        GuardianState::Active
    };

    let mut teleport_target = None;

    // 3. Anchor timer update
    if state == GuardianState::FrozenByAnchor {
        guardian.anchor_timer -= dt;
        if guardian.anchor_timer <= 0.0 {
            // Teleport to a random room
            let next = rng.next_room(guardian.room);
            guardian.room = next;
            guardian.anchor_timer = 30.0;
            teleport_target = Some(next);
        }
    } else {
        // Reset timer if not in anchor room
        guardian.anchor_timer = 30.0;
    }

    // 4. Step movement towards closest actor if active
    if state == GuardianState::Active && step_tick {
        let mut best_path: Option<Vec<RoomId>> = None;
        for actor in actors.iter() {
            if let Some(path) = find_shortest_path(world, guardian.room, actor.room)
                && best_path
                    .as_ref()
                    .is_none_or(|best| path.len() < best.len())
            {
                best_path = Some(path);
            }
        }
        if let Some(path) = best_path.filter(|p| p.len() > 1) {
            guardian.room = path[1];
        }
    }

    // 5. Collision detection / touch resolution with any actor
    let mut caught_actor_idx = None;
    for (idx, actor) in actors.iter_mut().enumerate() {
        if guardian.room == actor.room && !actor.is_teleported {
            let next = rng.next_room(actor.room);
            actor.room = next;
            actor.touch_count += 1;
            actor.is_teleported = true;
            caught_actor_idx = Some(idx);
            break; // Handle one collision per tick
        }
    }

    (state, teleport_target, caught_actor_idx)
}
