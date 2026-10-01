//! The kinetic tool under the Ascent rules: the rules own every Observer's charge, a
//! shot that lands is paid for, a miss is free, and an empty pool cannot fire.

use glam::Vec3;

use super::*;
use crate::ascent::economy::{KINETIC_SHOT_COST, MAX_CHARGE, PLUMB_SHOT_COST};
use crate::hex_wfc::{HexActionButtons, HexPlumbAim, HexReleasedKind};

const MINOR: u16 = 900;

/// One tick, the body standing still and pressing `actions`; the Architect idle.
fn press(game: &mut AscentMatch, actions: HexActionButtons) -> Vec<HexMatchEventKind> {
    command(
        game,
        HexPlayerCommand {
            actions,
            ..HexPlayerCommand::default()
        },
    )
}

/// One tick, the body standing still and firing a plumb armed straight up.
fn plumb(game: &mut AscentMatch) -> Vec<HexMatchEventKind> {
    command(
        game,
        HexPlayerCommand {
            plumb: Some(HexPlumbAim { pitch: 90, yaw: 0 }),
            ..HexPlayerCommand::default()
        },
    )
}

/// One tick, the body sending `body`; the Architect idle.
fn command(game: &mut AscentMatch, body: HexPlayerCommand) -> Vec<HexMatchEventKind> {
    let tick = game.rules().tick + 1;
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(BODY, body)]),
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ARCHITECT, SeatCommand::None)]),
    };
    game.step(&bodies, &seats).expect("a well-formed frame");
    game.physical()
        .recent_events
        .iter()
        .map(|event| event.kind)
        .collect()
}

const PUSH: HexActionButtons = HexActionButtons {
    interact: false,
    deploy_lantern: false,
    recover_lantern: false,
    deploy_pad: false,
    kinetic_push: true,
    kinetic_pull: false,
};

/// A minor three metres from the body along a line with nothing solid on it, and the
/// body looking at it.
fn minor_in_the_crosshair(game: &mut AscentMatch) {
    let physical = &mut game.physical;
    let centre = physical.body_position_for_tests(BODY);
    let (eye, _) = physical.eye_and_look(BODY).expect("a body");
    let clear = |direction: Vec3| {
        physical
            .solid_along_for_tests(eye, direction, 4.0)
            .is_none()
            && physical
                .solid_along_for_tests(centre, direction, 4.0)
                .is_none()
    };
    let direction = (0..48)
        .map(|step| {
            let angle = step as f32 * std::f32::consts::TAU / 48.0;
            Vec3::new(angle.sin(), 0.0, -angle.cos())
        })
        .find(|&direction| clear(direction))
        .expect("an open line from the spawn");
    let at = centre + direction * 3.0;
    if !physical.released.contains_key(&MINOR) {
        let cell = physical.players[&BODY].cell;
        assert!(physical.release_guardian(MINOR, HexReleasedKind::Minor, cell));
    }
    physical.stand_minor_for_tests(MINOR, at);
    physical.aim_body_for_tests(BODY, at);
}

fn charge(game: &AscentMatch) -> u32 {
    let observer = game.observer_for(BODY).expect("an Observer");
    game.rules().economy.charge(observer)
}

#[test]
fn a_shot_that_lands_is_paid_for_from_the_observers_charge() {
    let mut game = game(7);
    assert_eq!(charge(&game), MAX_CHARGE);
    minor_in_the_crosshair(&mut game);
    assert!(press(&mut game, PUSH).contains(&HexMatchEventKind::KineticPush));
    assert_eq!(charge(&game), MAX_CHARGE - KINETIC_SHOT_COST);
}

#[test]
fn a_miss_is_free() {
    let mut game = game(7);
    minor_in_the_crosshair(&mut game);
    // Turn round: the crosshair has nothing.
    let physical = &mut game.physical;
    let (eye, look) = physical.eye_and_look(BODY).expect("a body");
    physical.aim_body_for_tests(BODY, eye - look * 3.0);
    assert!(!press(&mut game, PUSH).contains(&HexMatchEventKind::KineticPush));
    assert_eq!(charge(&game), MAX_CHARGE);
}

#[test]
fn an_empty_pool_cannot_fire() {
    let mut game = game(7);
    let observer = game.observer_for(BODY).expect("an Observer");
    game.ascent
        .session
        .sim
        .economy
        .set_charge(observer, KINETIC_SHOT_COST - 1);
    minor_in_the_crosshair(&mut game);
    let events = press(&mut game, PUSH);
    assert!(
        !events.contains(&HexMatchEventKind::KineticPush),
        "fired on an empty pool"
    );
    assert_eq!(charge(&game), KINETIC_SHOT_COST - 1);
    assert_eq!(
        game.physical().kinetic_cooldown(BODY),
        0,
        "a cleared shot left a cooldown"
    );
}

#[test]
fn a_plumb_that_lands_costs_more_than_a_shove() {
    let mut game = game(7);
    minor_in_the_crosshair(&mut game);
    assert!(plumb(&mut game).contains(&HexMatchEventKind::KineticPlumb));
    assert_eq!(charge(&game), MAX_CHARGE - PLUMB_SHOT_COST);
    const { assert!(PLUMB_SHOT_COST > KINETIC_SHOT_COST) };
}

/// A pool that could still pay for a shove cannot pay for a plumb.
#[test]
fn a_pool_short_of_a_plumb_cannot_plumb() {
    let mut game = game(7);
    let observer = game.observer_for(BODY).expect("an Observer");
    game.ascent
        .session
        .sim
        .economy
        .set_charge(observer, PLUMB_SHOT_COST - 1);
    minor_in_the_crosshair(&mut game);
    assert!(
        !plumb(&mut game).contains(&HexMatchEventKind::KineticPlumb),
        "plumbed on a pool short of it"
    );
    assert_eq!(charge(&game), PLUMB_SHOT_COST - 1);
    assert!(press(&mut game, PUSH).contains(&HexMatchEventKind::KineticPush));
}
