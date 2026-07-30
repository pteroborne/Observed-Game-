//! Tests for hex presentation setup, streaming residency, and teardown (see `mod.rs`).

use super::residency::{cell_in_stream_range, footprint_in_range, plan_residency};
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
    );
    assert!(absent.spawn.is_empty());

    let present = plan_residency(
        &catalog,
        &resident([edge]),
        focus_position,
        focus,
        CELL_SPAWN_BUDGET,
        CELL_DESPAWN_BUDGET,
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
    );
    assert!(plan.despawn.is_empty());
    assert_eq!(plan.desired_cells, 1);
}
