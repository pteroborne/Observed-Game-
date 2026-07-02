//! Phase 25: **spatial maze layout** — the first lab of the Hybrid maze arc.
//!
//! The FPS arc proved the observe/decohere mechanic over a fixed-grid *portal*
//! scaffold: rooms sat on a grid and a doorway teleported you to its current graph
//! partner. This lab makes the connectivity **concrete**. It takes the proven room
//! graph ([`observation_lab`]'s `ObservationWorld`) and **embeds it in space**: the
//! nine rooms are placed deterministically (seeded, with dynamic size/position), and
//! every graph connection is routed as an **actual walkable corridor** of tiles
//! between the two rooms — no portals. The result is a connected, navigable maze
//! that is purely a function of the graph + seed, so it is reproducible and varies
//! by seed.
//!
//! The graph stays the topology; this is its spatial *embedding*. Corridor routing
//! is a breadth-first search through non-room space, so it realizes any connection —
//! including the non-adjacent "impossible" ones a decohered graph produces — which
//! is what the rerouting (Phase 26) builds on. Topology remains tile-grid logic,
//! while a deterministic integer height field turns it into three traversable
//! levels for the first-person `Camera3d`.

use std::collections::VecDeque;

use glam::{Vec2, Vec3};
use observed_core::{RoomId, SplitMix};
use observed_observation::{DoorId, ObservationWorld, ROOM_COUNT};
use observed_traversal::{Aabb3, FpsArena};

/// Tiles per plot side; the 3x3 grid of plots makes the maze. Enlarged for the
/// "bigger deliberate facility" — real chambers with breathing room for wide halls.
pub const PLOT: usize = 15;
pub const GRID_W: usize = PLOT * 3;
pub const GRID_H: usize = PLOT * 3;
pub const MIN_ROOM: usize = 6;
pub const MAX_ROOM: usize = 8;
/// Deliberate breathing room inside each plot. Three tiles on every side guarantee
/// enough corridor length to fit a pressure gate plus a separate safe bypass.
pub const ROOM_MARGIN: usize = 3;
/// Chebyshev half-width a corridor centreline is carved out to (1 ⇒ 3-tile halls).
pub const CORRIDOR_RADIUS: usize = 1;
/// World size of one tile (the lab renders the maze at this scale).
pub const TILE_SIZE: f32 = 2.2;
/// One deterministic stair rise. It stays below the shared controller's 0.45 step
/// limit, while three rises create a visibly distinct room level.
pub const ELEVATION_STEP_HEIGHT: f32 = 0.30;
pub const STEPS_PER_LEVEL: u8 = 3;
pub const LEVEL_HEIGHT: f32 = ELEVATION_STEP_HEIGHT * STEPS_PER_LEVEL as f32;
pub const LEVEL_COUNT: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Room(u32),
    Corridor,
}

impl Tile {
    pub fn is_floor(self) -> bool {
        !matches!(self, Tile::Wall)
    }
    pub fn is_room(self) -> bool {
        matches!(self, Tile::Room(_))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RoomRect {
    pub room: RoomId,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl RoomRect {
    pub fn center_tile(&self) -> (usize, usize) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

#[derive(Clone, Debug)]
pub struct Corridor {
    pub a: RoomId,
    pub b: RoomId,
    pub door_a: DoorId,
    pub door_b: DoorId,
    pub path: Vec<(usize, usize)>,
}

/// A spatial choice around one corridor segment. The risky route keeps the direct
/// centreline and crosses a pressure gate; the safe route leaves the main hall,
/// travels around the gate, and rejoins farther ahead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteChoice {
    pub risky_path: Vec<(usize, usize)>,
    pub safe_path: Vec<(usize, usize)>,
    pub trap_tiles: Vec<(usize, usize)>,
}

impl RouteChoice {
    pub fn safe_is_longer(&self) -> bool {
        self.safe_path.len() > self.risky_path.len()
    }
}

/// Build a deterministic bypass around a straight section of a routed corridor.
/// The pressure gate spans the full authored hall width, so avoiding it requires
/// taking the visibly longer side route rather than merely sidestepping one tile.
pub fn build_route_choice(
    base: &[Tile],
    direct_path: &[(usize, usize)],
    hall_radius: usize,
) -> Option<RouteChoice> {
    if direct_path.len() < 5 {
        return None;
    }
    let offset = hall_radius + 1;
    for center in 2..direct_path.len() - 2 {
        let run = &direct_path[center - 2..=center + 2];
        let direction = tile_delta(run[0], run[1])?;
        if !run
            .windows(2)
            .all(|pair| tile_delta(pair[0], pair[1]) == Some(direction))
        {
            continue;
        }

        let entry_index = center - 2;
        let exit_index = center + 2;
        let entry = direct_path[entry_index];
        let exit = direct_path[exit_index];
        let normals = [(-direction.1, direction.0), (direction.1, -direction.0)];
        for normal in normals {
            let mut bypass = vec![entry];
            let mut current = entry;
            let mut valid = true;
            for _ in 0..offset {
                let Some(next) = offset_tile(current, normal) else {
                    valid = false;
                    break;
                };
                bypass.push(next);
                current = next;
            }
            if !valid {
                continue;
            }
            for _ in entry_index..exit_index {
                let Some(next) = offset_tile(current, direction) else {
                    valid = false;
                    break;
                };
                bypass.push(next);
                current = next;
            }
            if !valid {
                continue;
            }
            for _ in 0..offset {
                let Some(next) = offset_tile(current, (-normal.0, -normal.1)) else {
                    valid = false;
                    break;
                };
                bypass.push(next);
                current = next;
            }
            if !valid || current != exit {
                continue;
            }
            if bypass
                .iter()
                .skip(1)
                .take(bypass.len().saturating_sub(2))
                .any(|&(x, y)| base[y * GRID_W + x].is_room())
            {
                continue;
            }

            let trap_tiles = thicken_path(&direct_path[center..=center], hall_radius);
            if bypass.iter().any(|tile| trap_tiles.contains(tile)) {
                continue;
            }

            let mut safe_path = direct_path[..=entry_index].to_vec();
            safe_path.extend_from_slice(&bypass[1..]);
            safe_path.extend_from_slice(&direct_path[exit_index + 1..]);
            let choice = RouteChoice {
                risky_path: direct_path.to_vec(),
                safe_path,
                trap_tiles,
            };
            if choice.safe_is_longer() {
                return Some(choice);
            }
        }
    }
    None
}

fn tile_delta(a: (usize, usize), b: (usize, usize)) -> Option<(isize, isize)> {
    let delta = (b.0 as isize - a.0 as isize, b.1 as isize - a.1 as isize);
    matches!(delta, (1, 0) | (-1, 0) | (0, 1) | (0, -1)).then_some(delta)
}

fn offset_tile(tile: (usize, usize), delta: (isize, isize)) -> Option<(usize, usize)> {
    let x = tile.0.checked_add_signed(delta.0)?;
    let y = tile.1.checked_add_signed(delta.1)?;
    (x < GRID_W && y < GRID_H).then_some((x, y))
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct MazeLayout {
    pub tiles: Vec<Tile>,
    /// Integer stair units keep the generated height field exactly comparable.
    pub elevation_steps: Vec<u8>,
    pub rooms: Vec<RoomRect>,
    pub corridors: Vec<Corridor>,
    pub seed: u64,
}

impl MazeLayout {
    pub fn at(&self, x: usize, y: usize) -> Tile {
        self.tiles[y * GRID_W + x]
    }

    pub fn elevation_step_at(&self, x: usize, y: usize) -> u8 {
        self.elevation_steps[y * GRID_W + x]
    }

    pub fn floor_height(&self, x: usize, y: usize) -> f32 {
        self.elevation_step_at(x, y) as f32 * ELEVATION_STEP_HEIGHT
    }

    pub fn room_floor_height(&self, room: RoomId) -> f32 {
        let (x, y) = self.rooms[room.0 as usize].center_tile();
        self.floor_height(x, y)
    }

    pub fn max_floor_height(&self) -> f32 {
        self.elevation_steps.iter().copied().max().unwrap_or(0) as f32 * ELEVATION_STEP_HEIGHT
    }

    pub fn arena(&self, wall_height: f32) -> FpsArena {
        build_elevated_arena(&self.tiles, &self.elevation_steps, wall_height)
    }

    /// Deterministically embed `graph`'s rooms and connections as a tile maze.
    pub fn generate(graph: &ObservationWorld, seed: u64) -> Self {
        // 1. Place rooms (fixed for a given seed).
        let (mut tiles, rooms) = place_rooms(seed);

        // 2. Route every open graph connection as a corridor between its two rooms.
        let mut corridors = Vec::new();
        for (door_a, door_b) in graph.connections() {
            let ra = graph.door(door_a).room.0 as usize;
            let rb = graph.door(door_b).room.0 as usize;
            if ra == rb {
                continue;
            }
            if let Some(path) = route_corridor(&tiles, &rooms[ra], &rooms[rb]) {
                for &(tx, ty) in &thicken_path(&path, CORRIDOR_RADIUS) {
                    if tiles[ty * GRID_W + tx] == Tile::Wall {
                        tiles[ty * GRID_W + tx] = Tile::Corridor;
                    }
                }
                corridors.push(Corridor {
                    a: RoomId(ra as u32),
                    b: RoomId(rb as u32),
                    door_a,
                    door_b,
                    path,
                });
            }
        }

        Self {
            tiles,
            elevation_steps: generated_elevation_steps(),
            rooms,
            corridors,
            seed,
        }
    }

    /// Which rooms are reachable on foot from room 0 over floor tiles.
    pub fn reachable_rooms(&self) -> usize {
        let start = self.rooms[0].center_tile();
        let mut seen_tiles = vec![false; GRID_W * GRID_H];
        let mut reached = [false; ROOM_COUNT];
        let mut queue = VecDeque::new();
        seen_tiles[start.1 * GRID_W + start.0] = true;
        queue.push_back(start);
        while let Some((x, y)) = queue.pop_front() {
            if let Tile::Room(r) = self.at(x, y) {
                reached[r as usize] = true;
            }
            for (nx, ny) in neighbours(x, y) {
                let idx = ny * GRID_W + nx;
                if !seen_tiles[idx]
                    && self.tiles[idx].is_floor()
                    && elevations_are_step_connected(
                        self.elevation_step_at(x, y),
                        self.elevation_step_at(nx, ny),
                    )
                {
                    seen_tiles[idx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        reached.iter().filter(|r| **r).count()
    }

    /// Every room reachable on foot from every other — a single navigable maze.
    pub fn navigable(&self) -> bool {
        self.reachable_rooms() == ROOM_COUNT
    }

    pub fn rooms_overlap(&self) -> bool {
        for i in 0..self.rooms.len() {
            for j in i + 1..self.rooms.len() {
                let a = &self.rooms[i];
                let b = &self.rooms[j];
                let disjoint =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                if !disjoint {
                    return true;
                }
            }
        }
        false
    }

    /// Tile centre in world space (XZ plane, centred on the origin).
    pub fn tile_world(&self, x: usize, y: usize) -> Vec2 {
        Vec2::new(
            (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
            (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
        )
    }

    pub fn room_world(&self, room: RoomId) -> Vec2 {
        let (cx, cy) = self.rooms[room.0 as usize].center_tile();
        self.tile_world(cx, cy)
    }
}

/// Generate three flat room levels with deterministic three-step stair bands in
/// the separator rows between plot bands. Room placement never occupies those
/// separator rows, so every chamber stays flat while any routed corridor crossing
/// north/south becomes a real staircase.
pub fn generated_elevation_steps() -> Vec<u8> {
    let mut steps = vec![0; GRID_W * GRID_H];
    for y in 0..GRID_H {
        let step = elevation_step_for_row(y);
        for x in 0..GRID_W {
            steps[y * GRID_W + x] = step;
        }
    }
    steps
}

pub fn elevation_step_for_row(y: usize) -> u8 {
    let first_start = PLOT - 1;
    let second_start = PLOT * 2 - 1;
    if y < first_start {
        0
    } else if y <= PLOT + 1 {
        (y - first_start + 1) as u8
    } else if y < second_start {
        STEPS_PER_LEVEL
    } else if y <= PLOT * 2 + 1 {
        STEPS_PER_LEVEL + (y - second_start + 1) as u8
    } else {
        STEPS_PER_LEVEL * 2
    }
}

pub fn elevations_are_step_connected(a: u8, b: u8) -> bool {
    a.abs_diff(b) <= 1
}

/// Build collision for a terraced maze. Raised floor tiles are solid pedestals;
/// the shared controller steps onto adjacent pedestals and walls extend upward
/// from the local height field.
pub fn build_elevated_arena(tiles: &[Tile], elevation_steps: &[u8], wall_height: f32) -> FpsArena {
    assert_eq!(tiles.len(), GRID_W * GRID_H);
    assert_eq!(elevation_steps.len(), GRID_W * GRID_H);
    let mut solids = Vec::new();
    let half = TILE_SIZE * 0.5;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let index = y * GRID_W + x;
            let height = elevation_steps[index] as f32 * ELEVATION_STEP_HEIGHT;
            let centre = Vec2::new(
                (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
                (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
            );
            match tiles[index] {
                Tile::Room(_) | Tile::Corridor if height > 0.0 => {
                    solids.push(Aabb3::from_center_half(
                        Vec3::new(centre.x, height * 0.5, centre.y),
                        Vec3::new(half, height * 0.5, half),
                    ));
                }
                Tile::Wall => {
                    let top = height + wall_height;
                    solids.push(Aabb3::from_center_half(
                        Vec3::new(centre.x, top * 0.5, centre.y),
                        Vec3::new(half, top * 0.5, half),
                    ));
                }
                Tile::Room(_) | Tile::Corridor => {}
            }
        }
    }
    FpsArena {
        solids,
        floor_y: 0.0,
        floor_half: GRID_W.max(GRID_H) as f32 * TILE_SIZE * 0.5,
    }
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

/// Place the nine rooms deterministically from a seed (no corridors). Returned as
/// `(tiles, rooms)` so callers — the generator and the Phase 26 rerouter — share
/// one room layout and route corridors into it.
pub fn place_rooms(seed: u64) -> (Vec<Tile>, Vec<RoomRect>) {
    let mut tiles = vec![Tile::Wall; GRID_W * GRID_H];
    let mut rng = SplitMix(seed ^ 0x2D5E_C0DE_F00D_1357);
    let mut rooms = Vec::with_capacity(ROOM_COUNT);
    for index in 0..ROOM_COUNT {
        let plot_x = (index % 3) * PLOT;
        let plot_y = (index / 3) * PLOT;
        let w = MIN_ROOM + rng.below(MAX_ROOM - MIN_ROOM + 1);
        let h = MIN_ROOM + rng.below(MAX_ROOM - MIN_ROOM + 1);
        let x = plot_x + ROOM_MARGIN + rng.below(PLOT - ROOM_MARGIN * 2 - w + 1);
        let y = plot_y + ROOM_MARGIN + rng.below(PLOT - ROOM_MARGIN * 2 - h + 1);
        let rect = RoomRect {
            room: RoomId(index as u32),
            x,
            y,
            w,
            h,
        };
        for ty in y..y + h {
            for tx in x..x + w {
                tiles[ty * GRID_W + tx] = Tile::Room(index as u32);
            }
        }
        rooms.push(rect);
    }
    (tiles, rooms)
}

/// Expand a corridor centreline into a wider band (Chebyshev `radius`), clamped to
/// the grid and returned in row-major order so carving is deterministic. Carving only
/// flips `Wall` tiles, so a band never eats into a room interior.
pub fn thicken_path(path: &[(usize, usize)], radius: usize) -> Vec<(usize, usize)> {
    if radius == 0 {
        return path.to_vec();
    }
    let mut seen = vec![false; GRID_W * GRID_H];
    for &(x, y) in path {
        let x0 = x.saturating_sub(radius);
        let x1 = (x + radius).min(GRID_W - 1);
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius).min(GRID_H - 1);
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                seen[ty * GRID_W + tx] = true;
            }
        }
    }
    let mut out = Vec::new();
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            if seen[y * GRID_W + x] {
                out.push((x, y));
            }
        }
    }
    out
}

/// Shortest corridor through non-room space connecting the perimeters of two rooms.
/// Multi-source BFS from the tiles hugging `ra` to any tile hugging `rb`.
pub fn route_corridor(tiles: &[Tile], ra: &RoomRect, rb: &RoomRect) -> Option<Vec<(usize, usize)>> {
    let is_room = |x: usize, y: usize| tiles[y * GRID_W + x].is_room();
    let adjacent_to = |x: usize, y: usize, room: u32| {
        neighbours(x, y).any(|(nx, ny)| tiles[ny * GRID_W + nx] == Tile::Room(room))
    };

    let mut dist = vec![usize::MAX; GRID_W * GRID_H];
    let mut parent: Vec<Option<(usize, usize)>> = vec![None; GRID_W * GRID_H];
    let mut queue = VecDeque::new();

    // Sources: non-room tiles touching room a.
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            if !is_room(x, y) && adjacent_to(x, y, ra.room.0) {
                let idx = y * GRID_W + x;
                if dist[idx] == usize::MAX {
                    dist[idx] = 0;
                    queue.push_back((x, y));
                }
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        if adjacent_to(x, y, rb.room.0) {
            // Reconstruct the path back to a source.
            let mut path = vec![(x, y)];
            let mut cur = (x, y);
            while let Some(prev) = parent[cur.1 * GRID_W + cur.0] {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }
        for (nx, ny) in neighbours(x, y) {
            let idx = ny * GRID_W + nx;
            if !is_room(nx, ny) && dist[idx] == usize::MAX {
                dist[idx] = dist[y * GRID_W + x] + 1;
                parent[idx] = Some((x, y));
                queue.push_back((nx, ny));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_traversal::{FIXED_DT, FpsBody, FpsConfig, step_body};
    use player_input::PlayerIntent;

    fn authored() -> ObservationWorld {
        ObservationWorld::authored()
    }

    fn floor_path(
        maze: &MazeLayout,
        start: (usize, usize),
        goal: (usize, usize),
    ) -> Vec<(usize, usize)> {
        let mut parent = vec![None; GRID_W * GRID_H];
        let mut seen = vec![false; GRID_W * GRID_H];
        let mut queue = VecDeque::new();
        seen[start.1 * GRID_W + start.0] = true;
        queue.push_back(start);
        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == goal {
                let mut path = vec![goal];
                let mut current = goal;
                while current != start {
                    current = parent[current.1 * GRID_W + current.0].expect("path parent");
                    path.push(current);
                }
                path.reverse();
                return path;
            }
            for (nx, ny) in neighbours(x, y) {
                let index = ny * GRID_W + nx;
                if !seen[index]
                    && maze.at(nx, ny).is_floor()
                    && elevations_are_step_connected(
                        maze.elevation_step_at(x, y),
                        maze.elevation_step_at(nx, ny),
                    )
                {
                    seen[index] = true;
                    parent[index] = Some((x, y));
                    queue.push_back((nx, ny));
                }
            }
        }
        panic!("generated maze has no path from {start:?} to {goal:?}");
    }

    fn world_tile(position: Vec3) -> Option<(usize, usize)> {
        let x = (position.x / TILE_SIZE + GRID_W as f32 * 0.5).floor();
        let y = (position.z / TILE_SIZE + GRID_H as f32 * 0.5).floor();
        (x >= 0.0 && y >= 0.0 && x < GRID_W as f32 && y < GRID_H as f32)
            .then_some((x as usize, y as usize))
    }

    #[test]
    fn the_graph_embeds_as_a_connected_navigable_maze() {
        let maze = MazeLayout::generate(&authored(), 1);
        assert!(maze.navigable(), "every room reachable on foot");
        assert_eq!(maze.reachable_rooms(), ROOM_COUNT);
        assert!(!maze.rooms_overlap(), "rooms must not overlap");
    }

    #[test]
    fn every_open_connection_becomes_a_real_corridor() {
        let graph = authored();
        let maze = MazeLayout::generate(&graph, 7);
        let open = graph.connections().len();
        assert_eq!(
            maze.corridors.len(),
            open,
            "each open graph connection is routed as a corridor"
        );
        // No corridor is a portal: each is a contiguous tile path of length >= 1.
        for corridor in &maze.corridors {
            assert!(!corridor.path.is_empty());
            for window in corridor.path.windows(2) {
                let (ax, ay) = window[0];
                let (bx, by) = window[1];
                assert_eq!(ax.abs_diff(bx) + ay.abs_diff(by), 1, "path is 4-connected");
            }
        }
    }

    #[test]
    fn corridors_never_run_through_other_rooms() {
        let graph = authored();
        let maze = MazeLayout::generate(&graph, 3);
        for corridor in &maze.corridors {
            for &(x, y) in &corridor.path {
                // A corridor tile is either wall-turned-corridor or shared corridor,
                // never inside a room interior.
                assert!(
                    !maze.at(x, y).is_room(),
                    "corridor must avoid room interiors"
                );
            }
        }
    }

    #[test]
    fn it_embeds_a_decohered_graph_too() {
        // The real test of "embed the graph": after decoherence, connections become
        // non-adjacent, and the BFS routing must still realize them navigably.
        let mut graph = authored();
        for _ in 0..6 {
            graph.decohere();
        }
        let maze = MazeLayout::generate(&graph, 11);
        assert!(maze.navigable(), "a decohered graph still embeds navigably");
        assert_eq!(maze.corridors.len(), graph.connections().len());
    }

    #[test]
    fn generation_is_deterministic() {
        let graph = authored();
        let a = MazeLayout::generate(&graph, 42);
        let b = MazeLayout::generate(&graph, 42);
        assert_eq!(a.tiles, b.tiles);
    }

    #[test]
    fn rooms_use_the_enlarged_size_bounds() {
        let (_, rooms) = place_rooms(5);
        assert_eq!(rooms.len(), ROOM_COUNT);
        for room in &rooms {
            assert!(
                (MIN_ROOM..=MAX_ROOM).contains(&room.w) && (MIN_ROOM..=MAX_ROOM).contains(&room.h),
                "room {:?} is {}x{}, outside [{MIN_ROOM},{MAX_ROOM}]",
                room.room,
                room.w,
                room.h
            );
        }
        // The enlarged facility really is bigger than the old 7-tile cap.
        assert!(rooms.iter().any(|room| room.w >= 8 || room.h >= 8));
    }

    #[test]
    fn corridors_are_carved_wider_than_a_single_tile() {
        let maze = MazeLayout::generate(&authored(), 1);
        let centreline: usize = maze.corridors.iter().map(|c| c.path.len()).sum();
        let carved = maze
            .tiles
            .iter()
            .filter(|t| matches!(t, Tile::Corridor))
            .count();
        assert!(
            carved > centreline,
            "thickened corridors carve more than their centrelines ({carved} <= {centreline})"
        );
    }

    #[test]
    fn thicken_path_stays_in_bounds_and_widens() {
        let single = thicken_path(&[(0, 0)], 1);
        // A corner centre tile expands to a clamped 2x2 block, never out of bounds.
        assert_eq!(single, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert!(thicken_path(&[(5, 5)], 1).len() == 9);
    }

    #[test]
    fn different_seeds_produce_different_layouts() {
        let graph = authored();
        let a = MazeLayout::generate(&graph, 1);
        let b = MazeLayout::generate(&graph, 2);
        assert_ne!(a.tiles, b.tiles, "the seed varies room placement");
        // ...but both are still valid navigable mazes.
        assert!(a.navigable() && b.navigable());
    }

    #[test]
    fn generated_maze_has_three_flat_room_levels_and_step_sized_transitions() {
        let maze = MazeLayout::generate(&authored(), 13);
        let mut room_levels = std::collections::BTreeSet::new();
        for room in &maze.rooms {
            let first = maze.elevation_step_at(room.x, room.y);
            for y in room.y..room.y + room.h {
                for x in room.x..room.x + room.w {
                    assert_eq!(
                        maze.elevation_step_at(x, y),
                        first,
                        "room {:?} must have one flat floor",
                        room.room
                    );
                }
            }
            room_levels.insert(first);
        }
        assert_eq!(
            room_levels,
            [0, STEPS_PER_LEVEL, STEPS_PER_LEVEL * 2]
                .into_iter()
                .collect()
        );
        assert_eq!(
            maze.max_floor_height(),
            LEVEL_HEIGHT * (LEVEL_COUNT - 1) as f32
        );
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                for (nx, ny) in neighbours(x, y) {
                    assert!(
                        elevations_are_step_connected(
                            maze.elevation_step_at(x, y),
                            maze.elevation_step_at(nx, ny)
                        ),
                        "adjacent tiles exceed one stair rise"
                    );
                }
            }
        }
    }

    #[test]
    fn elevated_collision_contains_walkable_stair_pedestals() {
        let maze = MazeLayout::generate(&authored(), 14);
        let arena = maze.arena(4.0);
        assert!(
            arena
                .solids
                .iter()
                .any(|solid| (solid.max.y - ELEVATION_STEP_HEIGHT).abs() < 0.001),
            "the first stair rise is represented in collision"
        );
        assert!(
            arena
                .solids
                .iter()
                .any(|solid| (solid.max.y - maze.max_floor_height()).abs() < 0.001),
            "the highest room level is represented in collision"
        );
    }

    #[test]
    fn route_choice_has_a_short_risky_gate_and_long_safe_bypass() {
        let maze = MazeLayout::generate(&authored(), 16);
        let (base, _) = place_rooms(16);
        let choice = maze
            .corridors
            .iter()
            .find_map(|corridor| build_route_choice(&base, &corridor.path, 1))
            .expect("at least one generated corridor supports a route choice");
        assert!(choice.safe_is_longer());
        assert!(!choice.trap_tiles.is_empty());
        assert!(
            choice
                .safe_path
                .iter()
                .all(|tile| !choice.trap_tiles.contains(tile)),
            "the safe route never crosses the pressure gate"
        );
        for path in [&choice.risky_path, &choice.safe_path] {
            for pair in path.windows(2) {
                assert_eq!(
                    pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1),
                    1,
                    "both choices are continuous floor paths"
                );
            }
        }
    }

    #[test]
    fn shared_first_person_controller_climbs_from_lowest_to_highest_room_band() {
        let maze = MazeLayout::generate(&authored(), 15);
        let start = maze.rooms[0].center_tile();
        let goal = maze.rooms[8].center_tile();
        let path = floor_path(&maze, start, goal);
        let config = FpsConfig::default();
        let start_world = maze.tile_world(start.0, start.1);
        let mut body = FpsBody::spawned(
            Vec3::new(
                start_world.x,
                maze.floor_height(start.0, start.1) + config.half_height,
                start_world.y,
            ),
            0.0,
        );
        body.grounded = true;
        let arena = maze.arena(4.0);

        for &next in path.iter().skip(1) {
            let target = maze.tile_world(next.0, next.1);
            let mut reached = false;
            for _ in 0..180 {
                let to = target - Vec2::new(body.position.x, body.position.z);
                if to.length() < 0.12 {
                    reached = true;
                    body.velocity = Vec3::ZERO;
                    break;
                }
                body.yaw = to.x.atan2(-to.y);
                step_body(
                    &mut body,
                    PlayerIntent {
                        movement: Vec2::new(0.0, 1.0),
                        ..Default::default()
                    },
                    &arena,
                    &config,
                    FIXED_DT,
                );
            }
            assert!(reached, "controller could not enter path tile {next:?}");
        }

        let feet = body.position.y - config.half_height;
        assert_eq!(world_tile(body.position), Some(goal));
        assert!(
            (feet - maze.room_floor_height(RoomId(8))).abs() < 0.08,
            "controller climbed to the highest generated room level: feet={feet}"
        );
    }
}
