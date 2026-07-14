use bevy::app::AppExit;
use bevy::prelude::*;
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

#[derive(Resource)]
pub(crate) struct BotPovCaptureRequest {
    pub(super) dir: PathBuf,
    pub(super) phase: u8,
    pub(super) next_shot_at: f32,
    pub(super) shot: usize,
    pub(super) route_place: Option<teleport::Place>,
    pub(super) route: Vec<Vec2>,
    /// Parallel to `route`: whether the leg arriving at that waypoint should hold
    /// `jump_pressed` (set when piloting a Gantry hallway's deck).
    pub(super) route_jumps: Vec<bool>,
    /// Whether `route` was planned on a Gantry deck — lets the driver notice a fall to the
    /// understory (unreachable deck waypoints) and re-plan a ground recovery.
    pub(super) route_deck: bool,
    pub(super) waypoint: usize,
    pub(super) finished: bool,
    pub(super) blocked_ticks: u32,
    /// Force the first place into a wellshaft so the descent is guaranteed on camera
    /// (the ordinary walkthrough only hits one if the generated spine happens to).
    pub(super) force_wellshaft: bool,
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
            route_jumps: Vec::new(),
            route_deck: false,
            waypoint: 0,
            finished: false,
            blocked_ticks: 0,
            force_wellshaft: false,
        }
    }

    /// A bot-POV run staged to begin inside a wellshaft, so the descent is captured
    /// even on seeds whose spine has no vertical edge.
    pub(super) fn new_wellshaft(dir: String) -> Self {
        Self {
            force_wellshaft: true,
            ..Self::new(dir)
        }
    }

    pub(super) fn image_path(&self) -> String {
        self.dir
            .join(format!("bot_pov_{:03}.png", self.shot))
            .to_string_lossy()
            .into_owned()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_bot_pov_progress(
    time: Res<Time>,
    mut request: ResMut<BotPovCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<MatchDirector>>,
    keys: Option<ResMut<keystones::KeystoneState>>,
    mut tp: Option<ResMut<TeleportState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    tac_state: Option<ResMut<crate::view::components::TacMapState>>,
    mut panel: Query<&mut Visibility, With<crate::view::components::TacMapPanel>>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            if runtime.is_some() {
                request.phase = 1;
            } else {
                next.set(GameState::Match);
            }
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

                // Optionally drop the bot straight into a wellshaft so the descent is
                // guaranteed on camera. The body enters at the top (its entry gap
                // targets `from`); the ordinary descent piloting handles the rest.
                if request.force_wellshaft
                    && let (Some(to), Some(tp), Some(items)) = (
                        runtime.live.host_match().local_target(),
                        tp.as_deref_mut(),
                        item_state.as_deref(),
                    )
                {
                    let from = runtime.live.host_match().local_room();
                    let variation = crate::hallway::TEMPLATES
                        .iter()
                        .position(|t| t.flavor == crate::hallway::HallwayFlavor::Wellshaft)
                        .expect("wellshaft template");
                    let place = teleport::Place::legacy_hallway(from, to, variation);
                    crate::screens::match_runtime::debug_place_into(
                        tp, &runtime, place, from, &keys, items,
                    );
                    // `debug_place_into` drops the body at the base floor, but the
                    // wellshaft's entry is elevated — start it on the top landing
                    // facing down the first flight so the descent plays from the rim.
                    let top = crate::hallway::wellshaft_landing_rest(
                        crate::hallway::WELL_SHAFT_LEVELS - 1,
                    );
                    let next = crate::hallway::wellshaft_landing_rest(
                        crate::hallway::WELL_SHAFT_LEVELS - 2,
                    );
                    let y_off = crate::teleport::place_y_offset(place);
                    let half_h = tp.config.half_height;
                    tp.body.position = Vec3::new(
                        top.0,
                        y_off + crate::hallway::WELL_SHAFT_HEIGHT + half_h,
                        top.1,
                    );
                    let facing = Vec2::new(next.0 - top.0, next.1 - top.1).normalize_or_zero();
                    tp.body.yaw = facing.x.atan2(-facing.y);
                    tp.body.grounded = true;
                    // Let the buried-dark register settle before the first frame.
                    request.next_shot_at = elapsed + 1.0;
                }
            }
        }
        2 => {
            if elapsed >= request.next_shot_at {
                if let Some(ref t) = tp {
                    info!(
                        "BOT_CAPTURE_SHOT: Taking shot {} at place {:?} position {:?}",
                        request.shot, t.place, t.body.position
                    );
                    crate::evidence::driver::screenshot_to(&mut commands, request.image_path());
                    request.shot += 1;
                    request.next_shot_at = elapsed + BOT_CAPTURE_INTERVAL;
                } else {
                    info!("BOT_CAPTURE_SHOT: TeleportState is missing, finishing capture.");
                    request.finished = true;
                }
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
    runtime: Res<MatchDirector>,
    mut tp: ResMut<TeleportState>,
    items: Res<items::ItemsState>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
    guardian: Option<Res<crate::guardian::Guardian>>,
) {
    if request.phase < 2 || request.finished {
        return;
    }

    let in_same_room = if let Some(ref g) = guardian {
        matches!(tp.place, teleport::Place::Room(room) if room == g.room)
    } else {
        false
    };
    if in_same_room && items.carried(crate::items::ItemKind::AnchorTorch) > 0 {
        item_intent.torch_action = true;
        info!("BOT_NAV: Bot dropped anchor torch to freeze guardian!");
    }

    let exit_room = runtime.live.host_match().competitive.exit_room();
    if matches!(tp.place, teleport::Place::Room(room) if Some(room) == exit_room) {
        info!("BOT_NAV: Exit room reached!");
        request.finished = true;
        intent.0 = PlayerIntent::default();
        return;
    }

    let here = Vec2::new(tp.body.position.x, tp.body.position.z);
    let local_feet_y = bot::local_feet_y(tp.body.position.y - tp.config.half_height, tp.place);
    let y_offset = crate::teleport::place_y_offset(tp.place);
    let primitives = crate::teleport::place_structural_primitives(
        &tp.geom,
        y_offset,
        crate::layout::WALL_HEIGHT,
    );
    let fell_off_deck =
        request.route_deck && !bot::at_deck_height(local_feet_y) && tp.body.grounded;
    if request.route_place != Some(tp.place)
        || request.waypoint >= request.route.len()
        || request.route.is_empty()
        || fell_off_deck
    {
        let Some(gap) = bot::target_gap_for_place(tp.place, &tp.geom, here, local_feet_y) else {
            let hm = runtime.live.host_match();
            info!(
                "BOT_NAV: No target gap available in place {:?}. local_room={:?}, local_target={:?}, gaps={:?}, rendered_routes={:?}",
                tp.place,
                hm.local_room(),
                hm.local_target(),
                tp.geom
                    .gaps
                    .iter()
                    .map(|g| (g.target, g.kind))
                    .collect::<Vec<_>>(),
                hm.rendered
                    .iter()
                    .map(|r| (r.rooms.0, r.rooms.1))
                    .collect::<Vec<_>>()
            );
            request.finished = runtime.live.host_match().local_target().is_none();
            intent.0 = PlayerIntent::default();
            return;
        };
        let start = Vec2::new(tp.body.position.x, tp.body.position.z);
        // On a Gantry hallway's deck, pilot the platform-centre jump line toward the upper
        // exit; if the body fell to the understory, recover down the clear bypass lane;
        // otherwise take the ordinary 2D route to whatever onward exit the feet-height
        // gate selected.
        let wellshaft_path = tp
            .geom
            .is_wellshaft()
            .then(|| bot::wellshaft_route(&tp.geom, &tp.config, local_feet_y, &gap))
            .flatten();
        let deck_pilot = (!tp.geom.is_wellshaft() && bot::at_deck_height(local_feet_y))
            .then(|| bot::gantry_deck_route(&tp.geom, start, &gap))
            .flatten();
        let in_gantry_understory = !tp.geom.is_wellshaft()
            && !tp.geom.decks.is_empty()
            && !bot::at_deck_height(local_feet_y);
        if let Some(path) = wellshaft_path {
            info!(
                "BOT_NAV: Stair-piloting {:?} from {:?} through the wellshaft. Waypoints: {}",
                tp.place,
                start,
                path.waypoints.len()
            );
            request.route_place = Some(tp.place);
            request.route_jumps = vec![false; path.waypoints.len()];
            request.route = path.waypoints;
            request.route_deck = false;
            request.waypoint = 0;
            request.blocked_ticks = 0;
        } else if let Some(pilot) = deck_pilot {
            info!(
                "BOT_NAV: Deck-piloting {:?} from {:?} toward upper exit (center: {:?}). Waypoints: {}",
                tp.place,
                start,
                gap.center,
                pilot.waypoints.len()
            );
            let (waypoints, jumps): (Vec<_>, Vec<_>) = pilot.waypoints.into_iter().unzip();
            request.route_place = Some(tp.place);
            request.route = waypoints;
            request.route_jumps = jumps;
            request.route_deck = true;
            request.waypoint = 0;
            request.blocked_ticks = 0;
        } else if in_gantry_understory {
            info!("BOT_NAV: Fell off the gantry deck — recovering to the bypass exit.");
            let path = bot::gantry_ground_recovery_route(&tp.config, start, &gap);
            request.route_place = Some(tp.place);
            request.route_jumps = vec![false; path.waypoints.len()];
            request.route = path.waypoints;
            request.route_deck = false;
            request.waypoint = 0;
            request.blocked_ticks = 0;
        } else if let Some(path) = bot::route_to_gap(&tp.geom, &primitives, &tp.config, start, &gap)
        {
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
            request.route_jumps = vec![false; request.route.len()];
            request.route_deck = false;
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
    let leg_needs_jump = request
        .route_jumps
        .get(request.waypoint)
        .copied()
        .unwrap_or(false);
    let jump_pressed = bot::gantry_jump_pressed_for_leg(
        &tp.geom,
        here,
        local_feet_y,
        tp.body.grounded,
        leg_needs_jump,
    );
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

    for prim in &primitives {
        // Only avoid solids overlapping the bot's vertical height range
        if cy - hy < prim.center.y + prim.half.y && cy + hy > prim.center.y - prim.half.y {
            let local_x = (here.x - prim.center.x) * prim.yaw.cos()
                + (here.y - prim.center.z) * prim.yaw.sin();
            let local_z = -(here.x - prim.center.x) * prim.yaw.sin()
                + (here.y - prim.center.z) * prim.yaw.cos();
            let closest_local_x = local_x.clamp(-prim.half.x, prim.half.x);
            let closest_local_z = local_z.clamp(-prim.half.z, prim.half.z);
            let closest_x =
                prim.center.x + closest_local_x * prim.yaw.cos() - closest_local_z * prim.yaw.sin();
            let closest_z =
                prim.center.z + closest_local_x * prim.yaw.sin() + closest_local_z * prim.yaw.cos();
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
    intent.0.jump_pressed = jump_pressed;

    // Per-tick coordinates are available at trace level; route/waypoint transitions stay
    // at info so long evidence runs remain reviewable and do not bury threshold faults.
    trace!(
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
