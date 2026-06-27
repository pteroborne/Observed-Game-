//! Phase 26: **rerouting passages** — the second lab of the Hybrid maze arc.
//!
//! Phase 25 embedded the graph as a static spatial maze: fixed rooms joined by real
//! corridors. This lab makes the maze *live*. When a region is unobserved and the
//! graph decoheres, the affected corridors must **re-route in space** — a corridor
//! that led to one room now leads to another — but, reusing Phase 22's discipline,
//! the spatial change is committed as one **atomic swap that only happens
//! off-camera and never under the player's feet**. So you can walk the maze, look
//! away, and find a corridor leads somewhere new on return, with no visible pop and
//! no being stranded mid-passage.
//!
//! Rooms stay fixed; only corridors reroute. The model keeps a `rendered` layout
//! (what you see) and a `target` layout (what the current graph wants). Each
//! decoherence updates `target`; `try_commit` reconciles `rendered` to `target`
//! atomically, but only when every tile that would change is out of view and clear
//! of the player. Visibility and the player tile are passed in, so the model is
//! pure and deterministic; the lab supplies the real camera frustum.

use std::collections::HashSet;

use bevy::prelude::Resource;
use constraint_lab::model::ConstraintWorld;
use fps_controller_lab::controller::FpsArena;
use fps_maze_lab::maze::{
    ELEVATION_STEP_HEIGHT, GRID_H, GRID_W, RoomRect, Tile, build_elevated_arena,
    elevations_are_step_connected, generated_elevation_steps, place_rooms, route_corridor,
};
use observation_lab::model::{DoorId, ROOM_COUNT};
use observed_core::RoomId;

pub use fps_maze_lab::maze::TILE_SIZE;
pub use fps_maze_lab::maze::{GRID_H as MAZE_H, GRID_W as MAZE_W};

/// A corridor realizing one graph connection, with the tiles it occupies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorridorRoute {
    /// The graph connection (ordered door pair) this corridor realizes.
    pub key: (DoorId, DoorId),
    pub rooms: (RoomId, RoomId),
    pub path: Vec<(usize, usize)>,
    pub spine: bool,
}

#[derive(Resource, Clone, Debug)]
pub struct RerouteMaze {
    pub world: ConstraintWorld,
    pub seed: u64,
    pub base: Vec<Tile>,
    pub elevation_steps: Vec<u8>,
    pub rooms: Vec<RoomRect>,
    /// What is currently displayed.
    pub rendered: Vec<CorridorRoute>,
    /// What the current graph wants displayed.
    pub target: Vec<CorridorRoute>,
    pub rendered_tiles: Vec<Tile>,
    pub decohere_count: u32,
    pub commit_count: u32,
    pub deferred_count: u32,
    pub last_event: String,
}

impl RerouteMaze {
    pub fn authored(seed: u64) -> Self {
        let mut world = ConstraintWorld::authored();
        // Observation is driven by where the player stands; start with nothing
        // observed so the player's room (set each frame by the lab) is the only pin.
        world.graph.players = Vec::new();
        let (base, rooms) = place_rooms(seed);
        let elevation_steps = generated_elevation_steps();
        let rendered = route_all(&world, &base, &rooms);
        let rendered_tiles = rebuild(&base, &rendered);
        let target = rendered.clone();
        Self {
            world,
            seed,
            base,
            elevation_steps,
            rooms,
            rendered,
            target,
            rendered_tiles,
            decohere_count: 0,
            commit_count: 0,
            deferred_count: 0,
            last_event: "Walk away from a passage and let it rewire behind you.".to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored(self.seed);
    }

    /// The room the player occupies is the observed room: pin it so its doors do not
    /// rewire (the simulation-level freeze), independent of the off-camera gate.
    pub fn observe(&mut self, room: Option<RoomId>) {
        self.world.graph.players = room.into_iter().collect();
    }

    pub fn observed_rooms(&self) -> &[RoomId] {
        &self.world.graph.players
    }

    /// Rewire the unobserved structure. Updates the `target` layout; `rendered`
    /// stays put until a safe commit reconciles to it.
    pub fn decohere(&mut self) {
        self.world.decohere();
        self.decohere_count += 1;
        self.target = route_all(&self.world, &self.base, &self.rooms);
        self.last_event = format!(
            "Decohered (#{}) — {} passage tiles want to reroute.",
            self.decohere_count,
            self.affected_tiles().len()
        );
    }

    fn target_tiles(&self) -> Vec<Tile> {
        rebuild(&self.base, &self.target)
    }

    /// Tiles whose contents differ between what is shown and what the graph wants —
    /// exactly the passage tiles a reroute would change.
    pub fn affected_tiles(&self) -> HashSet<(usize, usize)> {
        let target_tiles = self.target_tiles();
        let mut out = HashSet::new();
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let idx = y * GRID_W + x;
                if self.rendered_tiles[idx] != target_tiles[idx] {
                    out.insert((x, y));
                }
            }
        }
        out
    }

    pub fn in_sync(&self) -> bool {
        self.affected_tiles().is_empty()
    }

    /// Try to reroute: commit `rendered` to `target` in one atomic swap, but only if
    /// every changing tile is out of view (`visible`) and not under the player.
    /// Returns true if it committed.
    pub fn try_commit(
        &mut self,
        visible: &HashSet<(usize, usize)>,
        player: Option<(usize, usize)>,
    ) -> bool {
        let affected = self.affected_tiles();
        if affected.is_empty() {
            return false;
        }
        let blocked = affected
            .iter()
            .any(|tile| visible.contains(tile) || Some(*tile) == player);
        if blocked {
            self.deferred_count += 1;
            self.last_event =
                "Reroute deferred — a changing passage is in view or underfoot.".to_string();
            return false;
        }
        self.rendered = self.target.clone();
        self.rendered_tiles = self.target_tiles();
        self.commit_count += 1;
        self.last_event = "Passages rerouted off-camera — no pop, no stranding.".to_string();
        true
    }

    /// The room-pairs currently walkable as direct corridors (for "it leads
    /// somewhere new" checks).
    pub fn rendered_room_pairs(&self) -> HashSet<(u32, u32)> {
        self.rendered
            .iter()
            .map(|c| ordered_pair(c.rooms.0.0, c.rooms.1.0))
            .collect()
    }

    pub fn reachable_rooms(&self) -> usize {
        reachable(&self.rendered_tiles, &self.elevation_steps, &self.rooms)
    }

    pub fn navigable(&self) -> bool {
        self.reachable_rooms() == ROOM_COUNT
    }

    pub fn floor_height(&self, x: usize, y: usize) -> f32 {
        self.elevation_steps[y * GRID_W + x] as f32 * ELEVATION_STEP_HEIGHT
    }

    pub fn room_floor_height(&self, room: RoomId) -> f32 {
        let (x, y) = self.rooms[room.0 as usize].center_tile();
        self.floor_height(x, y)
    }

    pub fn arena(&self, wall_height: f32) -> FpsArena {
        build_elevated_arena(&self.rendered_tiles, &self.elevation_steps, wall_height)
    }
}

fn ordered_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b { (a, b) } else { (b, a) }
}

fn route_all(world: &ConstraintWorld, base: &[Tile], rooms: &[RoomRect]) -> Vec<CorridorRoute> {
    let mut out = Vec::new();
    for (door_a, door_b) in world.graph.connections() {
        let ra = world.graph.door(door_a).room.0 as usize;
        let rb = world.graph.door(door_b).room.0 as usize;
        if ra == rb {
            continue;
        }
        if let Some(path) = route_corridor(base, &rooms[ra], &rooms[rb]) {
            let key = if door_a.0 <= door_b.0 {
                (door_a, door_b)
            } else {
                (door_b, door_a)
            };
            out.push(CorridorRoute {
                key,
                rooms: (RoomId(ra as u32), RoomId(rb as u32)),
                path,
                spine: world.is_protected(door_a),
            });
        }
    }
    out
}

fn rebuild(base: &[Tile], corridors: &[CorridorRoute]) -> Vec<Tile> {
    let mut tiles = base.to_vec();
    for corridor in corridors {
        for &(x, y) in &corridor.path {
            if tiles[y * GRID_W + x] == Tile::Wall {
                tiles[y * GRID_W + x] = Tile::Corridor;
            }
        }
    }
    tiles
}

fn neighbours(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    let mut out = Vec::with_capacity(4);
    if y > 0 {
        out.push((x, y - 1));
    }
    if x + 1 < GRID_W {
        out.push((x + 1, y));
    }
    if y + 1 < GRID_H {
        out.push((x, y + 1));
    }
    if x > 0 {
        out.push((x - 1, y));
    }
    out.into_iter()
}

fn reachable(tiles: &[Tile], elevation_steps: &[u8], rooms: &[RoomRect]) -> usize {
    let start = rooms[0].center_tile();
    let mut seen = vec![false; GRID_W * GRID_H];
    let mut reached = [false; ROOM_COUNT];
    let mut queue = std::collections::VecDeque::new();
    seen[start.1 * GRID_W + start.0] = true;
    queue.push_back(start);
    while let Some((x, y)) = queue.pop_front() {
        if let Tile::Room(r) = tiles[y * GRID_W + x] {
            reached[r as usize] = true;
        }
        for (nx, ny) in neighbours(x, y) {
            let idx = ny * GRID_W + nx;
            if !seen[idx]
                && tiles[idx].is_floor()
                && elevations_are_step_connected(
                    elevation_steps[y * GRID_W + x],
                    elevation_steps[idx],
                )
            {
                seen[idx] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    reached.iter().filter(|r| **r).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_floor_tiles(maze: &RerouteMaze) -> HashSet<(usize, usize)> {
        let mut out = HashSet::new();
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if maze.rendered_tiles[y * GRID_W + x].is_floor() {
                    out.insert((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn authored_is_in_sync_and_navigable() {
        let maze = RerouteMaze::authored(1);
        assert!(maze.in_sync());
        assert!(maze.navigable());
        assert_eq!(maze.reachable_rooms(), ROOM_COUNT);
    }

    #[test]
    fn decoherence_makes_passages_want_to_reroute() {
        let mut maze = RerouteMaze::authored(2);
        maze.decohere();
        assert!(!maze.in_sync(), "the target layout now differs");
        assert!(!maze.affected_tiles().is_empty());
        // Rendered is unchanged and still navigable while the reroute is pending.
        assert!(maze.navigable());
    }

    #[test]
    fn a_reroute_in_view_is_deferred() {
        let mut maze = RerouteMaze::authored(3);
        let before = maze.rendered_tiles.clone();
        maze.decohere();
        let affected = maze.affected_tiles();
        // Looking right at the changing tiles: must not commit.
        assert!(!maze.try_commit(&affected, None));
        assert_eq!(maze.rendered_tiles, before, "nothing changed while watched");
        assert_eq!(maze.deferred_count, 1);
    }

    #[test]
    fn a_reroute_commits_off_camera_and_leads_somewhere_new() {
        let mut maze = RerouteMaze::authored(4);
        let before_pairs = maze.rendered_room_pairs();
        // Rewire until the desired room-pairs actually change, then commit unseen.
        let mut committed = false;
        for _ in 0..6 {
            maze.decohere();
            if maze.try_commit(&HashSet::new(), None) {
                committed = true;
                if maze.rendered_room_pairs() != before_pairs {
                    break;
                }
            }
        }
        assert!(committed, "off-camera reroute must commit");
        assert!(maze.in_sync());
        assert!(maze.navigable(), "the rerouted maze is still navigable");
        assert_ne!(
            maze.rendered_room_pairs(),
            before_pairs,
            "a corridor now leads to a different room"
        );
    }

    #[test]
    fn a_reroute_never_changes_tiles_under_the_player() {
        let mut maze = RerouteMaze::authored(5);
        maze.decohere();
        let affected = maze.affected_tiles();
        let underfoot = *affected.iter().next().expect("something wants to reroute");
        // Player standing on a changing tile, looking nowhere: still deferred.
        assert!(!maze.try_commit(&HashSet::new(), Some(underfoot)));
        assert!(
            !maze.in_sync(),
            "the passage under the player did not change"
        );
    }

    #[test]
    fn observed_rooms_do_not_reroute() {
        // Observe room 4; its connections are pinned, so decoherence leaves its
        // corridors' room-pairs untouched even after an off-camera commit.
        let mut maze = RerouteMaze::authored(6);
        maze.observe(Some(RoomId(4)));
        let watched: HashSet<(u32, u32)> = maze
            .rendered
            .iter()
            .filter(|c| c.rooms.0 == RoomId(4) || c.rooms.1 == RoomId(4))
            .map(|c| ordered_pair(c.rooms.0.0, c.rooms.1.0))
            .collect();
        for _ in 0..5 {
            maze.decohere();
            maze.try_commit(&HashSet::new(), None);
        }
        let after: HashSet<(u32, u32)> = maze
            .rendered
            .iter()
            .filter(|c| c.rooms.0 == RoomId(4) || c.rooms.1 == RoomId(4))
            .map(|c| ordered_pair(c.rooms.0.0, c.rooms.1.0))
            .collect();
        assert_eq!(watched, after, "an observed room's passages stay put");
    }

    #[test]
    fn rerouting_is_deterministic() {
        let mut a = RerouteMaze::authored(7);
        let mut b = RerouteMaze::authored(7);
        for _ in 0..5 {
            a.decohere();
            a.try_commit(&HashSet::new(), None);
            b.decohere();
            b.try_commit(&HashSet::new(), None);
        }
        assert_eq!(a.rendered_tiles, b.rendered_tiles);
        assert_eq!(a.commit_count, b.commit_count);
    }

    #[test]
    fn reset_restores_the_in_sync_maze() {
        let mut maze = RerouteMaze::authored(8);
        let floors = all_floor_tiles(&maze);
        for _ in 0..4 {
            maze.decohere();
            maze.try_commit(&HashSet::new(), None);
        }
        maze.reset();
        assert!(maze.in_sync());
        assert_eq!(maze.decohere_count, 0);
        assert_eq!(all_floor_tiles(&maze), floors, "same authored layout");
    }

    #[test]
    fn reroutes_preserve_the_multi_level_height_field() {
        let mut maze = RerouteMaze::authored(9);
        let heights = maze.elevation_steps.clone();
        let top = maze.room_floor_height(RoomId(8));
        assert!(top > maze.room_floor_height(RoomId(0)));
        for _ in 0..5 {
            maze.decohere();
            maze.try_commit(&HashSet::new(), None);
            assert_eq!(maze.elevation_steps, heights);
            assert!(maze.navigable());
        }
    }
}
