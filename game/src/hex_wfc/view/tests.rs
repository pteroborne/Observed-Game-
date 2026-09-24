//! Tests for hex presentation setup, streaming residency, and teardown (see `mod.rs`).

use std::collections::BTreeSet;

use super::residency::{Reach, cell_in_stream_range, footprint_in_range, plan_residency};
use super::*;
use observed_hex::hex_origin;

fn catalog(
    footprints: impl IntoIterator<Item = (HexCoord, Vec<HexCoord>)>,
) -> shell::HexGeometryCatalog {
    shell::HexGeometryCatalog {
        generation: 0,
        cells: footprints
            .into_iter()
            .map(|(coord, footprint)| {
                (
                    coord,
                    shell::CellGeometryIndex {
                        footprint,
                        piece_indices: Vec::new(),
                        light_indices: Vec::new(),
                    },
                )
            })
            .collect(),
        boundary_piece_indices: Vec::new(),
    }
}

fn resident(coords: impl IntoIterator<Item = HexCoord>) -> BTreeMap<HexCoord, ResidentCell> {
    coords
        .into_iter()
        .map(|coord| {
            (
                coord,
                ResidentCell {
                    entity: Entity::PLACEHOLDER,
                    child_pieces: 1,
                },
            )
        })
        .collect()
}

#[test]
fn streaming_window_is_bounded() {
    // A cell directly under the runner is streamed; far plan distance or
    // level gap is culled.
    let close = HexCoord {
        q: 5,
        r: 5,
        level: 0,
    };
    let focus_position = Vec3::from_array(hex_origin(close));
    assert!(cell_in_stream_range(
        close,
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));

    let far_plan = HexCoord {
        q: 45,
        r: 5,
        level: 0,
    };
    assert!(!cell_in_stream_range(
        far_plan,
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));

    let far_level = HexCoord {
        q: 5,
        r: 5,
        level: 5,
    };
    assert!(!cell_in_stream_range(
        far_level,
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));
}

#[test]
fn footprint_in_range_is_a_no_op_for_single_cell_footprints() {
    // An ordinary tile's footprint is always `[coord]`; the ANY-of-footprint
    // check used for whole-room modules must degrade to exactly the
    // single-cell check it replaced, near or far.
    let coord = HexCoord {
        q: 10,
        r: 10,
        level: 1,
    };
    let near_focus = Vec3::from_array(hex_origin(coord));
    assert_eq!(
        footprint_in_range(
            &[coord],
            near_focus,
            1,
            STREAM_ENTER_RADIUS,
            STREAM_ENTER_LEVELS
        ),
        cell_in_stream_range(
            coord,
            near_focus,
            1,
            STREAM_ENTER_RADIUS,
            STREAM_ENTER_LEVELS
        )
    );

    let far_focus = near_focus + Vec3::new(500.0, 0.0, 0.0);
    assert_eq!(
        footprint_in_range(
            &[coord],
            far_focus,
            1,
            STREAM_ENTER_RADIUS,
            STREAM_ENTER_LEVELS
        ),
        cell_in_stream_range(
            coord,
            far_focus,
            1,
            STREAM_ENTER_RADIUS,
            STREAM_ENTER_LEVELS
        )
    );
}

#[test]
fn footprint_in_range_covers_a_whole_room_footprint() {
    // A whole-room module's single presentation parent is keyed by its anchor,
    // which can sit far from the player while another footprint cell is
    // close — the room must still stream in, which is exactly the bug
    // this stream fixes (previously only the anchor coordinate was
    // checked, so a large room could vanish while the player stood in
    // one of its far corners).
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let near_cell = HexCoord {
        q: 5,
        r: 5,
        level: 0,
    };
    let focus_position = Vec3::from_array(hex_origin(near_cell));

    assert!(!cell_in_stream_range(
        anchor,
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));
    assert!(cell_in_stream_range(
        near_cell,
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));
    assert!(footprint_in_range(
        &[anchor, near_cell],
        focus_position,
        0,
        STREAM_ENTER_RADIUS,
        STREAM_ENTER_LEVELS
    ));
}

#[test]
fn residency_hysteresis_keeps_a_cell_between_enter_and_exit_radii() {
    let focus = HexCoord::default();
    let focus_position = Vec3::from_array(hex_origin(focus));
    let edge = HexCoord {
        q: 2,
        r: 1,
        level: 0,
    };
    let catalog = catalog([(edge, vec![edge])]);

    let absent = plan_residency(
        &catalog,
        &BTreeMap::new(),
        focus_position,
        focus,
        CELL_SPAWN_BUDGET,
        CELL_DESPAWN_BUDGET,
        Reach::play(),
    );
    assert!(absent.spawn.is_empty());

    let present = plan_residency(
        &catalog,
        &resident([edge]),
        focus_position,
        focus,
        CELL_SPAWN_BUDGET,
        CELL_DESPAWN_BUDGET,
        Reach::play(),
    );
    assert!(present.despawn.is_empty());
    assert_eq!(present.desired_cells, 1);
}

#[test]
fn residency_plan_never_exceeds_the_cell_spawn_budget() {
    let focus = HexCoord::default();
    let focus_position = Vec3::from_array(hex_origin(focus));
    let catalog = catalog((0..20).map(|index| {
        let key = HexCoord {
            q: index,
            r: 0,
            level: 0,
        };
        // Artificially colocated footprints isolate the planner's budget rule.
        (key, vec![focus])
    }));
    let plan = plan_residency(
        &catalog,
        &BTreeMap::new(),
        focus_position,
        focus,
        3,
        CELL_DESPAWN_BUDGET,
        Reach::play(),
    );
    assert_eq!(plan.spawn.len(), 3);
    assert_eq!(plan.pending_cells, 20);
}

#[test]
fn occupied_room_footprint_is_never_retired_even_when_its_anchor_is_far() {
    let focus = HexCoord {
        q: 9,
        r: 9,
        level: 0,
    };
    let anchor = HexCoord::default();
    let catalog = catalog([(anchor, vec![anchor, focus])]);
    let plan = plan_residency(
        &catalog,
        &resident([anchor]),
        Vec3::from_array(hex_origin(focus)) + Vec3::new(500.0, 0.0, 0.0),
        focus,
        CELL_SPAWN_BUDGET,
        CELL_DESPAWN_BUDGET,
        Reach::play(),
    );
    assert!(plan.despawn.is_empty());
    assert_eq!(plan.desired_cells, 1);
}

fn test_runtime() -> crate::hex_wfc::sim::HexWfcRuntime {
    use crate::hex_wfc::sim::load_prototypes;
    use observed_core::PlayerId;
    use observed_match::hex_wfc::{HexBotDriver, HexMatchConfig, HexWfcMatch};
    use std::collections::BTreeSet;

    let prototypes = load_prototypes();
    let game = HexWfcMatch::new(
        44,
        HexMatchConfig {
            guardian: false,
            teams: 2,
            members_per_team: 1,
            ..HexMatchConfig::default()
        },
        &prototypes,
    )
    .expect("match generates");
    let local_player = PlayerId(0);
    let map_level = game.players[&local_player].cell.level;
    let presented_revisions = game.facility.cell_revisions.clone();
    crate::hex_wfc::sim::HexWfcRuntime {
        match_state: game,
        bot_driver: HexBotDriver::new(),
        local_player,
        pending_visual_cells: BTreeSet::new(),
        presented_revisions,
        status: String::new(),
        map_open: false,
        map_level,
        results_delay_frames: 0,
        networked: false,
        resync_attempts: 0,
    }
}

#[test]
fn cell_entity_count_falls_with_merged_hull_meshes() {
    let runtime = test_runtime();
    let catalog = shell::HexGeometryCatalog::build(&runtime);
    let mut world = World::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);

    // Pick a non-trivial cell with multiple raw pieces, all of them its tile's own:
    // a cell with open edges carries lips and railings too, which this does not measure.
    let pieces = &runtime.match_state.geometry.pieces;
    let (coord, cell_index) = catalog
        .cells
        .iter()
        .find(|(_, index)| {
            index.piece_indices.len() >= 10
                && index
                    .piece_indices
                    .iter()
                    .all(|&i| pieces[i].part == observed_match::hex_wfc::HexPiecePart::Authored)
        })
        .expect("must have a walled cell with >= 10 raw pieces");

    let raw_piece_count = cell_index.piece_indices.len();
    assert!(
        raw_piece_count >= 10,
        "precondition: cell has multiple raw collider pieces (got {raw_piece_count})"
    );

    let requested = BTreeSet::from([*coord]);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let spawned = shell::spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        &runtime,
        &catalog,
        &requested,
    );
    queue.apply(&mut world);

    assert_eq!(spawned.len(), 1);
    let child_pieces = spawned[0].child_pieces;

    let mut query = world.query::<(
        Entity,
        &ChildOf,
        Option<&Mesh3d>,
        Option<&PointLight>,
        &Name,
    )>();
    let structural_hull_mesh_count = query
        .iter(&world)
        .filter(|(_, child_of, mesh, _, name)| {
            child_of.parent() == spawned[0].entity
                && mesh.is_some()
                && name.as_str().starts_with("Hex cell")
        })
        .count();

    // The cell's 28 raw collider hull pieces were merged into exactly 9 mesh entities,
    // one per surface group, dropping structural hull entities by two thirds. It was a 24-piece cell until open edges: that cell
    // now opens onto the outside, carries lips and railings, and is no longer a
    // measurement of one tile's own hulls, so the walled cell measured here is the
    // next one along.
    assert_eq!(raw_piece_count, 28);
    assert_eq!(structural_hull_mesh_count, 9);
    assert_eq!(child_pieces, 13);
    assert!(
        structural_hull_mesh_count < raw_piece_count,
        "structural hull meshes ({structural_hull_mesh_count}) must be strictly less than raw pieces ({raw_piece_count})"
    );
}

#[test]
fn despawned_cell_rebuilds_identically_when_re_entered() {
    let runtime = test_runtime();
    let catalog = shell::HexGeometryCatalog::build(&runtime);
    let mut world = World::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);

    let coord = *catalog
        .cells
        .keys()
        .next()
        .expect("must have at least one cell in catalog");

    let requested = BTreeSet::from([coord]);

    // First spawn:
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let spawned1 = shell::spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        &runtime,
        &catalog,
        &requested,
    );
    queue.apply(&mut world);

    let first_entity = spawned1[0].entity;
    let first_count = spawned1[0].child_pieces;

    // Collect child entities state:
    let mut query = world.query::<(
        Entity,
        &ChildOf,
        Option<&Mesh3d>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        &Transform,
        Option<&super::spectate::Cutaway>,
        &Name,
    )>();
    let first_children: Vec<_> = query
        .iter(&world)
        .filter(|(_, child_of, ..)| child_of.parent() == first_entity)
        .map(|(_, _, mesh, mat, trans, cut, name)| {
            (
                mesh.map(|m| m.0.clone()),
                mat.map(|m| m.0.clone()),
                *trans,
                cut.copied(),
                name.clone(),
            )
        })
        .collect();

    assert_eq!(first_children.len(), first_count);

    // Despawn the cell:
    world.entity_mut(first_entity).despawn();

    // Verify all children were despawned with the parent:
    let mut child_query = world.query::<&ChildOf>();
    let surviving_children = child_query
        .iter(&world)
        .filter(|child_of| child_of.parent() == first_entity)
        .count();
    assert_eq!(
        surviving_children, 0,
        "all child entities must be despawned"
    );

    // Second spawn (cell re-entered):
    let mut commands = Commands::new(&mut queue, &world);
    let spawned2 = shell::spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        &runtime,
        &catalog,
        &requested,
    );
    queue.apply(&mut world);

    let second_entity = spawned2[0].entity;
    let second_count = spawned2[0].child_pieces;

    assert_eq!(
        first_count, second_count,
        "child piece count must match across rebuilds"
    );

    let second_children: Vec<_> = query
        .iter(&world)
        .filter(|(_, child_of, ..)| child_of.parent() == second_entity)
        .map(|(_, _, mesh, mat, trans, cut, name)| {
            (
                mesh.map(|m| m.0.clone()),
                mat.map(|m| m.0.clone()),
                *trans,
                cut.copied(),
                name.clone(),
            )
        })
        .collect();

    assert_eq!(first_children.len(), second_children.len());
    for (i, (mesh1, mat1, trans1, cut1, name1)) in first_children.iter().enumerate() {
        let (mesh2, mat2, trans2, cut2, name2) = &second_children[i];
        assert_eq!(
            mesh1, mesh2,
            "rebuilt mesh handle must match cached mesh handle at index {i}"
        );
        assert_eq!(
            mat1, mat2,
            "rebuilt material handle must match at index {i}"
        );
        assert_eq!(
            trans1.translation, trans2.translation,
            "rebuilt transform must match at index {i}"
        );
        assert_eq!(cut1, cut2, "rebuilt cutaway must match at index {i}");
        assert_eq!(name1, name2, "rebuilt name must match at index {i}");
    }
}
