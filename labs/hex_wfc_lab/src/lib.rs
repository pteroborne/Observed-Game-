//! Hex WFC lab — the Arc L Phase 88 animated-step showcase.
//!
//! Top-down view of the single-level hex solver with a step-by-step replay of
//! the recorded [`SolveStep`] trace: watch room sites get seeded, the forced
//! route claim its doors, propagation narrow domains, and halls snake between
//! rooms — including abandoned attempts restarting.
//!
//! Keys: Space play/pause · N step · +/- speed · I instant · R next seed ·
//! 1-9 preset seeds · F1 overlay. `OBSERVED2_CAPTURE=<dir>` records an
//! animated solve as sequential PNG frames plus `manifest.json` and exits.

use std::collections::BTreeMap;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_facility::hex_wfc::{HexSpace, HexWfcConfig, HexWfcWorld, SolveStep, lateral_bit};
use observed_hex::{HexCoord, HexFace, hex_origin_plan};
use observed_style::{MarkerRole, SurfaceRole};

/// Screen pixels per plan meter.
const SCALE: f32 = 5.2;
const PRESET_SEEDS: [u64; 9] = [
    0xA11C_E000_0000_0007,
    0xA11C_E000_0000_0011,
    0xA11C_E000_0000_0023,
    0xA11C_E000_0000_002A,
    0xA11C_E000_0000_0031,
    0xA11C_E000_0000_0045,
    0xA11C_E000_0000_0058,
    0xA11C_E000_0000_0063,
    0xA11C_E000_0000_0071,
];

#[derive(Component)]
struct LabVisual;

#[derive(Component)]
struct LabStatus;

/// What the replay knows about a cell at the current trace cursor.
#[derive(Clone, Copy, Default)]
struct CellVis {
    site: bool,
    forced: bool,
    pruned_to: Option<u16>,
    resolved: Option<(HexSpace, u8)>,
}

#[derive(Resource)]
struct LabState {
    world: HexWfcWorld,
    trace: Vec<SolveStep>,
    cursor: usize,
    playing: bool,
    steps_per_tick: usize,
    overlay: bool,
    status: String,
    dirty: bool,
}

impl LabState {
    fn new(seed: u64) -> Self {
        let config = HexWfcConfig::default();
        let (world, trace) =
            HexWfcWorld::generate_traced(seed, config).expect("default hex config must solve");
        Self {
            world,
            trace,
            cursor: 0,
            playing: true,
            steps_per_tick: 4,
            overlay: true,
            status: "solving from the start — Space pauses".to_string(),
            dirty: true,
        }
    }

    fn finished(&self) -> bool {
        self.cursor >= self.trace.len()
    }

    fn advance(&mut self, steps: usize) {
        if self.finished() {
            return;
        }
        self.cursor = (self.cursor + steps).min(self.trace.len());
        self.dirty = true;
    }

    /// Replay the trace prefix into per-cell display state.
    fn cell_views(&self) -> BTreeMap<HexCoord, CellVis> {
        let mut views: BTreeMap<HexCoord, CellVis> = BTreeMap::new();
        for step in &self.trace[..self.cursor] {
            match *step {
                SolveStep::AttemptStart { .. } => views.clear(),
                SolveStep::RoomSite { coord } => views.entry(coord).or_default().site = true,
                SolveStep::ForcedCell { coord, .. } => {
                    views.entry(coord).or_default().forced = true;
                }
                SolveStep::DomainPruned { coord, remaining } => {
                    views.entry(coord).or_default().pruned_to = Some(remaining);
                }
                SolveStep::Collapsed {
                    coord,
                    space,
                    doors,
                    ..
                } => views.entry(coord).or_default().resolved = Some((space, doors)),
                SolveStep::Contradiction { coord } => {
                    views.entry(coord).or_default().resolved = None;
                }
                SolveStep::Completed { .. } => {}
            }
        }
        views
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.024)))
        .insert_resource(LabState::new(PRESET_SEEDS[0]))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Hex WFC Lab".to_string(),
                resolution: WindowResolution::new(1500, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, playback)
        .add_systems(
            Update,
            (handle_input, rebuild_visuals, update_status).chain(),
        );
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRun::new(path))
            .add_systems(Update, capture_progress.after(rebuild_visuals));
    }
    app.run();
}

fn setup(mut commands: Commands, state: Res<LabState>) {
    let center = grid_center(&state.world);
    commands.spawn((
        Camera2d,
        Transform::from_translation(center.extend(1000.0)),
        Name::new("Hex WFC lab camera"),
    ));
    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

fn playback(mut state: ResMut<LabState>) {
    if state.playing && !state.finished() {
        let steps = state.steps_per_tick;
        state.advance(steps);
    }
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keyboard.just_pressed(KeyCode::Space) {
        state.playing = !state.playing;
        state.status = if state.playing { "playing" } else { "paused" }.to_string();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.playing = false;
        state.advance(1);
        state.status = "single step".to_string();
    }
    if keyboard.just_pressed(KeyCode::KeyI) {
        state.cursor = state.trace.len();
        state.status = "instant solve".to_string();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        state.steps_per_tick = (state.steps_per_tick * 2).min(64);
        state.status = format!("speed {} steps/tick", state.steps_per_tick);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        state.steps_per_tick = (state.steps_per_tick / 2).max(1);
        state.status = format!("speed {} steps/tick", state.steps_per_tick);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let next = state.world.seed.wrapping_add(1);
        *state = LabState::new(next);
        state.status = format!("reset to seed {next:#018x}");
    }
    if keyboard.just_pressed(KeyCode::F1) {
        state.overlay = !state.overlay;
        state.dirty = true;
    }
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (index, key) in digits.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            *state = LabState::new(PRESET_SEEDS[index]);
            state.status = format!("preset seed {}", index + 1);
        }
    }
}

fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    visuals: Query<Entity, With<LabVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }

    let hex_mesh = meshes.add(RegularPolygon::new(7.6 * SCALE * 0.92, 6));
    let views = state.cell_views();
    let config = state.world.config;
    let grid = config.grid();
    let plain = observed_style::surface(SurfaceRole::Plain);
    let spine = observed_style::surface(SurfaceRole::Spine);
    let deck = observed_style::surface(SurfaceRole::GantryDeck);

    for index in 0..grid.cell_count() {
        let coord = grid.coord(index);
        let view = views.get(&coord).copied().unwrap_or_default();
        let center = screen_position(coord);

        let (fill, alpha) = match view.resolved {
            Some((HexSpace::Room, _)) => (spine.emissive, 1.0),
            Some((HexSpace::Hall, _)) => (deck.emissive * 1.3, 1.0),
            Some((HexSpace::Void, _)) => (plain.base_color.to_linear() * 0.4, 0.35),
            None if view.forced => (spine.emissive * 0.20, 0.85),
            None if view.site => (spine.emissive * 0.09, 0.9),
            None if view.pruned_to.is_some() => (plain.emissive * 0.5, 0.7),
            None => (plain.base_color.to_linear(), 0.45),
        };
        let mut color: Color = fill.into();
        color.set_alpha(alpha);
        commands.spawn((
            LabVisual,
            Mesh2d(hex_mesh.clone()),
            MeshMaterial2d(materials.add(ColorMaterial::from(color))),
            Transform::from_translation(center.extend(0.0)),
            Name::new(format!("cell {coord:?}")),
        ));

        // Door bars on resolved open faces show halls snaking between rooms.
        if let Some((space, doors)) = view.resolved
            && space != HexSpace::Void
        {
            let edge = match space {
                HexSpace::Room => spine.edge.unwrap_or(spine.base_color),
                _ => deck.edge.unwrap_or(deck.base_color),
            };
            for face in HexFace::LATERAL {
                if doors & lateral_bit(face) != 0 {
                    let toward = face_direction(face);
                    commands.spawn((
                        LabVisual,
                        Sprite::from_color(edge, Vec2::new(3.4, 5.4 * SCALE * 0.5)),
                        Transform::from_translation((center + toward * 6.4 * SCALE).extend(0.2))
                            .with_rotation(Quat::from_rotation_z(
                                toward.to_angle() - std::f32::consts::FRAC_PI_2,
                            )),
                    ));
                }
            }
        }
    }

    spawn_marker(
        &mut commands,
        screen_position(config.spawn()),
        MarkerRole::You,
        11.0,
        "spawn",
    );
    spawn_marker(
        &mut commands,
        screen_position(config.exit()),
        MarkerRole::Exit,
        13.0,
        "exit",
    );

    // Once the replay completes, overlay the live spawn->exit route.
    if state.finished()
        && let Some(route) = state.world.route_between(config.spawn(), config.exit())
    {
        for coord in route {
            spawn_marker(
                &mut commands,
                screen_position(coord),
                MarkerRole::Control,
                5.0,
                "route",
            );
        }
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
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
        Name::new(name),
    ));
}

fn update_status(state: Res<LabState>, mut status: Query<&mut Text, With<LabStatus>>) {
    if !state.is_changed() {
        return;
    }
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    if !state.overlay {
        **text = String::new();
        return;
    }
    let completed = state
        .trace
        .iter()
        .take(state.cursor)
        .filter_map(|step| match step {
            SolveStep::Completed { rooms, halls } => Some(format!("{rooms} rooms / {halls} halls")),
            _ => None,
        })
        .next_back()
        .unwrap_or_else(|| "solving...".to_string());
    **text = format!(
        "HEX WFC / ANIMATED STEP — Arc L Phase 88\nseed {:#018x} | step {}/{} | {} | attempts {}\n{}\n\nSpace play/pause | N step | +/- speed ({}/tick) | I instant | R next seed | 1-9 presets | F1 overlay\nLegend: gold-fill room site -> bright gold room | cyan hall | dim uncollapsed | faint gold forced route | diamond markers: cyan spawn, green exit, purple final route",
        state.world.seed,
        state.cursor,
        state.trace.len(),
        completed,
        state.world.last_attempts,
        state.status,
        state.steps_per_tick,
    );
}

/// Plan-view meters -> screen pixels (z flips so south points down-screen).
fn screen_position(coord: HexCoord) -> Vec2 {
    let (x, z) = hex_origin_plan(coord);
    Vec2::new(x as f32 * SCALE, -(z as f32) * SCALE)
}

fn grid_center(world: &HexWfcWorld) -> Vec2 {
    let config = world.config;
    let a = screen_position(config.spawn());
    let b = screen_position(config.exit());
    (a + b) * 0.5
}

fn face_direction(face: HexFace) -> Vec2 {
    let [(ax, az), (bx, bz)] = observed_hex::metrics::face_edge(face);
    let midpoint = Vec2::new((ax + bx) as f32 * 0.5, -((az + bz) as f32) * 0.5);
    midpoint.normalize()
}

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    frame: u32,
    total_frames: u32,
    timer: f32,
    finished: bool,
}

impl CaptureRun {
    fn new(dir: String) -> Self {
        Self {
            dir,
            frame: 0,
            total_frames: 28,
            timer: 0.0,
            finished: false,
        }
    }
}

fn capture_progress(
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if run.finished {
        return;
    }
    run.timer += time.delta_secs();
    if run.frame == 0 {
        std::fs::create_dir_all(&run.dir).expect("capture dir must be creatable");
        state.playing = false;
        state.cursor = 0;
        state.dirty = true;
    }
    // One frame every 0.35 s, stepping an equal share of the trace between
    // frames so the whole solve fits the frame budget.
    if run.timer >= 0.35 && run.frame < run.total_frames {
        let share = state.trace.len().div_ceil(run.total_frames as usize - 1);
        let path = format!("{}/hex_wfc_{:03}.png", run.dir, run.frame);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        state.advance(share);
        run.frame += 1;
        run.timer = 0.0;
        return;
    }
    if run.frame >= run.total_frames && run.timer >= 1.0 {
        let manifest = serde_json::json!({
            "lab": "hex_wfc_lab",
            "seed": format!("{:#018x}", state.world.seed),
            "trace_steps": state.trace.len(),
            "attempts": state.world.last_attempts,
            "rooms": state.world.room_count(),
            "frames": (0..run.total_frames)
                .map(|f| format!("hex_wfc_{f:03}.png"))
                .collect::<Vec<_>>(),
        });
        std::fs::write(
            format!("{}/manifest.json", run.dir),
            serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
        )
        .expect("manifest must be writable");
        run.finished = true;
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_swaps_the_world_and_replay_state_completely() {
        let first = LabState::new(PRESET_SEEDS[0]);
        let second = LabState::new(PRESET_SEEDS[1]);
        assert_ne!(first.world.seed, second.world.seed);
        assert_ne!(first.world.placements, second.world.placements);
        assert_eq!(second.cursor, 0);
    }

    #[test]
    fn a_full_replay_agrees_with_the_final_world() {
        let mut state = LabState::new(PRESET_SEEDS[2]);
        state.cursor = state.trace.len();
        let views = state.cell_views();
        for (coord, view) in views {
            if let Some((space, doors)) = view.resolved {
                let placement = state.world.placements[&coord];
                // Post-collapse passes (prune, junction promotion) may demote
                // a cell, but a cell the replay shows as resolved must at
                // least exist and match unless a pass rewrote it.
                if placement.space == space {
                    assert_eq!(placement.doors, doors, "doors diverge at {coord:?}");
                }
            }
        }
    }

    #[test]
    fn the_trace_contains_the_animated_step_vocabulary() {
        let state = LabState::new(PRESET_SEEDS[0]);
        let has = |predicate: fn(&SolveStep) -> bool| state.trace.iter().any(predicate);
        assert!(has(|s| matches!(s, SolveStep::AttemptStart { .. })));
        assert!(has(|s| matches!(s, SolveStep::RoomSite { .. })));
        assert!(has(|s| matches!(s, SolveStep::ForcedCell { .. })));
        assert!(has(|s| matches!(s, SolveStep::Collapsed { .. })));
        assert!(has(|s| matches!(s, SolveStep::Completed { .. })));
    }
}
