//! Mutation catalogue and replacement lifecycle regressions.
use super::{
    HexPresentationReadiness, HexPresentationResidency, HexWfcVisualAssets, Reach, ResidentCell,
    residency, shell, test_runtime, visibility,
};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn catalogue_delta_survives_packed_removals_and_restores_whole_owners() {
    let mut runtime = test_runtime();
    let mut catalog = shell::HexGeometryCatalog::build(
        &runtime.match_state.facility,
        &runtime.match_state.geometry,
    );
    let target = runtime
        .match_state
        .facility
        .placements
        .values()
        .find(|p| {
            p.space == observed_facility::hex_wfc::HexSpace::Hall
                && !runtime
                    .match_state
                    .facility
                    .blueprints
                    .iter()
                    .any(|b| b.cells.contains(&p.coord))
        })
        .copied()
        .unwrap();
    let mut empty = target;
    empty.space = observed_facility::hex_wfc::HexSpace::Void;
    empty.archetype = observed_facility::hex_wfc::HexArchetype::Void;
    empty.doors = 0;
    empty.low_doors = 0;
    empty.up = observed_hex::PortClass::Sealed;
    empty.down = observed_hex::PortClass::Sealed;
    for placement in [empty, target, empty, target] {
        runtime
            .match_state
            .apply_directed_change(BTreeMap::from([(target.coord, placement)]))
            .unwrap();
        catalog.update_changed(&runtime, &runtime.match_state.last_geometry_cells);
        let fresh = shell::HexGeometryCatalog::build(
            &runtime.match_state.facility,
            &runtime.match_state.geometry,
        );
        assert_eq!(
            catalog.cells.keys().collect::<Vec<_>>(),
            fresh.cells.keys().collect::<Vec<_>>()
        );
        for (&owner, index) in &catalog.cells {
            let mut actual = index.piece_ids.clone();
            actual.sort();
            let mut expected = fresh.cells[&owner].piece_ids.clone();
            expected.sort();
            assert_eq!(actual, expected);
            assert_eq!(index.lights, fresh.cells[&owner].lights);
            for &id in &index.piece_ids {
                assert_eq!(
                    runtime.match_state.geometry.piece(id).unwrap().source_cell,
                    owner
                );
            }
        }
    }
}

#[test]
fn changed_parent_remains_until_a_complete_replacement_is_spawned() {
    let mut runtime = test_runtime();
    let mut catalog = shell::HexGeometryCatalog::build(
        &runtime.match_state.facility,
        &runtime.match_state.geometry,
    );
    let coord = *catalog.cells.keys().next().unwrap();
    catalog.cells.retain(|&cell, _| cell == coord);
    runtime.pending_visual_cells.insert(coord);
    let mut app = App::new();
    let old = app.world_mut().spawn_empty().id();
    let mut materials = Assets::<StandardMaterial>::default();
    app.insert_resource(HexWfcVisualAssets::for_test(&mut materials));
    app.insert_resource(Assets::<Mesh>::default());
    app.insert_resource(runtime);
    app.insert_resource(HexPresentationReadiness::default());
    app.insert_resource(HexPresentationResidency {
        catalog,
        resident: BTreeMap::from([(
            coord,
            ResidentCell {
                shown: true,
                entity: old,
                child_pieces: 1,
            },
        )]),
        replacements: BTreeSet::new(),
        defer_incremental_once: false,
        capture_unbounded: true,
        reach: Reach::play(),
        window: visibility::Window::default(),
    });
    app.add_systems(Update, residency::sync_changed_geometry);
    app.update();
    assert!(
        app.world().get_entity(old).is_ok(),
        "invalidation must not leave a missing cell"
    );
    assert!(
        app.world()
            .resource::<HexPresentationResidency>()
            .replacements
            .contains(&coord)
    );
    app.add_systems(
        Update,
        residency::sync_streamed_cells.after(residency::sync_changed_geometry),
    );
    app.update();
    let resident = &app.world().resource::<HexPresentationResidency>().resident[&coord];
    assert_ne!(resident.entity, old);
    assert!(app.world().get_entity(old).is_err());
    assert!(app.world().get_entity(resident.entity).is_ok());
    assert!(
        app.world()
            .resource::<HexPresentationResidency>()
            .replacements
            .is_empty()
    );
}

#[test]
fn a_parent_with_cold_decorations_is_not_published_or_leaked() {
    let mut runtime = test_runtime();
    let catalog = shell::HexGeometryCatalog::build(
        &runtime.match_state.facility,
        &runtime.match_state.geometry,
    );
    let coord = *catalog
        .cells
        .keys()
        .find(|&&coord| {
            let pieces: Vec<_> = runtime.match_state.geometry.pieces_in_cell(coord).collect();
            pieces
                .first()
                .is_some_and(|piece| piece.role == observed_match::hex_wfc::HexStructureRole::Hall)
                && super::super::library::has_bookcase_support(
                    &runtime.match_state.facility,
                    coord,
                    &pieces,
                )
        })
        .expect("a real corridor has a full wall supporting Library bookcases");
    // This compact fixture does not include the Library district. Dressing a
    // real corridor in that district exercises its generated book recipes.
    runtime.match_state.facility.architecture.insert(
        coord,
        observed_content::ArchitectureRegister::InfiniteGallery,
    );
    let pieces: Vec<_> = runtime.match_state.geometry.pieces_in_cell(coord).collect();
    let key = shell::cell_mesh_key(&pieces, coord).unwrap();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut meshes = Assets::<Mesh>::default();
    for (group, data) in super::super::mesh_group::gather(&pieces) {
        assets.merged_mesh_for(&mut meshes, Some(&key), group, &data.hulls);
    }
    for replacing in [true, false] {
        assets.cache_misses += 1;
        let retained = if replacing {
            BTreeSet::from([coord])
        } else {
            BTreeSet::new()
        };
        let mut world = World::new();
        let initial_entities = world.entity_count();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let spawned = shell::spawn_cells_bounded(
            &mut commands,
            &mut assets,
            &mut meshes,
            (&runtime.match_state.facility, &runtime.match_state.geometry),
            &catalog,
            &[coord],
            Some(shell::SpawnBudget {
                time: std::time::Duration::from_millis(3),
                retained: &retained,
            }),
        );
        queue.apply(&mut world);
        world.flush();
        assert!(assets.missing_meshes);
        assert!(!assets.preparing_cell);
        if replacing {
            assert!(
                spawned.is_empty(),
                "an existing cell must wait for complete dressing"
            );
            assert_eq!(
                world.entity_count(),
                initial_entities,
                "discarded staging must not leak"
            );
        } else {
            assert_eq!(
                spawned.len(),
                1,
                "new cells must show their structural shell before dressing finishes"
            );
            assert!(spawned[0].needs_decoration);
            assert!(world.query::<&Mesh3d>().iter(&world).count() > 0);
            world.despawn(spawned[0].entity);
            world.flush();
            assert_eq!(world.entity_count(), initial_entities);
        }
    }
}

#[test]
fn prepared_residents_do_not_disappear_when_the_portal_window_turns_away() {
    let mut runtime = test_runtime();
    let id = runtime.local_player;
    let player = runtime.match_state.players.get_mut(&id).unwrap();
    player.yaw = 0.0;
    player.pitch = 0.0;
    player.position = Vec3::Y * 1.5;
    let coord = observed_hex::HexCoord {
        q: 0,
        r: 8,
        level: 0,
    };
    let catalog = super::catalog([(coord, vec![coord])]);
    let mut window = visibility::Window::default();
    assert!(
        !window
            .update(&runtime, &catalog, super::super::STREAM_ENTER_RADIUS)
            .contains(&coord)
    );
    let mut app = App::new();
    let parent = app.world_mut().spawn(Visibility::Hidden).id();
    app.world_mut().spawn((
        crate::view::components::GameCam,
        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
    ));
    let mut materials = Assets::<StandardMaterial>::default();
    app.insert_resource(HexWfcVisualAssets::for_test(&mut materials));
    app.insert_resource(Assets::<Mesh>::default());
    app.insert_resource(runtime);
    app.insert_resource(HexPresentationReadiness::default());
    app.insert_resource(HexPresentationResidency {
        catalog,
        resident: BTreeMap::from([(
            coord,
            ResidentCell {
                shown: false,
                entity: parent,
                child_pieces: 1,
            },
        )]),
        replacements: BTreeSet::new(),
        defer_incremental_once: false,
        capture_unbounded: false,
        reach: Reach::play(),
        window,
    });
    app.add_systems(Update, residency::sync_streamed_cells);
    app.update();
    assert!(app.world().resource::<HexPresentationResidency>().resident[&coord].shown);
    assert_eq!(
        app.world().get::<Visibility>(parent),
        Some(&Visibility::Inherited)
    );
}
