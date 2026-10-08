use super::*;
use crate::ascent::sim::{CardKind, District, TileShape};
use std::sync::Arc;

fn production(enabled: bool) -> AscentMatch {
    let mut catalog = crate::hex_wfc::test_catalog().clone();
    catalog.composition.initial_hall_compositions = enabled;
    let profile = observed_authoring::CompositionBuild::new(catalog.composition.clone()).unwrap();
    catalog.simulation_content_hash = observed_authoring::fold_simulation_content_hash(
        include_str!("../../../../../../assets/tiles/compiled_catalog.sha256").trim(),
        &profile.content_hash,
    );
    let physical = HexWfcMatch::new_with_content(
        1,
        HexMatchConfig {
            teams: 1,
            members_per_team: 1,
            guardian: false,
            wfc: HexWfcConfig::arc_default(),
        },
        Arc::new(crate::hex_wfc::HexMatchContent::from_runtime_catalog(
            catalog,
        )),
    )
    .unwrap();
    AscentMatch::new(
        physical,
        1,
        BTreeMap::from([(
            ARCHITECT,
            Seat {
                role: Role::Architect(TEAM),
                bot: false,
            },
        )]),
    )
    .unwrap()
}

#[test]
fn initial_halls_add_no_fixed_structure_or_automatic_team_discovery() {
    let original = production(false);
    let composed = production(true);
    for &cell in original.physical().facility.placements.keys() {
        assert_eq!(
            original.rules().fixed_structure(cell),
            composed.rules().fixed_structure(cell),
            "new fixed structure at {cell:?}"
        );
    }
    let known = &composed.rules().team_knowledge[&TEAM].discovered_cells;
    assert!(
        composed
            .physical()
            .facility
            .initial_modules
            .keys()
            .any(|cell| !known.contains(cell)),
        "initial metadata revealed every landmark"
    );
    assert_eq!(
        original.physical().facility.blueprints,
        composed.physical().facility.blueprints
    );
}

#[test]
fn a_legal_same_shape_card_rebuild_replaces_an_initial_module() {
    let mut game = production(true);
    let (target, shape, rotation) = game
        .physical()
        .facility
        .initial_modules
        .keys()
        .copied()
        .filter(|cell| {
            !game.rules().fixed_structure(*cell)
                && !game.rules().observed.contains(cell)
                && !game.rules().occupied().contains(cell)
                && !game.rules().anchored.contains(cell)
                && !game.rules().prison_core.contains(cell)
        })
        .find_map(|target| {
            TileShape::AUTHORED
                .iter()
                .copied()
                .flat_map(|shape| (0..6).map(move |rotation| (shape, rotation)))
                .find(|&(shape, rotation)| {
                    game.rules().played_placement(shape, target, rotation)
                        == game.physical().facility.placements[&target]
                })
                .map(|(shape, rotation)| (target, shape, rotation))
        })
        .expect("mutable initial hall describable by an ordinary card");
    game.ascent
        .session
        .sim
        .team_knowledge
        .get_mut(&TEAM)
        .unwrap()
        .discovered_cells
        .insert(target);
    let district = District::for_floor(target.level, game.rules().world.config.levels);
    let deck = &mut game.ascent.session.hands.get_mut(&TEAM).unwrap().deck;
    assert!(deck.stage_in_district(CardKind::Tile(shape), district));
    let card = deck
        .hand
        .iter()
        .find(|card| card.kind == CardKind::Tile(shape) && card.district == Some(district))
        .unwrap()
        .id;
    let command = ArchitectCommand::Play {
        card,
        target,
        rotation,
    };
    assert_eq!(game.session().architect_refusal(ARCHITECT, command), None);
    assert!(step(&mut game, Body::Turn(0.0), SeatCommand::Architect(command)).is_empty());
    assert_eq!(
        game.physical().facility.initial_module_variant(target),
        None
    );
    assert_eq!(game.rules().world.initial_module_variant(target), None);
    assert_eq!(
        game.physical().facility.cell_revision(target),
        game.rules().world.cell_revision(target)
    );
    assert_geometry_is_fresh(&game);
}
