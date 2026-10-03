//! Real sight under the Ascent rules: an embodied Observer wards and knows by what its body
//! actually sees (`hex_wfc::sight`), and its team's two maps hold the same cells.

use std::collections::BTreeSet;

use super::*;
use crate::ascent::sim::WARD_REACH;

/// An embodied Observer wards its own cell and every cell it sees within a cell's reach,
/// and its team sees exactly what its body sees.
#[test]
fn an_embodied_observer_wards_and_knows_by_what_its_body_sees() {
    let mut game = game(7);
    for _ in 0..240 {
        step(&mut game, Body::Explore, SeatCommand::None);
        let Some(sight) = game.physical().sight(BODY).cloned() else {
            continue;
        };
        let rules = game.rules();
        for (cell, &distance) in &sight {
            if distance <= WARD_REACH {
                assert!(
                    rules.observed.contains(cell),
                    "{cell:?} seen at {distance} unwarded"
                );
            }
        }
        let visible: BTreeSet<HexCoord> = sight.keys().copied().collect();
        assert_eq!(rules.team_knowledge[&TEAM].visible_cells, visible);
    }
}

/// A cell seen further than a cell's reach is known but not warded: a look across a room
/// protects the room, not the far end of every corridor it opens onto.
#[test]
fn what_is_seen_far_off_is_known_but_not_warded() {
    let mut game = game(7);
    // Turn on the spot until the body looks down something long: what the spawn happens
    // to face is the layout's to say.
    let far_off = |game: &AscentMatch| {
        let own = game.physical().players[&BODY].cell;
        let sight = game.physical().sight(BODY).cloned().expect("a body");
        sight
            .iter()
            .filter(|&(&cell, &distance)| {
                distance > WARD_REACH + 4.0
                    && observed_hex::travel_distance(own, cell) > 2
                    && !game.rules().fixed_structure(cell)
            })
            .map(|(&cell, _)| cell)
            .collect::<Vec<HexCoord>>()
    };
    let mut far = Vec::new();
    for _ in 0..120 {
        step(&mut game, Body::Turn(0.2), SeatCommand::None);
        far = far_off(&game);
        if !far.is_empty() {
            break;
        }
    }
    assert!(
        !far.is_empty(),
        "nothing seen far off from the spawn, all round"
    );
    let rules = game.rules();
    for cell in far {
        assert!(rules.team_knowledge[&TEAM].visible_cells.contains(&cell));
        assert!(!rules.observed.contains(&cell), "{cell:?} warded from afar");
    }
}

/// A dark floor costs its Observers their sight: each knows only its own cell.
#[test]
fn a_dark_floor_costs_everything_beyond_the_bodys_own_cell() {
    let mut game = game(7);
    step(&mut game, Body::Turn(0.0), SeatCommand::None);
    let own = game.physical().players[&BODY].cell;
    game.ascent.stage_power(own.level, false);
    step(&mut game, Body::Turn(0.0), SeatCommand::None);
    assert_eq!(
        game.rules().team_knowledge[&TEAM].visible_cells,
        BTreeSet::from([own])
    );
}

/// What the Architect targets and what the in-play map shows are fed by one sight, so they
/// hold the same cells.
#[test]
fn the_rules_and_the_in_play_map_know_the_same_cells() {
    let mut game = game(7);
    let team = game.physical().players[&BODY].team;
    for tick in 0..1_200 {
        let body = if (tick / 200) % 2 == 1 {
            Body::Turn(0.6)
        } else {
            Body::Explore
        };
        step(&mut game, body, SeatCommand::None);
        let rules: BTreeSet<HexCoord> = game.rules().team_knowledge[&TEAM].discovered_cells.clone();
        let map: BTreeSet<HexCoord> = game.physical().map_knowledge[&team]
            .cells
            .keys()
            .copied()
            .collect();
        assert_eq!(rules, map, "tick {tick}");
    }
}
