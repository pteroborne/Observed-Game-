use super::*;

#[test]
fn workspace_reserves_a_large_map_canvas() {
    let layout = WorkspaceLayout::for_window(Vec2::new(1600.0, 1000.0));
    assert_eq!(layout.sidebar_width, 304.0);
    assert_eq!(layout.hand_height, 270.0);
    assert_eq!(layout.map_size, Vec2::new(1296.0, 730.0));
    assert!(layout.map_size.x * layout.map_size.y > 900_000.0);
}

#[test]
fn cursor_anchored_zoom_preserves_the_world_point() {
    let mut state = MapCameraState {
        viewport_offset: Vec2::new(300.0, 0.0),
        viewport_size: Vec2::new(1300.0, 730.0),
        ..default()
    };
    let cursor = Vec2::new(1320.0, 180.0);
    let offset = Vec2::new(370.0, 185.0);
    let before = state.pan + offset * state.zoom;
    state.zoom_around(0.8, cursor);
    let after = state.pan + offset * state.zoom;
    assert!(before.distance(after) < 0.001);
}

#[test]
fn floors_read_as_one_ascending_diagonal() {
    let lower = floor_offset(0, 2);
    let upper = floor_offset(1, 2);
    assert!(upper.x > lower.x);
    assert!(upper.y > lower.y);
    assert_eq!(upper - lower, FLOOR_ASCENT);
}

#[test]
fn one_floor_mode_is_centered_and_gets_a_close_camera() {
    assert_eq!(floor_offset(0, 1), Vec2::ZERO);
    assert!(default_zoom(ArchitectMode::Pocket) < default_zoom(ArchitectMode::FullAscent));
}

#[test]
fn map_pointer_picks_the_visualized_hex() {
    let session = LabSession::default();
    let target = session.target().expect("the seeded board has targets");
    let visual_center = board_position(session.sim.world.config, target);
    assert_eq!(pick_target(&session, visual_center), Some(target));
    assert_eq!(pick_target(&session, Vec2::splat(10_000.0)), None);
}

#[test]
fn every_face_points_at_its_actual_neighbor_in_the_native_view() {
    let config = ArchitectMode::FullAscent.config();
    let center = HexCoord {
        q: 4,
        r: 4,
        level: 0,
    };
    for face in HexFace::LATERAL {
        let next = config.grid().neighbor(center, face).unwrap();
        let delta = (board_position(config, next) - board_position(config, center)).normalize();
        assert!(
            delta.distance(face_vector(face)) < 0.0001,
            "{face:?} points away from its neighbor"
        );
    }
}

#[test]
fn five_floors_read_as_distinct_ascending_registers_and_titles() {
    let mut registers = std::collections::HashSet::new();
    let mut titles = std::collections::HashSet::new();
    for level in 0..5 {
        let register = crate::sim::floor_register(level);
        let title = crate::sim::floor_title(level);
        assert!(!title.is_empty(), "floor {level} must have non-empty title");
        assert!(
            registers.insert(register),
            "duplicate register for floor {level}"
        );
        assert!(titles.insert(title), "duplicate title for floor {level}");
    }
}
