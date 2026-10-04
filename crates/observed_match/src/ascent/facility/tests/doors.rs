//! Doors on the real facility: an Architect's door card stands a closed door in the world,
//! a body opens and closes it with interact, and a floor without power freezes it.

use glam::Vec3;

use super::*;
use crate::ascent::sim::{CardKind, DoorState};
use crate::hex_wfc::{HexActionButtons, door_pose};

const INTERACT: HexActionButtons = HexActionButtons {
    interact: true,
    deploy_lantern: false,
    recover_lantern: false,
    deploy_pad: false,
    kinetic_push: false,
    kinetic_pull: false,
};

/// One tick, the body standing still and pressing `actions`; the Architect idle.
fn press(game: &mut AscentMatch, actions: HexActionButtons) {
    let tick = game.rules().tick + 1;
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(
            BODY,
            HexPlayerCommand {
                actions,
                ..HexPlayerCommand::default()
            },
        )]),
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ARCHITECT, SeatCommand::None)]),
    };
    game.step(&bodies, &seats).expect("a well-formed frame");
}

/// Walk until a door card can be played, and play it.
fn deploy_a_door(game: &mut AscentMatch) -> (HexCoord, observed_hex::HexFace) {
    game.ascent
        .stage_card(ARCHITECT, CardKind::Door)
        .expect("stage a real door card");
    let play = explore_until_playable(game, |game, _, _, index| {
        game.session().hands[&TEAM].deck.hand[index].kind == CardKind::Door
    });
    let before: Vec<_> = game.rules().doors.keys().copied().collect();
    let refusals = step(game, Body::Turn(0.0), SeatCommand::Architect(play));
    assert!(refusals.is_empty(), "{refusals:?}");
    let key = *game
        .rules()
        .doors
        .keys()
        .find(|key| !before.contains(key))
        .expect("the play deployed a door");
    (key.cell, key.face)
}

/// Stand the body on whichever side of the door has floor, facing it.
fn stand_at_door(game: &mut AscentMatch, cell: HexCoord, face: observed_hex::HexFace) {
    let (floor, _) = door_pose(cell, face);
    let grid = game.rules().world.config.grid();
    let next = grid
        .neighbor(cell, face)
        .expect("a threshold has two sides");
    for (side, from) in [(cell, next), (next, cell)] {
        let away = (Vec3::from_array(observed_hex::hex_origin(side))
            - Vec3::from_array(observed_hex::hex_origin(from)))
        .with_y(0.0)
        .normalize();
        if game
            .physical
            .stage_body_facing(BODY, side, floor + away * 1.4, floor + Vec3::Y * 1.5)
        {
            return;
        }
    }
    panic!("no floor to stand on beside the door");
}

#[test]
fn a_door_card_stands_a_closed_door_in_the_facility() {
    let mut game = game(7);
    let (cell, face) = deploy_a_door(&mut game);
    let door = game
        .physical()
        .doors()
        .find(|door| door.cell == cell && door.face == face)
        .expect("the rules' door stands in the facility");
    assert!(door.closed, "a door card deploys a closed door");
    let next = game
        .rules()
        .world
        .config
        .grid()
        .neighbor(cell, face)
        .expect("two sides");
    assert!(game.physical().closed_door_between(cell, next));
}

#[test]
fn interact_at_a_door_opens_it_and_closes_it_again() {
    let mut game = game(7);
    let (cell, face) = deploy_a_door(&mut game);
    let key = ThresholdKeyOf { cell, face };
    stand_at_door(&mut game, cell, face);
    press(&mut game, HexActionButtons::default());
    let (_, at) = game
        .ascent
        .at_door(&game.physical, BODY)
        .expect("standing at the door");
    assert!(at.closed && at.operable && at.powered);
    press(&mut game, INTERACT);
    assert_eq!(key.state(&game), Some(DoorState::Open));
    assert!(game.physical().doors().any(|door| !door.closed));
    press(&mut game, HexActionButtons::default());
    press(&mut game, INTERACT);
    assert_eq!(key.state(&game), Some(DoorState::Closed));
    assert!(game.physical().doors().all(|door| door.closed));
}

#[test]
fn a_floor_without_power_freezes_its_doors() {
    let mut game = game(7);
    let (cell, face) = deploy_a_door(&mut game);
    let key = ThresholdKeyOf { cell, face };
    stand_at_door(&mut game, cell, face);
    game.ascent.stage_power(cell.level, false);
    press(&mut game, HexActionButtons::default());
    let (_, at) = game
        .ascent
        .at_door(&game.physical, BODY)
        .expect("standing at the door");
    assert!(!at.powered && !at.operable);
    press(&mut game, INTERACT);
    assert_eq!(
        key.state(&game),
        Some(DoorState::Closed),
        "a frozen door moved"
    );
    game.ascent.stage_power(cell.level, true);
    press(&mut game, HexActionButtons::default());
    press(&mut game, INTERACT);
    assert_eq!(key.state(&game), Some(DoorState::Open));
}

#[test]
fn a_door_between_two_cells_of_one_room_is_refused() {
    let mut game = game(7);
    game.ascent
        .stage_card(ARCHITECT, CardKind::Door)
        .expect("stage a real door card");
    // The team has found every room but the prison's lobby, whose cells are refused as the
    // prison core before a door is even considered.
    let lobby = game.rules().prison_core.clone();
    let rooms: Vec<HexCoord> = game
        .rules()
        .world
        .blueprints
        .iter()
        .filter(|room| room.cells.iter().all(|cell| !lobby.contains(cell)))
        .flat_map(|room| room.cells.iter().copied())
        .collect();
    game.ascent
        .session
        .sim
        .team_knowledge
        .get_mut(&TEAM)
        .expect("the team's knowledge")
        .discovered_cells
        .extend(rooms);
    let rules = game.rules();
    let grid = rules.world.config.grid();
    let door = game.session().hands[&TEAM]
        .deck
        .hand
        .iter()
        .find(|card| card.kind == CardKind::Door)
        .expect("a door card in hand")
        .id;
    let known = &rules.team_knowledge[&TEAM].discovered_cells;
    let lobby = &lobby;
    // Every threshold inside a room the team knows, and what the rules say to a door there.
    let refusals: Vec<_> = rules
        .world
        .blueprints
        .iter()
        .flat_map(|room| {
            room.cells.iter().flat_map(move |&cell| {
                (0..6u8).filter_map(move |rotation| {
                    let face = observed_hex::HexFace::LATERAL[usize::from(rotation)];
                    let next = grid.neighbor(cell, face)?;
                    (known.contains(&cell) && !lobby.contains(&cell) && room.cells.contains(&next))
                        .then_some((cell, rotation))
                })
            })
        })
        .map(|(target, rotation)| {
            game.session().architect_refusal(
                ARCHITECT,
                ArchitectCommand::Play {
                    card: door,
                    target,
                    rotation,
                },
            )
        })
        .collect();
    assert!(
        !refusals.is_empty(),
        "the team never found a room of more than one cell"
    );
    assert!(
        refusals.iter().all(Option::is_some),
        "a door is legal inside a room: {refusals:?}"
    );
    assert!(
        refusals.contains(&Some(Refusal::Architect(CommandRefusal::InvalidThreshold))),
        "{refusals:?}"
    );
}

struct ThresholdKeyOf {
    cell: HexCoord,
    face: observed_hex::HexFace,
}

impl ThresholdKeyOf {
    fn state(&self, game: &AscentMatch) -> Option<DoorState> {
        game.rules()
            .doors
            .get(&crate::ascent::sim::ThresholdKey {
                cell: self.cell,
                face: self.face,
            })
            .copied()
    }
}
