//! Hex WFC lab — the Arc L Phase 90 animated-step showcase.

mod capture;
mod facility_3d;
mod plan_input;
mod relayout_demo;

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use observed_facility::hex_wfc::blueprint;
use observed_facility::hex_wfc::{
    HexSpace, HexWfcConfig, HexWfcWorld, PortClass, SolveStep, lateral_bit,
};
use observed_hex::{HexCoord, HexFace, hex_origin_plan};
use observed_style::{MarkerRole, SurfaceRole};

/// Screen pixels per plan meter.
const SCALE: f32 = 5.2;
const PRESET_SEEDS: [u64; 9] = [
    // Preset 1 is the pinned 3D showcase seed: full-height wellshaft + ramp chain.
    0xA11C_E3D0_0000_0008,
    0xA11C_E3D0_0000_0011,
    0xA11C_E3D0_0000_0023,
    0xA11C_E3D0_0000_002A,
    0xA11C_E3D0_0000_0031,
    0xA11C_E3D0_0000_0045,
    0xA11C_E3D0_0000_0058,
    0xA11C_E3D0_0000_0063,
    0xA11C_E3D0_0000_0071,
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
    resolved: Option<(HexSpace, u8, PortClass, PortClass)>,
    role: Option<observed_facility::map_spec::RoomRole>,
}

#[derive(Resource)]
pub(crate) struct LabState {
    pub(crate) world: HexWfcWorld,
    pub(crate) trace: Vec<SolveStep>,
    pub(crate) cursor: usize,
    pub(crate) playing: bool,
    steps_per_tick: usize,
    overlay: bool,
    status: String,
    pub(crate) dirty: bool,
    pub(crate) current_level: u8,
}

impl LabState {
    fn new(seed: u64) -> Self {
        let config = HexWfcConfig {
            cols: 12,
            rows: 9,
            levels: 4, // accepted Phase 90/P93 plan-view fixture
            min_rooms: 4,
            max_rooms: 8,
            retry_budget: 100,
            min_room_distance: 2,
        };
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
            current_level: 0,
        }
    }

    fn finished(&self) -> bool {
        self.cursor >= self.trace.len()
    }

    pub(crate) fn advance(&mut self, steps: usize) {
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
                SolveStep::BlueprintCell { coord, role } => {
                    let cell_vis = views.entry(coord).or_default();
                    cell_vis.site = true;
                    cell_vis.role = Some(role);
                }
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
                    up,
                    down,
                    ..
                } => {
                    views.entry(coord).or_default().resolved = Some((space, doors, up, down));
                }
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
        .add_systems(
            FixedUpdate,
            playback.run_if(facility_3d::plan_or_relayout_mode_active),
        )
        .add_systems(
            Update,
            (plan_input::handle, rebuild_visuals, update_status)
                .chain()
                .before(relayout_demo::cancel_requested)
                .run_if(facility_3d::plan_or_relayout_mode_active),
        );
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(capture::CaptureRun::new(path))
            .add_systems(Update, capture::capture_progress.after(rebuild_visuals));
    }
    facility_3d::register(&mut app);
    relayout_demo::register(&mut app);
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
    let bypass = observed_style::surface(SurfaceRole::SafeBypass);
    let stone = observed_style::surface(SurfaceRole::WellshaftStone);

    for index in 0..grid.cell_count() {
        let coord = grid.coord(index);
        if coord.level != state.current_level {
            continue;
        }

        let view = views.get(&coord).copied().unwrap_or_default();
        let center = screen_position(coord);

        let fill = match view.resolved {
            Some((HexSpace::Room, _, _, _)) => spine.emissive,
            Some((HexSpace::Hall, _, up, down)) => {
                if up == PortClass::RampOpen || down == PortClass::RampOpen {
                    bypass.emissive
                } else if up == PortClass::ShaftOpen || down == PortClass::ShaftOpen {
                    stone.emissive * 3.0
                } else {
                    deck.emissive * 1.3
                }
            }
            Some((HexSpace::Void, _, _, _)) => plain.base_color.to_linear() * 0.4,
            None if view.forced => spine.emissive * 0.20,
            None if view.site => spine.emissive * 0.09,
            None if view.pruned_to.is_some() => plain.emissive * 0.5,
            None => plain.base_color.to_linear(),
        };
        let alpha = match view.resolved {
            Some((HexSpace::Void, _, _, _)) => 0.35,
            _ => 1.0,
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

        // Draw lateral doors
        if let Some((space, doors, up, down)) = view.resolved
            && space != HexSpace::Void
        {
            let edge = match space {
                HexSpace::Room => spine.edge.unwrap_or(spine.base_color),
                _ => {
                    if up == PortClass::RampOpen || down == PortClass::RampOpen {
                        bypass.edge.unwrap_or(Color::srgb(0.2, 0.9, 1.0))
                    } else if up == PortClass::ShaftOpen || down == PortClass::ShaftOpen {
                        stone.edge.unwrap_or(Color::srgb(0.85, 0.5, 0.25))
                    } else {
                        deck.edge.unwrap_or(deck.base_color)
                    }
                }
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

            // Draw vertical indicators
            let mut label = String::new();
            if up == PortClass::RampOpen {
                label.push_str("+R");
            }
            if down == PortClass::RampOpen {
                label.push_str("-R");
            }
            if up == PortClass::ShaftOpen {
                label.push_str("+S");
            }
            if down == PortClass::ShaftOpen {
                label.push_str("-S");
            }
            if !label.is_empty() {
                commands.spawn((
                    LabVisual,
                    Text2d::new(label),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(edge),
                    Transform::from_translation(center.extend(0.5)),
                ));
            }
        }

        // Draw blueprint label if present
        if let Some(role) = view.role {
            let label = blueprint::blueprint_for_role(role).name;
            commands.spawn((
                LabVisual,
                Text2d::new(label),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.08, 0.10, 0.15)),
                Transform::from_translation(center.extend(0.6)),
            ));
        }
    }

    if state.current_level == config.spawn().level {
        spawn_marker(
            &mut commands,
            screen_position(config.spawn()),
            MarkerRole::You,
            11.0,
            "spawn",
        );
    }
    if state.current_level == config.exit().level {
        spawn_marker(
            &mut commands,
            screen_position(config.exit()),
            MarkerRole::Exit,
            13.0,
            "exit",
        );
    }

    // Once replay completes, overlay the route
    if state.finished()
        && let Some(route) = state.world.route_between(config.spawn(), config.exit())
    {
        for coord in route {
            if coord.level == state.current_level {
                spawn_marker(
                    &mut commands,
                    screen_position(coord),
                    MarkerRole::Control,
                    5.0,
                    "route",
                );
            }
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
        "HEX WFC / ANIMATED STEP — Arc L Phase 90\nseed {:#018x} | step {}/{} | {} | attempts {}\n{}\nlevel {}/{} (PgUp/PgDn or [/] to slice)\n\nSpace play/pause | N step | +/- speed ({}/tick) | I instant | R next seed | 1-9 presets | F1 overlay\nLegend: gold-fill room site -> bright gold room | cyan hall | purple/blue ramp | orange/yellow shaft | dim uncollapsed | diamond markers: spawn, exit, route",
        state.world.seed,
        state.cursor,
        state.trace.len(),
        completed,
        state.world.last_attempts,
        state.status,
        state.current_level,
        state.world.config.levels,
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
            if let Some((space, doors, _, _)) = view.resolved {
                let placement = state.world.placements[&coord];
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
        assert!(has(|s| matches!(s, SolveStep::BlueprintCell { .. })));
        assert!(has(|s| matches!(s, SolveStep::ForcedCell { .. })));
        assert!(has(|s| matches!(s, SolveStep::Collapsed { .. })));
        assert!(has(|s| matches!(s, SolveStep::Completed { .. })));
    }

    #[test]
    fn every_numbered_preset_solves_and_starts_at_a_clean_replay() {
        for seed in PRESET_SEEDS {
            let state = LabState::new(seed);
            assert_eq!(state.world.seed, seed);
            assert_eq!(state.cursor, 0);
            assert!(!state.trace.is_empty());
        }
    }

    #[test]
    fn lab_rust_sources_stay_below_the_architecture_size_limit() {
        fn visit(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read src tree") {
                let path = entry.expect("directory entry").path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }

        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        visit(&src, &mut files);
        for file in files {
            let lines = std::fs::read_to_string(&file)
                .expect("read Rust source")
                .lines()
                .count();
            assert!(
                lines < 600,
                "{} has {lines} lines; split it below 600",
                file.display()
            );
        }
    }
}
