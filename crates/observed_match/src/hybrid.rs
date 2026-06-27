//! Phase 27: the full competitive match played in the concrete, rerouting maze.
//!
//! `CompetitiveFacility` remains authoritative for observation, the protected
//! spine, competition, exits, and the facility director. This module adds only
//! the spatial projection and first-person action boundary:
//!
//! - the graph is embedded as real floor tiles and corridors;
//! - the local player uses the deterministic Phase 20 controller;
//! - entering the next protected-spine room emits `Advance`;
//! - graph changes produce a target maze, committed atomically only off-camera
//!   and clear of the player, following Phase 26;
//! - a tape of local round actions reconstructs match state, rendered/target
//!   mazes, and the canonical first-person pose exactly.

use std::collections::{HashSet, VecDeque};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::competition::RaceAction;
use crate::director::Role;
use crate::facility::{CompetitiveFacility, TEAM_COUNT};
use crate::maze::{
    ELEVATION_STEP_HEIGHT, GRID_H, GRID_W, RoomRect, TILE_SIZE, Tile, build_elevated_arena,
    build_route_choice, elevations_are_step_connected, generated_elevation_steps, place_rooms,
    route_corridor, thicken_path,
};
use crate::mutable::spine_next;
use glam::{Vec2, Vec3};
use observed_core::{RoomId, TeamId};
use observed_observation::{DoorId, ROOM_COUNT};
use observed_traversal::{FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
use player_input::PlayerIntent;

pub const LOCAL_TEAM: TeamId = TeamId(0);
pub const CONTROL_ROOM: RoomId = RoomId(3);
const MAX_ROUNDS: usize = 64;
/// The protected spine is carved into a wide, readable hall (3-tile) while side
/// passages stay 1-tile — wide enough to read, narrow enough that solid wall always
/// survives between rooms so they stay distinct chambers.
const SPINE_RADIUS: usize = 1;
const SIDE_RADIUS: usize = 0;
const CONTROL_RADIUS: f32 = TILE_SIZE * 1.25;
const WALL_HEIGHT: f32 = 4.0;
const VIEW_RANGE_TILES: f32 = 16.0;
const VIEW_HALF_DEG: f32 = 48.0;
pub const TRAP_PERIOD_TICKS: u64 = 120;
pub const TRAP_ACTIVE_TICKS: u64 = 72;
pub const TRAP_SETBACK_TICKS: u16 = 30;
pub const REROUTE_FEEDBACK_TICKS: u16 = 45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalAction {
    Advance,
    Seize,
    Wait,
}

impl LocalAction {
    fn race_action(self) -> RaceAction {
        match self {
            Self::Seize => RaceAction::Seize,
            Self::Advance | Self::Wait => RaceAction::Advance,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HybridFrame {
    pub local: LocalAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorridorRoute {
    pub key: (DoorId, DoorId),
    pub rooms: (RoomId, RoomId),
    pub path: Vec<(usize, usize)>,
    pub spine: bool,
    pub safe_path: Vec<(usize, usize)>,
    pub trap_tiles: Vec<(usize, usize)>,
}

/// Comparable fingerprint of the complete integration state.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridSnapshot {
    pub round: u32,
    pub local_room: RoomId,
    pub player_room: Option<RoomId>,
    pub body_position: Vec3,
    pub body_yaw: f32,
    pub body_pitch: f32,
    pub team_rooms: Vec<RoomId>,
    pub roles: Vec<u8>,
    pub placements: Vec<Option<u8>>,
    pub graph_links: Vec<DoorId>,
    pub control_holder: Option<TeamId>,
    pub purge_line: f32,
    pub winner: Option<TeamId>,
    pub finished: bool,
    pub rendered_routes: Vec<(DoorId, DoorId)>,
    pub target_routes: Vec<(DoorId, DoorId)>,
    pub maze_tiles: Vec<Tile>,
    pub elevation_steps: Vec<u8>,
    pub safe_tiles: Vec<(usize, usize)>,
    pub trap_tiles: Vec<(usize, usize)>,
    pub reroute_commits: u32,
}

#[derive(Clone, Debug)]
pub struct HybridMatch {
    pub competitive: CompetitiveFacility,
    pub seed: u64,
    pub base: Vec<Tile>,
    pub elevation_steps: Vec<u8>,
    pub rooms: Vec<RoomRect>,
    /// Corridors currently presented and collided against.
    pub rendered: Vec<CorridorRoute>,
    /// Corridors requested by the authoritative graph.
    pub target: Vec<CorridorRoute>,
    pub maze_tiles: Vec<Tile>,
    pub spine_tiles: HashSet<(usize, usize)>,
    pub safe_tiles: HashSet<(usize, usize)>,
    pub trap_tiles: HashSet<(usize, usize)>,
    pub body: FpsBody,
    pub config: FpsConfig,
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
        let competitive = CompetitiveFacility::authored();
        let (base, rooms) = place_rooms(seed);
        let elevation_steps = generated_elevation_steps();
        let rendered = route_all(&competitive, &base, &rooms);
        let maze_tiles = rebuild(&base, &rendered);
        let spine_tiles = collect_spine_tiles(&rendered);
        let safe_tiles = collect_safe_tiles(&rendered);
        let trap_tiles = collect_trap_tiles(&rendered);
        let config = FpsConfig::default();
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
        spine_next(self.local_room()).map(|(room, _)| room)
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

    /// Fixed-step live controller. A round is emitted only after the player
    /// physically enters the next protected-spine room.
    pub fn step_player(&mut self, intent: PlayerIntent, top_down: bool) -> Option<LocalAction> {
        if self.competitive.finished {
            return None;
        }
        self.hazard_tick = self.hazard_tick.wrapping_add(1);
        self.trap_cooldown_ticks = self.trap_cooldown_ticks.saturating_sub(1);
        self.reroute_feedback_ticks = self.reroute_feedback_ticks.saturating_sub(1);
        if intent.interact_pressed && self.can_seize() {
            self.resolve_round(LocalAction::Seize);
            self.reconcile_current_view(top_down);
            return Some(LocalAction::Seize);
        }

        let arena = self.arena();
        let mut movement_intent = intent;
        if self.trap_cooldown_ticks > 0 {
            movement_intent.movement = Vec2::ZERO;
            movement_intent.jump_pressed = false;
        }
        step_body(
            &mut self.body,
            movement_intent,
            &arena,
            &self.config,
            FIXED_DT,
        );
        if self.trap_cooldown_ticks == 0
            && self.trap_active()
            && self
                .player_tile()
                .is_some_and(|tile| self.trap_tiles.contains(&tile))
        {
            self.trap_hits += 1;
            self.trap_cooldown_ticks = TRAP_SETBACK_TICKS;
            self.place_body_in_room(self.local_room());
            self.last_event = format!(
                "Pressure gate pulsed: returned to checkpoint. {} hit{}, no progress lost.",
                self.trap_hits,
                if self.trap_hits == 1 { "" } else { "s" }
            );
            self.reconcile_current_view(top_down);
            return None;
        }
        if self
            .local_target()
            .is_some_and(|target| self.player_room() == Some(target))
        {
            self.resolve_round(LocalAction::Advance);
            self.reconcile_current_view(top_down);
            return Some(LocalAction::Advance);
        }
        self.reconcile_current_view(top_down);
        None
    }

    /// Deterministic action playback. `Advance` traverses a real floor path to the
    /// next spine room before resolving the competitive round. A `Seize` is treated
    /// as **authoritative** — the live controller already validated `can_seize`
    /// before emitting it, so the replica applies it deterministically rather than
    /// re-running the spatial gate against its own (canonical, not live) body, which
    /// would otherwise desync a networked seize.
    pub fn apply_action(&mut self, action: LocalAction) -> bool {
        if self.competitive.finished {
            return false;
        }
        match action {
            LocalAction::Advance => {
                let Some(target) = self.local_target() else {
                    return false;
                };
                if !self.script_walk_to_room(target) {
                    return false;
                }
            }
            LocalAction::Seize | LocalAction::Wait => {}
        }
        self.resolve_round(action);
        self.turn_away_from_pending();
        self.reconcile_current_view(false);
        true
    }

    fn resolve_round(&mut self, local: LocalAction) {
        let before_placements: Vec<Option<u8>> = self
            .competitive
            .teams
            .iter()
            .map(|team| team.placement)
            .collect();
        let before_roles: Vec<Role> = self
            .competitive
            .teams
            .iter()
            .map(|team| team.role)
            .collect();

        // Preserve Phase 24's deterministic facility-director action.
        if self.competitive.round > 0
            && self.competitive.round.is_multiple_of(3)
            && let Some(team) = self
                .competitive
                .teams
                .iter()
                .find(|team| team.role == Role::Director && team.placement.is_none())
                .map(|team| team.id)
        {
            self.competitive.scramble(team);
        }

        let round = self.competitive.round as usize;
        let intents = self
            .competitive
            .teams
            .iter()
            .filter(|team| team.active_runner())
            .map(|team| {
                let action = if team.id == LOCAL_TEAM {
                    local.race_action()
                } else {
                    bot_policy(round, team.id)
                };
                (team.id, action)
            })
            .collect::<Vec<_>>();
        self.competitive.advance_round(&intents);
        self.target = route_all(&self.competitive, &self.base, &self.rooms);

        // A control-holder can earn more than one graph step in a round. Keep the
        // first-person pose authoritative by placing it in the resulting team room;
        // normal rounds have already physically crossed the one required corridor.
        self.place_body_in_room(self.local_room());
        self.last_event = round_event(
            &self.competitive,
            &before_placements,
            &before_roles,
            local,
            self.affected_tiles().len(),
        );
    }

    fn place_body_in_room(&mut self, room: RoomId) {
        let centre = room_world(room, &self.rooms);
        let yaw = facing_for_next_room(room, &self.maze_tiles, &self.elevation_steps, &self.rooms);
        self.body = FpsBody::spawned(
            Vec3::new(
                centre.x,
                self.room_floor_height(room) + self.config.half_height,
                centre.y,
            ),
            yaw,
        );
        self.body.grounded = true;
    }

    fn script_walk_to_room(&mut self, room: RoomId) -> bool {
        let Some(start) = self.player_tile() else {
            return false;
        };
        let goal = self.rooms[room.0 as usize].center_tile();
        let Some(path) = path_between_avoiding(
            &self.maze_tiles,
            &self.elevation_steps,
            start,
            goal,
            &self.trap_tiles,
        ) else {
            return false;
        };
        if let Some(window) = path.windows(2).last() {
            self.body.yaw = yaw_for_step(window[0], window[1]);
        }
        let world = tile_world(goal.0, goal.1);
        self.body.position = Vec3::new(
            world.x,
            self.floor_height(goal.0, goal.1) + self.config.half_height,
            world.y,
        );
        self.body.velocity = Vec3::ZERO;
        self.body.grounded = true;
        self.last_traversal = path;
        true
    }

    fn arena(&self) -> FpsArena {
        build_elevated_arena(&self.maze_tiles, &self.elevation_steps, WALL_HEIGHT)
    }

    pub fn affected_tiles(&self) -> HashSet<(usize, usize)> {
        let target_tiles = rebuild(&self.base, &self.target);
        let mut affected = HashSet::new();
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let index = y * GRID_W + x;
                if self.maze_tiles[index] != target_tiles[index] {
                    affected.insert((x, y));
                }
            }
        }
        affected
    }

    pub fn in_sync(&self) -> bool {
        self.affected_tiles().is_empty()
    }

    pub fn visible_tiles(&self, top_down: bool) -> HashSet<(usize, usize)> {
        if top_down {
            return (0..GRID_H)
                .flat_map(|y| (0..GRID_W).map(move |x| (x, y)))
                .collect();
        }
        let eye = Vec2::new(self.body.position.x, self.body.position.z);
        let forward = Vec2::new(self.body.yaw.sin(), -self.body.yaw.cos());
        let range = VIEW_RANGE_TILES * TILE_SIZE;
        let cos_half = VIEW_HALF_DEG.to_radians().cos();
        let mut visible = HashSet::new();
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let to = tile_world(x, y) - eye;
                let distance = to.length();
                if distance <= range
                    && (distance < TILE_SIZE || to.normalize_or_zero().dot(forward) >= cos_half)
                {
                    visible.insert((x, y));
                }
            }
        }
        visible
    }

    /// Commit the target maze as one atomic swap only if every changed tile is
    /// outside the supplied view and clear of the player's collision footprint.
    pub fn try_commit_reroute(&mut self, visible: &HashSet<(usize, usize)>) -> bool {
        let affected = self.affected_tiles();
        if affected.is_empty() {
            return false;
        }
        let player = Vec2::new(self.body.position.x, self.body.position.z);
        let clearance = TILE_SIZE * 0.5 + self.config.radius;
        let blocked = affected.iter().any(|&(x, y)| {
            let centre = tile_world(x, y);
            visible.contains(&(x, y))
                || (centre.x - player.x).abs() <= clearance
                    && (centre.y - player.y).abs() <= clearance
        });
        if blocked {
            self.reroute_deferrals += 1;
            return false;
        }

        self.rendered = self.target.clone();
        self.maze_tiles = rebuild(&self.base, &self.rendered);
        self.spine_tiles = collect_spine_tiles(&self.rendered);
        self.safe_tiles = collect_safe_tiles(&self.rendered);
        self.trap_tiles = collect_trap_tiles(&self.rendered);
        self.reroute_commits += 1;
        self.reroute_feedback_ticks = REROUTE_FEEDBACK_TICKS;
        self.last_event
            .push_str(" Passages rerouted off-camera in one atomic swap.");
        assert!(self.navigable(), "an atomic reroute must remain navigable");
        true
    }

    pub fn reconcile_current_view(&mut self, top_down: bool) -> bool {
        let visible = self.visible_tiles(top_down);
        self.try_commit_reroute(&visible)
    }

    /// Scripted recordings deliberately turn away before the atomic commit; the
    /// same pose is reconstructed during replay.
    fn turn_away_from_pending(&mut self) {
        let affected = self.affected_tiles();
        if affected.is_empty() {
            return;
        }
        let player = Vec2::new(self.body.position.x, self.body.position.z);
        // HashSet iteration order is intentionally unspecified. Sum in grid order
        // so the canonical replay pose is bit-for-bit deterministic.
        let mut sum = Vec2::ZERO;
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if affected.contains(&(x, y)) {
                    sum += tile_world(x, y);
                }
            }
        }
        let centre = sum / affected.len() as f32;
        let away = (player - centre).normalize_or_zero();
        if away.length_squared() > 0.0 {
            self.body.yaw = away.x.atan2(-away.y).rem_euclid(TAU);
        }
    }

    pub fn reachable_rooms(&self) -> usize {
        reachable(&self.maze_tiles, &self.elevation_steps, &self.rooms)
    }

    pub fn navigable(&self) -> bool {
        self.reachable_rooms() == ROOM_COUNT
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

fn bot_policy(round: usize, team: TeamId) -> RaceAction {
    if team == TeamId(3) && round < 4 {
        RaceAction::Seize
    } else {
        RaceAction::Advance
    }
}

fn route_all(
    competitive: &CompetitiveFacility,
    base: &[Tile],
    rooms: &[RoomRect],
) -> Vec<CorridorRoute> {
    let mut routes = Vec::new();
    for (door_a, door_b) in competitive.structure.graph.connections() {
        let room_a = competitive.structure.graph.door(door_a).room;
        let room_b = competitive.structure.graph.door(door_b).room;
        if room_a == room_b {
            continue;
        }
        if let Some(path) =
            route_corridor(base, &rooms[room_a.0 as usize], &rooms[room_b.0 as usize])
        {
            let key = if door_a.0 <= door_b.0 {
                (door_a, door_b)
            } else {
                (door_b, door_a)
            };
            let spine = competitive.structure.is_protected(door_a);
            let choice = spine
                .then(|| build_route_choice(base, &path, SPINE_RADIUS))
                .flatten();
            routes.push(CorridorRoute {
                key,
                rooms: (room_a, room_b),
                path,
                spine,
                safe_path: choice
                    .as_ref()
                    .map(|choice| choice.safe_path.clone())
                    .unwrap_or_default(),
                trap_tiles: choice.map(|choice| choice.trap_tiles).unwrap_or_default(),
            });
        }
    }
    routes
}

fn route_radius(route: &CorridorRoute) -> usize {
    if route.spine {
        SPINE_RADIUS
    } else {
        SIDE_RADIUS
    }
}

fn rebuild(base: &[Tile], routes: &[CorridorRoute]) -> Vec<Tile> {
    let mut tiles = base.to_vec();
    for route in routes {
        for &(x, y) in &thicken_path(&route.path, route_radius(route)) {
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

fn collect_spine_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    let mut tiles = HashSet::new();
    for route in routes.iter().filter(|route| route.spine) {
        tiles.extend(thicken_path(&route.path, SPINE_RADIUS));
        tiles.extend(route.safe_path.iter().copied());
    }
    tiles
}

fn collect_safe_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    routes
        .iter()
        .flat_map(|route| route.safe_path.iter().copied())
        .collect()
}

fn collect_trap_tiles(routes: &[CorridorRoute]) -> HashSet<(usize, usize)> {
    routes
        .iter()
        .flat_map(|route| route.trap_tiles.iter().copied())
        .collect()
}

fn sorted_tiles(tiles: &HashSet<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out = tiles.iter().copied().collect::<Vec<_>>();
    out.sort_unstable();
    out
}

fn tile_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
        (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
    )
}

fn world_tile(position: Vec2) -> Option<(usize, usize)> {
    let x = (position.x / TILE_SIZE + GRID_W as f32 * 0.5).floor();
    let y = (position.y / TILE_SIZE + GRID_H as f32 * 0.5).floor();
    if x < 0.0 || y < 0.0 || x >= GRID_W as f32 || y >= GRID_H as f32 {
        None
    } else {
        Some((x as usize, y as usize))
    }
}

fn room_world(room: RoomId, rooms: &[RoomRect]) -> Vec2 {
    let (x, y) = rooms[room.0 as usize].center_tile();
    tile_world(x, y)
}

fn room_floor_height(room: RoomId, rooms: &[RoomRect], elevation_steps: &[u8]) -> f32 {
    let (x, y) = rooms[room.0 as usize].center_tile();
    elevation_steps[y * GRID_W + x] as f32 * ELEVATION_STEP_HEIGHT
}

fn neighbours(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    let mut neighbours = Vec::with_capacity(4);
    if y > 0 {
        neighbours.push((x, y - 1));
    }
    if x + 1 < GRID_W {
        neighbours.push((x + 1, y));
    }
    if y + 1 < GRID_H {
        neighbours.push((x, y + 1));
    }
    if x > 0 {
        neighbours.push((x - 1, y));
    }
    neighbours.into_iter()
}

fn path_between(
    tiles: &[Tile],
    elevation_steps: &[u8],
    start: (usize, usize),
    goal: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    path_between_avoiding(tiles, elevation_steps, start, goal, &HashSet::new())
}

fn path_between_avoiding(
    tiles: &[Tile],
    elevation_steps: &[u8],
    start: (usize, usize),
    goal: (usize, usize),
    blocked: &HashSet<(usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    if !tiles[start.1 * GRID_W + start.0].is_floor() || !tiles[goal.1 * GRID_W + goal.0].is_floor()
    {
        return None;
    }
    let mut seen = vec![false; GRID_W * GRID_H];
    let mut parent = vec![None; GRID_W * GRID_H];
    let mut queue = VecDeque::new();
    seen[start.1 * GRID_W + start.0] = true;
    queue.push_back(start);
    while let Some((x, y)) = queue.pop_front() {
        if (x, y) == goal {
            let mut path = vec![goal];
            let mut current = goal;
            while current != start {
                current = parent[current.1 * GRID_W + current.0]?;
                path.push(current);
            }
            path.reverse();
            return Some(path);
        }
        for (nx, ny) in neighbours(x, y) {
            let index = ny * GRID_W + nx;
            if !seen[index]
                && tiles[index].is_floor()
                && !blocked.contains(&(nx, ny))
                && elevations_are_step_connected(
                    elevation_steps[y * GRID_W + x],
                    elevation_steps[index],
                )
            {
                seen[index] = true;
                parent[index] = Some((x, y));
                queue.push_back((nx, ny));
            }
        }
    }
    None
}

fn reachable(tiles: &[Tile], elevation_steps: &[u8], rooms: &[RoomRect]) -> usize {
    let start = rooms[0].center_tile();
    let mut seen = vec![false; GRID_W * GRID_H];
    let mut reached = [false; ROOM_COUNT];
    let mut queue = VecDeque::new();
    seen[start.1 * GRID_W + start.0] = true;
    queue.push_back(start);
    while let Some((x, y)) = queue.pop_front() {
        if let Tile::Room(room) = tiles[y * GRID_W + x] {
            reached[room as usize] = true;
        }
        for (nx, ny) in neighbours(x, y) {
            let index = ny * GRID_W + nx;
            if !seen[index]
                && tiles[index].is_floor()
                && elevations_are_step_connected(
                    elevation_steps[y * GRID_W + x],
                    elevation_steps[index],
                )
            {
                seen[index] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    reached.iter().filter(|value| **value).count()
}

fn yaw_for_step(from: (usize, usize), to: (usize, usize)) -> f32 {
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

fn facing_for_next_room(
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
    path_between(tiles, elevation_steps, start, goal)
        .and_then(|path| path.get(1).copied())
        .map_or(0.0, |next| yaw_for_step(start, next))
}

fn round_event(
    match_state: &CompetitiveFacility,
    before_placements: &[Option<u8>],
    before_roles: &[Role],
    local: LocalAction,
    pending_tiles: usize,
) -> String {
    let mut events = vec![match local {
        LocalAction::Advance => "Local team crossed into the next spine room.".to_string(),
        LocalAction::Seize => "Local team seized the shared control.".to_string(),
        LocalAction::Wait => {
            "Local team resolved while the remaining runners advanced.".to_string()
        }
    }];
    for (index, team) in match_state.teams.iter().enumerate() {
        if team.placement.is_some() && before_placements[index].is_none() {
            events.push(format!("{} escaped.", team.id.label()));
        }
        if team.role == Role::Director && before_roles[index] == Role::Runner {
            events.push(format!("{} joined the facility director.", team.id.label()));
        }
    }
    if pending_tiles > 0 {
        events.push(format!(
            "{pending_tiles} passage tiles await a safe off-camera swap."
        ));
    }
    if match_state.finished {
        events.push(format!(
            "Match over: {} escaped, {} absorbed.",
            match_state.escaped_count(),
            match_state.absorbed_count()
        ));
    }
    events.join(" ")
}

#[derive(Clone, Debug, Default)]
pub struct HybridTape {
    pub frames: Vec<HybridFrame>,
    pub snapshots: Vec<HybridSnapshot>,
    pub seed: u64,
}

impl HybridTape {
    pub fn record_demo(seed: u64) -> Self {
        let mut session = HybridMatch::authored(seed);
        let mut tape = Self {
            seed,
            snapshots: vec![session.snapshot()],
            ..Default::default()
        };
        while !session.competitive.finished && tape.frames.len() < MAX_ROUNDS {
            let local_active = session
                .competitive
                .team(LOCAL_TEAM)
                .is_some_and(|team| team.active_runner());
            let local = if local_active {
                LocalAction::Advance
            } else {
                LocalAction::Wait
            };
            assert!(session.apply_action(local));
            tape.frames.push(HybridFrame { local });
            tape.snapshots.push(session.snapshot());
        }
        assert!(
            session.competitive.finished,
            "the hybrid match must resolve"
        );
        tape
    }

    pub fn replay_to(&self, round: usize) -> HybridMatch {
        let mut session = HybridMatch::authored(self.seed);
        for frame in self.frames.iter().take(round.min(self.frames.len())) {
            session.apply_action(frame.local);
        }
        session
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn exact_at(&self, round: usize) -> bool {
        let index = round.min(self.frames.len());
        self.snapshots
            .get(index)
            .is_some_and(|expected| self.replay_to(index).snapshot() == *expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facility::{EXIT_CAPACITY, START_ROOM};

    #[test]
    fn the_hybrid_match_resolves_to_the_competitive_result() {
        let tape = HybridTape::record_demo(1);
        let end = tape.replay_to(tape.len());
        assert!(end.competitive.finished);
        assert_eq!(end.competitive.escaped_count(), EXIT_CAPACITY as usize);
        assert_eq!(end.competitive.winner, Some(LOCAL_TEAM));
        assert_eq!(
            end.competitive.escaped_count() + end.competitive.absorbed_count(),
            TEAM_COUNT
        );
    }

    #[test]
    fn replay_reproduces_match_maze_and_first_person_pose_exactly() {
        let tape = HybridTape::record_demo(2);
        for round in [
            0,
            1,
            tape.len() / 2,
            tape.len().saturating_sub(1),
            tape.len(),
        ] {
            assert!(tape.exact_at(round), "exact at round {round}");
            assert_eq!(tape.replay_to(round).snapshot(), tape.snapshots[round]);
        }
    }

    #[test]
    fn sequential_playback_matches_seek() {
        let tape = HybridTape::record_demo(3);
        let mut session = HybridMatch::authored(tape.seed);
        for round in 0..=tape.len() {
            assert_eq!(session.snapshot(), tape.replay_to(round).snapshot());
            if let Some(frame) = tape.frames.get(round) {
                session.apply_action(frame.local);
            }
        }
    }

    #[test]
    fn advance_uses_a_contiguous_real_floor_path() {
        let mut session = HybridMatch::authored(4);
        let before = session.maze_tiles.clone();
        let start = session.rooms[START_ROOM as usize].center_tile();
        let target = session.local_target().expect("spine continues");
        let goal = session.rooms[target.0 as usize].center_tile();
        assert!(session.apply_action(LocalAction::Advance));
        assert_eq!(session.last_traversal.first(), Some(&start));
        assert_eq!(session.last_traversal.last(), Some(&goal));
        for tile in &session.last_traversal {
            assert!(before[tile.1 * GRID_W + tile.0].is_floor());
        }
        for pair in session.last_traversal.windows(2) {
            assert_eq!(
                pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1),
                1
            );
        }
    }

    #[test]
    fn live_advance_is_spatially_gated_to_the_target_room() {
        let mut session = HybridMatch::authored(5);
        assert_eq!(session.local_room(), RoomId(START_ROOM));
        assert_eq!(
            session.step_player(PlayerIntent::default(), false),
            None,
            "standing at spawn does not advance the match"
        );
        let target = session.local_target().expect("spine target");
        session.place_body_in_room(target);
        assert_eq!(
            session.step_player(PlayerIntent::default(), false),
            Some(LocalAction::Advance)
        );
        assert_eq!(session.local_room(), target);
    }

    #[test]
    fn reroutes_defer_in_view_and_commit_atomically_off_camera() {
        let mut session = HybridMatch::authored(6);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        let affected = session.affected_tiles();
        assert!(!affected.is_empty());
        let before = session.maze_tiles.clone();
        assert!(!session.try_commit_reroute(&affected));
        assert_eq!(session.maze_tiles, before);
        session.place_body_in_room(session.local_room());
        assert!(session.try_commit_reroute(&HashSet::new()));
        assert!(session.in_sync());
        assert!(session.navigable());
    }

    #[test]
    fn a_reroute_never_changes_the_player_footprint() {
        let mut session = HybridMatch::authored(7);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        let affected = session.affected_tiles();
        let &(x, y) = affected.iter().next().expect("reroute changes tiles");
        let point = tile_world(x, y);
        session.body.position.x = point.x;
        session.body.position.z = point.y;
        assert!(!session.try_commit_reroute(&HashSet::new()));
        assert!(!session.in_sync());
    }

    #[test]
    fn the_rendered_maze_stays_navigable_every_round() {
        let tape = HybridTape::record_demo(8);
        for round in 0..=tape.len() {
            let session = tape.replay_to(round);
            assert!(session.navigable(), "navigable at round {round}");
            assert!(session.player_on_floor(), "player remains on floor");
        }
    }

    #[test]
    fn match_replay_and_network_snapshots_include_multi_level_elevation() {
        let tape = HybridTape::record_demo(81);
        assert!(
            tape.snapshots
                .iter()
                .all(|snapshot| snapshot.elevation_steps.len() == GRID_W * GRID_H)
        );
        assert!(
            tape.snapshots
                .iter()
                .any(|snapshot| snapshot.body_position.y > FpsConfig::default().half_height + 0.5),
            "the canonical match path reaches an elevated room"
        );
        for round in 0..=tape.len() {
            assert_eq!(
                tape.replay_to(round).snapshot().elevation_steps,
                tape.snapshots[round].elevation_steps,
                "elevation field is replay-exact at round {round}"
            );
        }
    }

    #[test]
    fn every_spine_leg_offers_a_short_trapped_route_and_long_safe_bypass() {
        for seed in [1, 2, 17, 82, 999] {
            let session = HybridMatch::authored(seed);
            let spine = session
                .rendered
                .iter()
                .filter(|route| route.spine)
                .collect::<Vec<_>>();
            assert!(!spine.is_empty());
            for route in spine {
                assert!(
                    !route.safe_path.is_empty() && !route.trap_tiles.is_empty(),
                    "seed {seed} spine leg {:?} must expose both choices",
                    route.rooms
                );
                assert!(
                    route.safe_path.len() > route.path.len(),
                    "safe route is the deliberate detour"
                );
                assert!(
                    route
                        .safe_path
                        .iter()
                        .all(|tile| !route.trap_tiles.contains(tile)),
                    "safe bypass avoids its pressure gate"
                );
            }
        }
    }

    #[test]
    fn scripted_replay_uses_the_safe_route() {
        let mut session = HybridMatch::authored(83);
        let traps = session.trap_tiles.clone();
        assert!(session.apply_action(LocalAction::Advance));
        assert!(
            session
                .last_traversal
                .iter()
                .all(|tile| !traps.contains(tile)),
            "canonical replay path takes the safe bypass"
        );
    }

    #[test]
    fn active_pressure_gate_sets_back_without_removing_progress() {
        let mut session = HybridMatch::authored(84);
        let trap = *session.trap_tiles.iter().next().expect("generated trap");
        let point = tile_world(trap.0, trap.1);
        session.body.position = Vec3::new(
            point.x,
            session.floor_height(trap.0, trap.1) + session.config.half_height,
            point.y,
        );
        session.body.velocity = Vec3::ZERO;
        session.body.grounded = true;
        let room_before = session.local_room();
        let round_before = session.competitive.round;

        assert_eq!(session.step_player(PlayerIntent::default(), false), None);
        assert_eq!(session.trap_hits, 1);
        assert_eq!(session.local_room(), room_before);
        assert_eq!(
            session.competitive.round, round_before,
            "trap costs time and position, never earned progress"
        );
        assert_eq!(session.player_room(), Some(room_before));
        assert!(session.trap_cooldown_ticks > 0);
    }

    #[test]
    fn inactive_pressure_gate_allows_the_risky_shortcut() {
        let mut session = HybridMatch::authored(85);
        let trap = *session.trap_tiles.iter().next().expect("generated trap");
        let point = tile_world(trap.0, trap.1);
        session.body.position = Vec3::new(
            point.x,
            session.floor_height(trap.0, trap.1) + session.config.half_height,
            point.y,
        );
        session.body.velocity = Vec3::ZERO;
        session.body.grounded = true;
        session.hazard_tick = TRAP_ACTIVE_TICKS;

        session.step_player(PlayerIntent::default(), false);
        assert_eq!(session.trap_hits, 0);
        assert_eq!(session.player_tile(), Some(trap));
    }

    #[test]
    fn committed_reroute_emits_first_person_feedback() {
        let mut session = HybridMatch::authored(86);
        session.competitive.advance_round(&[]);
        session.target = route_all(&session.competitive, &session.base, &session.rooms);
        session.place_body_in_room(session.local_room());
        assert!(session.try_commit_reroute(&HashSet::new()));
        assert_eq!(session.reroute_feedback_ticks, REROUTE_FEEDBACK_TICKS);
    }

    #[test]
    fn the_rendered_maze_actually_reroutes_during_the_match() {
        let tape = HybridTape::record_demo(9);
        let first = &tape.snapshots[0].rendered_routes;
        assert!(
            tape.snapshots
                .iter()
                .any(|snapshot| &snapshot.rendered_routes != first),
            "at least one authoritative reroute commits to the rendered maze"
        );
    }

    #[test]
    fn the_control_is_spatially_gated() {
        let mut session = HybridMatch::authored(10);
        assert!(!session.can_seize());
        while session.local_room() != CONTROL_ROOM {
            assert!(session.apply_action(LocalAction::Advance));
        }
        assert!(session.can_seize());
        assert!(session.apply_action(LocalAction::Seize));
        assert_eq!(session.competitive.control_holder, Some(LOCAL_TEAM));
    }

    #[test]
    fn deterministic_recordings_are_identical() {
        let a = HybridTape::record_demo(11);
        let b = HybridTape::record_demo(11);
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.snapshots, b.snapshots);
    }
}
