use super::*;

fn aim(world: &mut KineticWorld, point: Vec3) {
    let d = (point - world.eye()).normalize();
    world.player.yaw = d.x.atan2(-d.z);
    world.player.pitch = d.y.asin();
}
fn settle(world: &mut KineticWorld) {
    for _ in 0..60 {
        world.step(Command::default());
    }
}
fn isolate(world: &mut KineticWorld) {
    for a in world.actors.values_mut() {
        a.alive = false;
        world.physics.bodies[a.body].set_enabled(false);
        world.physics.colliders[a.collider].set_enabled(false);
    }
}
#[test]
fn crosshair_uses_full_pitch_and_first_hit() {
    let mut w = KineticWorld::new(Mode::Practice);
    settle(&mut w);
    isolate(&mut w);
    let id = w.spawn(Kind::Minor, Vec3::new(-11., 2., 2.));
    w.physics.step();
    let point = w.pose(id).position;
    aim(&mut w, point);
    assert_eq!(w.target().unwrap().id, id);
    let nearer = w.spawn(Kind::Prop, w.eye().lerp(w.pose(id).position, 0.5));
    w.physics.step();
    assert_eq!(w.target().unwrap().id, nearer);
    w.player.yaw += 0.5;
    assert!(w.target().is_err());
}
#[test]
fn range_and_walls_refuse_without_charge_or_impulse() {
    let mut w = KineticWorld::new(Mode::Encounter);
    isolate(&mut w);
    let id = w.spawn(Kind::Minor, Vec3::new(-11., 0.6, -3.));
    w.physics.step();
    let point = w.pose(id).position;
    aim(&mut w, point);
    assert_eq!(w.target(), Err(Refusal::TooFar));
    let before = w.charge;
    w.fire(Action::Push);
    assert_eq!(w.charge, before);
    w.physics.bodies[w.actors[&id].body].set_translation(rv(Vec3::new(-5., 1., -8.)), true);
    w.physics.step();
    let point = w.pose(id).position;
    aim(&mut w, point);
    assert_eq!(w.target(), Err(Refusal::Blocked));
}
#[test]
fn impulse_charge_cooldown_and_pull_are_authoritative() {
    let mut w = KineticWorld::new(Mode::Encounter);
    isolate(&mut w);
    let id = w.spawn(Kind::Minor, Vec3::new(-11., 0.6, 2.));
    w.physics.step();
    let point = w.pose(id).position;
    aim(&mut w, point);
    w.fire(Action::Push);
    assert_eq!(w.charge, 90.);
    assert!(w.velocity(id).dot(w.player.look_dir()) > 9.);
    let velocity = w.velocity(id);
    w.fire(Action::Push);
    assert_eq!(w.charge, 90.);
    assert_eq!(w.velocity(id), velocity);
    w.cooldown = 0;
    w.charge = 0.;
    w.fire(Action::Pull);
    assert_eq!(w.events.last(), Some(&Event::Refused(Refusal::EmptyCharge)));
    w.charge = 100.;
    w.physics.bodies[w.actors[&id].body].set_linvel(Vector::ZERO, true);
    w.fire(Action::Pull);
    assert!(
        w.velocity(id)
            .dot((w.eye() - w.pose(id).position).normalize())
            > 6.9
    );
}
#[test]
fn snapshot_continuation_and_independent_runs_match_every_tick() {
    let mut a = KineticWorld::new(Mode::Practice);
    let mut b = KineticWorld::new(Mode::Practice);
    for t in 0..600 {
        let command = Command {
            action: if t == 30 { Action::Push } else { Action::None },
            movement: PlayerIntent {
                look: glam::Vec2::new(if t < 15 { 0.02 } else { 0. }, 0.),
                ..Default::default()
            },
        };
        a.step(command);
        b.step(command);
        assert_eq!(a.digest(), b.digest(), "tick {t}");
        if t == 35 {
            b = a.clone();
        }
    }
    b.config.push += 1.;
    assert_ne!(a.digest(), b.digest());
}
#[test]
fn high_speed_impacts_stop_without_killing_or_tunneling() {
    let mut w = KineticWorld::new(Mode::Practice);
    isolate(&mut w);
    let id = w.spawn(Kind::Minor, Vec3::new(-13., 0.6, 0.));
    w.physics.step();
    w.physics.bodies[w.actors[&id].body].set_linvel(Vector::new(-150., 0., 0.), true);
    w.actors.get_mut(&id).unwrap().stagger = 100;
    for _ in 0..90 {
        w.step(Command::default());
    }
    assert!(w.actors[&id].alive);
    assert!(w.pose(id).position.x > -15.);
    assert_eq!(w.kills, 0);
}
#[test]
fn props_transfer_momentum_but_never_deal_damage() {
    let mut w = KineticWorld::new(Mode::Practice);
    isolate(&mut w);
    let minor = w.spawn(Kind::Minor, Vec3::new(-11., 0.6, 0.));
    let prop = w.spawn(Kind::Prop, Vec3::new(-11., 0.5, 2.));
    w.physics.step();
    w.physics.bodies[w.actors[&prop].body].set_linvel(Vector::new(0., 0., -12.), true);
    let mut moved = false;
    for _ in 0..60 {
        w.step(Command::default());
        moved |= w.pose(minor).position.z < -0.5;
    }
    assert!(moved);
    assert!(w.actors[&minor].alive);
    assert_eq!(w.kills, 0);
}
#[test]
fn recoverable_fall_survives_but_true_void_eliminates() {
    let mut w = KineticWorld::new(Mode::Practice);
    isolate(&mut w);
    let safe = w.spawn(Kind::Minor, Vec3::new(2., 4., -5.));
    let lost = w.spawn(Kind::Minor, Vec3::new(7., 2., 0.));
    for _ in 0..150 {
        w.step(Command::default());
    }
    assert!(w.actors[&safe].alive);
    assert!(w.pose(safe).position.y > -3.);
    assert!(!w.actors[&lost].alive);
    assert_eq!(w.kills, 1);
}
#[test]
fn retracting_bridge_warns_then_removes_real_support_and_navigation() {
    let mut w = KineticWorld::new(Mode::Practice);
    isolate(&mut w);
    let id = w.spawn(Kind::Minor, Vec3::new(3., 0.6, 6.5));
    w.player.position = arena::PANEL + Vec3::Y * w.player_config.half_height;
    w.interact();
    assert_eq!(w.bridge_warning, Some(120));
    for _ in 0..119 {
        w.step(Command::default());
    }
    assert!(w.bridge_present);
    w.step(Command::default());
    assert!(!w.bridge_present);
    assert!(!w.supported(Vec3::new(3., 0., 6.5)));
    for _ in 0..150 {
        w.step(Command::default());
    }
    assert!(!w.actors[&id].alive);
}
#[test]
fn station_requires_power_and_practice_is_unlimited() {
    let mut w = KineticWorld::new(Mode::Encounter);
    w.player.position = arena::STATION + Vec3::Y * w.player_config.half_height;
    w.charge = 50.;
    for _ in 0..60 {
        w.step(Command::default());
    }
    assert!((w.charge - 75.).abs() < 0.01);
    w.powered = false;
    let before = w.charge;
    for _ in 0..60 {
        w.step(Command::default());
    }
    assert_eq!(w.charge, before);
    let mut practice = KineticWorld::new(Mode::Practice);
    settle(&mut practice);
    let id = ActorId(1000);
    let point = practice.pose(id).position;
    aim(&mut practice, point);
    practice.fire(Action::Push);
    assert_eq!(practice.charge, 100.);
}
#[test]
fn navigation_routes_around_partition_and_rejects_void() {
    let w = KineticWorld::new(Mode::Encounter);
    assert!(!w.walkable(Vec3::new(-11., 0., -9.), Vec3::new(-5., 0., -9.)));
    assert!(!w.walkable(Vec3::new(0., 0., 0.), Vec3::new(9., 0., 0.)));
    assert!(w.route_direction(Vec3::new(-12., 0., -9.)).length() > 0.9);
}
#[test]
fn waves_capture_and_terminal_freeze() {
    let mut w = KineticWorld::new(Mode::Encounter);
    w.wave_delay = 0;
    w.step(Command::default());
    assert_eq!(w.wave, 1);
    assert_eq!(
        w.actors
            .values()
            .filter(|a| a.alive && a.kind == Kind::Minor)
            .count(),
        2
    );
    for wave in 2..=3 {
        isolate(&mut w);
        w.wave_delay = 0;
        w.step(Command::default());
        assert_eq!(w.wave, wave);
    }
    isolate(&mut w);
    w.step(Command::default());
    assert_eq!(w.outcome, Outcome::Cleared);
    let digest = w.digest();
    w.step(Command {
        action: Action::Push,
        ..Default::default()
    });
    assert_eq!(digest, w.digest());
    let mut caught = KineticWorld::new(Mode::Encounter);
    let p = caught.player.position + Vec3::new(0., 0., -0.8);
    caught.spawn(Kind::Minor, p);
    for _ in 0..120 {
        caught.step(Command::default());
    }
    assert_eq!(caught.outcome, Outcome::Captured);
}

#[test]
fn pursuing_minors_reach_the_observer_around_corners_and_up_stairs() {
    for (spawn, player) in [
        (Vec3::new(-12., 0.6, -9.), Vec3::new(-11., 0., 7.)),
        (Vec3::new(-10., 0.6, 4.), Vec3::new(-2., 3., 2.)),
        (Vec3::new(2., -2.4, -5.), Vec3::new(-2., 0., 7.)),
    ] {
        let mut w = KineticWorld::new(Mode::Encounter);
        isolate(&mut w);
        w.player.position = player + Vec3::Y * w.player_config.half_height;
        let id = w.spawn(Kind::Minor, spawn);
        for _ in 0..1800 {
            w.step(Command::default());
            if w.outcome != Outcome::Playing {
                break;
            }
        }
        assert_eq!(
            w.outcome,
            Outcome::Captured,
            "spawn {spawn:?}, minor {:?}, behavior {:?}, direction {:?}, velocity {:?}",
            w.pose(id),
            w.actors[&id].behavior,
            w.route_direction(w.pose(id).position - Vec3::Y * 0.55),
            w.velocity(id)
        );
    }
}
#[test]
fn bridge_removal_drops_the_player_and_configuration_changes_replay_identity() {
    let mut w = KineticWorld::new(Mode::Encounter);
    w.player.position = Vec3::new(3., w.player_config.half_height, 6.5);
    w.bridge_warning = Some(30);
    for _ in 0..180 {
        w.step(Command::default());
    }
    assert_eq!(w.outcome, Outcome::Fell);
    let before = w.digest();
    w.player_config.gravity += 1.;
    assert_ne!(before, w.digest());
}
