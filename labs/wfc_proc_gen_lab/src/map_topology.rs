//! Catalogue-v2 `MapSpec` topology proof.
//!
//! This is the lab's default view. It renders room architecture registers and
//! corridor traversal archetypes from `generate_liminal_map_v2`, including a full
//! production-safe legend. `V` switches to the retained v1 topology regression;
//! `M` opens the archived abstract tile-WFC feasibility view.

use bevy::prelude::*;
use observed_facility::map_spec::{
    ArchitectureRegister, CorridorRole, MapSpec, RoomRole, TraversalArchetype,
};
use observed_facility::wfc::{WfcMapConfig, generate_liminal_map, generate_liminal_map_v2};

/// World-space size of a schematic grid cell, matching `main.rs`'s view scale.
const CELL_SIZE: f32 = 64.0;
const ROOM_BOX: f32 = 46.0;

#[derive(Component)]
pub struct MapTopologyTile;

#[derive(Component)]
pub struct MapTopologyLegendText;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogueRevision {
    V1Regression,
    V2Catalogue,
}

impl CatalogueRevision {
    fn label(self) -> &'static str {
        match self {
            Self::V1Regression => "v1 topology regression",
            Self::V2Catalogue => "v2 architecture catalogue",
        }
    }
}

#[derive(Resource)]
pub struct MapTopologyState {
    pub active: bool,
    pub seed: u64,
    pub revision: CatalogueRevision,
    pub config: WfcMapConfig,
    pub spec: Option<MapSpec>,
    pub attempts_note: String,
}

impl Default for MapTopologyState {
    fn default() -> Self {
        let mut state = Self {
            active: true,
            seed: 0,
            revision: CatalogueRevision::V2Catalogue,
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
        let generated = match self.revision {
            CatalogueRevision::V1Regression => generate_liminal_map(self.seed, &self.config),
            CatalogueRevision::V2Catalogue => generate_liminal_map_v2(self.seed, &self.config),
        };
        match generated {
            Ok(spec) => {
                let design_note = spec.designs.as_ref().map_or_else(
                    || "role-only legacy topology".to_string(),
                    |designs| {
                        format!(
                            "{} connected architecture regions",
                            designs.register_count()
                        )
                    },
                );
                self.attempts_note = format!(
                    "{} | seed {} -> {} rooms, {} corridors | {design_note}",
                    self.revision.label(),
                    self.seed,
                    spec.room_count(),
                    spec.corridors().len()
                );
                self.spec = Some(spec);
            }
            Err(err) => {
                self.attempts_note = format!(
                    "{} | seed {} FAILED: {err:?}",
                    self.revision.label(),
                    self.seed
                );
                self.spec = None;
            }
        }
    }

    fn reset(&mut self) {
        self.seed = 0;
        self.revision = CatalogueRevision::V2Catalogue;
        self.regenerate();
    }

    fn toggle_revision(&mut self) {
        self.revision = match self.revision {
            CatalogueRevision::V1Regression => CatalogueRevision::V2Catalogue,
            CatalogueRevision::V2Catalogue => CatalogueRevision::V1Regression,
        };
        self.regenerate();
    }
}

pub fn setup_map_topology_ui(mut commands: Commands) {
    commands.spawn((
        MapTopologyLegendText,
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.92, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(14.0),
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
    if keys.just_pressed(KeyCode::KeyV) {
        state.toggle_revision();
    }
    if keys.just_pressed(KeyCode::KeyR) {
        state.reset();
    }
}

fn architecture_color(register: ArchitectureRegister) -> Color {
    match register {
        ArchitectureRegister::ShadowScreen => Color::srgb(0.28, 0.34, 0.72),
        ArchitectureRegister::Monolith => Color::srgb(0.72, 0.31, 0.22),
        ArchitectureRegister::OverlitGrid => Color::srgb(0.94, 0.88, 0.55),
        ArchitectureRegister::Institutional => Color::srgb(0.58, 0.84, 0.68),
        ArchitectureRegister::FacetMonument => Color::srgb(0.67, 0.39, 0.88),
        ArchitectureRegister::Megastructure => Color::srgb(0.12, 0.68, 0.64),
        ArchitectureRegister::Wellshaft => Color::srgb(0.16, 0.55, 0.96),
        ArchitectureRegister::InfiniteGallery => Color::srgb(0.91, 0.32, 0.66),
        ArchitectureRegister::Thinning => Color::srgb(0.59, 0.62, 0.67),
    }
}

fn architecture_short_label(register: ArchitectureRegister) -> &'static str {
    match register {
        ArchitectureRegister::ShadowScreen => "Shadow",
        ArchitectureRegister::Monolith => "Monolith",
        ArchitectureRegister::OverlitGrid => "Overlit",
        ArchitectureRegister::Institutional => "Institutional",
        ArchitectureRegister::FacetMonument => "Facet",
        ArchitectureRegister::Megastructure => "Mega",
        ArchitectureRegister::Wellshaft => "Wellshaft",
        ArchitectureRegister::InfiniteGallery => "Gallery",
        ArchitectureRegister::Thinning => "Thinning",
    }
}

fn architecture_text_color(register: ArchitectureRegister) -> Color {
    match register {
        ArchitectureRegister::OverlitGrid
        | ArchitectureRegister::Institutional
        | ArchitectureRegister::Thinning => Color::srgb(0.015, 0.02, 0.035),
        _ => Color::WHITE,
    }
}

fn traversal_color(traversal: TraversalArchetype) -> Color {
    match traversal {
        TraversalArchetype::Straight => Color::srgb(0.70, 0.72, 0.77),
        TraversalArchetype::Long => Color::srgb(0.80, 0.72, 0.20),
        TraversalArchetype::Pressure => Color::srgb(1.00, 0.30, 0.24),
        TraversalArchetype::Climb => Color::srgb(0.20, 0.76, 1.00),
        TraversalArchetype::Maze => Color::srgb(0.66, 0.28, 0.90),
        TraversalArchetype::Chicane => Color::srgb(0.26, 0.96, 0.72),
        TraversalArchetype::GantryExpanse => Color::srgb(1.00, 0.42, 0.82),
        TraversalArchetype::Wellshaft => Color::srgb(0.22, 0.58, 1.00),
        TraversalArchetype::Colonnade => Color::srgb(0.96, 0.63, 0.22),
        TraversalArchetype::Orthogonal => Color::srgb(0.72, 0.95, 0.82),
    }
}

fn traversal_label(traversal: TraversalArchetype) -> &'static str {
    match traversal {
        TraversalArchetype::Straight => "Straight",
        TraversalArchetype::Long => "Long",
        TraversalArchetype::Pressure => "Pressure",
        TraversalArchetype::Climb => "Climb",
        TraversalArchetype::Maze => "Maze",
        TraversalArchetype::Chicane => "Chicane",
        TraversalArchetype::GantryExpanse => "Gantry Expanse",
        TraversalArchetype::Wellshaft => "Wellshaft",
        TraversalArchetype::Colonnade => "Colonnade",
        TraversalArchetype::Orthogonal => "Orthogonal",
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
                "WFC MapSpec catalogue (M: archived grid, N/P: seed, V: v1/v2, R: reset)\n{}",
                state.attempts_note
            );
        }
        return;
    };

    let designs = spec.designs.as_ref();
    let min = spec
        .rooms
        .iter()
        .map(|room| room.schematic)
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO);
    let max = spec
        .rooms
        .iter()
        .map(|room| room.schematic)
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO);
    let schematic_center = (min + max) * 0.5;

    // Corridors render first so room-region boxes remain legible above them.
    for corridor in spec.corridors() {
        let Some(first) = corridor.endpoints.first() else {
            continue;
        };
        let Some(room_a) = spec.room(first.room) else {
            continue;
        };
        for endpoint in corridor.endpoints.iter().skip(1) {
            let Some(room_b) = spec.room(endpoint.room) else {
                continue;
            };
            let start = (room_a.schematic - schematic_center) * CELL_SIZE;
            let end = (room_b.schematic - schematic_center) * CELL_SIZE;
            let mid = (start + end) * 0.5;
            let delta = end - start;
            let length = delta.length().max(1.0);
            let angle = delta.y.atan2(delta.x);
            let traversal = designs
                .and_then(|assignments| assignments.corridor(corridor.id))
                .map(|design| design.traversal);
            let color =
                traversal.map_or_else(|| corridor_role_color(corridor.role), traversal_color);
            let width = match traversal {
                Some(TraversalArchetype::GantryExpanse) => 7.0,
                Some(TraversalArchetype::Wellshaft) => 6.0,
                _ => 4.0,
            };

            commands.spawn((
                MapTopologyTile,
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(length, width)),
                    ..default()
                },
                Transform::from_xyz(mid.x, mid.y, 0.5).with_rotation(Quat::from_rotation_z(angle)),
            ));
        }
    }

    for room in &spec.rooms {
        let pos = (room.schematic - schematic_center) * CELL_SIZE;
        let is_terminal = matches!(room.role, RoomRole::Start | RoomRole::Exit);
        let size = if is_terminal {
            ROOM_BOX + 10.0
        } else {
            ROOM_BOX
        };
        let room_design = designs.and_then(|assignments| assignments.room(room.id));
        let color = room_design.map_or_else(
            || room_role_color(room.role),
            |d| architecture_color(d.register),
        );

        if is_terminal {
            commands.spawn((
                MapTopologyTile,
                Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::splat(size + 8.0)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.9),
            ));
        }

        commands.spawn((
            MapTopologyTile,
            Sprite {
                color,
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 1.0),
        ));

        let label = room_design.map_or_else(
            || format!("{}\n{}", room.id.0, room.role.label()),
            |design| {
                format!(
                    "{}\n{}",
                    room.id.0,
                    architecture_short_label(design.register)
                )
            },
        );
        let label_color = room_design.map_or(Color::WHITE, |design| {
            architecture_text_color(design.register)
        });
        commands.spawn((
            MapTopologyTile,
            Text2d::new(label),
            TextFont {
                font_size: 8.0,
                ..default()
            },
            TextColor(label_color),
            Transform::from_xyz(pos.x, pos.y, 2.0),
        ));
    }

    if let Ok(mut text) = legend.single_mut() {
        text.0 = if designs.is_some() {
            format!(
                "WFC MapSpec catalogue (M: archived grid, N/P: seed, V: v1/v2, R: reset)\n{}\n\n\
                 [Room fill = architecture register; white border = Start / Exit]\n\
                 Shadow Screen=indigo | Monolith=rust | Overlit Grid=yellow | Institutional=mint\n\
                 Facet Monument=violet | Megastructure=teal | Wellshaft=blue\n\
                 Infinite Gallery=magenta | Thinning=grey\n\n\
                 [Corridor line = traversal archetype; thick = major vertical/gantry course]\n\
                 {}=grey | {}=yellow | {}=red | {}=cyan | {}=violet\n\
                 {}=green | {}=pink/thick | {}=blue/thick\n\
                 {}=orange | {}=mint",
                state.attempts_note,
                traversal_label(TraversalArchetype::Straight),
                traversal_label(TraversalArchetype::Long),
                traversal_label(TraversalArchetype::Pressure),
                traversal_label(TraversalArchetype::Climb),
                traversal_label(TraversalArchetype::Maze),
                traversal_label(TraversalArchetype::Chicane),
                traversal_label(TraversalArchetype::GantryExpanse),
                traversal_label(TraversalArchetype::Wellshaft),
                traversal_label(TraversalArchetype::Colonnade),
                traversal_label(TraversalArchetype::Orthogonal),
            )
        } else {
            format!(
                "WFC v1 regression (M: archived grid, N/P: seed, V: v1/v2, R: reset)\n{}\n\n\
                 [Room roles] Start=green Exit=red Decision=grey DecoherenceFork=purple\n\
                 AnchorCheckpoint=gold TeleportRelay=cyan Keystone=yellow DualStation=lt-blue\n\
                 GuardianControl=orange Monitor=teal Recovery=lime\n\
                 [Corridor roles] Connector=grey LongRoute=olive Mystery=violet\n\
                 Vertical=blue Bypass=amber Gantry=pink",
                state.attempts_note
            )
        };
    }
}
