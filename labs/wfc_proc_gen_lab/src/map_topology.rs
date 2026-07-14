//! MapSpec topology visualization mode (Phase 45 / Arc D).
//!
//! The lab's primary showcase (`main.rs`) is a 2D WFC feasibility demo over an
//! abstract void/corridor/entrance/exit grid. This module is a second, independent
//! mode: it drives `observed_facility::wfc::generate_liminal_map` and renders the
//! resulting `MapSpec` — rooms as boxes colored by `RoomRole`, edges as lines styled
//! by `CorridorRole`, with Start/Exit highlighted and an on-screen legend. `M` toggles
//! between the two modes; `N`/`P` step to the next/previous seed while in map-topology
//! mode; `R` resets back to seed 0. Toggling modes or resetting tears down every
//! entity this module owns (`MapTopologyTile`) so it never leaks into the WFC demo.

use bevy::prelude::*;
use observed_facility::map_spec::{CorridorRole, MapSpec, RoomRole};
use observed_facility::wfc::{WfcMapConfig, generate_liminal_map};

/// World-space size of a schematic grid cell, matching `main.rs`'s `VIEW_TILE_SIZE`
/// scale so both modes read at a similar zoom level.
const CELL_SIZE: f32 = 64.0;
const ROOM_BOX: f32 = 46.0;

#[derive(Component)]
pub struct MapTopologyTile;

#[derive(Component)]
pub struct MapTopologyLegendText;

#[derive(Resource)]
pub struct MapTopologyState {
    pub active: bool,
    pub seed: u64,
    pub config: WfcMapConfig,
    pub spec: Option<MapSpec>,
    pub attempts_note: String,
}

impl Default for MapTopologyState {
    fn default() -> Self {
        let mut state = Self {
            active: false,
            seed: 0,
            config: WfcMapConfig::default(),
            spec: None,
            attempts_note: String::new(),
        };
        state.regenerate();
        state
    }
}

impl MapTopologyState {
    fn regenerate(&mut self) {
        match generate_liminal_map(self.seed, &self.config) {
            Ok(spec) => {
                self.attempts_note = format!(
                    "seed {} -> {} rooms, {} edges",
                    self.seed,
                    spec.room_count(),
                    spec.edges.len()
                );
                self.spec = Some(spec);
            }
            Err(err) => {
                self.attempts_note = format!("seed {} FAILED: {err:?}", self.seed);
                self.spec = None;
            }
        }
    }

    fn reset(&mut self) {
        self.seed = 0;
        self.regenerate();
    }
}

pub fn setup_map_topology_ui(mut commands: Commands) {
    commands.spawn((
        MapTopologyLegendText,
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.92, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
    ));
}

pub fn handle_map_topology_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<MapTopologyState>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        state.active = !state.active;
    }
    if !state.active {
        return;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        state.seed = state.seed.wrapping_add(1);
        state.regenerate();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.seed = state.seed.wrapping_sub(1);
        state.regenerate();
    }
    if keys.just_pressed(KeyCode::KeyR) {
        state.reset();
    }
}

fn room_role_color(role: RoomRole) -> Color {
    match role {
        RoomRole::Start => Color::srgb(0.2, 0.95, 0.4),
        RoomRole::Exit => Color::srgb(1.0, 0.25, 0.25),
        RoomRole::Decision => Color::srgb(0.5, 0.55, 0.62),
        RoomRole::DecoherenceFork => Color::srgb(0.75, 0.3, 0.95),
        RoomRole::AnchorCheckpoint => Color::srgb(1.0, 0.78, 0.3),
        RoomRole::TeleportRelay => Color::srgb(0.2, 0.9, 1.0),
        RoomRole::Keystone => Color::srgb(1.0, 0.84, 0.26),
        RoomRole::DualStation => Color::srgb(0.42, 0.92, 1.0),
        RoomRole::GuardianControl => Color::srgb(1.0, 0.45, 0.15),
        RoomRole::Monitor => Color::srgb(0.16, 1.0, 0.82),
        RoomRole::Recovery => Color::srgb(0.6, 0.85, 0.4),
    }
}

fn corridor_role_color(role: CorridorRole) -> Color {
    match role {
        CorridorRole::Connector => Color::srgb(0.35, 0.4, 0.48),
        CorridorRole::LongRoute => Color::srgb(0.5, 0.5, 0.2),
        CorridorRole::Mystery => Color::srgb(0.65, 0.25, 0.85),
        CorridorRole::Vertical => Color::srgb(0.2, 0.75, 1.0),
        CorridorRole::Bypass => Color::srgb(0.9, 0.55, 0.15),
        CorridorRole::Gantry => Color::srgb(1.0, 0.42, 0.85),
    }
}

pub fn render_map_topology(
    mut commands: Commands,
    state: Res<MapTopologyState>,
    tiles: Query<Entity, With<MapTopologyTile>>,
    mut legend: Query<&mut Text, With<MapTopologyLegendText>>,
) {
    if !state.is_changed() {
        return;
    }

    for entity in &tiles {
        commands.entity(entity).despawn();
    }

    if !state.active {
        if let Ok(mut text) = legend.single_mut() {
            text.0.clear();
        }
        return;
    }

    let Some(spec) = &state.spec else {
        if let Ok(mut text) = legend.single_mut() {
            text.0 = format!(
                "WFC MapSpec topology (M to toggle, N/P seed, R reset)\n{}",
                state.attempts_note
            );
        }
        return;
    };

    // Edges first (so room boxes render on top of the lines meeting them).
    for edge in &spec.edges {
        let Some(room_a) = spec.room(edge.a.room) else {
            continue;
        };
        let Some(room_b) = spec.room(edge.b.room) else {
            continue;
        };
        let start = room_a.schematic * CELL_SIZE;
        let end = room_b.schematic * CELL_SIZE;
        let mid = (start + end) * 0.5;
        let delta = end - start;
        let length = delta.length().max(1.0);
        let angle = delta.y.atan2(delta.x);

        commands.spawn((
            MapTopologyTile,
            Sprite {
                color: corridor_role_color(edge.role),
                custom_size: Some(Vec2::new(length, 4.0)),
                ..default()
            },
            Transform::from_xyz(mid.x, mid.y, 0.5).with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    for room in &spec.rooms {
        let pos = room.schematic * CELL_SIZE;
        let is_terminal = matches!(room.role, RoomRole::Start | RoomRole::Exit);
        let size = if is_terminal {
            ROOM_BOX + 10.0
        } else {
            ROOM_BOX
        };

        commands.spawn((
            MapTopologyTile,
            Sprite {
                color: room_role_color(room.role),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 1.0),
        ));

        if is_terminal {
            commands.spawn((
                MapTopologyTile,
                Sprite {
                    color: Color::srgb(1.0, 1.0, 1.0),
                    custom_size: Some(Vec2::splat(size + 8.0)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.9),
            ));
        }
    }

    if let Ok(mut text) = legend.single_mut() {
        text.0 = format!(
            "WFC MapSpec topology (M to toggle, N/P seed, R reset)\n\
             {}\n\n\
             [Room roles]  Start=green  Exit=red  Decision=grey  DecoherenceFork=purple\n\
             AnchorCheckpoint=gold  TeleportRelay=cyan  Keystone=yellow  DualStation=lt-blue\n\
             GuardianControl=orange  Monitor=teal  Recovery=lime\n\
             [Corridor roles]  Connector=grey  LongRoute=olive  Mystery=violet\n\
             Vertical=blue  Bypass=amber",
            state.attempts_note
        );
    }
}
