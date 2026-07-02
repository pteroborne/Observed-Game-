use crate::items::{ItemKind, ItemsState};
use crate::screens::{MatchPaused, MatchRuntime, TeleportState};
use crate::teleport::Place;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_observation::{ObservationWorld, ROOM_COUNT, Side};
use std::collections::{HashSet, VecDeque};

const GUARDIAN_SPEED: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianState {
    Active,
    FrozenByPlayer,
    FrozenByAnchor,
}

#[derive(Resource, Clone, Debug)]
pub struct Guardian {
    pub room: RoomId,
    pub pos: Vec3, // 3D local position inside current room
    pub anchor_timer: f32,
    pub state: GuardianState,
    pub reassigned_target: Option<u8>,
}

impl Default for Guardian {
    fn default() -> Self {
        Self {
            room: RoomId(8), // Starts in room 8 (bottom-right)
            pos: Vec3::new(0.0, 0.76, 0.0),
            anchor_timer: 30.0,
            state: GuardianState::Active,
            reassigned_target: None,
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct ActionLog {
    pub entries: Vec<String>,
    pub capture_queue: Vec<String>,
}

impl ActionLog {
    pub fn add(&mut self, entry: String) {
        self.entries.push(entry.clone());
        if self.entries.len() > 5 {
            self.entries.remove(0);
        }
        if std::env::var("OBSERVED2_CAPTURE_BOT").is_ok() {
            self.capture_queue.push(entry);
        }
    }
}

/// Simple seeded PRNG for banishment choose.
pub struct SimpleRng(u64);

impl Default for SimpleRng {
    fn default() -> Self {
        Self(98765)
    }
}

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

/// Marker component for the Weeping Angel Guardian model in the 3D place.
#[derive(Component)]
pub struct GuardianModel;

pub fn yaw_to_side(yaw: f32) -> Side {
    let fwd = Vec2::new(yaw.sin(), -yaw.cos());
    let mut best_side = Side::North;
    let mut best_dot = -2.0;
    for side in Side::ALL {
        let offset_dir = side.vector();
        let dot = offset_dir.dot(fwd);
        if dot > best_dot {
            best_dot = dot;
            best_side = side;
        }
    }
    best_side
}

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
            let door = world.door_id(curr, facing);
            if !world.is_sealed(door) {
                let partner = world.partner(door);
                let dest = world.door(partner).room;
                if visible.insert(dest) {
                    queue.push_back((dest, 1));
                }
            }
        } else {
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

pub(crate) struct GuardianMoveTimer(Timer);
impl Default for GuardianMoveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.0, TimerMode::Repeating))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_guardian_in_match(
    time: Res<Time>,
    mut guardian: ResMut<Guardian>,
    mut tp: ResMut<TeleportState>,
    items: Res<ItemsState>,
    mut log: ResMut<ActionLog>,
    mut rng: Local<SimpleRng>,
    runtime: Res<MatchRuntime>,
    paused: Res<MatchPaused>,
    mut move_timer: Local<GuardianMoveTimer>,
    mut guardian_q: Query<&mut Transform, With<GuardianModel>>,
    mut commands: Commands,
    mut screenshot_count: Local<usize>,
    mut pending_screenshots: Local<Vec<(String, u32)>>,
    mut anim: ResMut<crate::screens::TeleportAnimation>,
) {
    if paused.0 || runtime.done {
        return;
    }

    // Initialize SimpleRng local state on first run
    if rng.0 == 0 {
        *rng = SimpleRng::new(98765);
    }

    // Process event-triggered capture queue if BOT capture is active
    if std::env::var("OBSERVED2_CAPTURE_BOT").is_ok() {
        // 1. Pop from ActionLog capture queue and queue a screenshot
        while !log.capture_queue.is_empty() {
            let msg = log.capture_queue.remove(0);
            let clean_name = msg
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .replace("___", "_")
                .replace("__", "_");
            // Delay screenshot by 3 frames to let the HUD update and render
            pending_screenshots.push((clean_name, 3));
        }

        // 2. Process pending screenshots
        let mut i = 0;
        while i < pending_screenshots.len() {
            if pending_screenshots[i].1 > 0 {
                pending_screenshots[i].1 -= 1;
                i += 1;
            } else {
                let (name, _) = pending_screenshots.remove(i);
                let path = format!("docs/evidence/event_{:02}_{}.png", *screenshot_count, name);
                info!("EVENT_CAPTURE: Taking screenshot of event: {}", path);
                commands
                    .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(path));
                *screenshot_count += 1;
            }
        }
    }

    let is_static_capture = std::env::var("OBSERVED2_CAPTURE_TOUR").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_MATCH").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_ROOM").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_KEYSTONE").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_RIVALS").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_DOORWAY_HALL").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_DOORWAY").is_ok()
        || std::env::var("OBSERVED2_CAPTURE_CEILING").is_ok();
    if is_static_capture {
        guardian.state = GuardianState::FrozenByPlayer;
        for mut transform in &mut guardian_q {
            transform.translation = guardian.pos;
        }
        return;
    }

    let game = runtime.live.host_match();
    let facility = &game.competitive;
    let world = &facility.structure.graph;

    // 1. Resolve current player room and facing
    let (player_room, in_same_room) = match tp.place {
        Place::Room(r) => (r, r == guardian.room),
        Place::Hallway { from, .. } => (from, false),
    };

    // 2. Gaze detection: is the player observing the guardian?
    let mut seen_by_player = false;
    if in_same_room {
        // Player is in the same room. Check first-person look vector overlap.
        let fwd = Vec3::new(tp.body.yaw.sin(), 0.0, -tp.body.yaw.cos());
        let to_guardian = guardian.pos - tp.body.position;
        if to_guardian.length() > 0.01 {
            let dot = fwd.dot(to_guardian.normalize());
            seen_by_player = dot > 0.3; // 70-degree vision cone
        }
    } else {
        // Threshold check: Player looks towards the doorway connections
        let facing = yaw_to_side(tp.body.yaw);
        let visible = visible_rooms_from_view(world, player_room, facing);
        seen_by_player = visible.contains(&guardian.room);
    }

    // 3. Rival occupancy check
    let mut seen_by_rival = false;
    for idx in 0..observed_match::facility::TEAM_COUNT {
        if idx as u8 != crate::flow::LOCAL_TEAM.0
            && facility.teams[idx].active_runner()
            && facility.team_room(idx) == guardian.room
        {
            seen_by_rival = true;
            break;
        }
    }

    // 4. Anchor torch check
    let seen_by_anchor = items
        .placed
        .iter()
        .any(|item| item.kind == ItemKind::AnchorTorch && item.place == Place::Room(guardian.room));

    // Reset reassigned target immediately upon entry to any tethered room
    if seen_by_anchor && guardian.reassigned_target.is_some() {
        guardian.reassigned_target = None;
        log.add(format!(
            "Guardian reassignment reset by player tether in Room {}!",
            guardian.room.0
        ));
    }

    // 5. State resolution
    guardian.state = if seen_by_player || seen_by_rival {
        GuardianState::FrozenByPlayer
    } else if seen_by_anchor {
        GuardianState::FrozenByAnchor
    } else {
        GuardianState::Active
    };

    // 6. Anchor timer tickdown
    if guardian.state == GuardianState::FrozenByAnchor {
        guardian.anchor_timer -= time.delta_secs();
        if guardian.anchor_timer <= 0.0 {
            // Banished! Teleport to a random room
            let next = rng.next_room(guardian.room);
            guardian.room = next;
            guardian.pos = Vec3::new(0.0, 0.76, 0.0);
            guardian.anchor_timer = 30.0;
            tp.rendered = None; // trigger rebuild if we are there
            log.add(format!(
                "Guardian banished by anchor light to Room {}!",
                next.0
            ));
        }
    } else {
        guardian.anchor_timer = 30.0;
    }

    // Determine current hunt target
    let target_room = if let Some(t) = guardian.reassigned_target {
        facility.team_room(t as usize)
    } else {
        player_room
    };

    // If hunting a rival, check if we arrived at their room and caught them
    if guardian.reassigned_target.is_some() && guardian.room == target_room {
        let caught_team = guardian.reassigned_target.unwrap();
        guardian.reassigned_target = None;
        let next = rng.next_room(guardian.room);
        guardian.room = next;
        guardian.pos = Vec3::new(0.0, 0.76, 0.0);
        tp.rendered = None;
        log.add(format!(
            "Guardian caught Rival Team {}! Target reset to Local Player.",
            caught_team
        ));
    }

    // 7. Movement / catching execution
    if guardian.state == GuardianState::Active {
        // If hunting player and in the same room, slide in real-time
        if guardian.reassigned_target.is_none() && in_same_room {
            let target = tp.body.position;
            let dir = (target - guardian.pos).normalize_or_zero();
            guardian.pos += dir * GUARDIAN_SPEED * time.delta_secs();

            // Check touch collision
            if (target - guardian.pos).length() < 1.1 {
                // CAUGHT! Teleport player to a random room
                let next = rng.next_room(player_room);
                tp.place = Place::Room(next);
                tp.body.position = Vec3::new(0.0, tp.config.half_height, 0.0);
                tp.rendered = None; // force reconstruct place scene
                log.add(format!("CAUGHT! Teleported to Room {}!", next.0));

                // Trigger 2s neon red flash overlay animation
                anim.trigger(2.0, Color::srgba(1.0, 0.05, 0.1, 1.0));
            }
        } else {
            // Stepping room-by-room on a timer towards the target room (player or rival)
            move_timer.0.tick(time.delta());
            if move_timer.0.just_finished()
                && let Some(path) =
                    find_shortest_path(world, guardian.room, target_room).filter(|p| p.len() > 1)
            {
                let prev_room = guardian.room;
                guardian.room = path[1];
                guardian.pos = Vec3::new(0.0, 0.76, 0.0);
                tp.rendered = None; // force rebuild if entering player's room
                log.add(format!(
                    "Guardian moved Room {} -> {}.",
                    prev_room.0, guardian.room.0
                ));
            }
        }
    }

    // 8. Synchronize Bevy 3D entity transform
    for mut transform in &mut guardian_q {
        transform.translation = guardian.pos;
    }
}
