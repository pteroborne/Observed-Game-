//! A lantern's anchor holds in the rules too: no card rewrites the cells either side of the
//! doorway it anchors.

use observed_hex::HexCoord;

use super::*;
use crate::ascent::sim::CommandRefusal;

/// The open room doorways of the facility: the port, and the cell it opens onto.
fn doorways(
    game: &AscentMatch,
) -> Vec<(
    observed_facility::hex_wfc::HexThresholdKey,
    HexCoord,
    HexCoord,
)> {
    let grid = game.physical().facility.config.grid();
    game.physical()
        .door_states()
        .into_iter()
        .filter(|door| door.open)
        .filter_map(|door| {
            let outside = grid.neighbor(door.room_cell, door.face)?;
            Some((door.key, door.room_cell, outside))
        })
        .collect()
}

#[test]
fn a_lantern_anchor_refuses_a_play_outside_its_doorway() {
    let mut game = game(7);
    step(&mut game, Body::Explore, SeatCommand::None);
    let outsides: Vec<HexCoord> = doorways(&game).iter().map(|&(_, _, out)| out).collect();
    let outsides = &outsides;
    let play = explore_until_playable(&mut game, move |_, target, _, _| outsides.contains(&target));
    let target = target_of(play);
    let (key, room_cell, _) = doorways(&game)
        .into_iter()
        .find(|&(_, _, out)| out == target)
        .expect("the play is outside a doorway");
    let anchored = game.physical.lanterns.deploy(
        BODY,
        key,
        room_cell,
        glam::Vec3::from_array(observed_hex::hex_origin(room_cell)),
    );
    assert!(anchored.is_some(), "the body anchors it");
    step(&mut game, Body::Turn(0.0), SeatCommand::None);
    assert!(game.rules().anchored.contains(&target));
    assert!(game.rules().anchored.contains(&room_cell));
    assert_eq!(
        game.session().architect_refusal(ARCHITECT, play),
        Some(Refusal::Architect(CommandRefusal::Anchored)),
        "the anchor holds what it anchors"
    );
}
