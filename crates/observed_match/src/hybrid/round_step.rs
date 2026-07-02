//! Hybrid match stepping, physics-step checking, and corridor routing.

use super::match_state::{
    collect_safe_tiles, collect_spine_tiles, collect_trap_tiles, facing_for_next_room, rebuild,
    room_world, tile_world,
};
use super::{
    CorridorRoute, HybridMatch, LOCAL_TEAM, LocalAction, REROUTE_FEEDBACK_TICKS, SPINE_RADIUS,
    TRAP_SETBACK_TICKS, VIEW_HALF_DEG, VIEW_RANGE_TILES, WALL_HEIGHT,
};
use crate::competition::RaceAction;
use crate::director::Role;
use crate::facility::CompetitiveFacility;
use crate::maze::{
    GRID_H, GRID_W, RoomRect, TILE_SIZE, Tile, build_elevated_arena, build_route_choice,
    elevations_are_step_connected, route_corridor,
};
use glam::{Vec2, Vec3};
use observed_core::{RoomId, TeamId};
use observed_traversal::{FIXED_DT, FpsArena, FpsBody, step_body};
use player_input::PlayerIntent;
use std::collections::{HashSet, VecDeque};
use std::f32::consts::TAU;

impl HybridMatch {
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

    /// Deterministic action playback.
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

    pub fn resolve_round(&mut self, local: LocalAction) {
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
        // first-person pose authoritative by placing it in the resulting team room.
        self.place_body_in_room(self.local_room());
        self.last_event = round_event(
            &self.competitive,
            &before_placements,
            &before_roles,
            local,
            self.affected_tiles().len(),
        );
    }

    pub fn place_body_in_room(&mut self, room: RoomId) {
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

    pub fn arena(&self) -> FpsArena {
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

    fn turn_away_from_pending(&mut self) {
        let affected = self.affected_tiles();
        if affected.is_empty() {
            return;
        }
        let player = Vec2::new(self.body.position.x, self.body.position.z);
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
        self.reachable_rooms() == self.competitive.structure.graph.room_count
    }
}

pub fn route_all(
    facility: &CompetitiveFacility,
    base: &[Tile],
    rooms: &[RoomRect],
) -> Vec<CorridorRoute> {
    let mut routes = Vec::new();
    for (door_a, door_b) in facility.structure.graph.connections() {
        let room_a = facility.structure.graph.door(door_a).room;
        let room_b = facility.structure.graph.door(door_b).room;
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
            let spine = facility.map_spec.is_none() && facility.structure.is_protected(door_a);
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

pub fn neighbours(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
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

pub fn path_between(
    tiles: &[Tile],
    elevation_steps: &[u8],
    start: (usize, usize),
    goal: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    path_between_avoiding(tiles, elevation_steps, start, goal, &HashSet::new())
}

pub fn path_between_avoiding(
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

pub fn reachable(tiles: &[Tile], elevation_steps: &[u8], rooms: &[RoomRect]) -> usize {
    let start = rooms[0].center_tile();
    let mut seen = vec![false; GRID_W * GRID_H];
    let mut reached = vec![false; rooms.len()];
    let mut queue = VecDeque::new();
    seen[start.1 * GRID_W + start.0] = true;
    queue.push_back(start);
    while let Some((x, y)) = queue.pop_front() {
        if let Tile::Room(room) = tiles[y * GRID_W + x]
            && let Some(reached) = reached.get_mut(room as usize)
        {
            *reached = true;
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

pub fn bot_policy(round: usize, team: TeamId) -> RaceAction {
    if team == TeamId(3) && round < 4 {
        RaceAction::Seize
    } else {
        RaceAction::Advance
    }
}
