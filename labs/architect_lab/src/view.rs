//! Map-first presentation and pointer navigation for the Rogue Architect.

pub(crate) mod ui;

use bevy::camera::{Viewport, visibility::RenderLayers};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace};
use observed_style::{MarkerRole, SchematicRole, TacticsRole};
use observed_ui::theme::{ChromeRole, chrome};

use crate::LabSession;
use crate::sim::{ArchitectMode, CardKind, DoorState, ObserverState};

pub use ui::{
    handle_ui_actions, sync_action_buttons, sync_card_accents, sync_card_art, sync_card_buttons,
    sync_card_text, sync_charge_pips, sync_dynamic_text, sync_layout,
};

const MAP_RENDER_LAYER: usize = 0;
const HUD_RENDER_LAYER: usize = 1;
const HEX_RADIUS: f32 = 30.0;
const FLOOR_ASCENT: Vec2 = Vec2::new(600.0, 330.0);
const MIN_ZOOM: f32 = 0.54;
const MAX_ZOOM: f32 = 2.25;
const DEFAULT_PAN: Vec2 = Vec2::new(0.0, 28.0);

const fn default_zoom(mode: ArchitectMode) -> f32 {
    match mode {
        ArchitectMode::Pocket => 0.62,
        ArchitectMode::QuickClimb => 0.9,
        ArchitectMode::FullAscent => 1.12,
        // Five stacked floors need more of the board in frame at once.
        ArchitectMode::DeepStack => 1.30,
    }
}

#[derive(Component)]
pub(crate) struct BoardCamera;

#[derive(Component)]
pub(crate) struct BoardVisual;

type UiControlFilter = Or<(With<ui::ArchitectButton>, With<ui::CardButton>)>;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapCameraState {
    pub pan: Vec2,
    pub zoom: f32,
    viewport_offset: Vec2,
    viewport_size: Vec2,
    panning: bool,
}

impl Default for MapCameraState {
    fn default() -> Self {
        Self {
            pan: DEFAULT_PAN,
            zoom: default_zoom(ArchitectMode::Pocket),
            viewport_offset: Vec2::ZERO,
            viewport_size: Vec2::ONE,
            panning: false,
        }
    }
}

impl MapCameraState {
    pub fn reset_for_mode(&mut self, mode: ArchitectMode) {
        self.pan = DEFAULT_PAN;
        self.zoom = default_zoom(mode);
        self.panning = false;
    }

    pub fn zoom_centered(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn zoom_around(&mut self, factor: f32, cursor: Vec2) {
        let old_zoom = self.zoom;
        let new_zoom = (old_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let cursor_from_center = Vec2::new(
            cursor.x - self.viewport_offset.x - self.viewport_size.x * 0.5,
            -(cursor.y - self.viewport_offset.y - self.viewport_size.y * 0.5),
        );
        self.pan += cursor_from_center * (old_zoom - new_zoom);
        self.zoom = new_zoom;
    }

    fn contains(&self, cursor: Vec2) -> bool {
        cursor.x >= self.viewport_offset.x
            && cursor.y >= self.viewport_offset.y
            && cursor.x < self.viewport_offset.x + self.viewport_size.x
            && cursor.y < self.viewport_offset.y + self.viewport_size.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WorkspaceLayout {
    pub sidebar_width: f32,
    pub hand_height: f32,
    pub map_offset: Vec2,
    pub map_size: Vec2,
}

impl WorkspaceLayout {
    pub fn for_window(size: Vec2) -> Self {
        let sidebar_width = (size.x * 0.19).clamp(270.0, 320.0).min(size.x - 640.0);
        let hand_height = (size.y * 0.27).clamp(238.0, 286.0).min(size.y - 440.0);
        Self {
            sidebar_width,
            hand_height,
            map_offset: Vec2::new(sidebar_width, 0.0),
            map_size: Vec2::new(
                (size.x - sidebar_width).max(1.0),
                (size.y - hand_height).max(1.0),
            ),
        }
    }
}

pub fn setup(mut commands: Commands) {
    commands.spawn((
        BoardCamera,
        Camera2d,
        Camera {
            clear_color: observed_style::schematic_screen().into(),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scale: default_zoom(ArchitectMode::Pocket),
            ..OrthographicProjection::default_2d()
        }),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_xyz(DEFAULT_PAN.x, DEFAULT_PAN.y, 0.0),
        Name::new("Rogue Architect facility camera"),
    ));
    let hud_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            bevy::ui::IsDefaultUiCamera,
            RenderLayers::layer(HUD_RENDER_LAYER),
            Name::new("Rogue Architect interface camera"),
        ))
        .id();
    ui::spawn(&mut commands, hud_camera);
}

pub fn sync_camera_viewport(
    windows: Query<&Window>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut Projection), With<BoardCamera>>,
    mut state: ResMut<MapCameraState>,
) {
    let (Ok(window), Ok((mut camera, mut transform, mut projection))) =
        (windows.single(), cameras.single_mut())
    else {
        return;
    };
    let layout = WorkspaceLayout::for_window(Vec2::new(window.width(), window.height()));
    let scale = window.scale_factor();
    let physical_offset = (layout.map_offset * scale).as_uvec2();
    let physical_size = (layout.map_size * scale).as_uvec2().max(UVec2::ONE);
    if camera
        .viewport
        .as_ref()
        .map(|viewport| (viewport.physical_position, viewport.physical_size))
        != Some((physical_offset, physical_size))
    {
        camera.viewport = Some(Viewport {
            physical_position: physical_offset,
            physical_size,
            depth: 0.0..1.0,
        });
    }
    state.viewport_offset = layout.map_offset;
    state.viewport_size = layout.map_size;
    transform.translation = state.pan.extend(0.0);
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: state.zoom,
        ..OrthographicProjection::default_2d()
    });
}

pub fn camera_controls(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window>,
    interactions: Query<&Interaction, UiControlFilter>,
    mut state: ResMut<MapCameraState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position();
    let over_control = interactions
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let over_map = cursor.is_some_and(|cursor| state.contains(cursor)) && !over_control;

    if (mouse.just_pressed(MouseButton::Right) || mouse.just_pressed(MouseButton::Middle))
        && over_map
    {
        state.panning = true;
    }
    if !mouse.pressed(MouseButton::Right) && !mouse.pressed(MouseButton::Middle) {
        state.panning = false;
    }
    if over_map && scroll.delta.y.abs() > f32::EPSILON {
        state.zoom_around(1.14_f32.powf(-scroll.delta.y), cursor.unwrap_or_default());
    }
    if state.panning {
        let zoom = state.zoom;
        state.pan += Vec2::new(-motion.delta.x, motion.delta.y) * zoom;
    }
}

pub fn map_pointer_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    interactions: Query<&Interaction, UiControlFilter>,
    mut session: ResMut<LabSession>,
) {
    let (Ok(window), Ok((camera, transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let over_control = interactions
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let picked = if over_control {
        None
    } else {
        window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world_2d(transform, cursor).ok())
            .and_then(|world| pick_target(&session, world))
    };
    if picked != session.hovered_target {
        session.hovered_target = picked;
        session.dirty = true;
    }
    if mouse.just_pressed(MouseButton::Left)
        && let Some(target) = picked
    {
        session.select_target(target);
    }
}

fn pick_target(session: &LabSession, world: Vec2) -> Option<HexCoord> {
    session
        .sim
        .mutable_targets()
        .into_iter()
        .map(|cell| {
            let distance = board_position(session.sim.world.config, cell).distance(world);
            (cell, distance)
        })
        .filter(|(_, distance)| *distance <= HEX_RADIUS)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(cell, _)| cell)
}

pub fn rebuild_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    visuals: Query<Entity, With<BoardVisual>>,
    mut session: ResMut<LabSession>,
) {
    if !session.dirty {
        return;
    }
    session.dirty = false;
    for entity in &visuals {
        commands.entity(entity).despawn();
    }

    let halo = meshes.add(RegularPolygon::new(HEX_RADIUS * 1.06, 6));
    let hex = meshes.add(RegularPolygon::new(HEX_RADIUS * 0.92, 6));
    let observer_disc = meshes.add(Circle::new(HEX_RADIUS * 0.49));
    let pupil = meshes.add(Circle::new(HEX_RADIUS * 0.17));
    let guardian_triangle = meshes.add(RegularPolygon::new(HEX_RADIUS * 0.58, 3));
    let door_bar = meshes.add(Rectangle::new(HEX_RADIUS * 0.92, 5.0));
    let topology_arm = meshes.add(Rectangle::new(HEX_RADIUS * 0.72, 3.2));
    let preview_arm = meshes.add(Rectangle::new(HEX_RADIUS * 0.84, 6.5));
    let floor_label_plate = meshes.add(Rectangle::new(380.0, 36.0));
    let target = session.target();

    if session.sim.world.config.levels > 1 {
        let levels = session.sim.world.config.levels;
        let p_start = floor_offset(0, levels);
        let p_end = floor_offset(levels - 1, levels);
        let span = p_end - p_start;
        let length = span.length();
        let angle = span.y.atan2(span.x);
        let center = (p_start + p_end) * 0.5;
        let line_mesh = meshes.add(Rectangle::new(length, 3.0));
        spawn_mesh(
            &mut commands,
            &mut materials,
            line_mesh,
            observed_style::schematic(SchematicRole::Grid).base_color,
            center.extend(-2.0),
            angle,
            "Floor ascent axis".to_string(),
        );
    }
    let target_floor = target.map(|c| c.level);
    for level in 0..session.sim.world.config.levels {
        let register = crate::sim::floor_register(level);
        let label_position = floor_offset(level, session.sim.world.config.levels) + Vec2::Y * 180.0;
        let is_targeted = target_floor == Some(level);
        let plate_color = if is_targeted {
            observed_style::schematic(SchematicRole::Selected)
                .base_color
                .with_alpha(0.92)
        } else {
            chrome(ChromeRole::Surface).with_alpha(0.92)
        };
        spawn_mesh(
            &mut commands,
            &mut materials,
            floor_label_plate.clone(),
            plate_color,
            label_position.extend(3.5),
            0.0,
            format!("Floor {} label plate", level + 1),
        );
        let power_tag = if session.debug_overlay {
            if session.sim.economy.is_powered(level) {
                "  [PWR:ON]"
            } else {
                "  [PWR:OFF]"
            }
        } else {
            ""
        };
        let title_color = if is_targeted {
            chrome(ChromeRole::Surface)
        } else {
            observed_style::architecture_tactical(register).base_color
        };
        commands.spawn((
            BoardVisual,
            Text2d::new(format!(
                "FLOOR {:02}  //  {}{power_tag}",
                level + 1,
                crate::sim::floor_title(level)
            )),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(title_color),
            TextLayout::justify(Justify::Center),
            Transform::from_translation(label_position.extend(4.0)),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Name::new(format!("Floor {} title", level + 1)),
        ));
    }

    for (&cell, placement) in &session.sim.world.placements {
        if placement.space == observed_facility::hex_wfc::HexSpace::Void
            && !session.sim.mutable_targets().contains(&cell)
        {
            continue;
        }
        let center = board_position(session.sim.world.config, cell);
        let halo_role = if Some(cell) == target {
            Some(SchematicRole::Selected)
        } else if Some(cell) == session.hovered_target {
            Some(SchematicRole::Grid)
        } else {
            None
        };
        if let Some(role) = halo_role {
            spawn_mesh(
                &mut commands,
                &mut materials,
                halo.clone(),
                observed_style::schematic(role).base_color,
                center.extend(-0.5),
                0.0,
                format!("Target halo {cell:?}"),
            );
        }
        let color = if placement.space == observed_facility::hex_wfc::HexSpace::Void {
            observed_style::tactics(TacticsRole::DevContext).base_color
        } else if session.sim.contradictions.contains(&cell) {
            observed_style::tactics(TacticsRole::Blocked).base_color
        } else if session.sim.observed.contains(&cell) {
            observed_style::tactics(TacticsRole::ReachableRoute).base_color
        } else if session.sim.prison_core.contains(&cell) {
            observed_style::tactics(TacticsRole::RouteLimit).base_color
        } else {
            let register = session
                .sim
                .world
                .architecture
                .get(&cell)
                .copied()
                .unwrap_or_else(|| crate::sim::floor_register(cell.level));
            observed_style::architecture_tactical(register).base_color
        };
        spawn_mesh(
            &mut commands,
            &mut materials,
            hex.clone(),
            color,
            center.extend(0.0),
            0.0,
            format!("Tile {cell:?}"),
        );
        for face in HexFace::LATERAL
            .into_iter()
            .filter(|&face| placement.is_open(face))
        {
            let direction = face_vector(face);
            spawn_mesh(
                &mut commands,
                &mut materials,
                topology_arm.clone(),
                observed_style::tactics(TacticsRole::DevGrid).base_color,
                (center + direction * HEX_RADIUS * 0.47).extend(1.0),
                direction.y.atan2(direction.x),
                format!("Topology arm {cell:?} {face:?}"),
            );
        }
    }

    // Active condemned tile hazard halo and visible countdown (gameplay-critical, always legible):
    if let Some((condemned_cell, commit_tick)) = session.sim.condemned {
        let pos = board_position(session.sim.world.config, condemned_cell);
        let ticks_left = commit_tick.saturating_sub(session.sim.tick);
        let secs = ticks_left.div_ceil(60);
        let hazard_color = observed_style::tactics(TacticsRole::RouteLimit).base_color;
        spawn_mesh(
            &mut commands,
            &mut materials,
            halo.clone(),
            hazard_color,
            pos.extend(2.8),
            0.0,
            format!("Condemned hazard halo {condemned_cell:?}"),
        );
        commands.spawn((
            BoardVisual,
            Text2d::new(format!("CONDEMNED\n{:02}s ({:03}t)", secs, ticks_left)),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(hazard_color),
            TextLayout::justify(Justify::Center),
            Transform::from_translation((pos + Vec2::new(0.0, -14.0)).extend(3.2)),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Name::new(format!("Condemned countdown text {condemned_cell:?}")),
        ));
    }

    if session.debug_overlay {
        if let Some(target_cell) = session.sim.next_retraction() {
            if session
                .sim
                .condemned
                .map_or(true, |(c, _)| c != target_cell)
            {
                let target_pos = board_position(session.sim.world.config, target_cell);
                let ticks_left = session
                    .sim
                    .next_retraction_tick
                    .unwrap_or(0)
                    .saturating_sub(session.sim.tick);
                let secs = ticks_left.div_ceil(60);
                spawn_mesh(
                    &mut commands,
                    &mut materials,
                    halo.clone(),
                    observed_style::tactics(TacticsRole::Blocked).base_color,
                    target_pos.extend(2.5),
                    0.0,
                    format!("Retraction telegraph halo {target_cell:?}"),
                );
                commands.spawn((
                    BoardVisual,
                    Text2d::new(format!("RETRACT\n{:02}s ({:03}t)", secs, ticks_left)),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                        ..default()
                    },
                    TextColor(observed_style::tactics(TacticsRole::Blocked).base_color),
                    TextLayout::justify(Justify::Center),
                    Transform::from_translation((target_pos + Vec2::new(0.0, -12.0)).extend(3.0)),
                    RenderLayers::layer(MAP_RENDER_LAYER),
                    Name::new(format!("Retraction telegraph text {target_cell:?}")),
                ));
            }
        }

        for (&cell, placement) in &session.sim.world.placements {
            if placement.space == observed_facility::hex_wfc::HexSpace::Void || cell.level == 0 {
                continue;
            }
            let center = board_position(session.sim.world.config, cell);
            let lower = crate::falls::find_lower_surviving_structure(&session.sim, cell);
            let (glyph, text_color) = match lower {
                Some(surviving) => (
                    format!("v F{}", surviving.level + 1),
                    observed_style::schematic(SchematicRole::Pinned).base_color,
                ),
                None => (
                    "X VOID".to_string(),
                    observed_style::tactics(TacticsRole::Blocked).base_color,
                ),
            };
            commands.spawn((
                BoardVisual,
                Text2d::new(glyph),
                TextFont {
                    font_size: FontSize::Px(8.5),
                    ..default()
                },
                TextColor(text_color),
                TextLayout::justify(Justify::Center),
                Transform::from_translation((center + Vec2::new(0.0, -10.0)).extend(1.5)),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Name::new(format!("Fall candidate debug {cell:?}")),
            ));
        }

        for &station in &session.sim.economy.stations {
            let center = board_position(session.sim.world.config, station);
            let powered = session.sim.economy.is_powered(station.level);
            let (status, color) = if powered {
                (
                    "GEN:ON",
                    observed_style::schematic(SchematicRole::Pinned).base_color,
                )
            } else {
                (
                    "GEN:OFF",
                    observed_style::tactics(TacticsRole::Blocked).base_color,
                )
            };
            commands.spawn((
                BoardVisual,
                Text2d::new(status),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(color),
                TextLayout::justify(Justify::Center),
                Transform::from_translation((center + Vec2::new(0.0, 10.0)).extend(2.0)),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Name::new(format!("Generator debug {station:?}")),
            ));
        }
    }

    draw_selected_card(
        &mut commands,
        &mut materials,
        &session,
        target,
        preview_arm,
        door_bar.clone(),
    );
    draw_doors(&mut commands, &mut materials, &session, door_bar);
    draw_actors(
        &mut commands,
        &mut materials,
        &session,
        observer_disc,
        pupil,
        guardian_triangle,
    );
}

fn draw_selected_card(
    commands: &mut Commands,
    materials: &mut Assets<ColorMaterial>,
    session: &LabSession,
    target: Option<HexCoord>,
    preview_arm: Handle<Mesh>,
    door_bar: Handle<Mesh>,
) {
    let (Some(target), Some(card)) = (
        target,
        session.sim.deck.hand.get(session.selected_card).copied(),
    ) else {
        return;
    };
    let center = board_position(session.sim.world.config, target);
    match card.kind {
        CardKind::Tile(shape) => {
            let doors = shape.doors(session.rotation);
            for face in HexFace::LATERAL
                .into_iter()
                .filter(|face| doors & (1 << face.index()) != 0)
            {
                let direction = face_vector(face);
                spawn_mesh(
                    commands,
                    materials,
                    preview_arm.clone(),
                    observed_style::tactics(TacticsRole::ClickPulse).base_color,
                    (center + direction * HEX_RADIUS * 0.47).extend(3.0),
                    direction.y.atan2(direction.x),
                    format!("Card preview {target:?} {face:?}"),
                );
            }
        }
        CardKind::Door => {
            let face = HexFace::LATERAL[(session.rotation % 6) as usize];
            if let Some(next) = session.sim.world.config.grid().neighbor(target, face) {
                let to = board_position(session.sim.world.config, next);
                let delta = to - center;
                spawn_mesh(
                    commands,
                    materials,
                    door_bar,
                    observed_style::tactics(TacticsRole::ClickPulse).base_color,
                    ((center + to) * 0.5).extend(3.0),
                    delta.y.atan2(delta.x),
                    format!("Door preview {target:?} {face:?}"),
                );
            }
        }
    }
}

fn draw_doors(
    commands: &mut Commands,
    materials: &mut Assets<ColorMaterial>,
    session: &LabSession,
    door_bar: Handle<Mesh>,
) {
    for (&key, &state) in &session.sim.doors {
        let Some(next) = session.sim.world.config.grid().neighbor(key.cell, key.face) else {
            continue;
        };
        let from = board_position(session.sim.world.config, key.cell);
        let to = board_position(session.sim.world.config, next);
        let delta = to - from;
        let color = match state {
            DoorState::Open => observed_style::schematic(SchematicRole::Pinned).base_color,
            DoorState::Closed => observed_style::tactics(TacticsRole::Blocked).base_color,
        };
        spawn_mesh(
            commands,
            materials,
            door_bar.clone(),
            color,
            ((from + to) * 0.5).extend(2.0),
            delta.y.atan2(delta.x),
            format!("{state:?} door {key:?}"),
        );
    }
}

fn draw_actors(
    commands: &mut Commands,
    materials: &mut Assets<ColorMaterial>,
    session: &LabSession,
    observer_disc: Handle<Mesh>,
    pupil: Handle<Mesh>,
    guardian_triangle: Handle<Mesh>,
) {
    let observer_color = observed_style::team(1).base_color;
    let pupil_color = observed_style::schematic_screen();
    for observer in session.sim.observers.values() {
        if observer.state == ObserverState::Corrupted {
            continue;
        }
        let mut at = board_position(session.sim.world.config, observer.cell);
        if observer.state == ObserverState::Jailed {
            at += Vec2::new(f32::from(observer.id.0) * 14.0 - 7.0, 0.0);
        }
        spawn_mesh(
            commands,
            materials,
            observer_disc.clone(),
            observer_color,
            at.extend(5.0),
            0.0,
            format!("Observer eye {}", observer.id.0),
        );
        let facing = face_vector(observer.facing) * HEX_RADIUS * 0.18;
        spawn_mesh(
            commands,
            materials,
            pupil.clone(),
            pupil_color,
            (at + facing).extend(6.0),
            0.0,
            format!("Observer pupil {}", observer.id.0),
        );
        commands.spawn((
            BoardVisual,
            Text2d::new(format!("F{}", observer.cell.level + 1)),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextMain)),
            TextLayout::justify(Justify::Center),
            Transform::from_translation((at + Vec2::new(0.0, 16.0)).extend(7.0)),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Name::new(format!("Observer {} floor tag", observer.id.0)),
        ));

        if session.debug_overlay {
            let lower = crate::falls::find_lower_surviving_structure(&session.sim, observer.cell);
            let (fall_label, fall_color) = if observer.cell.level == 0 {
                (
                    "GROUND",
                    observed_style::schematic(SchematicRole::Pinned).base_color,
                )
            } else if lower.is_some() {
                (
                    "SAFE FALL",
                    observed_style::schematic(SchematicRole::Pinned).base_color,
                )
            } else {
                (
                    "FATAL VOID",
                    observed_style::tactics(TacticsRole::Blocked).base_color,
                )
            };
            commands.spawn((
                BoardVisual,
                Text2d::new(fall_label),
                TextFont {
                    font_size: FontSize::Px(8.5),
                    ..default()
                },
                TextColor(fall_color),
                TextLayout::justify(Justify::Center),
                Transform::from_translation((at + Vec2::new(0.0, -18.0)).extend(7.0)),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Name::new(format!("Observer {} fall prognosis", observer.id.0)),
            ));
        }
    }
    let guardian_color = observed_style::marker(MarkerRole::Director).base_color;
    for guardian in session.sim.guardians.values() {
        let at = board_position(session.sim.world.config, guardian.cell);
        spawn_mesh(
            commands,
            materials,
            guardian_triangle.clone(),
            guardian_color,
            at.extend(5.0),
            0.0,
            format!("Guardian pyramid {}", guardian.id.0),
        );
        commands.spawn((
            BoardVisual,
            Text2d::new(format!("F{}", guardian.cell.level + 1)),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextMain)),
            TextLayout::justify(Justify::Center),
            Transform::from_translation((at + Vec2::new(0.0, 16.0)).extend(7.0)),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Name::new(format!("Guardian {} floor tag", guardian.id.0)),
        ));
    }
}

fn spawn_mesh(
    commands: &mut Commands,
    materials: &mut Assets<ColorMaterial>,
    mesh: Handle<Mesh>,
    color: Color,
    at: Vec3,
    rotation: f32,
    name: String,
) {
    commands.spawn((
        BoardVisual,
        Mesh2d(mesh),
        MeshMaterial2d(materials.add(ColorMaterial::from(color))),
        Transform::from_translation(at).with_rotation(Quat::from_rotation_z(rotation)),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Name::new(name),
    ));
}

fn board_position(config: observed_facility::hex_wfc::HexWfcConfig, cell: HexCoord) -> Vec2 {
    let x = 3.0_f32.sqrt() * HEX_RADIUS * (f32::from(cell.q) + f32::from(cell.r) * 0.5);
    let y = -HEX_RADIUS * 1.5 * f32::from(cell.r);
    let center_x = 3.0_f32.sqrt()
        * HEX_RADIUS
        * (f32::from(config.cols - 1) + f32::from(config.rows - 1) * 0.5)
        * 0.5;
    let center_y = -HEX_RADIUS * 1.5 * f32::from(config.rows - 1) * 0.5;
    Vec2::new(x - center_x, y - center_y) + floor_offset(cell.level, config.levels)
}

fn floor_offset(level: u8, levels: u8) -> Vec2 {
    let midpoint = f32::from(levels.saturating_sub(1)) * 0.5;
    FLOOR_ASCENT * (f32::from(level) - midpoint)
}

fn face_vector(face: HexFace) -> Vec2 {
    let angle = -(face.index() as f32) * std::f32::consts::TAU / 6.0;
    Vec2::new(angle.cos(), angle.sin())
}

#[cfg(test)]
mod tests;
