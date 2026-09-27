//! The kinetic tool in a physical match: what the crosshair selects, what a push and a
//! pull do to a minor, the cooldown, and that a shove steps identically on every peer.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::hex_wfc::HexWfcConfig;

use super::*;
use crate::hex_wfc::model::{
    HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig, HexPlayerCommand, HexReleasedKind,
};

const BODY: PlayerId = PlayerId(0);
const MINOR: u16 = 1;

fn game() -> HexWfcMatch {
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let mut game = HexWfcMatch::new_with_content(
        7,
        config,
        crate::hex_wfc::compatibility_test_content().clone(),
    )
    .expect("a two-level facility solves");
    game.send_catches_to_prison();
    game
}

/// One tick, the body standing still and pressing `actions`.
fn step(game: &mut HexWfcMatch, actions: HexActionButtons) -> Vec<HexMatchEventKind> {
    let frame = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick: game.tick + 1,
        commands: BTreeMap::from([(
            BODY,
            HexPlayerCommand {
                actions,
                ..HexPlayerCommand::default()
            },
        )]),
    };
    game.step(&frame).iter().map(|event| event.kind).collect()
}

const PUSH: HexActionButtons = HexActionButtons {
    interact: false,
    deploy_lantern: false,
    recover_lantern: false,
    deploy_pad: false,
    kinetic_push: true,
    kinetic_pull: false,
};

const PULL: HexActionButtons = HexActionButtons {
    kinetic_push: false,
    kinetic_pull: true,
    ..PUSH
};

const IDLE: HexActionButtons = HexActionButtons {
    kinetic_push: false,
    ..PUSH
};

/// Level directions from the body, round the compass.
fn compass() -> impl Iterator<Item = Vec3> {
    (0..48).map(|step| {
        let angle = step as f32 * std::f32::consts::TAU / 48.0;
        Vec3::new(angle.sin(), 0.0, -angle.cos())
    })
}

/// Whether nothing solid stands along `direction` from the body for `clear` metres, at eye
/// height and at the body's own.
fn clear_along(game: &HexWfcMatch, direction: Vec3, clear: f32) -> bool {
    let (eye, _) = game.eye_and_look(BODY).expect("a body");
    let centre = game.body_position_for_tests(BODY);
    game.physics.ray_distance(eye, direction, clear).is_none()
        && game
            .physics
            .ray_distance(centre, direction, clear)
            .is_none()
}

/// Whether there is floor under the body's line along `direction` at `distance`.
fn floored_at(game: &HexWfcMatch, direction: Vec3, distance: f32) -> bool {
    let centre = game.body_position_for_tests(BODY);
    game.physics
        .ray_distance(centre + direction * distance, Vec3::NEG_Y, 2.5)
        .is_some()
}

/// A level direction from the body that is clear and floored for `clear` metres.
fn open_direction(game: &HexWfcMatch, clear: f32) -> Vec3 {
    compass()
        .find(|&direction| {
            clear_along(game, direction, clear)
                && (1..=clear as u32).all(|metre| floored_at(game, direction, metre as f32))
        })
        .expect("an open, floored line from the spawn")
}

/// A minor standing `distance` from the body along `direction`, and the body looking at it.
fn minor_ahead(game: &mut HexWfcMatch, direction: Vec3, distance: f32) -> Vec3 {
    let at = game.body_position_for_tests(BODY) + direction * distance;
    let cell = game.players[&BODY].cell;
    assert!(game.release_guardian(MINOR, HexReleasedKind::Minor, cell));
    game.stand_minor_for_tests(MINOR, at);
    game.aim_body_for_tests(BODY, at);
    at
}

fn minor(game: &HexWfcMatch) -> &super::super::HexMinorState {
    match &game.released[&MINOR] {
        HexReleasedGuardian::Minor(minor) => minor,
        HexReleasedGuardian::Major(_) => panic!("a major"),
    }
}

#[test]
fn a_level_ray_meets_the_cylinder_at_its_near_side() {
    let hit = ray_meets_upright(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, -5.0), 0.5, 1.0);
    assert!((hit.expect("hit") - 4.5).abs() < 1e-4);
}

#[test]
fn a_ray_that_passes_beside_over_or_away_misses() {
    let beside = ray_meets_upright(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.8, 0.0, -5.0), 0.5, 1.0);
    let over = ray_meets_upright(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::new(0.0, -1.5, -5.0),
        0.5,
        1.0,
    );
    let behind = ray_meets_upright(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0.0, 0.0, 5.0), 0.5, 1.0);
    assert_eq!((beside, over, behind), (None, None, None));
}

#[test]
fn a_steep_ray_meets_it_through_the_top() {
    let down = Vec3::new(0.0, -1.0, -1.0).normalize();
    // Down onto the top cap, two metres below the eye.
    let top =
        ray_meets_upright(Vec3::ZERO, down, Vec3::new(0.0, -3.0, -2.2), 0.5, 1.0).expect("hit");
    assert!((top - 2.0 * 2f32.sqrt()).abs() < 1e-3, "{top}");
    // Past the cap's edge, it comes in through the side.
    let side =
        ray_meets_upright(Vec3::ZERO, down, Vec3::new(0.0, -3.0, -3.0), 0.5, 1.0).expect("hit");
    assert!((side - 2.5 * 2f32.sqrt()).abs() < 1e-3, "{side}");
    let straight_down =
        ray_meets_upright(Vec3::ZERO, Vec3::NEG_Y, Vec3::new(0.2, -3.0, 0.0), 0.5, 1.0);
    assert!((straight_down.expect("hit") - 2.0).abs() < 1e-4);
}

/// A push sends the minor in the crosshair sliding away along the look. It neither walks
/// nor catches while it slides, and hunts again once it has stopped.
#[test]
fn a_push_slides_the_minor_away_along_the_look() {
    let mut game = game();
    let direction = open_direction(&game, 16.0);
    let start = minor_ahead(&mut game, direction, 4.0);
    assert_eq!(
        game.kinetic_target(BODY).map(|target| target.guardian),
        Some(MINOR)
    );

    let events = step(&mut game, PUSH);
    assert!(
        events.contains(&HexMatchEventKind::KineticPush),
        "{events:?}"
    );
    assert!(minor(&game).staggered());
    let mut ticks = 0;
    while minor(&game).staggered() {
        let events = step(&mut game, IDLE);
        assert!(!events.contains(&HexMatchEventKind::GuardianCatch));
        ticks += 1;
        assert!(ticks <= KINETIC_STAGGER_TICKS, "the stagger never ended");
    }
    let slid = (minor(&game).position - start).dot(direction);
    assert!(
        (4.0..7.0).contains(&slid),
        "slid {slid:.2} m along the look in {ticks} ticks"
    );
    // Recovered, it comes for the body again.
    for _ in 0..40 {
        step(&mut game, IDLE);
    }
    assert!(minor(&game).target.is_some(), "it never hunted again");
}

/// A pull draws the minor back toward the Observer, and it does not catch while it slides.
#[test]
fn a_pull_slides_the_minor_toward_the_observer() {
    let mut game = game();
    let direction = open_direction(&game, 16.0);
    let start = minor_ahead(&mut game, direction, 7.0);
    let events = step(&mut game, PULL);
    assert!(
        events.contains(&HexMatchEventKind::KineticPull),
        "{events:?}"
    );
    while minor(&game).staggered() {
        let events = step(&mut game, IDLE);
        assert!(!events.contains(&HexMatchEventKind::GuardianCatch));
    }
    let drawn = (start - minor(&game).position).dot(direction);
    assert!(
        (2.5..5.0).contains(&drawn),
        "drawn {drawn:.2} m toward the body"
    );
}

/// What kills is the architecture: a minor pushed off an unrailed edge falls out of the
/// facility and is gone.
#[test]
fn a_minor_pushed_off_an_open_edge_is_lost() {
    let mut game = game();
    let (direction, edge) = compass()
        .find_map(|direction| {
            let edge = (1..=12).find(|&metre| !floored_at(&game, direction, metre as f32))?;
            (edge >= 4 && clear_along(&game, direction, edge as f32 + 3.0))
                .then_some((direction, edge))
        })
        .expect("an unrailed edge near the spawn");
    minor_ahead(&mut game, direction, edge as f32 - 2.5);
    assert!(step(&mut game, PUSH).contains(&HexMatchEventKind::KineticPush));
    let mut lost = false;
    for _ in 0..240 {
        if step(&mut game, IDLE).contains(&HexMatchEventKind::GuardianLost) {
            lost = true;
            break;
        }
    }
    assert!(lost, "the minor was never lost over the edge");
    assert!(!game.released.contains_key(&MINOR));
}

/// After a shot the tool waits out its cooldown. A press during it does nothing.
#[test]
fn the_tool_fires_again_only_after_its_cooldown() {
    let mut game = game();
    let direction = open_direction(&game, 16.0);
    minor_ahead(&mut game, direction, 4.0);
    step(&mut game, PUSH);
    assert_eq!(game.kinetic_cooldown(BODY), KINETIC_COOLDOWN_TICKS);
    let at = minor(&game).position;
    game.aim_body_for_tests(BODY, at);
    let events = step(&mut game, PULL);
    assert!(
        !events.contains(&HexMatchEventKind::KineticPull),
        "fired inside its cooldown"
    );
    for _ in 1..KINETIC_COOLDOWN_TICKS {
        step(&mut game, IDLE);
    }
    assert_eq!(game.kinetic_cooldown(BODY), 0);
}

/// A shot at nothing does nothing: no event, and no cooldown to wait out.
#[test]
fn a_shot_at_nothing_does_nothing() {
    let mut game = game();
    let direction = open_direction(&game, 16.0);
    let at = minor_ahead(&mut game, direction, 4.0);
    let (eye, _) = game.eye_and_look(BODY).expect("a body");
    game.aim_body_for_tests(BODY, eye - (at - eye));
    assert_eq!(game.kinetic_target(BODY), None);
    let events = step(&mut game, PUSH);
    assert!(!events.contains(&HexMatchEventKind::KineticPush));
    assert_eq!(game.kinetic_cooldown(BODY), 0);
    assert!(!minor(&game).staggered());
}

/// A minor beyond reach, or behind a wall, is not in the crosshair.
#[test]
fn reach_and_walls_bound_the_crosshair() {
    let mut game = game();
    let direction = open_direction(&game, 16.0);
    minor_ahead(&mut game, direction, KINETIC_REACH + 1.5);
    assert_eq!(game.kinetic_target(BODY), None, "beyond reach");

    let (eye, _) = game.eye_and_look(BODY).expect("a body");
    let centre = game.body_position_for_tests(BODY);
    let (wall, distance) = (0..48)
        .map(|step| {
            let angle = step as f32 * std::f32::consts::TAU / 48.0;
            Vec3::new(angle.sin(), 0.0, -angle.cos())
        })
        .find_map(|direction| {
            let eye_hit = game.physics.ray_distance(eye, direction, 6.0)?;
            let body_hit = game.physics.ray_distance(centre, direction, 6.0)?;
            (eye_hit > 1.5 && body_hit > 1.5).then_some((direction, eye_hit.min(body_hit)))
        })
        .expect("a wall near the spawn");
    let behind = centre + wall * (distance + 1.0);
    game.stand_minor_for_tests(MINOR, behind);
    game.aim_body_for_tests(BODY, behind);
    assert_eq!(game.kinetic_target(BODY), None, "through a wall");
}

/// The same shots on two peers leave the same world.
#[test]
fn a_shove_steps_identically_on_every_peer() {
    let mut a = game();
    let direction = open_direction(&a, 16.0);
    minor_ahead(&mut a, direction, 4.0);
    let mut b = a.clone();
    for tick in 0..240 {
        let actions = match tick {
            0 => PUSH,
            90 => PULL,
            _ => IDLE,
        };
        step(&mut a, actions);
        step(&mut b, actions);
        assert_eq!(a.snapshot().digest, b.snapshot().digest, "tick {}", a.tick);
    }
}
