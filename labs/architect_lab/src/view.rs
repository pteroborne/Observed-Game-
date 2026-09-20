//! The Architect's isometric deck and screen-space interaction boundary.
mod models;
mod scene;
pub(crate) mod ui;

use crate::{LabSession, sim::ArchitectMode};
use bevy::camera::{Viewport, visibility::RenderLayers};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use observed_hex::HexCoord;
use observed_style::architect::{Role, color};

#[cfg(test)]
pub(crate) use scene::BoardVisual;
pub use scene::{rebuild_board, sync_previews};
pub use ui::{
    handle_ui_actions, sync_action_buttons, sync_card_buttons, sync_card_text, sync_charge_pips,
    sync_dynamic_text, sync_layout,
};

const MAP_RENDER_LAYER: usize = 0;
const HUD_RENDER_LAYER: usize = 1;
pub(crate) const DEFAULT_ZOOM: f32 = 0.6;
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 2.5;

#[derive(Component)]
pub(crate) struct BoardCamera;
type UiControlFilter = Or<(With<ui::ArchitectButton>, With<ui::CardButton>)>;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapCameraState {
    /// Pan in camera-plane metres, zoom relative to the fitted active deck.
    pub pan: Vec2,
    pub zoom: f32,
    pub floor: u8,
    pub overview: bool,
    pub lab_controls: bool,
    pub details: bool,
    focused_target: Option<HexCoord>,
    framed: bool,
    viewport_offset: Vec2,
    viewport_size: Vec2,
    units_per_pixel: f32,
    panning: bool,
}
impl Default for MapCameraState {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: DEFAULT_ZOOM,
            floor: 0,
            overview: false,
            lab_controls: false,
            details: false,
            focused_target: None,
            framed: false,
            viewport_offset: Vec2::ZERO,
            viewport_size: Vec2::ONE,
            units_per_pixel: 0.1,
            panning: false,
        }
    }
}
impl MapCameraState {
    pub fn reset_for_mode(&mut self, _mode: ArchitectMode) {
        self.pan = Vec2::ZERO;
        self.zoom = DEFAULT_ZOOM;
        self.focused_target = None;
        self.framed = false;
        self.floor = 0;
        self.overview = false;
        self.panning = false;
    }
    pub fn center(&mut self) {
        self.pan = Vec2::ZERO;
        self.zoom = DEFAULT_ZOOM;
        self.focused_target = None;
        self.framed = false;
    }
    /// Snap only on a new selection, leaving subsequent manual pan/zoom untouched.
    fn sync_selection(&mut self, session: &LabSession) {
        let target = session.target().filter(|c| c.level == self.floor);
        if target != self.focused_target || !self.framed {
            // Begin near an actionable, already-known tile without arming a command.
            let focus = target.or_else(|| {
                if self.framed {
                    None
                } else {
                    session
                        .sim
                        .mutable_targets()
                        .into_iter()
                        .find(|c| c.level == self.floor)
                }
            });
            if let Some(cell) = focus {
                let point = board_position(session.sim.world.config, cell) + Vec3::Y * 0.5;
                self.pan = (camera_rotation().inverse() * point).truncate();
                self.zoom = DEFAULT_ZOOM;
            }
            self.focused_target = target;
            self.framed = true;
        }
    }
    pub(crate) fn placement_visible(&self, session: &LabSession) -> bool {
        !self.lab_controls
            && !self.details
            && session.target().is_some_and(|c| c.level == self.floor)
    }
    fn accepts_board_pointer(&self, cursor: Vec2, session: &LabSession) -> bool {
        let end = self.viewport_offset + self.viewport_size;
        let inspector = Rect::from_corners(end - Vec2::new(276.0, 236.0), end - Vec2::splat(16.0));
        !self.lab_controls
            && !self.details
            && self.contains(cursor)
            && !(self.placement_visible(session) && inspector.contains(cursor))
    }
    pub fn change_floor(&mut self, delta: i8, levels: u8) {
        self.floor =
            (i16::from(self.floor) + i16::from(delta)).rem_euclid(i16::from(levels.max(1))) as u8;
        self.center();
        self.overview = false;
    }
    pub fn zoom_centered(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }
    fn zoom_around(&mut self, factor: f32, cursor: Vec2) {
        let old = self.zoom;
        self.zoom_centered(factor);
        let offset = Vec2::new(
            cursor.x - self.viewport_offset.x - self.viewport_size.x * 0.5,
            -(cursor.y - self.viewport_offset.y - self.viewport_size.y * 0.5),
        );
        self.pan += offset * self.units_per_pixel * (1.0 - self.zoom / old);
    }
    fn contains(&self, cursor: Vec2) -> bool {
        cursor.cmpge(self.viewport_offset).all()
            && cursor
                .cmplt(self.viewport_offset + self.viewport_size)
                .all()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WorkspaceLayout {
    pub hand_height: f32,
    pub map_offset: Vec2,
    pub map_size: Vec2,
}
impl WorkspaceLayout {
    pub fn for_window(size: Vec2) -> Self {
        let hand_height = (size.y * 0.21).clamp(180.0, 220.0);
        Self {
            hand_height,
            map_offset: Vec2::new(0.0, 66.0),
            map_size: Vec2::new(size.x.max(1.0), (size.y - hand_height - 66.0).max(1.0)),
        }
    }
}

fn camera_rotation() -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        observed_style::iso::detent_yaw(0),
        observed_style::iso::ISO_PITCH,
        0.0,
    )
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let models = models::Models::new(&mut meshes, &mut materials);
    commands.insert_resource(models);
    commands.spawn((
        BoardCamera,
        Camera3d::default(),
        Camera {
            clear_color: color(Role::Background).into(),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scale: 0.15,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(100.0, 100.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Name::new("Rogue Architect facility camera"),
    ));
    let hud = commands
        .spawn((
            Camera2d,
            Camera {
                order: 10,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            bevy::ui::IsDefaultUiCamera,
            RenderLayers::layer(HUD_RENDER_LAYER),
            Name::new("Rogue Architect interface camera"),
        ))
        .id();
    let previews = scene::setup_previews(&mut commands, &mut images);
    ui::spawn(&mut commands, hud, &previews);
    commands.insert_resource(previews);
    commands.spawn((
        DirectionalLight {
            illuminance: 6500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(40.0, 90.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::from_layers(&[0, 2, 3, 4, 5, 6]),
        Name::new("Architect studio key"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: color(Role::Text),
        brightness: 360.0,
        ..default()
    });
}

pub fn focus_selected_tile(session: Res<LabSession>, mut state: ResMut<MapCameraState>) {
    state.sync_selection(&session);
}

pub fn sync_camera_viewport(
    windows: Query<&Window>,
    session: Res<LabSession>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut Projection), With<BoardCamera>>,
    mut state: ResMut<MapCameraState>,
) {
    let (Ok(window), Ok((mut camera, mut transform, mut projection))) =
        (windows.single(), cameras.single_mut())
    else {
        return;
    };
    let layout = WorkspaceLayout::for_window(Vec2::new(window.width(), window.height()));
    let physical_position = (layout.map_offset * window.scale_factor()).as_uvec2();
    let physical_size = (layout.map_size * window.scale_factor())
        .as_uvec2()
        .max(UVec2::ONE);
    camera.viewport = Some(Viewport {
        physical_position,
        physical_size,
        depth: 0.0..1.0,
    });
    state.viewport_offset = layout.map_offset;
    state.viewport_size = layout.map_size;
    let config = session.sim.world.config;
    let first = board_position(
        config,
        HexCoord {
            q: 0,
            r: 0,
            level: 0,
        },
    );
    let last = board_position(
        config,
        HexCoord {
            q: config.cols - 1,
            r: config.rows - 1,
            level: 0,
        },
    );
    let framing = observed_style::iso::frame(
        first
            - Vec3::new(
                10.0,
                if state.overview {
                    f32::from(state.floor.min(2)) * 12.0
                } else {
                    0.0
                },
                10.0,
            ),
        last + Vec3::new(10.0, 5.0, 10.0),
        0,
        layout.map_size.x,
        layout.map_size.y,
    );
    state.units_per_pixel = framing.units_per_pixel * state.zoom;
    let center = Vec3::new(0.0, 0.0, 0.0) + camera_rotation() * state.pan.extend(0.0);
    *transform = Transform::from_translation(center + camera_rotation() * Vec3::Z * 300.0)
        .with_rotation(camera_rotation());
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: state.units_per_pixel,
        far: 1000.0,
        ..OrthographicProjection::default_3d()
    });
}

pub fn camera_controls(
    session: Res<LabSession>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window>,
    interactions: Query<&Interaction, UiControlFilter>,
    mut state: ResMut<MapCameraState>,
) {
    let Ok(window) = windows.single() else { return };
    let cursor = window.cursor_position();
    let over_map = cursor.is_some_and(|p| state.accepts_board_pointer(p, &session))
        && !interactions.iter().any(|i| *i != Interaction::None);
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
        let scale = state.units_per_pixel;
        state.pan += Vec2::new(-motion.delta.x, motion.delta.y) * scale;
    }
}

pub fn map_pointer_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    state: Res<MapCameraState>,
    mut session: ResMut<LabSession>,
) {
    let (Ok(window), Ok((camera, transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let picked = window
        .cursor_position()
        .filter(|&p| state.accepts_board_pointer(p, &session))
        .and_then(|p| {
            let ray = camera.viewport_to_world(transform, p).ok()?;
            let distance = ray.intersect_plane(Vec3::Y * 0.5, InfinitePlane3d::new(Vec3::Y))?;
            pick_target(&session, ray.get_point(distance), state.floor)
        });
    if picked != session.hovered_target {
        session.hovered_target = picked;
        session.dirty = true;
    }
    if mouse.just_pressed(MouseButton::Left)
        && let Some(cell) = picked
    {
        session.select_target(cell);
    }
}

fn pick_target(session: &LabSession, point: Vec3, floor: u8) -> Option<HexCoord> {
    // Only the active deck owns pointer hits. Context floors never steal a click.
    session
        .sim
        .mutable_targets()
        .into_iter()
        .filter(|c| c.level == floor)
        .find(|&cell| {
            let local = point - board_position(session.sim.world.config, cell);
            local.x.abs() <= 7.0 && local.z.abs() <= 8.0 - local.x.abs() * 4.0 / 7.0
        })
}
fn board_position(config: observed_facility::hex_wfc::HexWfcConfig, cell: HexCoord) -> Vec3 {
    let mut at = Vec3::from_array(observed_hex::hex_origin(cell));
    at.x -= (f32::from(config.cols - 1) * 14.0 + f32::from(config.rows - 1) * 7.0) * 0.5;
    at.z -= f32::from(config.rows - 1) * 6.0;
    at.y = 0.0;
    at
}

#[cfg(test)]
mod tests;
