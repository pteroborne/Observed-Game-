use super::*;
use crate::sim::{ArchitectLab, CardKind, TileShape};
use observed_hex::HexFace;

#[test]
fn viewport_never_sits_beneath_the_controls_at_supported_sizes() {
    for size in [
        Vec2::new(1200.0, 800.0),
        Vec2::new(1600.0, 1000.0),
        Vec2::new(2560.0, 1440.0),
    ] {
        let layout = WorkspaceLayout::for_window(size);
        assert_eq!(layout.map_offset.x, 0.0);
        assert_eq!(layout.map_offset.x + layout.map_size.x, size.x);
        assert_eq!(
            layout.map_offset.y + layout.map_size.y,
            size.y - layout.hand_height
        );
        assert!(layout.map_size.x > 700.0);
        assert!(layout.map_size.y > 480.0);
    }
}
#[test]
fn cursor_anchored_zoom_preserves_the_camera_plane_point() {
    let mut state = MapCameraState {
        viewport_offset: Vec2::new(232.0, 66.0),
        viewport_size: Vec2::new(1096.0, 680.0),
        units_per_pixel: 0.15,
        ..default()
    };
    let cursor = Vec2::new(900.0, 160.0);
    let offset = Vec2::new(cursor.x - 232.0 - 548.0, -(cursor.y - 66.0 - 340.0));
    let before = state.pan + offset * state.units_per_pixel;
    state.zoom_around(0.8, cursor);
    let after = state.pan + offset * 0.15 * state.zoom / DEFAULT_ZOOM;
    assert!(before.distance(after) < 0.0001);
}
#[test]
fn picking_uses_the_quantized_hex_and_only_the_active_floor() {
    let session = LabSession {
        sim: ArchitectLab::for_mode(ArchitectMode::DeepStack).unwrap(),
        ..default()
    };
    for target in session.sim.mutable_targets() {
        let center = board_position(session.sim.world.config, target);
        assert_eq!(pick_target(&session, center, target.level), Some(target));
    }
    assert_eq!(pick_target(&session, Vec3::splat(10_000.0), 0), None);
}
#[test]
fn floor_controls_wrap_and_recenter_without_changing_simulation() {
    let mut state = MapCameraState::default();
    state.change_floor(-1, 5);
    assert_eq!(state.floor, 4);
    state.pan = Vec2::ONE;
    state.zoom = 2.0;
    state.change_floor(1, 5);
    assert_eq!(state.floor, 0);
    assert_eq!(state.pan, Vec2::ZERO);
    assert_eq!(state.zoom, DEFAULT_ZOOM);
}
#[test]
fn authored_representatives_never_lie_about_card_ports() {
    let register = observed_content::ArchitectureRegister::Institutional;
    let mut matches = 0;
    for shape in TileShape::ALL {
        for rotation in 0..6 {
            let mask = shape.doors(rotation);
            if let Some(tile) = models::matching_tile(register, mask) {
                matches += 1;
                for face in HexFace::LATERAL {
                    assert_eq!(
                        tile.signature.port(face) != observed_hex::PortClass::Sealed,
                        mask & (1 << face.index()) != 0
                    );
                }
            }
        }
    }
    assert!(
        matches >= 24,
        "the hand must primarily use actual authored geometry"
    );
}
#[test]
fn unseen_or_illegal_targets_cannot_be_played_from_the_inspector() {
    let mut session = LabSession::default();
    let mut camera = MapCameraState::default();
    assert!(session.paused);
    assert!(!ui::can_submit(&session, &camera));
    let command = session.sim.legal_commands().into_iter().next().unwrap();
    let crate::sim::ArchitectCommand::Play {
        card,
        target,
        rotation,
    } = command
    else {
        panic!("play expected")
    };
    session.selected_card = session
        .sim
        .deck
        .hand
        .iter()
        .position(|c| c.id == card)
        .unwrap();
    session.rotation = rotation;
    session.select_target(target);
    camera.floor = target.level;
    assert!(matches!(
        session.sim.deck.hand[session.selected_card].kind,
        CardKind::Tile(_) | CardKind::Door
    ));
    assert!(ui::can_submit(&session, &camera));
    camera.details = true;
    assert!(!ui::can_submit(&session, &camera));
    camera.details = false;
    camera.lab_controls = true;
    assert!(!ui::can_submit(&session, &camera));
    camera.lab_controls = false;
    camera.floor = target.level + 1;
    assert!(!ui::can_submit(&session, &camera));
    camera.floor = target.level;
    session.sim.cooldown = 10;
    assert!(!ui::can_submit(&session, &camera));
    session.sim.cooldown = 0;
    session.selected_target = None;
    assert!(!ui::can_submit(&session, &camera));
}

#[test]
fn selection_keeps_its_coordinate_when_targets_change() {
    let mut session = LabSession::default();
    let targets = session.sim.mutable_targets();
    let target = *targets.last().unwrap();
    session.select_target(target);
    session.sim.known.remove(&targets[0]);
    assert_eq!(session.target(), Some(target));
}

#[test]
fn selection_snaps_to_tile_then_leaves_manual_framing_alone() {
    let mut session = LabSession::default();
    let mut camera = MapCameraState::default();
    assert!(
        camera.zoom < 1.0,
        "opening view should be closer than fit-to-deck"
    );
    let targets = session.sim.mutable_targets();
    let cell = targets[0];
    camera.floor = cell.level;
    session.select_target(cell);
    camera.sync_selection(&session);
    let tile = board_position(session.sim.world.config, cell) + Vec3::Y * 0.5;
    let screen_plane = (camera_rotation().inverse() * tile).truncate() - camera.pan;
    assert!(
        screen_plane.length() < 0.0001,
        "selected tile must project to viewport center"
    );
    camera.pan += Vec2::new(7.0, 3.0);
    camera.zoom = 0.4;
    let manually_framed = camera;
    camera.sync_selection(&session);
    assert_eq!(camera, manually_framed);
    session.select_target(targets[1]);
    camera.sync_selection(&session);
    assert_ne!(camera.pan, manually_framed.pan);
    assert_eq!(camera.zoom, DEFAULT_ZOOM);
    camera.center();
    camera.sync_selection(&session);
    assert_ne!(
        camera.pan,
        Vec2::ZERO,
        "focus should return to the selection"
    );
    camera.change_floor(1, 2);
    camera.sync_selection(&session);
    assert_eq!(
        camera.pan,
        Vec2::ZERO,
        "hidden-floor selection must not pull the camera"
    );
}

#[test]
fn contextual_panel_does_not_allow_click_through_to_board() {
    let mut session = LabSession::default();
    let mut camera = MapCameraState {
        viewport_offset: Vec2::new(0.0, 66.0),
        viewport_size: Vec2::new(1200.0, 554.0),
        ..default()
    };
    let point = Vec2::new(1100.0, 550.0);
    assert!(camera.accepts_board_pointer(point, &session));
    let cell = session.sim.mutable_targets()[0];
    session.select_target(cell);
    camera.floor = cell.level;
    assert!(camera.placement_visible(&session));
    assert!(!camera.accepts_board_pointer(point, &session));
    assert!(camera.accepts_board_pointer(Vec2::new(600.0, 300.0), &session));
    camera.details = true;
    assert!(!camera.placement_visible(&session));
    assert!(!camera.accepts_board_pointer(Vec2::new(600.0, 300.0), &session));
}

#[test]
fn opening_focus_uses_known_editable_geometry_without_arming_a_card() {
    let session = LabSession::default();
    let mut camera = MapCameraState::default();
    let cell = session.sim.mutable_targets()[0];
    camera.sync_selection(&session);
    let point = board_position(session.sim.world.config, cell) + Vec3::Y * 0.5;
    assert!(((camera_rotation().inverse() * point).truncate() - camera.pan).length() < 0.0001);
    assert_eq!(session.target(), None);
    assert!(!ui::can_submit(&session, &camera));
    camera.pan += Vec2::ONE;
    let manual = camera.pan;
    camera.sync_selection(&session);
    assert_eq!(camera.pan, manual);
}
