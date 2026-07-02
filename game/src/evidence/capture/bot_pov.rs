use bevy::app::AppExit;
use bevy::prelude::*;
use observed_match::mutable::EXIT_ROOM;
use player_input::PlayerIntent;
use std::path::PathBuf;

use crate::GameState;
use crate::bot;
use crate::camera;
use crate::items;
use crate::keystones;
use crate::sim::director::MatchDirector;
use crate::sim::state::{ItemIntent, MatchIntent, TeleportState};
use crate::teleport;
use crate::view::components::GameCam;

const BOT_CAPTURE_INTERVAL: f32 = 1.0;
const BOT_CAPTURE_MAX_SHOTS: usize = 120;
const BOT_WAYPOINT_RADIUS: f32 = 0.9;
const BOT_CROSS_RADIUS: f32 = 1.2;

#[derive(Resource)]
pub(crate) struct BotPovCaptureRequest {
    pub(super) dir: PathBuf,
    pub(super) phase: u8,
    pub(super) next_shot_at: f32,
    pub(super) shot: usize,
    pub(super) route_place: Option<teleport::Place>,
    pub(super) route: Vec<Vec2>,
    pub(super) waypoint: usize,
    pub(super) finished: bool,
    pub(super) blocked_ticks: u32,
}

impl BotPovCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        Self {
            dir: PathBuf::from(dir),
            phase: 0,
            next_shot_at: 0.0,
            shot: 0,
            route_place: None,
            route: Vec::new(),
            waypoint: 0,
            finished: false,
            blocked_ticks: 0,
        }
    }

    pub(super) fn image_path(&self) -> String {
        self.dir
            .join(format!("bot_pov_{:03}.png", self.shot))
            .to_string_lossy()
            .into_owned()
    }

    pub(super) fn clear_route(&mut self) {
        self.route_place = None;
        self.route.clear();
        self.waypoint = 0;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_bot_pov_progress(
    time: Res<Time>,
    mut request: ResMut<BotPovCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchDirector>>,
    keys: Option<ResMut<keystones::KeystoneState>>,
    tp: Option<Res<TeleportState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    tac_state: Option<ResMut<crate::view::components::TacMapState>>,
    mut panel: Query<&mut Visibility, With<crate::view::components::TacMapPanel>>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut runtime), Some(mut keys)) = (runtime, keys) {
                // Unlock the final gate so this diagnostic follows the full spine to the
                // exit instead of turning into a keystone collection test.
                let rooms = keys.rooms.clone();
                for room in rooms {
                    keys.collect(room);
                }
                runtime.done = false;
                runtime.suppress_reroute_feedback();

                if let Some(mut ts) = tac_state {
                    ts.0 = true;
                }
                if let Ok(mut vis) = panel.single_mut() {
                    *vis = Visibility::Visible;
                }

                request.phase = 2;
                request.next_shot_at = elapsed + 1.0;
            }
        }
        2 => {
            if elapsed >= request.next_shot_at {
                if let Some(ref t) = tp {
                    info!(
                        "BOT_CAPTURE_SHOT: Taking shot {} at place {:?} position {:?}",
                        request.shot, t.place, t.body.position
                    );
                } else {
                    info!(
                        "BOT_CAPTURE_SHOT: Taking shot {} but TeleportState is missing",
                        request.shot
                    );
                }
                crate::evidence::driver::screenshot_to(&mut commands, request.image_path());
                request.shot += 1;
                request.next_shot_at = elapsed + BOT_CAPTURE_INTERVAL;
            }
            if request.finished || request.shot >= BOT_CAPTURE_MAX_SHOTS {
                request.phase = 3;
                request.next_shot_at = elapsed + 1.0;
            }
        }
        3 if elapsed >= request.next_shot_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }

    if request.phase >= 2
        && let Some(tp) = tp
        && let Ok(mut transform) = cam.single_mut()
    {
        camera::bot_view(&tp.body, &tp.config).apply_to(&mut transform);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_bot_pov_capture(
    mut request: ResMut<BotPovCaptureRequest>,
    mut runtime: ResMut<MatchDirector>,
    mut tp: ResMut<TeleportState>,
    keys: Res<keystones::KeystoneState>,
    items: Res<items::ItemsState>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
    guardian: Res<crate::guardian::Guardian>,
) {
    if request.phase < 2 || request.finished {
        return;
    }

    let in_same_room = matches!(tp.place, teleport::Place::Room(room) if room == guardian.room);
    if in_same_room && items.carried(crate::items::ItemKind::AnchorTorch) > 0 {
        item_intent.torch_action = true;
        info!("BOT_NAV: Bot dropped anchor torch to freeze guardian!");
    }

    if matches!(tp.place, teleport::Place::Room(room) if room.0 == EXIT_ROOM) {
        info!("BOT_NAV: Exit room reached!");
        request.finished = true;
        intent.0 = PlayerIntent::default();
        return;
    }

    if let Some(gap) = bot::target_gap_for_place(
        tp.place,
        &tp.geom,
        Vec2::new(tp.body.position.x, tp.body.position.z),
    ) {
        let here = Vec2::new(tp.body.position.x, tp.body.position.z);
        let rel = here - gap.center;
        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let at_aperture =
            rel.dot(gap.normal) > -0.45 && rel.dot(tangent).abs() <= gap.width * 0.5 + 0.35;
        if here.distance(gap.center) <= BOT_CROSS_RADIUS || at_aperture {
            info!(
                "BOT_NAV: Crossing gap in {:?} (gap center: {:?}, normal: {:?}). Distance: {}, at_aperture: {}",
                tp.place,
                gap.center,
                gap.normal,
                here.distance(gap.center),
                at_aperture
            );
            crate::screens::match_runtime::debug_cross_gap_for_capture(
                &mut tp,
                &mut runtime,
                gap,
                &keys,
                &items,
            );
            info!("BOT_NAV: Crossed into new place: {:?}", tp.place);
            request.clear_route();
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    if request.route_place != Some(tp.place)
        || request.waypoint >= request.route.len()
        || request.route.is_empty()
    {
        let Some(gap) = bot::target_gap_for_place(
            tp.place,
            &tp.geom,
            Vec2::new(tp.body.position.x, tp.body.position.z),
        ) else {
            info!("BOT_NAV: No target gap available in place {:?}", tp.place);
            request.finished = runtime.live.host_match().local_target().is_none();
            intent.0 = PlayerIntent::default();
            return;
        };
        let start = Vec2::new(tp.body.position.x, tp.body.position.z);
        if let Some(path) = bot::route_to_gap(&tp.geom, &tp.arena, &tp.config, start, &gap) {
            info!(
                "BOT_NAV: Computed new route in {:?} from {:?} to gap (center: {:?}, normal: {:?}). Waypoints count: {}. Path: {:?}",
                tp.place,
                start,
                gap.center,
                gap.normal,
                path.waypoints.len(),
                path.waypoints
            );
            request.route_place = Some(tp.place);
            request.route = path.waypoints;
            request.waypoint = 0;
            request.blocked_ticks = 0;
        } else {
            request.blocked_ticks += 1;
            if request.blocked_ticks.is_multiple_of(30) || request.blocked_ticks == 1 {
                info!(
                    "BOT_NAV: Route calculation FAILED in {:?} from {:?} to gap (center: {:?}). Blocked ticks: {}",
                    tp.place, start, gap.center, request.blocked_ticks
                );
            }
            intent.0 = PlayerIntent::default();
            if request.blocked_ticks > 90 {
                info!("BOT_NAV: Bot is completely BLOCKED. Aborting capture.");
                request.finished = true;
            }
            return;
        }
    }

    let here = Vec2::new(tp.body.position.x, tp.body.position.z);
    let old_wp = request.waypoint;
    while request.waypoint + 1 < request.route.len()
        && here.distance(request.route[request.waypoint]) <= BOT_WAYPOINT_RADIUS
    {
        request.waypoint += 1;
    }
    if request.waypoint != old_wp {
        info!(
            "BOT_NAV: Advanced waypoint to index {} (target: {:?}) in {:?}",
            request.waypoint, request.route[request.waypoint], tp.place
        );
    }

    let target = request.route[request.waypoint];
    let to = target - here;
    if to.length_squared() < 0.04 {
        intent.0 = PlayerIntent::default();
        return;
    }

    // Dynamic obstacle avoidance: detect nearby solids and steer away to prevent getting stuck
    let mut avoidance = Vec2::ZERO;
    let safety_dist = tp.config.radius + 0.05; // Collision radius plus safety margin
    let cy = tp.body.position.y;
    let hy = tp.config.half_height;

    for solid in &tp.arena.solids {
        // Only avoid solids overlapping the bot's vertical height range
        if cy - hy < solid.max.y && cy + hy > solid.min.y {
            let closest_x = here.x.clamp(solid.min.x, solid.max.x);
            let closest_z = here.y.clamp(solid.min.z, solid.max.z); // here.y is body.position.z
            let closest = Vec2::new(closest_x, closest_z);
            let diff = here - closest;
            let dist = diff.length();
            if dist > 0.0 && dist < safety_dist {
                // Apply a repulsion force that grows stronger as we get closer to the obstacle
                let weight = (safety_dist - dist) / safety_dist;
                avoidance += diff.normalize() * weight * 1.8;
            }
        }
    }

    let mut dir = to.normalize_or_zero();
    if avoidance.length_squared() > 1e-4 {
        dir = (dir + avoidance).normalize_or_zero();
    }

    // Heading control: slow down to standard walk speed if we need to make a sharp turn
    let forward_dir = Vec2::new(tp.body.forward().x, tp.body.forward().z).normalize_or_zero();
    let is_sharp_turn = forward_dir.dot(dir) < 0.65; // turn angle > ~50 degrees

    tp.body.yaw = dir.x.atan2(-dir.y);
    tp.body.pitch = -0.22;

    // Set movement intent instead of overriding position so the physics/collision systems handle it.
    intent.0.movement = Vec2::new(0.0, 1.0); // Move forward relative to yaw
    intent.0.sprint_held = !is_sharp_turn; // Sprint only on straightaways or gentle turns to reduce inertia drift

    // Coordinate logging to track progress
    info!(
        "BOT_NAV: pos=({:.3}, {:.3}), yaw={:.3}, wp={}/{}, target={:?}, blocked={}",
        here.x,
        here.y,
        tp.body.yaw,
        request.waypoint,
        request.route.len(),
        target,
        request.blocked_ticks
    );
}
