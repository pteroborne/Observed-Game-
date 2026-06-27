//! FPS-feasibility core: compute the **observed set from first-person
//! line-of-sight** instead of room occupancy.
//!
//! Every other lab observes a room only when a player *stands in it*. In a
//! first-person game the natural rule is that you observe what you can *see* —
//! and, crucially, the game's connections live in the graph, so looking through a
//! doorway shows you (and freezes) *whatever that doorway currently leads to*, not
//! the room physically next door. That is exactly the observe/decohere mechanic of
//! [`observation_lab`], now driven by where the camera is pointed.
//!
//! This module is the only new logic: given the player's room, the camera's
//! horizontal facing, and the current graph, it returns the set of rooms in view.
//! The lab then writes that set into `ObservationWorld::players`, so the proven
//! pinning + deterministic decoherence machinery is reused wholesale — observed
//! (seen) connections freeze, unseen ones rewire.

use bevy::math::Vec3;
use observation_lab::model::{ObservationWorld, Side};
use observed_core::RoomId;

pub const GRID: u32 = 3;
/// World distance between adjacent room centres (XZ plane; Y is up).
pub const ROOM_SPACING: f32 = 12.0;
/// Interior half-extent of a room; doorways sit on the walls at this offset.
pub const ROOM_HALF: f32 = 5.0;
/// Camera eye height above the floor.
pub const EYE_HEIGHT: f32 = 2.8;
/// Height of the room wall posts drawn in the debug wireframe.
pub const WALL_HEIGHT: f32 = 3.4;
/// Half-angle of the view cone used to decide which doorway you are facing.
pub const FOV_HALF_DEG: f32 = 40.0;
/// How many doorways deep a clear line of sight carries (range, in hops).
pub const MAX_DEPTH: u32 = 2;

/// Centre of a room on the floor plane. Row 0 is to the north (`-Z`); column 0 is
/// to the west (`-X`), so the layout matches `observation_lab`'s 3×3 grid.
pub fn room_center(room: RoomId) -> Vec3 {
    let r = room.0 / GRID;
    let c = room.0 % GRID;
    Vec3::new(
        (c as f32 - (GRID as f32 - 1.0) * 0.5) * ROOM_SPACING,
        0.0,
        (r as f32 - (GRID as f32 - 1.0) * 0.5) * ROOM_SPACING,
    )
}

/// The world direction a wall faces. North is `-Z`, south `+Z`, east `+X`,
/// west `-X`.
pub fn side_direction(side: Side) -> Vec3 {
    match side {
        Side::North => Vec3::NEG_Z,
        Side::East => Vec3::X,
        Side::South => Vec3::Z,
        Side::West => Vec3::NEG_X,
    }
}

/// The world position of a doorway (room centre pushed out to its wall).
pub fn door_position(room: RoomId, side: Side) -> Vec3 {
    room_center(room) + side_direction(side) * ROOM_HALF
}

/// A horizontal forward vector from a yaw angle (radians). Yaw 0 looks north
/// (`-Z`); increasing yaw turns east.
pub fn forward_from_yaw(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, -yaw.cos())
}

/// The rooms the camera can currently see, given the player's room and a
/// horizontal `forward` direction.
///
/// Line of sight starts in the player's room (always observed) and passes through
/// any **open** doorway that lies within the view cone, following its current
/// graph link to whatever room it leads to, up to [`MAX_DEPTH`] hops. A sealed
/// wall — or a doorway outside the cone — blocks sight, so what you freeze depends
/// on where you look.
pub fn visible_rooms(graph: &ObservationWorld, player: RoomId, forward: Vec3) -> Vec<RoomId> {
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let cos_half = FOV_HALF_DEG.to_radians().cos();

    let mut visible = vec![player];
    let mut frontier = vec![(player, 0u32)];

    while let Some((room, depth)) = frontier.pop() {
        if depth >= MAX_DEPTH {
            continue;
        }
        for side in Side::ALL {
            let door = graph.door_id(room, side);
            if graph.is_sealed(door) {
                continue; // a wall blocks the line of sight
            }
            // At the player's own room you must be facing the doorway; once a clear
            // line of sight is established it carries on down the corridor.
            if depth == 0 && side_direction(side).dot(forward) < cos_half {
                continue;
            }
            let destination = graph.door(graph.partner(door)).room;
            if !visible.contains(&destination) {
                visible.push(destination);
                frontier.push((destination, depth + 1));
            }
        }
    }

    visible.sort_by_key(|room| room.0);
    visible
}

#[cfg(test)]
mod tests {
    use super::*;

    fn east() -> Vec3 {
        side_direction(Side::East)
    }

    #[test]
    fn the_player_room_is_always_observed() {
        let graph = ObservationWorld::authored();
        // Look at a sealed boundary wall (north from the top-left room): only self.
        let seen = visible_rooms(&graph, RoomId(0), side_direction(Side::North));
        assert_eq!(seen, vec![RoomId(0)]);
    }

    #[test]
    fn looking_through_an_open_door_reveals_what_it_leads_to() {
        let graph = ObservationWorld::authored();
        // Room 0 East links to room 1 in the authored lattice; looking east reveals
        // the corridor (room 0 → 1 → 2, plus room 1's southern neighbour 4).
        let seen = visible_rooms(&graph, RoomId(0), east());
        assert!(seen.contains(&RoomId(1)), "the faced room is observed");
        assert!(
            seen.contains(&RoomId(2)),
            "line of sight carries down the corridor"
        );
    }

    #[test]
    fn looking_away_hides_the_room() {
        let graph = ObservationWorld::authored();
        // Facing west from room 0 — the only opening (east) is behind us.
        let seen = visible_rooms(&graph, RoomId(0), side_direction(Side::West));
        assert_eq!(seen, vec![RoomId(0)]);
    }

    #[test]
    fn a_sealed_wall_blocks_the_line_of_sight() {
        let mut graph = ObservationWorld::authored();
        // Seal room 0's east door; now looking east reveals nothing beyond room 0.
        let door = graph.door_id(RoomId(0), Side::East);
        let partner = graph.partner(door);
        graph.links[door.0 as usize] = door; // self-seal
        graph.links[partner.0 as usize] = partner;
        let seen = visible_rooms(&graph, RoomId(0), east());
        assert_eq!(seen, vec![RoomId(0)]);
    }

    #[test]
    fn line_of_sight_follows_the_current_link_not_physical_adjacency() {
        // Rewire room 0's east door to lead to room 8 (the far corner). Looking east
        // should now observe room 8, proving sight follows the graph, not geometry.
        let mut graph = ObservationWorld::authored();
        let a = graph.door_id(RoomId(0), Side::East);
        let b = graph.door_id(RoomId(8), Side::South);
        // Detach both from their old partners, then link a <-> b.
        let old_a = graph.partner(a);
        let old_b = graph.partner(b);
        graph.links[old_a.0 as usize] = old_a;
        graph.links[old_b.0 as usize] = old_b;
        graph.links[a.0 as usize] = b;
        graph.links[b.0 as usize] = a;

        let seen = visible_rooms(&graph, RoomId(0), east());
        assert!(
            seen.contains(&RoomId(8)),
            "sight follows the doorway's current link"
        );
    }

    #[test]
    fn observation_is_deterministic() {
        let graph = ObservationWorld::authored();
        let a = visible_rooms(&graph, RoomId(4), east());
        let b = visible_rooms(&graph, RoomId(4), east());
        assert_eq!(a, b);
    }

    #[test]
    fn feeding_the_visible_set_freezes_seen_connections_under_decoherence() {
        // The integration the lab relies on: write the visible set into the graph's
        // observers, and the proven decoherence freezes exactly those connections.
        let mut graph = ObservationWorld::authored();
        let seen = visible_rooms(&graph, RoomId(0), east());
        graph.players = seen.clone();

        // The doorways of every seen room (and their partners) must survive a rewire.
        let mut watched = Vec::new();
        for room in &seen {
            for side in Side::ALL {
                let d = graph.door_id(*room, side);
                watched.push((d, graph.partner(d)));
            }
        }

        graph.decohere();
        for (door, partner) in watched {
            assert_eq!(
                graph.partner(door),
                partner,
                "a seen connection must stay frozen"
            );
        }
    }
}
