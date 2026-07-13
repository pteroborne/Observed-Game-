//! Spatial projection and snapshot mapping for Hybrid matches.

use super::{
    CONTROL_RADIUS, CONTROL_ROOM, CorridorRoute, HybridSnapshot, LOCAL_TEAM, SIDE_RADIUS,
    SPINE_RADIUS, TRAP_ACTIVE_TICKS, TRAP_PERIOD_TICKS,
};
use crate::director::Role;
use crate::facility::{CompetitiveFacility, TEAM_COUNT};
use crate::maze::{
    ELEVATION_STEP_HEIGHT, GRID_H, GRID_W, RoomRect, TILE_SIZE, Tile, generated_elevation_steps,
    place_rooms,
};
use crate::mutable::spine_next;
use glam::{Vec2, Vec3};
use observed_core::RoomId;
use observed_facility::map_spec::MapSpec;
use observed_traversal::{ArenaSpec, FpsBody, FpsConfig, PhysicsBackend, TraversalWorld};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct HybridMatch {
    pub competitive: CompetitiveFacility,
    pub seed: u64,
    pub base: Vec<Tile>,
    pub elevation_steps: Vec<u8>,
    pub rooms: Vec<RoomRect>,
    pub rendered: Vec<CorridorRoute>,
    pub target: Vec<CorridorRoute>,
    pub maze_tiles: Vec<Tile>,
    pub spine_tiles: HashSet<(usize, usize)>,
    pub safe_tiles: HashSet<(usize, usize)>,
    pub trap_tiles: HashSet<(usize, usize)>,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub physics_backend: PhysicsBackend,
    pub traversal_world: TraversalWorld,
    pub reroute_commits: u32,
    pub reroute_deferrals: u32,
    pub hazard_tick: u64,
    pub trap_hits: u32,
    pub trap_cooldown_ticks: u16,
    pub reroute_feedback_ticks: u16,
    pub last_traversal: Vec<(usize, usize)>,
    pub last_event: String,
}

impl HybridMatch {
    pub fn authored(seed: u64) -> Self {
        Self::authored_with_backend(seed, PhysicsBackend::LegacyAabb)
    }

    pub fn authored_with_backend(seed: u64, physics_backend: PhysicsBackend) -> Self {
        let competitive = CompetitiveFacility::authored();
        let (base, rooms) = place_rooms(seed);
        let elevation_steps = generated_elevation_steps();
        let rendered = super::round_step::route_all(&competitive, &base, &rooms);
        let maze_tiles = rebuild(&base, &rendered);
        let spine_tiles = collect_spine_tiles(&rendered);
        let safe_tiles = collect_safe_tiles(&rendered);
        let trap_tiles = collect_trap_tiles(&rendered);
        let config = match physics_backend {
            PhysicsBackend::LegacyAabb => FpsConfig::default(),
            PhysicsBackend::Rapier => FpsConfig::deliberate_rapier(),
        };
        let arena =
            crate::maze::build_elevated_arena(&maze_tiles, &elevation_steps, super::WALL_HEIGHT);
        let traversal_world =
            TraversalWorld::from_spec(ArenaSpec::from_legacy(&arena), physics_backend, &config);
        let start = room_world(RoomId(crate::facility::START_ROOM), &rooms);
        let mut body = FpsBody::spawned(
            Vec3::new(
                start.x,
                room_floor_height(
                    RoomId(crate::facility::START_ROOM),
                    &rooms,
                    &elevation_steps,
                ) + config.half_height,
                start.y,
            ),
            facing_for_next_room(
                RoomId(crate::facility::START_ROOM),
                &maze_tiles,
                &elevation_steps,
                &rooms,
            ),
        );
        body.grounded = true;
        Self {
            competitive,
            seed,
            base,
            elevation_steps,
            rooms,
            target: rendered.clone(),
            rendered,
            maze_tiles,
            spine_tiles,
            safe_tiles,
            trap_tiles,
            body,
            config,
            physics_backend,
            traversal_world,
            reroute_commits: 0,
            reroute_deferrals: 0,
            hazard_tick: 0,
            trap_hits: 0,
            trap_cooldown_ticks: 0,
            reroute_feedback_ticks: 0,
            last_traversal: Vec::new(),
            last_event: "Choose: red pressure-gate shortcut or cyan safe bypass.".to_string(),
        }
    }

    pub fn for_map_spec(seed: u64, spec: MapSpec) -> Self {
        Self::for_map_spec_with_backend(seed, spec, PhysicsBackend::LegacyAabb)
    }

    pub fn for_map_spec_with_backend(
        seed: u64,
        spec: MapSpec,
        physics_backend: PhysicsBackend,
    ) -> Self {
        let competitive = CompetitiveFacility::for_map_spec(spec.clone());
        let (base, rooms) = place_map_rooms(&spec);
        let elevation_steps = generated_elevation_steps();
        let rendered = super::round_step::route_all(&competitive, &base, &rooms);
        let maze_tiles = rebuild(&base, &rendered);
        let spine_tiles = collect_spine_tiles(&rendered);
        let safe_tiles = collect_safe_tiles(&rendered);
        let trap_tiles = collect_trap_tiles(&rendered);
        let config = match physics_backend {
            PhysicsBackend::LegacyAabb => FpsConfig::default(),
            PhysicsBackend::Rapier => FpsConfig::deliberate_rapier(),
        };
        let arena =
            crate::maze::build_elevated_arena(&maze_tiles, &elevation_steps, super::WALL_HEIGHT);
        let traversal_world =
            TraversalWorld::from_spec(ArenaSpec::from_legacy(&arena), physics_backend, &config);
        let start_room = spec.start_room().expect("validated start");
        let start = room_world(start_room, &rooms);
        let mut body = FpsBody::spawned(
            Vec3::new(
                start.x,
                room_floor_height(start_room, &rooms, &elevation_steps) + config.half_height,
                start.y,
            ),
            0.0,
        );
        body.grounded = true;
        Self {
            competitive,
            seed,
            base,
            elevation_steps,
            rooms,
            target: rendered.clone(),
            rendered,
            maze_tiles,
            spine_tiles,
            safe_tiles,
            trap_tiles,
            body,
            config,
            physics_backend,
            traversal_world,
            reroute_commits: 0,
            reroute_deferrals: 0,
            hazard_tick: 0,
            trap_hits: 0,
            trap_cooldown_ticks: 0,
            reroute_feedback_ticks: 0,
            last_traversal: Vec::new(),
            last_event: "Read the unstable graph; anchor and relay the route.".to_string(),
        }
    }

    pub fn local_index(&self) -> usize {
        self.competitive
            .teams
            .iter()
            .position(|team| team.id == LOCAL_TEAM)
            .expect("local team exists")
    }

    pub fn local_room(&self) -> RoomId {
        self.competitive.team_room(self.local_index())
    }

    pub fn local_target(&self) -> Option<RoomId> {
        self.competitive.next_room_for_team(self.local_index())
    }

    pub fn player_tile(&self) -> Option<(usize, usize)> {
        world_tile(Vec2::new(self.body.position.x, self.body.position.z))
    }

    pub fn player_room(&self) -> Option<RoomId> {
        self.player_tile()
            .and_then(|(x, y)| match self.maze_tiles[y * GRID_W + x] {
                Tile::Room(room) => Some(RoomId(room)),
                Tile::Wall | Tile::Corridor => None,
            })
    }

    pub fn player_on_floor(&self) -> bool {
        self.player_tile().is_some_and(|(x, y)| {
            self.maze_tiles[y * GRID_W + x].is_floor()
                && ((self.body.position.y - self.config.half_height) - self.floor_height(x, y))
                    .abs()
                    < 0.08
        })
    }

    pub fn floor_height(&self, x: usize, y: usize) -> f32 {
        self.elevation_steps[y * GRID_W + x] as f32 * ELEVATION_STEP_HEIGHT
    }

    pub fn room_floor_height(&self, room: RoomId) -> f32 {
        room_floor_height(room, &self.rooms, &self.elevation_steps)
    }

    pub fn trap_active(&self) -> bool {
        self.hazard_tick % TRAP_PERIOD_TICKS < TRAP_ACTIVE_TICKS
    }

    pub fn local_route_lengths(&self) -> Option<(usize, usize)> {
        let target = self.local_target()?;
        self.rendered
            .iter()
            .find(|route| {
                route.spine
                    && ((route.rooms.0 == self.local_room() && route.rooms.1 == target)
                        || (route.rooms.1 == self.local_room() && route.rooms.0 == target))
                    && !route.safe_path.is_empty()
            })
            .map(|route| (route.path.len(), route.safe_path.len()))
    }

    pub fn can_seize(&self) -> bool {
        if self.competitive.finished || self.player_room() != Some(CONTROL_ROOM) {
            return false;
        }
        let centre = room_world(CONTROL_ROOM, &self.rooms);
        Vec2::new(
            self.body.position.x - centre.x,
            self.body.position.z - centre.y,
        )
        .length()
            <= CONTROL_RADIUS
    }

    pub fn snapshot(&self) -> HybridSnapshot {
        HybridSnapshot {
            round: self.competitive.round,
            local_room: self.local_room(),
            player_room: self.player_room(),
            body_position: self.body.position,
            body_yaw: self.body.yaw,
            body_pitch: self.body.pitch,
            team_rooms: (0..TEAM_COUNT)
                .map(|index| self.competitive.team_room(index))
                .collect(),
            roles: self
                .competitive
                .teams
                .iter()
                .map(|team| match team.role {
                    Role::Runner => 0,
                    Role::Director => 1,
                })
                .collect(),
            placements: self
                .competitive
                .teams
                .iter()
                .map(|team| team.placement)
                .collect(),
            graph_links: self.competitive.structure.graph.links.clone(),
            control_holder: self.competitive.control_holder,
            purge_line: self.competitive.purge_line,
            winner: self.competitive.winner,
            finished: self.competitive.finished,
            rendered_routes: self.rendered.iter().map(|route| route.key).collect(),
            target_routes: self.target.iter().map(|route| route.key).collect(),
            maze_tiles: self.maze_tiles.clone(),
            elevation_steps: self.elevation_steps.clone(),
            safe_tiles: sorted_tiles(&self.safe_tiles),
            trap_tiles: sorted_tiles(&self.trap_tiles),
            reroute_commits: self.reroute_commits,
        }
    }
}

pub fn rebuild(base: &[Tile], routes: &[CorridorRoute]) -> Vec<Tile> {
    let mut tiles = base.to_vec();
    for route in routes {
        for &(x, y) in &crate::maze::thicken_path(&route.path, route_radius(route)) {
            if tiles[y * GRID_W + x] == Tile::Wall {
                tiles[y * GRID_W + x] = Tile::Corridor;
            }
        }
        for &(x, y) in &route.safe_path {
            if tiles[y * GRID_W + x] == Tile::Wall {
                tiles[y * GRID_W + x] = Tile::Corridor;
            }
        }
    }
    tiles
}

fn route_radius(route: &CorridorRoute) -> usize {
    if route.spine {
        SPINE_RADIUS
    } else {
        SIDE_RADIUS
    }
}

pub fn collect_spine_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    let mut tiles = HashSet::new();
    for route in routes.iter().filter(|route| route.spine) {
        tiles.extend(crate::maze::thicken_path(&route.path, SPINE_RADIUS));
        tiles.extend(route.safe_path.iter().copied());
    }
    tiles
}

pub fn collect_safe_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    routes
        .iter()
        .flat_map(|route| route.safe_path.iter().copied())
        .collect()
}

pub fn collect_trap_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    routes
        .iter()
        .flat_map(|route| route.trap_tiles.iter().copied())
        .collect()
}

pub fn sorted_tiles(tiles: &HashSet<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out = tiles.iter().copied().collect::<Vec<_>>();
    out.sort_unstable();
    out
}

pub fn tile_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
        (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
    )
}

pub fn world_tile(position: Vec2) -> Option<(usize, usize)> {
    let x = (position.x / TILE_SIZE + GRID_W as f32 * 0.5).floor();
    let y = (position.y / TILE_SIZE + GRID_H as f32 * 0.5).floor();
    if x < 0.0 || y < 0.0 || x >= GRID_W as f32 || y >= GRID_H as f32 {
        None
    } else {
        Some((x as usize, y as usize))
    }
}

pub fn room_world(room: RoomId, rooms: &[RoomRect]) -> Vec2 {
    let (x, y) = rooms[room.0 as usize].center_tile();
    tile_world(x, y)
}

pub fn room_floor_height(room: RoomId, rooms: &[RoomRect], elevation_steps: &[u8]) -> f32 {
    let (x, y) = rooms[room.0 as usize].center_tile();
    elevation_steps[y * GRID_W + x] as f32 * ELEVATION_STEP_HEIGHT
}

pub fn place_map_rooms(spec: &MapSpec) -> (Vec<Tile>, Vec<RoomRect>) {
    let mut tiles = vec![Tile::Wall; GRID_W * GRID_H];
    let min_x = spec
        .rooms
        .iter()
        .map(|room| room.schematic.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = spec
        .rooms
        .iter()
        .map(|room| room.schematic.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = spec
        .rooms
        .iter()
        .map(|room| room.schematic.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = spec
        .rooms
        .iter()
        .map(|room| room.schematic.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let sx = (max_x - min_x).max(1.0);
    let sy = (max_y - min_y).max(1.0);
    let mut rooms = vec![None; spec.room_count()];
    for room in &spec.rooms {
        let cx = 4.0 + ((room.schematic.x - min_x) / sx) * (GRID_W as f32 - 8.0);
        let cy = 4.0 + ((room.schematic.y - min_y) / sy) * (GRID_H as f32 - 8.0);
        let w = 5usize;
        let h = 5usize;
        let x =
            (cx.round() as isize - (w as isize / 2)).clamp(1, (GRID_W - w - 1) as isize) as usize;
        let y =
            (cy.round() as isize - (h as isize / 2)).clamp(1, (GRID_H - h - 1) as isize) as usize;
        let rect = RoomRect {
            room: room.id,
            x,
            y,
            w,
            h,
        };
        for yy in y..y + h {
            for xx in x..x + w {
                tiles[yy * GRID_W + xx] = Tile::Room(room.id.0);
            }
        }
        rooms[room.id.0 as usize] = Some(rect);
    }
    (
        tiles,
        rooms
            .into_iter()
            .map(|room| room.expect("room ids are dense for map specs"))
            .collect(),
    )
}

fn yaw_for_step(from: (usize, usize), to: (usize, usize)) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match (
        to.0 as isize - from.0 as isize,
        to.1 as isize - from.1 as isize,
    ) {
        (1, 0) => FRAC_PI_2,
        (-1, 0) => FRAC_PI_2 * 3.0,
        (0, 1) => PI,
        (0, -1) => 0.0,
        _ => 0.0,
    }
}

pub fn facing_for_next_room(
    room: RoomId,
    tiles: &[Tile],
    elevation_steps: &[u8],
    rooms: &[RoomRect],
) -> f32 {
    let Some((target, _)) = spine_next(room) else {
        return 0.0;
    };
    let start = rooms[room.0 as usize].center_tile();
    let goal = rooms[target.0 as usize].center_tile();
    super::round_step::path_between(tiles, elevation_steps, start, goal)
        .and_then(|path| path.get(1).copied())
        .map_or(0.0, |next| yaw_for_step(start, next))
}
