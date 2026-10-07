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

#[test]
fn windows_and_open_air_prefetch_disconnected_upper_floors_and_follow_pitch() {
    use crate::hex_wfc::view::shell::{CellGeometryIndex, HexGeometryCatalog};
    use observed_facility::hex_wfc::{HexArchetype, HexPlacement, HexSpace};
    use observed_hex::{HexCoord, PortClass, hex_origin};
    use std::collections::BTreeMap;
    let mut runtime = test_runtime();
    let current = HexCoord {
        q: 10,
        r: 0,
        level: 0,
    };
    let across = HexCoord {
        q: 19,
        r: 0,
        level: 5,
    };
    let behind = HexCoord {
        q: 1,
        r: 0,
        level: 5,
    };
    runtime.match_state.facility.config.cols = 24;
    runtime.match_state.facility.config.levels = 8;
    runtime.match_state.facility.blueprints.clear();
    runtime.match_state.facility.placements = [current, across, behind]
        .into_iter()
        .map(|coord| {
            (
                coord,
                HexPlacement {
                    coord,
                    space: HexSpace::Hall,
                    archetype: HexArchetype::Expanse,
                    doors: 0,
                    low_doors: 0,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                },
            )
        })
        .collect();
    let id = runtime.local_player;
    let player = runtime.match_state.players.get_mut(&id).unwrap();
    player.cell = current;
    player.position = Vec3::from_array(hex_origin(current)) + Vec3::Y * 1.5;
    player.yaw = std::f32::consts::FRAC_PI_2;
    let catalog = HexGeometryCatalog {
        generation: 0,
        cells: [current, across, behind]
            .into_iter()
            .map(|coord| {
                (
                    coord,
                    CellGeometryIndex {
                        footprint: vec![coord],
                        piece_ids: Vec::new(),
                        lights: Vec::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
        boundary_piece_ids: Vec::new(),
    };
    let mut window = super::Window::default();
    let ahead = window.update(&runtime, &catalog, 180.0).clone();
    assert!(
        ahead.contains(&across),
        "a walkable portal is not required to see another floor"
    );
    assert!(!ahead.contains(&behind));
    assert!(!window.update(&runtime, &catalog, 90.0).contains(&across));
    runtime.match_state.players.get_mut(&id).unwrap().pitch = -std::f32::consts::FRAC_PI_2;
    assert!(!window.update(&runtime, &catalog, 180.0).contains(&across));
    runtime.match_state.players.get_mut(&id).unwrap().pitch = std::f32::consts::FRAC_PI_2;
    assert!(
        window.update(&runtime, &catalog, 180.0).contains(&across),
        "pitch must invalidate the cached visibility window"
    );
}

#[test]
fn visible_upper_floors_are_prepared_before_hidden_current_floor_cells() {
    use observed_hex::{HexCoord, hex_origin};
    let mut runtime = test_runtime();
    let id = runtime.local_player;
    let player = runtime.match_state.players.get_mut(&id).unwrap();
    player.cell = HexCoord {
        q: 10,
        r: 10,
        level: 0,
    };
    player.position = Vec3::from_array(hex_origin(player.cell)) + Vec3::Y * 1.5;
    player.yaw = 0.0;
    player.pitch = 0.0;
    let upstairs = HexCoord {
        q: 10,
        r: 7,
        level: 4,
    };
    let behind = HexCoord {
        q: 10,
        r: 14,
        level: 0,
    };
    assert!(
        super::spawn_priority(&[upstairs], player, None)
            < super::spawn_priority(&[behind], player, None)
    );
    assert!(
        super::spawn_priority(&[player.cell], player, None)
            < super::spawn_priority(&[upstairs], player, None)
    );
}

#[test]
fn wider_views_reserve_cluster_capacity_without_reducing_custom_limits() {
    use bevy::light::cluster::{GlobalClusterGpuSettings, GlobalClusterSettings};
    for capacity in [128, 32768] {
        let mut app = App::new();
        app.insert_resource(GlobalClusterSettings {
            supports_storage_buffers: true,
            clustered_decals_are_usable: false,
            gpu_clustering: Some(GlobalClusterGpuSettings {
                initial_z_slice_list_capacity: capacity,
                initial_index_list_capacity: 65536,
            }),
            max_uniform_buffer_clusterable_objects: 512,
            view_cluster_bindings_max_indices: 65536,
        });
        app.add_systems(Update, crate::hex_wfc::view::lighting::configure_clusters);
        app.update();
        assert_eq!(
            app.world()
                .resource::<GlobalClusterSettings>()
                .gpu_clustering
                .unwrap()
                .initial_z_slice_list_capacity,
            capacity.max(16384)
        );
    }
}
