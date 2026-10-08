use super::*;
use std::{collections::BTreeMap, sync::Arc};

fn physical() -> crate::hex_wfc::HexWfcMatch {
    crate::hex_wfc::HexWfcMatch::new_with_content(
        1,
        crate::hex_wfc::HexMatchConfig {
            teams: 1,
            members_per_team: 1,
            guardian: false,
            wfc: HexWfcConfig::arc_default(),
        },
        Arc::new(crate::hex_wfc::HexMatchContent::from_runtime_catalog(
            crate::hex_wfc::test_catalog().clone(),
        )),
    )
    .expect("production match")
}

#[test]
fn initial_hall_choices_are_the_actual_projected_modules() {
    let game = physical();
    assert_eq!(game.facility.initial_modules.len(), 12);
    assert_eq!(game.snapshot().initial_modules.len(), 12);
    for (&cell, &variant) in &game.facility.initial_modules {
        let pieces = game
            .geometry
            .pieces
            .iter()
            .filter(|piece| piece.source_cell == cell && piece.role == HexStructureRole::Hall)
            .collect::<Vec<_>>();
        assert!(!pieces.is_empty());
        assert!(pieces.iter().all(|piece| {
            piece
                .tile
                .as_ref()
                .is_some_and(|tile| tile.variant == variant)
        }));
        assert!(
            game.standing_point(cell).is_some(),
            "no standing floor at {cell:?}"
        );
        assert!(
            game.geometry
                .lights
                .iter()
                .any(|light| light.source_cell == cell && light.attachment.is_some())
        );
    }
}

#[test]
fn rebuilding_the_same_topology_retires_only_its_initial_kit_and_preview_matches_commit() {
    let mut game = physical();
    let cell = *game.facility.initial_modules.keys().next().unwrap();
    let placement = game.facility.placements[&cell];
    let before = game.geometry.clone();
    let expected =
        project_hypothetical_cell(&game.facility, cell, placement, game.content().cells())
            .expect("card preview");
    let logical = game
        .apply_directed_change(BTreeMap::from([(cell, placement)]))
        .expect("mutable initial hall");
    assert!(logical.changed_cells.contains(&cell));
    assert_eq!(game.facility.cell_revision(cell), Some(1));
    assert_eq!(game.facility.initial_module_variant(cell), None);
    assert_eq!(game.snapshot().initial_modules.len(), 11);
    let current = game
        .geometry
        .pieces
        .iter()
        .filter(|piece| piece.source_cell == cell)
        .cloned()
        .collect::<Vec<_>>();
    let expected = expected
        .into_iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let current = current
        .into_iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(expected.len(), current.len());
    for (id, piece) in expected {
        assert!(
            current.get(&id) == Some(&piece),
            "preview/commit differ at {id:?}: preview={piece:?}, current={:?}",
            current.get(&id)
        );
    }
    for &other in game
        .facility
        .initial_modules
        .keys()
        .filter(|&&other| other != cell)
    {
        let pieces = |snapshot: &HexWfcGeometrySnapshot| {
            snapshot
                .pieces
                .iter()
                .filter(|piece| piece.source_cell == other)
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pieces(&before),
            pieces(&game.geometry),
            "unrelated landmark cell redrew"
        );
    }
    let fresh = HexWfcGeometrySnapshot::project_with_rooms(
        &game.facility,
        game.content().cells(),
        game.content().rooms(),
    )
    .unwrap();
    let by_id = |snapshot: &HexWfcGeometrySnapshot| {
        snapshot
            .pieces
            .iter()
            .map(|piece| (piece.id, piece.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert!(
        by_id(&fresh) == by_id(&game.geometry),
        "incremental pieces differ from fresh projection"
    );
    let colliders = |snapshot: &HexWfcGeometrySnapshot| {
        snapshot
            .arena
            .colliders
            .iter()
            .map(|collider| (collider.id, collider.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert!(
        colliders(&fresh) == colliders(&game.geometry),
        "incremental colliders differ from fresh projection"
    );
    assert_eq!(fresh.lights, game.geometry.lights);
    assert_eq!(fresh.guides, game.geometry.guides);
    assert_eq!(fresh.sockets, game.geometry.sockets);
    assert_eq!(
        fresh.rapier_scene().collider_count(),
        game.physics.collider_count()
    );
}
