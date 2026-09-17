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
    "RETRACTION / the Architect makes the hole",
    "THE DOORWAY / and the Observer shoves a minor through it",
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
        // The panel, the warning, and the tile that stops existing.
        3 => {
            empty(&mut world);
            place_player(&mut world, site.panel - Vec3::new(1.8, 0., 0.), Vec3::X);
        }
        // The hole the Architect just made, used as a hole.
        _ => {
            empty(&mut world);
            if let Some(threshold) = site.thresholds.first().copied() {
                let (stand, launch) = ledge_stage(site, threshold);
                target = Some(world.spawn(Kind::Minor, launch + Vec3::Y * 0.6));
                aim_from(&mut world, stand, launch + Vec3::Y * 0.6 + AIM_LIFT);
                // Retract first: this scene is about what the tool can do once
                // the floor has a hole in it, not about making the hole.
                world.retract_warning = Some(1);
                world.step(Command::default());
            }
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
    let action = match (scene, tick) {
        (1, 90) | (2, 90) | (4, 90) => Action::Push,
        (3, 60) => Action::Interact,
        _ => Action::None,
    };
    Command { movement, action }
}
