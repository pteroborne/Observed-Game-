//! Staged initial snapshots followed exclusively by ordinary player commands.
use crate::{
    model::{Action, ActorId, Command, Kind, KineticWorld, Mode},
    physics::rv,
};
use glam::{Vec2, Vec3};
use player_input::PlayerIntent;
pub const SCENES: [&str; 5] = [
    "PROP CONTACT / walls and crates do not kill",
    "PULL / immediate impulse toward the player",
    "RECOVERY / lower geometry catches a fall",
    "VOID / the missing floor eliminates",
    "RETRACTION / warning before support disappears",
];
pub const SCENE_TICKS: u32 = 240;
pub struct Demo {
    pub world: KineticWorld,
    pub target: Option<ActorId>,
}
pub fn stage(index: usize) -> Demo {
    let mut world = KineticWorld::new(Mode::Practice);
    for actor in world.actors.values_mut() {
        actor.alive = false;
        world.physics.bodies[actor.body].set_enabled(false);
        world.physics.colliders[actor.collider].set_enabled(false);
    }
    let feet = match index {
        2 => Vec3::new(-1., 3., -5.),
        3 => Vec3::new(-4., 0., 10.),
        4 => Vec3::new(-0.8, 0., 8.4),
        _ => Vec3::new(-11., 0., 7.),
    };
    world.player.position = feet + Vec3::Y * world.player_config.half_height;
    world.player.spawn = world.player.position;
    world.player.velocity = Vec3::ZERO;
    world.player.pitch = -0.35;
    world.physics.bodies[world.player_handle].set_translation(rv(world.player.position), true);
    world.physics.bodies[world.player_handle]
        .set_next_kinematic_translation(rv(world.player.position));
    let target = match index {
        0 => {
            world.spawn(Kind::Minor, Vec3::new(-11., 0.6, 0.));
            Some(world.spawn(Kind::Prop, Vec3::new(-11., 0.5, 2.)))
        }
        1 => Some(world.spawn(Kind::Minor, Vec3::new(-11., 0.6, 3.))),
        2 => None,
        3 => Some(world.spawn(Kind::Minor, Vec3::new(0.3, 0.6, 10.))),
        _ => Some(world.spawn(Kind::Minor, Vec3::new(3., 0.6, 6.5))),
    };
    world.physics.step();
    Demo { world, target }
}
pub fn command(index: usize, tick: u32, world: &KineticWorld, target: Option<ActorId>) -> Command {
    let mut movement = PlayerIntent::default();
    if let Some(id) = target {
        let direction = (world.pose(id).position - world.eye()).normalize_or_zero();
        let yaw = direction.x.atan2(-direction.z);
        let difference = (yaw - world.player.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        movement.look = Vec2::new(difference, -(direction.y.asin() - world.player.pitch))
            / world.player_config.look_step;
    }
    if index == 2 && tick < 42 {
        movement.movement.x = 1.;
    }
    let action = if tick == 30 {
        match index {
            0 | 3 => Action::Push,
            1 => Action::Pull,
            4 => Action::Interact,
            _ => Action::None,
        }
    } else {
        Action::None
    };
    Command { movement, action }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Event;
    #[test]
    fn walkthrough_actions_produce_the_advertised_physical_results() {
        for index in 0..SCENES.len() {
            let Demo { mut world, target } = stage(index);
            let initial = target.map(|id| world.pose(id).position);
            let mut fired = false;
            let mut retracted = false;
            for tick in 0..SCENE_TICKS {
                let input = command(index, tick, &world, target);
                world.step(input);
                fired |= world.events.iter().any(|e| matches!(e, Event::Fired(..)));
                retracted |= world.events.contains(&Event::Retracted);
            }
            match index {
                0 => {
                    assert!(fired);
                    assert_eq!(world.kills, 0);
                    assert!(
                        world
                            .pose(target.unwrap())
                            .position
                            .distance(initial.unwrap())
                            > 1.
                    );
                }
                1 => {
                    assert!(fired);
                    assert!(world.pose(target.unwrap()).position.z > initial.unwrap().z + 1.);
                }
                2 => {
                    assert!(world.player.grounded);
                    assert!(
                        world.player.position.y < -1.5,
                        "{:?}",
                        world.player.position
                    );
                }
                3 => {
                    assert!(fired);
                    assert_eq!(
                        world.kills,
                        1,
                        "scene {index}, {:?}",
                        world.pose(target.unwrap())
                    );
                }
                _ => {
                    assert!(retracted);
                    assert_eq!(
                        world.kills,
                        1,
                        "scene {index}, {:?}",
                        world.pose(target.unwrap())
                    );
                }
            }
        }
    }
}
