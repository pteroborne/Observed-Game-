use std::collections::{BTreeMap, BTreeSet};

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_facility::full_wfc::{
    CellCoord, FullWfcConfig, FullWfcWorld, ModuleFace, ModuleSpace, ObservationFrame, ThresholdKey,
};
use observed_style::{MarkerRole, SurfaceRole};
use player_input::PlayerId;

const CELL: f32 = 42.0;
const LEVEL_GAP: f32 = 390.0;
const PLAYER: PlayerId = PlayerId(0);

#[derive(Component)]
struct LabVisual;

#[derive(Component)]
struct LabStatus;

#[derive(Resource)]
struct LabState {
    world: FullWfcWorld,
    player: CellCoord,
    observed_face: Option<ModuleFace>,
    paused: bool,
    ticks_until_pulse: u32,
    status: String,
    dirty: bool,
}

impl LabState {
    fn new(seed: u64) -> Self {
        let config = FullWfcConfig::default();
        let world = FullWfcWorld::new(seed, config).expect("default full-WFC config must solve");
        Self {
            player: config.spawn(),
            observed_face: None,
            ticks_until_pulse: config.pulse_ticks,
            world,
            paused: false,
            status: "initial solve".to_string(),
            dirty: true,
        }
    }

    fn observation(&self) -> ObservationFrame {
        let mut frame = ObservationFrame {
            visible_cells: BTreeSet::from([self.player]),
            visible_thresholds: BTreeSet::new(),
            occupied_cells: BTreeMap::from([(PLAYER, self.player)]),
        };
        if let Some(face) = self.observed_face {
            frame.visible_thresholds.insert(ThresholdKey {
                room: self.player,
                face,
            });
        }
        frame
    }

    fn pulse(&mut self) {
        let frame = self.observation();
        match self.world.propose_relayout(&frame) {
            Ok(candidate) => match self.world.commit_relayout(candidate, frame) {
                Ok(()) => {
                    self.status = format!(
                        "pulse accepted: generation {} in {} attempt(s)",
                        self.world.generation, self.world.last_attempts
                    );
                }
                Err(error) => self.status = format!("pulse rejected: {error:?}"),
            },
            Err(error) => {
                self.status = format!(
                    "pulse solve failed after {} attempt(s): {error:?}",
                    self.world.config.retry_budget
                );
            }
        }
        self.ticks_until_pulse = self.world.config.pulse_ticks;
        self.dirty = true;
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.024)))
        .insert_resource(LabState::new(0xF011_FAC1_1177))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Full WFC Lab".to_string(),
                resolution: WindowResolution::new(1500, 880),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, timed_pulse)
        .add_systems(
            Update,
            (handle_input, rebuild_visuals, update_status).chain(),
        );
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress.after(rebuild_visuals));
    }
    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        state.paused = true;
        state.pulse();
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.8 {
        exit.write(AppExit::Success);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(-20.0, 0.0, 1000.0),
        Name::new("Full WFC lab camera"),
    ));
    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(18.0),
            ..default()
        },
    ));
}

fn timed_pulse(mut state: ResMut<LabState>) {
    if state.paused {
        return;
    }
    state.ticks_until_pulse = state.ticks_until_pulse.saturating_sub(1);
    if state.ticks_until_pulse == 0 {
        state.pulse();
    }
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let next_seed = state.world.seed.wrapping_add(1);
        *state = LabState::new(next_seed);
        state.status = format!("reset to seed {next_seed:#018x}");
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.paused = !state.paused;
        state.status = if state.paused {
            "pulses paused"
        } else {
            "pulses resumed"
        }
        .to_string();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        state.pulse();
    }

    let observed = [
        (KeyCode::Digit1, ModuleFace::East),
        (KeyCode::Digit2, ModuleFace::West),
        (KeyCode::Digit3, ModuleFace::South),
        (KeyCode::Digit4, ModuleFace::North),
        (KeyCode::Digit5, ModuleFace::Up),
        (KeyCode::Digit6, ModuleFace::Down),
    ]
    .into_iter()
    .find_map(|(key, face)| keyboard.just_pressed(key).then_some(face));
    if let Some(face) = observed {
        state.observed_face = Some(face);
        let frame = state.observation();
        state.world.update_observation(frame);
        state.status = format!("observing {} threshold", face_label(face));
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Digit0) {
        state.observed_face = None;
        let frame = state.observation();
        state.world.update_observation(frame);
        state.status = "threshold released".to_string();
        state.dirty = true;
    }

    let movement = if keyboard.just_pressed(KeyCode::ArrowRight) {
        Some(ModuleFace::East)
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        Some(ModuleFace::West)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        Some(ModuleFace::South)
    } else if keyboard.just_pressed(KeyCode::ArrowUp) {
        Some(ModuleFace::North)
    } else if keyboard.just_pressed(KeyCode::PageUp) {
        Some(ModuleFace::Up)
    } else if keyboard.just_pressed(KeyCode::PageDown) {
        Some(ModuleFace::Down)
    } else {
        None
    };
    if let Some(face) = movement {
        let current = state.player;
        let next = state.world.config.neighbor(current, face);
        let open = state
            .world
            .placements
            .get(&current)
            .is_some_and(|placement| placement.is_open(face));
        if let Some(next) = next.filter(|_| open) {
            state.player = next;
            state.observed_face = None;
            let frame = state.observation();
            state.world.update_observation(frame);
            state.status = format!("player entered {next:?}");
            state.dirty = true;
        } else {
            state.status = format!("{} is sealed", face_label(face));
            state.dirty = true;
        }
    }
}

fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    visuals: Query<Entity, With<LabVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }

    let config = state.world.config;
    let route = state
        .world
        .route(state.player)
        .map(|route| route.cells.into_iter().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let pins = state.world.pinned_cells();
    for placement in state.world.placements.values() {
        let center = screen_position(config, placement.coord);
        if placement.space == ModuleSpace::Void {
            continue;
        }
        let role = if route.contains(&placement.coord) {
            SurfaceRole::Spine
        } else if placement.space == ModuleSpace::Room {
            SurfaceRole::Plain
        } else {
            SurfaceRole::GantryDeck
        };
        let treatment = observed_style::surface(role);
        let size = if placement.space == ModuleSpace::Room {
            31.0
        } else {
            22.0
        };
        commands.spawn((
            LabVisual,
            Sprite::from_color(treatment.base_color, Vec2::splat(size)),
            Transform::from_translation(center.extend(0.0)),
            Name::new(format!("{:?} {:?}", placement.space, placement.coord)),
        ));
        for face in ModuleFace::ALL {
            if placement.is_open(face) {
                let delta = face_offset(face) * (CELL * 0.38);
                commands.spawn((
                    LabVisual,
                    Sprite::from_color(
                        treatment.edge.unwrap_or(treatment.base_color),
                        if matches!(face, ModuleFace::East | ModuleFace::West) {
                            Vec2::new(CELL * 0.35, 3.0)
                        } else {
                            Vec2::new(3.0, CELL * 0.35)
                        },
                    ),
                    Transform::from_translation((center + delta).extend(0.1)),
                ));
            }
        }
        if pins.contains(&placement.coord) {
            commands.spawn((
                LabVisual,
                Sprite::from_color(
                    observed_style::marker(MarkerRole::Control).base_color,
                    Vec2::new(size + 7.0, 3.0),
                ),
                Transform::from_translation((center + Vec2::Y * (size * 0.62)).extend(0.2)),
            ));
        }
    }

    spawn_marker(
        &mut commands,
        screen_position(config, config.exit()),
        MarkerRole::Exit,
        14.0,
        "exit",
    );
    spawn_marker(
        &mut commands,
        screen_position(config, state.player),
        MarkerRole::You,
        12.0,
        "player 0",
    );
    if let Some(face) = state.observed_face {
        let center = screen_position(config, state.player) + face_offset(face) * 20.0;
        spawn_marker(
            &mut commands,
            center,
            MarkerRole::Director,
            7.0,
            "observed threshold",
        );
    }
    for level in 0..config.levels {
        commands.spawn((
            LabVisual,
            Text2d::new(format!("LEVEL {level}")),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.58, 0.72, 0.9)),
            Transform::from_xyz(
                f32::from(level) * LEVEL_GAP - LEVEL_GAP,
                -(f32::from(config.rows) * CELL * 0.5 + 34.0),
                0.5,
            ),
        ));
    }
    state.dirty = false;
}

fn spawn_marker(
    commands: &mut Commands,
    position: Vec2,
    role: MarkerRole,
    size: f32,
    name: &'static str,
) {
    let treatment = observed_style::marker(role);
    commands.spawn((
        LabVisual,
        Sprite::from_color(treatment.base_color, Vec2::splat(size)),
        Transform::from_translation(position.extend(1.0))
            .with_rotation(Quat::from_rotation_z(0.785)),
        Name::new(name),
    ));
}

fn screen_position(config: FullWfcConfig, coord: CellCoord) -> Vec2 {
    let level_origin = f32::from(coord.level) * LEVEL_GAP - LEVEL_GAP;
    Vec2::new(
        level_origin + (f32::from(coord.x) - f32::from(config.cols - 1) * 0.5) * CELL,
        (f32::from(config.rows - 1) * 0.5 - f32::from(coord.z)) * CELL,
    )
}

fn face_offset(face: ModuleFace) -> Vec2 {
    match face {
        ModuleFace::East => Vec2::X,
        ModuleFace::West => Vec2::NEG_X,
        ModuleFace::South => Vec2::NEG_Y,
        ModuleFace::North => Vec2::Y,
        ModuleFace::Up => Vec2::new(0.7, 0.7),
        ModuleFace::Down => Vec2::new(-0.7, -0.7),
    }
}

fn face_label(face: ModuleFace) -> &'static str {
    match face {
        ModuleFace::East => "east",
        ModuleFace::West => "west",
        ModuleFace::South => "south",
        ModuleFace::North => "north",
        ModuleFace::Up => "up",
        ModuleFace::Down => "down",
    }
}

fn update_status(state: Res<LabState>, mut status: Query<&mut Text, With<LabStatus>>) {
    if !state.is_changed() {
        return;
    }
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    let route = state.world.route(state.player);
    let candle = state.world.candle_proximity(state.player);
    let observed = state.observed_face.map_or("none", face_label);
    **text = format!(
        "FULL WFC / CONTINUOUS FACILITY - simulation lab\nseed {:#018x} | generation {} | retry {} | pulse in {} ticks{}\nplayer {:?} | route {} | candle {:.0}% | observed threshold {}\n{}\n\nArrows move | PgUp/PgDn climb | 1-6 observe | 0 release | Space pulse | P pause | R reset\nLegend: cyan player | green exit | gold A* route | purple pinned | magenta observed threshold",
        state.world.seed,
        state.world.generation,
        state.world.last_attempts,
        state.ticks_until_pulse,
        if state.paused { " (paused)" } else { "" },
        state.player,
        route.map_or("MISSING".to_string(), |route| format!(
            "{} cells / {} cost",
            route.cells.len(),
            route.cost_millis
        )),
        candle * 100.0,
        observed,
        state.status,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_replaces_the_world_without_changing_player_identity() {
        let first = LabState::new(7);
        let second = LabState::new(8);
        assert_eq!(first.player, first.world.config.spawn());
        assert_eq!(second.player, second.world.config.spawn());
        assert_ne!(first.world.seed, second.world.seed);
        assert_eq!(PLAYER, PlayerId(0));
    }

    #[test]
    fn observation_frame_is_occupancy_and_threshold_shaped() {
        let mut state = LabState::new(9);
        state.observed_face = Some(ModuleFace::East);
        let frame = state.observation();
        assert_eq!(frame.occupied_cells[&PLAYER], state.player);
        assert!(frame.visible_cells.contains(&state.player));
        assert!(frame.visible_thresholds.contains(&ThresholdKey {
            room: state.player,
            face: ModuleFace::East,
        }));
    }
}
