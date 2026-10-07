use crate::hex_wfc::view::tests::test_runtime;
use bevy::prelude::*;
#[test]
fn portal_prefetch_keeps_the_occupied_cell_and_changes_with_look_and_doors() {
    let mut runtime = test_runtime();
    let catalog = crate::hex_wfc::view::shell::HexGeometryCatalog::build(
        &runtime.match_state.facility,
        &runtime.match_state.geometry,
    );
    let mut window = super::Window::default();
    let current = runtime.viewed().cell;
    let warm = window.update(&runtime, &catalog, 90.0).clone();
    assert!(warm.contains(&current));
    assert!(
        warm.len() < catalog.cells.len(),
        "visibility must not prepare the entire facility"
    );
    let mut behind = runtime.local().clone();
    behind.yaw += std::f32::consts::PI;
    runtime
        .match_state
        .players
        .insert(runtime.local_player, behind);
    let turned = window.update(&runtime, &catalog, 90.0).clone();
    assert!(turned.contains(&current));
    for face in observed_hex::HexFace::LATERAL {
        if runtime.match_state.facility.placements[&current].is_open(face) {
            runtime.match_state.set_doors([((current, face), true)]);
            let closed = window.update(&runtime, &catalog, 90.0).clone();
            assert!(closed.contains(&current));
            runtime.match_state.set_doors([]);
            assert_eq!(
                window.update(&runtime, &catalog, 90.0),
                &turned,
                "opening a door must restore the visibility window"
            );
            break;
        }
    }
}

#[test]
fn permanent_decorative_shadow_exclusions_survive_storey_changes() {
    use crate::hex_wfc::view::{NeverShadowCaster, spectate::Cutaway};
    use bevy::light::NotShadowCaster;
    let runtime = test_runtime();
    let mut app = App::new();
    app.insert_resource(runtime);
    app.add_systems(
        Update,
        crate::hex_wfc::view::lighting::sync_storey_shadow_casters,
    );
    let marker = app
        .world_mut()
        .spawn((
            Mesh3d::default(),
            NeverShadowCaster,
            NotShadowCaster,
            Cutaway {
                local: Vec3::ZERO,
                min_y: 0.0,
                max_y: 4.0,
                origin_y: 0.0,
                cell_level: 0,
                climb_wall: false,
            },
        ))
        .id();
    for level in [0, 1, 2, 0] {
        let mut runtime = app
            .world_mut()
            .resource_mut::<crate::hex_wfc::sim::HexWfcRuntime>();
        let id = runtime.local_player;
        runtime.match_state.players.get_mut(&id).unwrap().cell.level = level;
        app.update();
        assert!(app.world().entity(marker).contains::<NotShadowCaster>());
    }
}
