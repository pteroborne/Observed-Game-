//! Staged initial snapshots followed exclusively by ordinary player commands.
//!
//! Every scene sets up a world and then plays it with the same `Command` the
//! keyboard produces. Nothing here reaches into the simulation mid-scene, so a
//! recording is a thing that happened rather than a thing that was drawn.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use kinetic_lab::physics::rv;
use observed_hex::{HexCoord, HexFace, face_edge, hex_origin};
use player_input::PlayerIntent;

use crate::model::{Action, ActorId, Command, Kind, Mode, WfcKineticWorld};
use crate::site::Site;

pub const SCENES: [&str; 5] = [
    "THE SOLVED FLOOR / seven cells the solver placed",
    "ARCHITECTURE / a wall the solver chose stops the ray",
    "THE PARAPET / every face onto void comes back railed",
    "DECOHERENCE / look away and the pocket re-collapses",
    "OBSERVATION / look at it and the floor holds",
];
pub const SCENE_TICKS: u32 = 240;

pub struct Demo {
    pub world: WfcKineticWorld,
    pub target: Option<ActorId>,
}

/// The outward direction of a face, in the cell's plan frame.
pub fn face_direction(face: HexFace) -> Vec3 {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    Vec3::new((a.0 + b.0) as f32 * 0.5, 0., (a.1 + b.1) as f32 * 0.5).normalize_or_zero()
}

/// Where to stand and what to stand a minor on, for a shove off `ledge`.
///
/// Both points come from the site's own standable waypoints, so neither is a
/// position this lab asserted the floor has.
pub fn ledge_stage(site: &Site, ledge: crate::site::Ledge) -> (Vec3, Vec3) {
    let outward = face_direction(ledge.face);
    let centre = Vec3::from_array(hex_origin(ledge.cell));
    let near_edge = |point: &Vec3| (*point - centre).dot(outward);
    // The standable point furthest toward the open side, and a place to stand
    // behind it that is still on the same floor.
    let launch = site
        .nav
        .iter()
        .copied()
        .filter(|point| point.with_y(centre.y).distance(centre) < 7.5)
        .max_by(|a, b| near_edge(a).total_cmp(&near_edge(b)))
        .unwrap_or(centre);
    // Stand well back. The shove follows the look direction, so a short range
    // onto a body at floor level is a downward shove: it drives the minor into
    // the deck a metre from where it started instead of out over the edge.
    let stand = site
        .nav
        .iter()
        .copied()
        .filter(|point| (point.y - launch.y).abs() < 0.5)
        .filter(|point| point.distance(launch) > 4.5 && point.distance(launch) < 7.5)
        .min_by(|a, b| near_edge(a).total_cmp(&near_edge(b)))
        .unwrap_or(launch - outward * 5.5);
    (stand, launch)
}

/// Where on a minor to aim so the shove carries it out rather than down: its
/// upper body, which the ray still selects and which flattens the impulse.
pub const AIM_LIFT: Vec3 = Vec3::new(0.0, 0.45, 0.0);

/// Stand at `feet` looking at `at`.
///
/// The body is placed before the look is computed, because the eye is not the
/// body's centre: guessing it as `feet + half_height` aims low by the eye
/// offset, which at close range puts the ray into the floor instead of the
/// target.
pub fn aim_from(world: &mut WfcKineticWorld, feet: Vec3, at: Vec3) {
    place_player(world, feet, Vec3::X);
    let eye = world.player.eye(&world.player_config);
    place_player(world, feet, at - eye);
}

fn place_player(world: &mut WfcKineticWorld, feet: Vec3, looking: Vec3) {
    world.player.position = feet + Vec3::Y * world.player_config.half_height;
    world.player.spawn = world.player.position;
    world.player.velocity = Vec3::ZERO;
    let direction = looking.normalize_or_zero();
    world.player.yaw = direction.x.atan2(-direction.z);
    world.player.pitch = direction.y.asin().clamp(-1.2, 1.2);
    world.physics.bodies[world.player_handle].set_translation(rv(world.player.position), true);
    world.physics.bodies[world.player_handle]
        .set_next_kinematic_translation(rv(world.player.position));
}

/// Clear the world's starting cast so each scene shows one thing.
fn empty(world: &mut WfcKineticWorld) {
    let ids: Vec<ActorId> = world.actors.keys().copied().collect();
    for id in ids {
        let actor = world.actors.get_mut(&id).expect("listed actor");
        actor.alive = false;
        world.physics.bodies[actor.body].set_enabled(false);
        world.physics.colliders[actor.collider].set_enabled(false);
    }
}

/// A face of an occupied cell whose neighbour is not built: sealed, on this
/// corpus, by the tile's own parapet.
fn sealed_face(site: &Site) -> Option<(HexCoord, HexFace)> {
    let grid = crate::site::config().grid();
    site.cells.iter().copied().find_map(|cell| {
        HexFace::LATERAL.into_iter().find_map(|face| {
            let outside = grid
                .neighbor(cell, face)
                .is_none_or(|next| !site.cells.contains(&next));
            outside.then_some((cell, face))
        })
    })
}

/// Two built cells with a wall rather than a door between them.
fn walled_pair(site: &Site) -> Option<(HexCoord, HexFace)> {
    let grid = crate::site::config().grid();
    site.cells.iter().copied().find_map(|cell| {
        let placement = site.placement(cell)?;
        HexFace::LATERAL.into_iter().find_map(|face| {
            let neighbour = grid.neighbor(cell, face)?;
            (site.cells.contains(&neighbour) && !placement.is_open(face)).then_some((cell, face))
        })
    })
}

pub fn stage(index: usize, site: &Arc<Site>) -> Demo {
    let mut world = WfcKineticWorld::new(Arc::clone(site), Mode::Practice);
    let mut target = None;
    match index {
        // A look across the floor from where an Observer stands up.
        0 => {
            place_player(&mut world, site.spawn, site.spawn_facing);
        }
        // A minor a few metres away with a wall the solver chose between them.
        // The tool refuses: it never reaches through architecture.
        1 => {
            empty(&mut world);
            let grid = crate::site::config().grid();
            if let Some((cell, face)) = walled_pair(site)
                && let Some(neighbour) = grid.neighbor(cell, face)
            {
                let outward = face_direction(face);
                // One standable point each side of the shared face, so the wall
                // is genuinely between the eye and the target rather than
                // somewhere behind both of them.
                let here = Vec3::from_array(hex_origin(cell)) + outward * 4.5;
                let there = Vec3::from_array(hex_origin(neighbour)) - outward * 4.5;
                if let (Some(stand), Some(landing)) = (
                    standable_near(site, here, 3.0),
                    standable_near(site, there, 3.0),
                ) {
                    target = Some(world.spawn(Kind::Minor, landing + Vec3::Y * 0.6));
                    aim_from(&mut world, stand, landing + Vec3::Y * 0.6);
                }
            }
        }
        // A shove straight at the edge of the world, which does not work: the
        // authored tile rails its own open side.
        2 => {
            empty(&mut world);
            if let Some((cell, face)) = sealed_face(site) {
                let ledge = crate::site::Ledge {
                    cell,
                    face,
                    over_hole: false,
                };
                let (stand, launch) = ledge_stage(site, ledge);
                target = Some(world.spawn(Kind::Minor, launch + Vec3::Y * 0.6));
                aim_from(&mut world, stand, launch + Vec3::Y * 0.6 + AIM_LIFT);
            }
        }
        // The panel, the warning, and a pocket that re-collapses because
        // nobody was looking at it.
        //
        // Scene four is the same setup with the Observer facing the pocket
        // instead, and the floor survives. Between them they are the whole
        // mechanic: observation is what makes a rewrite illegal.
        _ => {
            empty(&mut world);
            place_player(&mut world, site.panel - Vec3::new(1.8, 0., 0.), Vec3::X);
        }
    }
    world.physics.step();
    Demo { world, target }
}

/// A standable waypoint within `radius` of a plan position.
fn standable_near(site: &Site, at: Vec3, radius: f32) -> Option<Vec3> {
    site.nav
        .iter()
        .copied()
        .filter(|point| point.with_y(at.y).distance(at.with_y(at.y)) < radius)
        .min_by(|a, b| {
            a.with_y(at.y)
                .distance_squared(at.with_y(at.y))
                .total_cmp(&b.with_y(at.y).distance_squared(at.with_y(at.y)))
        })
}

pub fn command(
    scene: usize,
    tick: u32,
    world: &WfcKineticWorld,
    target: Option<ActorId>,
) -> Command {
    let mut movement = PlayerIntent::default();
    // Keep the crosshair on the staged body without teleporting the view: the
    // correction is fed in as an ordinary mouse delta, scaled by the same
    // `look_step` the controller applies, so one unit of look is one step of
    // yaw. Dividing by anything else overshoots and the aim never settles —
    // which is what the first recording did, and why every shove was refused.
    if let Some(id) = target
        && world.actors.get(&id).is_some_and(|actor| actor.alive)
    {
        const LOOK_STEP: f32 = 0.035;
        let to_target = (world.pose(id).position - world.eye()).normalize_or_zero();
        let wanted_yaw = to_target.x.atan2(-to_target.z);
        let wanted_pitch = to_target.y.asin();
        let yaw_error = (wanted_yaw - world.player.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        movement.look = Vec2::new(
            (yaw_error / LOOK_STEP).clamp(-1., 1.),
            ((world.player.pitch - wanted_pitch) / LOOK_STEP).clamp(-1., 1.),
        );
    }
    // A slow walk in the opening scene, so the floor reads as a place.
    if scene == 0 && (40..200).contains(&tick) {
        movement.movement = Vec2::new(0., 0.55);
    }
    // Three looks away from what it just telegraphed and loses the pocket;
    // four stares at it and keeps the floor.
    if let (3 | 4, Some((_, candidate))) = (scene, world.telegraph.as_ref())
        && let Some(cell) = candidate.region.cells.iter().next()
    {
        let at = Vec3::from_array(hex_origin(*cell)) + Vec3::Y * 1.2;
        let toward = (at - world.eye()).with_y(0.).normalize_or_zero();
        let wanted = if scene == 4 { toward } else { -toward };
        movement.look = turn_toward(world, world.eye() + wanted * 10.);
    }
    let action = match (scene, tick) {
        (1, 90) | (2, 90) => Action::Push,
        (3 | 4, 60) => Action::Interact,
        _ => Action::None,
    };
    Command { movement, action }
}

/// How long a recorded gameplay loop runs, in fixed ticks.
pub const LOOP_TICKS: u32 = 3_600;

/// A deterministic Observer playing the encounter, for recorded evidence.
///
/// It is a fixed priority policy rather than a scripted sequence: every tick it
/// reads the world and chooses, so the recording is a thing that happened. It
/// emits the same `Command` a keyboard does and reaches into nothing.
///
/// The order of its priorities is the loop this floor actually has. A solved
/// floor seals every edge, so until a tile is retracted there is no way to
/// remove a minor at all — the tool only staggers them and the wave never
/// clears. So: make the hole first, then work minors into it, and top the tool
/// up when it runs dry.
pub fn loop_command(world: &WfcKineticWorld, tick: u32) -> Command {
    let feet = world.player.position - Vec3::Y * world.player_config.half_height;
    let eye = world.eye();

    // 1. Make a hole a body can actually be put through. The floor is dealt
    //    with two of its own, but the corpus seals them: without a doorway
    //    onto one, a hole is scenery.
    let thresholds = world.open_thresholds();
    if thresholds.is_empty() {
        if world.telegraph.is_some()
            && let Some((_, candidate)) = world.telegraph.as_ref()
        {
            // Look away from what was just telegraphed. Watching it is what
            // saves it, and the director is trying to lose this pocket.
            let cell = candidate.region.cells.iter().next().copied();
            let at = cell.map_or(feet + Vec3::X, |cell| {
                Vec3::from_array(hex_origin(cell)) + Vec3::Y * 1.2
            });
            let away = (feet - at).with_y(0.).normalize_or_zero();
            return go(world, feet, feet + away * 10.0 + Vec3::Y * 1.5);
        }
        // Retraction, not decoherence. A legal relayout can never point a door
        // at void, so rewriting the floor forever would never produce a hole a
        // body can be put through; retracting a tile is the verb that does.
        let control = world.site.demolition;
        let mut command = go(world, control, control + Vec3::Y * 1.2);
        if world.interaction() == Some(crate::model::Device::Demolition) {
            command.movement.movement = Vec2::ZERO;
            command.action = Action::Interact;
        }
        return command;
    }

    // 2. Keep the tool fed. Below a third, walking to the station beats
    //    standing next to a minor you cannot move.
    let station = world.site.station;
    let charging = world.charge < world.config.cost * 3.0;
    if charging && feet.distance(station) > 1.8 {
        return go(world, station, station + Vec3::Y * 1.2);
    }

    // 3. Hold the hole.
    //
    //    On a thirty-cell floor there is one hole and the minors arrive from
    //    everywhere, so chasing them is a losing errand: the director used to
    //    walk out to whichever minor was nearest and get caught crossing the
    //    floor without ever having somewhere to put it. Minors pursue, so the
    //    Observer does not have to. Stand by the hole and let them come.
    let Some(hole) = nearest_hole(world, feet) else {
        return go(
            world,
            world.site.station,
            world.site.station + Vec3::Y * 1.2,
        );
    };
    let Some((id, position)) = nearest_minor(world, feet) else {
        // Between waves: take the post and watch the approach.
        return go(world, post_beside(world, hole, None), hole + Vec3::Y * 1.5);
    };

    // Out of range of the hole, the shot is not worth taking at any angle: a
    // shove carries a body a few metres, not across a floor. Wait at the post
    // and keep the minor in view while it closes.
    let approach = position.with_y(hole.y).distance(hole);
    if approach > ENGAGE_RANGE {
        let post = post_beside(world, hole, Some(position));
        return go(world, post, position + AIM_LIFT);
    }

    // In range. Which verb depends on which side of the hole the minor is,
    // and that is not a detail — it is the thing the floor decides for you.
    //
    // A pursuing minor walks at the Observer and *stops at the hole*, because
    // the navigation graph has no edges across one. So it arrives on the far
    // side, facing across, and a push sends it further away. Pull drags it
    // toward the eye, which is over the hole. The director spent fifty seconds
    // lining up shoves that were all pointing the wrong way before this.
    let to_target = (position - eye).with_y(0.).normalize_or_zero();
    let to_hole = (hole - position).with_y(0.).normalize_or_zero();
    let alignment = to_target.dot(to_hole);
    let post = if alignment < 0. {
        // Hole between us: hold position and pull it across.
        post_beside(world, hole, Some(position))
    } else {
        // Minor between us and the hole: get behind it and push.
        snap_post(
            world,
            position + (position - hole).with_y(0.).normalize_or_zero() * 4.6,
            hole,
        )
    };
    let mut command = go(world, post, position + AIM_LIFT);

    let verb = if alignment > 0.82 {
        Some(Action::Push)
    } else if alignment < -0.82 {
        Some(Action::Pull)
    } else {
        None
    };
    if let Some(verb) = verb
        && world
            .fire_ready()
            .is_ok_and(|target| target.id == id && target.distance > 2.2)
    {
        command.action = verb;
        command.movement.movement = Vec2::ZERO;
    }
    // A slow sidestep while closing keeps the camera from locking rigid.
    if tick % 240 < 120 {
        command.movement.movement.x += 0.25;
    }
    command
}

/// How near a hole a minor has to be before a shove is worth spending.
const ENGAGE_RANGE: f32 = 10.0;

/// Somewhere to stand beside the hole, preferring the side away from whoever
/// is coming, so an arriving minor ends up between the Observer and the drop.
fn post_beside(world: &WfcKineticWorld, hole: Vec3, minor: Option<Vec3>) -> Vec3 {
    let away = minor
        .map(|at| (hole - at).with_y(0.).normalize_or_zero())
        .filter(|direction| direction.length_squared() > 0.01)
        .unwrap_or(Vec3::X);
    snap_post(world, hole + away * 4.5, hole)
}

/// The nearest place to `ideal` that is standable now, clear of the hole, and
/// somewhere the Observer can actually walk to.
///
/// Every post goes through here. A post computed as pure geometry is how the
/// director walked into its own demolition twice: once aiming at a waypoint
/// left hanging over the retracted tile, and once at a point derived from a
/// minor's position that happened to sit over the same hole.
fn snap_post(world: &WfcKineticWorld, ideal: Vec3, hole: Vec3) -> Vec3 {
    let feet = world.player.position - Vec3::Y * world.player_config.half_height;
    let mut candidates: Vec<Vec3> = world
        .site
        .nav
        .iter()
        .copied()
        // Standable *now*: the waypoints inside a retracted tile are still in
        // the graph, hanging over the hole. Walking to one is how the director
        // first killed itself with its own demolition.
        .filter(|point| world.standable(*point))
        .filter(|point| point.with_y(hole.y).distance(hole) > 2.5)
        .collect();
    candidates.sort_by(|a, b| {
        a.distance_squared(ideal)
            .total_cmp(&b.distance_squared(ideal))
    });
    // And reachable. The retracted tile is chosen as the floor's cut vertex, so
    // removing it severs the far side: the best post by distance is routinely
    // one the Observer can no longer walk to, and aiming at it just stops them
    // dead eleven metres from the fight.
    candidates
        .into_iter()
        .take(POST_CANDIDATES)
        .find(|point| world.navigable(feet, *point))
        .unwrap_or(ideal)
}

/// How many posts to test for reachability before giving up.
const POST_CANDIDATES: usize = 12;

/// The centre of the hole the floor currently offers, preferring one with a
/// doorway pointing into it — a body can only be put through an opening.
fn nearest_hole(world: &WfcKineticWorld, feet: Vec3) -> Option<Vec3> {
    let grid = crate::site::config().grid();
    world
        .open_thresholds()
        .into_iter()
        .filter_map(|(cell, face)| grid.neighbor(cell, face))
        .map(|hole| Vec3::from_array(hex_origin(hole)))
        .min_by(|a, b| {
            feet.distance_squared(*a)
                .total_cmp(&feet.distance_squared(*b))
        })
}

/// A look delta that turns toward a world point, as a mouse movement.
fn turn_toward(world: &WfcKineticWorld, at: Vec3) -> Vec2 {
    const LOOK_STEP: f32 = 0.035;
    let to_target = (at - world.eye()).normalize_or_zero();
    let wanted_yaw = to_target.x.atan2(-to_target.z);
    let wanted_pitch = to_target.y.asin();
    let yaw_error = (wanted_yaw - world.player.yaw + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    Vec2::new(
        (yaw_error / LOOK_STEP).clamp(-1., 1.),
        ((world.player.pitch - wanted_pitch) / LOOK_STEP).clamp(-1., 1.),
    )
}

/// The nearest live minor, if the floor has one.
fn nearest_minor(world: &WfcKineticWorld, feet: Vec3) -> Option<(ActorId, Vec3)> {
    world
        .actors
        .values()
        .filter(|actor| actor.alive && actor.kind == Kind::Minor)
        .map(|actor| (actor.id, world.pose(actor.id).position))
        .min_by(|a, b| {
            feet.distance_squared(a.1)
                .total_cmp(&feet.distance_squared(b.1))
        })
}

/// Walk toward `destination` while looking at `at`, as ordinary intent.
///
/// The walk is routed through the navigation graph. Walking straight at a
/// destination is how the first director spent its whole recording pressed
/// against a wall the solver had put between it and the panel.
fn go(world: &WfcKineticWorld, destination: Vec3, at: Vec3) -> Command {
    const LOOK_STEP: f32 = 0.035;
    let feet = world.player.position - Vec3::Y * world.player_config.half_height;
    let mut movement = PlayerIntent::default();

    let to_target = (at - world.eye()).normalize_or_zero();
    let wanted_yaw = to_target.x.atan2(-to_target.z);
    let wanted_pitch = to_target.y.asin();
    let yaw_error = (wanted_yaw - world.player.yaw + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    movement.look = Vec2::new(
        (yaw_error / LOOK_STEP).clamp(-1., 1.),
        ((world.player.pitch - wanted_pitch) / LOOK_STEP).clamp(-1., 1.),
    );

    // Movement is in the body's own frame, so the walk is expressed the way a
    // keyboard expresses it rather than as a world-space nudge.
    let offset = (destination - feet).with_y(0.);
    if offset.length() > 0.9 {
        // The graph gets you to the right cell; it does not get you to a device
        // standing in the middle of one. Once the route runs out — which it does
        // as soon as the nearest waypoint *is* the destination's — walk the last
        // few metres straight, or the director stalls two metres short of the
        // panel with nothing left to do.
        let routed = world.direction_toward(feet, destination);
        let direction = if routed.length_squared() > 0.01 {
            routed
        } else {
            // The last few metres may be walked without a route — the graph
            // gets you to the cell, not to a device in the middle of one — but
            // only over ground that is actually there. Walking blind at a post
            // with a hole in between is how the director kept stepping into
            // its own demolition.
            let straight = offset.normalize_or_zero();
            let clear = offset.length() < 6.0
                && world.standable(feet + straight * 1.5)
                && world.standable(feet + straight * 3.0);
            if clear { straight } else { Vec3::ZERO }
        };
        movement.movement = Vec2::new(
            direction.dot(world.player.right()),
            direction.dot(world.player.forward()),
        )
        .clamp_length_max(1.0);
    }
    Command {
        movement,
        action: Action::None,
    }
}
