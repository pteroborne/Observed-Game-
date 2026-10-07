//! Doors in a physical match: a closed door's panel stops a body and a minor's catch, an
//! open door and a removed one stop nothing, and a bot walking into a closed door opens it.

use std::collections::BTreeMap;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::{HexCoord, HexFace, hex_origin};
use player_input::PlayerIntent;

use super::*;
use crate::hex_wfc::model::{
    HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig, HexMatchEventKind, HexPlayerCommand,
};

const BODY: PlayerId = PlayerId(0);

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

#[test]
fn low_door_panel_tracks_the_actual_aperture_when_rebuilt() {
    let mut game = game();
    let cell = game.players[&BODY].cell;
    let face = HexFace::LATERAL
        .into_iter()
        .find(|&face| {
            game.facility.placements[&cell]
                .ports()
                .port(face)
                .is_doorway()
        })
        .expect("spawn threshold");
    let wanted = [((cell, face), true)];
    game.set_doors(wanted);
    let door = *game.doors().next().expect("door");
    assert_eq!(door.height, 3.0);
    assert_eq!(door.panel().center.y, door.pose().0.y + 1.5);
    game.facility
        .placements
        .get_mut(&cell)
        .expect("cell")
        .low_doors = 0;
    game.set_doors(wanted);
    let rebuilt = game.doors().next().expect("rebuilt door");
    assert_eq!(rebuilt.height, 4.0);
    assert_eq!(rebuilt.collider, door.collider);
    assert_eq!(rebuilt.panel().center.y, rebuilt.pose().0.y + 2.0);
}

fn walk(game: &mut HexWfcMatch, ticks: u32) -> Vec<HexMatchEventKind> {
    let mut events = Vec::new();
    for _ in 0..ticks {
        let frame = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick: game.tick + 1,
            commands: BTreeMap::from([(
                BODY,
                HexPlayerCommand {
                    intent: PlayerIntent {
                        movement: Vec2::new(0.0, 1.0),
                        ..PlayerIntent::default()
                    },
                    ..HexPlayerCommand::default()
                },
            )]),
        };
        events.extend(game.step(&frame).iter().map(|event| event.kind));
    }
    events
}

fn across(cell: HexCoord, next: HexCoord) -> Vec3 {
    (Vec3::from_array(hex_origin(next)) - Vec3::from_array(hex_origin(cell)))
        .with_y(0.0)
        .normalize()
}

/// A doorway on the ground floor a body walks straight through while it has no door:
/// the threshold, where to stand before it, and the cell beyond.
fn doorway(game: &HexWfcMatch) -> (HexCoord, HexFace, Vec3, HexCoord) {
    let grid = game.facility.config.grid();
    let mut cells: Vec<HexCoord> = game.facility.placements.keys().copied().collect();
    cells.sort();
    for cell in cells.into_iter().filter(|cell| cell.level == 0) {
        let placement = game.facility.placements[&cell];
        if !placement.space.built() {
            continue;
        }
        for face in HexFace::LATERAL {
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            let opens = placement.is_open(face)
                && game
                    .facility
                    .placements
                    .get(&next)
                    .is_some_and(|other| other.space.built() && other.is_open(face.opposite()));
            if !opens {
                continue;
            }
            let (floor, _) = door_pose(cell, face);
            let across = across(cell, next);
            let (Some(before), Some(_)) = (
                game.stands_at(floor - across * 3.0),
                game.stands_at(floor + across * 3.0),
            ) else {
                continue;
            };
            let mut trial = game.clone();
            if !trial.stage_body_facing(BODY, cell, before, floor + across * 6.0 + Vec3::Y * 1.5) {
                continue;
            }
            walk(&mut trial, 90);
            if trial.players[&BODY].cell == next {
                return (cell, face, before, next);
            }
        }
    }
    panic!("a ground-floor doorway a body walks through");
}

/// Stand at `before`, facing the doorway: its middle, where a closed door's panel is.
fn stand_before(game: &mut HexWfcMatch, cell: HexCoord, face: HexFace, before: Vec3) {
    let (floor, _) = door_pose(cell, face);
    assert!(game.stage_body_facing(BODY, cell, before, floor + Vec3::Y * 1.5));
}

#[test]
fn a_closed_door_stops_a_body_and_an_open_or_removed_one_does_not() {
    let mut game = game();
    let (cell, face, before, next) = doorway(&game);

    game.set_doors([((cell, face), true)]);
    stand_before(&mut game, cell, face, before);
    walk(&mut game, 90);
    assert_eq!(
        game.players[&BODY].cell, cell,
        "walked through a closed door"
    );
    assert!(game.closed_door_between(cell, next));
    assert!(game.closed_door_between(next, cell));

    game.set_doors([((cell, face), false)]);
    assert!(!game.closed_door_between(cell, next));
    stand_before(&mut game, cell, face, before);
    walk(&mut game, 90);
    assert_eq!(
        game.players[&BODY].cell, next,
        "an open door stopped the body"
    );

    // Closed again, then taken away: nothing stands in the doorway.
    game.set_doors([((cell, face), true)]);
    game.set_doors([]);
    assert_eq!(game.doors().count(), 0);
    stand_before(&mut game, cell, face, before);
    walk(&mut game, 90);
    assert_eq!(game.players[&BODY].cell, next, "a removed door still stood");
}

#[test]
fn a_minor_does_not_catch_through_a_closed_door() {
    let mut game = game();
    let (cell, face, _, next) = doorway(&game);
    let (floor, _) = door_pose(cell, face);
    let across = across(cell, next);
    game.set_doors([((cell, face), true)]);
    // Face to face across the panel, closer than a minor catches.
    let body = game
        .stands_at(floor - across * 0.6)
        .expect("floor before the door");
    assert!(game.stage_body_facing(BODY, cell, body, floor + Vec3::Y * 1.5));
    let minor = game
        .stands_at(floor + across * 0.6)
        .expect("floor beyond the door");
    assert!(game.stage_minor(7, next, minor));
    let mut caught = false;
    for _ in 0..120 {
        let frame = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick: game.tick + 1,
            commands: BTreeMap::new(),
        };
        caught |= game
            .step(&frame)
            .iter()
            .any(|event| event.kind == HexMatchEventKind::GuardianCatch);
    }
    assert!(!caught, "a minor caught a body through a closed door");
    assert!(game.released.contains_key(&7));
}

#[test]
fn a_bot_walking_into_a_closed_door_presses_to_open_it() {
    let mut game = game();
    let (cell, face, before, _) = doorway(&game);
    game.set_doors([((cell, face), true)]);
    let (floor, _) = door_pose(cell, face);
    let across = (floor - before).with_y(0.0).normalize();
    let near = game
        .stands_at(floor - across * 1.2)
        .expect("floor before the door");
    assert!(game.stage_body_facing(BODY, cell, near, floor + Vec3::Y * 1.5));
    assert!(game.walking_into_closed_door(BODY));
    // Turned away, it leaves the door alone.
    assert!(game.stage_body_facing(BODY, cell, near, near - across * 6.0 + Vec3::Y * 1.5));
    assert!(!game.walking_into_closed_door(BODY));
}
