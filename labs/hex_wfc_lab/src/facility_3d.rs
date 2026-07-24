//! Phase 92 continuous 3D walkthrough projection for the solved hex facility.
mod bot_pov;
mod capture;
mod controls;
mod presentation;
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_authoring::{CompiledTileCatalog, Manifest, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass, TILE_LEVEL_HEIGHT, face_edge, hex_origin};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;
use observed_traversal::rapier_controller::RapierTraversalScene;
use observed_traversal::{FpsBody, FpsConfig};

use crate::LabState;

#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LabViewMode {
    Plan2d,
    Facility3d,
    Relayout2d,
}

#[derive(Component)]
struct FacilityVisual;

#[derive(Component)]
struct FacilityCamera;

#[derive(Component)]
struct FacilityStatus;

#[derive(SystemParam)]
struct ModePresentation<'w, 's> {
    plan_cameras: Query<'w, 's, &'static mut Camera, (With<Camera2d>, Without<FacilityCamera>)>,
    facility_cameras: Query<'w, 's, &'static mut Camera, With<FacilityCamera>>,
    status: Query<'w, 's, &'static mut Visibility, With<FacilityStatus>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraMode {
    Walk,
    FreeFly,
}

#[derive(Resource)]
struct FacilityState {
    source_seed: u64,
    source_generation: u32,
    source_config: observed_facility::hex_wfc::HexWfcConfig,
    prototypes: Vec<TilePrototype>,
    snapshot: HexWfcGeometrySnapshot,
    scene: RapierTraversalScene,
    body: FpsBody,
    config: FpsConfig,
    camera_mode: CameraMode,
    fly_position: Vec3,
    fly_yaw: f32,
    fly_pitch: f32,
    look: Vec2,
    collider_view: bool,
    overlay: bool,
    dirty: bool,
}

impl FacilityState {
    fn load(world: &HexWfcWorld) -> Self {
        let prototypes = load_tiles();
        let snapshot = HexWfcGeometrySnapshot::project(world, &prototypes)
            .unwrap_or_else(|error| panic!("hex facility projection failed: {error:?}"));
        let scene = snapshot.rapier_scene();
        let config = FpsConfig::deliberate_rapier();
        let (spawn, yaw) = spawn_pose(world, &config);
        Self {
            source_seed: world.seed,
            source_generation: world.generation,
            source_config: world.config,
            prototypes,
            snapshot,
            scene,
            body: FpsBody::spawned(spawn, yaw),
            config,
            camera_mode: CameraMode::Walk,
            fly_position: spawn + Vec3::Y * 12.0,
            fly_yaw: yaw,
            fly_pitch: -0.25,
            look: Vec2::ZERO,
            collider_view: false,
            overlay: true,
            dirty: true,
        }
    }

    fn rebuild(&mut self, world: &HexWfcWorld) {
        self.snapshot = HexWfcGeometrySnapshot::project(world, &self.prototypes)
            .unwrap_or_else(|error| panic!("hex facility projection failed: {error:?}"));
        self.scene = self.snapshot.rapier_scene();
        self.source_seed = world.seed;
        self.source_generation = world.generation;
        self.source_config = world.config;
        let (spawn, yaw) = spawn_pose(world, &self.config);
        self.body = FpsBody::spawned(spawn, yaw);
        self.fly_position = spawn + Vec3::Y * 12.0;
        self.fly_yaw = yaw;
        self.fly_pitch = -0.25;
        self.dirty = true;
    }
}

pub(crate) fn register(app: &mut App) {
    app.insert_resource(LabViewMode::Plan2d)
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            controls::step_walk.run_if(facility_mode_active.and(bot_pov_inactive)),
        )
        .add_systems(
            Update,
            (
                toggle_mode,
                controls::handle_input,
                controls::reset_world,
                presentation::rebuild_geometry,
                sync_camera,
                update_status,
            )
                .chain(),
        );
    if let Ok(path) = std::env::var("OBSERVED2_HEX_3D_CAPTURE") {
        app.insert_resource(capture::CaptureRun::new(path))
            .add_systems(Update, capture::capture_progress.after(sync_camera));
    }
    if std::env::var("OBSERVED2_HEX_BOT_POV").is_ok() {
        app.add_systems(Startup, bot_pov::setup.after(setup))
            .add_systems(Update, bot_pov::run.after(sync_camera));
    }
}

/// True unless a bot-POV capture is driving the walk camera, which owns the
/// body pose itself and must not be overwritten by the free-walk stepper.
fn bot_pov_inactive() -> bool {
    std::env::var("OBSERVED2_HEX_BOT_POV").is_err()
}

pub(crate) fn plan_or_relayout_mode_active(mode: Res<LabViewMode>) -> bool {
    *mode != LabViewMode::Facility3d
}

fn facility_mode_active(mode: Res<LabViewMode>) -> bool {
    *mode == LabViewMode::Facility3d
}

fn tile_dir() -> PathBuf {
    let workspace = PathBuf::from("assets/tiles");
    if workspace.exists() {
        workspace
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
    }
}

fn load_tiles() -> Vec<TilePrototype> {
    let base = tile_dir();
    // Legacy manifest entries plus the strict compiled catalog: since the
    // corpus went all-strict (`register_scope: all`), the compatibility
    // manifest is empty and every runtime cell lives in the catalog.
    let mut cells = Manifest::load(&base.join("manifest.ron"))
        .expect("tile manifest loads")
        .load_tiles(&base)
        .expect("tile prototypes validate");
    let text =
        std::fs::read_to_string(base.join("compiled_catalog.ron")).expect("compiled catalog loads");
    let compiled = CompiledTileCatalog::from_ron(&text).expect("compiled catalog parses");
    let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
    let strict = compiled
        .runtime_catalog(&slugs)
        .expect("compiled catalog expands");
    cells.extend(strict.cells);
    cells
}

fn spawn_pose(world: &HexWfcWorld, config: &FpsConfig) -> (Vec3, f32) {
    let coord = world.config.spawn();
    let origin = Vec3::from_array(hex_origin(coord));
    let facing = world
        .route_between(coord, world.config.exit())
        .and_then(|route| route.get(1).copied())
        .map(|next| {
            let delta = Vec3::from_array(hex_origin(next)) - origin;
            Vec2::new(delta.x, delta.z).normalize_or_zero()
        })
        .unwrap_or(Vec2::X);
    (
        origin + Vec3::new(0.0, config.half_height + 0.5, 0.0),
        facing.x.atan2(-facing.y),
    )
}

fn setup(
    mut commands: Commands,
    world: Res<LabState>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let state = FacilityState::load(&world.world);
    let camera_entity = commands
        .spawn((
            FacilityCamera,
            Camera3d::default(),
            Camera {
                is_active: false,
                ..default()
            },
            Transform::from_translation(state.body.position),
            Name::new("Hex facility walkthrough camera"),
        ))
        .with_children(|camera| {
            camera.spawn((
                PointLight {
                    intensity: 450_000.0,
                    range: 48.0,
                    color: Color::srgb(0.82, 0.91, 1.0),
                    shadows_enabled: false,
                    ..default()
                },
                Name::new("Walkthrough headlamp"),
            ));
        })
        .id();
    commands.spawn((
        FacilityVisual,
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.7, 0.0)),
        Name::new("Hex facility key light"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.45, 0.58, 0.80),
        brightness: 80.0,
        ..default()
    });
    let overlay_background = observed_style::surface(observed_style::SurfaceRole::Plain)
        .base_color
        .with_alpha(0.88);
    commands.spawn((
        FacilityStatus,
        UiTargetCamera(camera_entity),
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        BackgroundColor(overlay_background),
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(16.0),
            max_width: Val::Px(980.0),
            padding: UiRect::all(Val::Px(9.0)),
            ..default()
        },
    ));
    if std::env::var("OBSERVED2_HEX_3D_CAPTURE").is_err()
        && std::env::var("OBSERVED2_CAPTURE").is_err()
        && let Ok(mut cursor) = cursors.single_mut()
    {
        cursor.grab_mode = CursorGrabMode::Confined;
        cursor.visible = false;
    }
    commands.insert_resource(state);
}

fn toggle_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<LabViewMode>,
    world: Res<LabState>,
    mut facility: ResMut<FacilityState>,
    mut cancel_relayout: MessageWriter<crate::relayout_demo::CancelRelayout>,
    mut presentation: ModePresentation,
) {
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }
    let leaving_relayout = *mode == LabViewMode::Relayout2d;
    *mode = match *mode {
        LabViewMode::Plan2d => LabViewMode::Facility3d,
        LabViewMode::Facility3d => LabViewMode::Plan2d,
        LabViewMode::Relayout2d => LabViewMode::Facility3d,
    };
    if *mode == LabViewMode::Facility3d {
        synchronize_if_stale(&world.world, &mut facility);
    }
    if leaving_relayout {
        cancel_relayout.write(Default::default());
    }
    for mut camera in &mut presentation.plan_cameras {
        camera.is_active = *mode == LabViewMode::Plan2d;
    }
    for mut camera in &mut presentation.facility_cameras {
        camera.is_active = *mode == LabViewMode::Facility3d;
    }
    for mut visibility in &mut presentation.status {
        *visibility = if *mode == LabViewMode::Facility3d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn synchronize_if_stale(world: &HexWfcWorld, state: &mut FacilityState) -> bool {
    if state.source_seed == world.seed
        && state.source_generation == world.generation
        && state.source_config == world.config
    {
        return false;
    }
    state.rebuild(world);
    true
}

fn sync_camera(state: Res<FacilityState>, mut camera: Query<&mut Transform, With<FacilityCamera>>) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let (position, yaw, pitch) = match state.camera_mode {
        CameraMode::Walk => (
            state.body.eye(&state.config),
            state.body.yaw,
            state.body.pitch,
        ),
        CameraMode::FreeFly => (state.fly_position, state.fly_yaw, state.fly_pitch),
    };
    transform.translation = position;
    transform.rotation = Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch);
}

fn update_status(
    mode: Res<LabViewMode>,
    state: Res<FacilityState>,
    mut status: Query<&mut Text, With<FacilityStatus>>,
) {
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    if *mode != LabViewMode::Facility3d || !state.overlay {
        **text = String::new();
        return;
    }
    let eye = match state.camera_mode {
        CameraMode::Walk => state.body.eye(&state.config),
        CameraMode::FreeFly => state.fly_position,
    };
    **text = format!(
        "HEX WFC / 3D FACILITY â€” Arc L Phase 92\nseed {:#018x} | {}x{}x{} | pieces/colliders {} | rooms {} | ramp heads {}\nmode {:?} | eye {:.1?} | grounded {} | collider debug {} | scale 8m/level, {:.0}m floor span\n\nF3 2D/3D | V walk/free-fly | WASD+mouse | E/Q fly vertical | Shift sprint | Space jump | C exact collider facets (alternating light/dark) | R next solve | Backspace respawn | F1 overlay\nLegend: gold room | blue hall | bright cyan ramp | orange wellshaft | dark shell",
        state.source_seed,
        state.source_config.cols,
        state.source_config.rows,
        state.source_config.levels,
        state.snapshot.pieces.len(),
        state.snapshot.blueprint_instances,
        state.snapshot.ramp_heads,
        state.camera_mode,
        eye,
        state.body.grounded,
        state.collider_view,
        f32::from(state.source_config.levels.saturating_sub(1)) * TILE_LEVEL_HEIGHT,
    );
}

fn face_plan_dir(face: HexFace) -> Vec2 {
    let [a, b] = face_edge(face);
    Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5).normalize()
}

fn ramp_exit(world: &HexWfcWorld, coord: HexCoord) -> Option<HexFace> {
    let placement = world.placements.get(&coord)?;
    (placement.archetype == HexArchetype::RampUp)
        .then(|| {
            HexFace::LATERAL
                .into_iter()
                .find(|&face| placement.is_open(face))
                .map(HexFace::opposite)
        })
        .flatten()
}

#[derive(Clone, Copy, Debug)]
struct ShaftColumn {
    top: HexCoord,
    bottom: HexCoord,
    door: HexFace,
    cells: u8,
}

impl ShaftColumn {
    fn structural_depth_m(self) -> f32 {
        f32::from(self.cells) * TILE_LEVEL_HEIGHT
    }

    fn rim_drop_m(self) -> f32 {
        f32::from(self.cells.saturating_sub(1)) * TILE_LEVEL_HEIGHT
    }
}

fn shaft_view(world: &HexWfcWorld) -> Option<ShaftColumn> {
    world
        .placements
        .values()
        .filter(|placement| placement.archetype == HexArchetype::Shaft)
        .filter_map(|placement| {
            let door = HexFace::LATERAL
                .into_iter()
                .find(|&face| placement.is_open(face))?;
            let mut bottom = placement.coord;
            let mut cells = 1u8;
            while world.placements[&bottom].down == PortClass::ShaftOpen {
                let next = world.config.grid().neighbor(bottom, HexFace::Down)?;
                let next_placement = world.placements.get(&next)?;
                if next_placement.archetype != HexArchetype::Shaft
                    || next_placement.up != PortClass::ShaftOpen
                {
                    break;
                }
                bottom = next;
                cells += 1;
            }
            (cells >= 5).then_some(ShaftColumn {
                top: placement.coord,
                bottom,
                door,
                cells,
            })
        })
        .max_by_key(|column| (column.cells, column.top.level))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_reprojects_and_restores_the_walk_spawn() {
        let first = LabState::new(super::super::PRESET_SEEDS[0]);
        let second = LabState::new(super::super::PRESET_SEEDS[1]);
        let mut state = FacilityState::load(&first.world);
        state.body.position += Vec3::splat(9.0);
        state.rebuild(&second.world);
        assert_eq!(state.snapshot.generation, second.world.generation);
        assert_eq!(state.body.position, state.body.spawn);
        assert_eq!(state.scene.collider_count(), state.snapshot.pieces.len());
    }

    #[test]
    fn shared_four_level_fixture_has_a_ramp_and_full_height_shaft() {
        let lab = LabState::new(super::super::PRESET_SEEDS[0]);
        assert!(
            lab.world
                .placements
                .keys()
                .copied()
                .any(|coord| ramp_exit(&lab.world, coord).is_some())
        );
        let longest_shaft = lab
            .world
            .placements
            .values()
            .filter(|placement| {
                placement.archetype == HexArchetype::Shaft && placement.up != PortClass::ShaftOpen
            })
            .map(|placement| {
                let mut coord = placement.coord;
                let mut cells = 1u8;
                while lab.world.placements[&coord].down == PortClass::ShaftOpen {
                    coord = lab
                        .world
                        .config
                        .grid()
                        .neighbor(coord, HexFace::Down)
                        .expect("shaft stays in grid");
                    cells += 1;
                }
                cells
            })
            .max()
            .unwrap_or(0);
        assert_eq!(longest_shaft, lab.world.config.levels);
    }

    #[test]
    fn entering_3d_after_relayout_synchronizes_generation_pieces_and_arena() {
        use observed_facility::hex_wfc::{HexObservationFrame, HexRelayoutProgress};

        let mut lab = LabState::new(super::super::PRESET_SEEDS[0]);
        let mut state = FacilityState::load(&lab.world);
        lab.world.config.retry_budget = 1;
        let observation = HexObservationFrame::default();
        let work = lab.world.begin_relayout(&observation);
        let candidate = match lab.world.advance_relayout(work).expect("advance") {
            HexRelayoutProgress::Ready(candidate) => candidate,
            HexRelayoutProgress::Pending(_) => panic!("retry budget one finishes"),
        };
        lab.world
            .commit_relayout(candidate, &observation)
            .expect("commit");
        assert!(synchronize_if_stale(&lab.world, &mut state));
        let expected = HexWfcGeometrySnapshot::project(&lab.world, &state.prototypes)
            .expect("fresh projection");
        assert_eq!(state.source_generation, lab.world.generation);
        assert_eq!(state.snapshot.pieces, expected.pieces);
        assert_eq!(state.snapshot.arena, expected.arena);
        assert!(!synchronize_if_stale(&lab.world, &mut state));
    }
}
