//! Isometric facility observer — Arc O's composition instrument.
//!
//! Arc O changes what the hex WFC composes: contiguous districts, per-district
//! profiles, an `Expanse` archetype, authored vertical kits. None of that is
//! falsifiable from a first-person camera inside a corridor, so this lab makes a
//! whole solved facility legible at a glance.
//!
//! It draws as a ship's console would: line work on a dark screen, with hue
//! spent on exactly one question — **green will not rewire, red can**. That is
//! the distinction the solver actually enforces (rooms, ramps and shafts are
//! structural; plain circulation is what relayout eats), and it is the one a
//! reader needs before any other.
//!
//! Density is managed by the layer cycle rather than by drawing less. On a
//! single layer the schematic draws the **real authored hulls**, so you see what
//! the tiles genuinely compose; across all layers it drops to cell shells,
//! because a hundred and twenty thousand hulls is not a diagram.
//!
//! A dev toggle restores the district-coloured solid massing, which answers the
//! other question — where the districts are. Clicking a hex pins it and reports
//! what the solver and the tile compiler decided for it.
//!
//! This is the lab, not the game: it renders ground truth. The in-game map
//! promoted from it reads player knowledge and must never see a cell the team
//! has not discovered.

mod capture;
mod draw;
mod inspect;
mod wire;

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::FRAC_PI_4;
use std::sync::OnceLock;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{PresentMode, WindowResolution};
use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;
use observed_style::{HexSketch, HexSketchRole, SchematicRole, hex_sketch, schematic_screen};

use crate::draw::DrawReport;
use crate::wire::EdgeCache;

/// The pinned baseline corpus. Every Arc O phase re-captures these same five
/// seeds so before/after comparisons are like-for-like; changing this list
/// invalidates the comparison, so treat it as part of the evidence contract.
pub const PRESET_SEEDS: [u64; 5] = [
    0xa11c_e3d0_0000_0008,
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
];

/// True isometric pitch: `atan(1 / sqrt(2))`, the angle at which the three axes
/// of a cube project to equal screen lengths.
const ISO_PITCH: f32 = -0.615_479_7;
const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 1000.0;
/// Only the three forward lateral faces are walked, so each undirected edge is
/// counted once rather than twice from opposite ends.
const FORWARD_FACES: [HexFace; 3] = [HexFace::East, HexFace::SouthEast, HexFace::SouthWest];
/// Default zoom, as a multiple of the fit-everything scale. Well under 1.0
/// because a fitted production layer is 544 cells across a 1600px window - you
/// cannot read a doorway at that size, and the doorways are the point.
const DEFAULT_ZOOM: f32 = 0.34;
const MIN_ZOOM: f32 = 0.04;
const MAX_ZOOM: f32 = 2.0;

#[derive(Component)]
pub(crate) struct LabVisual;

#[derive(Component)]
struct LabStatus;

#[derive(Component)]
struct LabDiagnostics;

#[derive(Component)]
struct LabCamera;

/// Which levels are drawn. The cycle runs `0, 1, … N-1, All` and wraps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layer {
    Single(u8),
    #[default]
    All,
}

impl Layer {
    /// Advance the cycle. `All` is the last stop, not a separate mode.
    #[must_use]
    pub fn next(self, levels: u8) -> Self {
        match self {
            Self::Single(level) if level + 1 < levels => Self::Single(level + 1),
            Self::Single(_) => Self::All,
            Self::All => Self::Single(0),
        }
    }

    #[must_use]
    pub fn previous(self, levels: u8) -> Self {
        match self {
            Self::Single(0) => Self::All,
            Self::Single(level) => Self::Single(level - 1),
            Self::All => Self::Single(levels.saturating_sub(1)),
        }
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Single(level) => format!("layer {level}"),
            Self::All => "all layers".to_string(),
        }
    }
}

/// Schematic is the instrument; solid is the dev view it replaced.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderMode {
    #[default]
    Schematic,
    Solid,
}

#[derive(Resource)]
pub struct LabState {
    pub world: HexWfcWorld,
    /// The projector's answer for every cell: which authored tile, and its real
    /// hulls. `None` when the catalog could not be loaded or projected, in which
    /// case the schematic falls back to cell shells and says so.
    pub geometry: Option<HexWfcGeometrySnapshot>,
    pub seed_index: usize,
    pub layer: Layer,
    pub mode: RenderMode,
    pub selected: Option<HexCoord>,
    /// Draw the real authored hulls instead of the symbolic outline.
    pub detail: bool,
    /// Multiplier on the fit-everything camera scale; smaller is closer in.
    pub zoom: f32,
    /// Screen-aligned offset from the framed centre, in world units.
    pub pan: Vec2,
    /// The fit-everything framing, recomputed on rebuild and then modified by
    /// zoom and pan every frame - so panning never costs a rebuild.
    base_frame: (Transform, f32, f32),
    pub dirty: bool,
    pub reset_count: u32,
    pub(crate) edges: EdgeCache,
    report: DrawReport,
    status: String,
}

/// The lab resolves the workspace tile corpus itself rather than through the
/// game's asset root, so it keeps working regardless of how the shipped
/// executable locates its assets.
fn tile_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("assets/tiles");
    if cwd_relative.exists() {
        return cwd_relative;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
}

fn corpus() -> &'static (Vec<TilePrototype>, Vec<RoomPrototype>) {
    static CORPUS: OnceLock<(Vec<TilePrototype>, Vec<RoomPrototype>)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        match RuntimeHexCatalog::load(&tile_dir(), &slugs) {
            Ok(loaded) => (loaded.cells, loaded.rooms),
            // A missing catalog degrades the schematic to shells rather than
            // taking the lab down; the status line reports it.
            Err(error) => {
                warn!("tile catalog unavailable, drawing shells only: {error}");
                (Vec::new(), Vec::new())
            }
        }
    })
}

impl LabState {
    /// Solve the preset at `seed_index` at production facility scale and project
    /// its geometry once.
    #[must_use]
    pub fn new(seed_index: usize) -> Self {
        let index = seed_index % PRESET_SEEDS.len();
        let seed = PRESET_SEEDS[index];
        let world = HexWfcWorld::generate(seed, HexWfcConfig::arc_default())
            .expect("the production hex config must solve at every preset seed");
        let (cells, rooms) = corpus();
        let geometry = if cells.is_empty() {
            None
        } else {
            match HexWfcGeometrySnapshot::project_with_rooms(&world, cells, rooms) {
                Ok(snapshot) => Some(snapshot),
                Err(error) => {
                    warn!("projection failed, drawing shells only: {error:?}");
                    None
                }
            }
        };
        Self {
            world,
            geometry,
            seed_index: index,
            layer: Layer::Single(0),
            mode: RenderMode::Schematic,
            selected: None,
            detail: false,
            zoom: DEFAULT_ZOOM,
            pan: Vec2::ZERO,
            base_frame: (Transform::IDENTITY, 1.0, 1_000.0),
            dirty: true,
            reset_count: 0,
            edges: EdgeCache::default(),
            report: DrawReport::default(),
            status: String::new(),
        }
    }

    fn reload(&mut self, seed_index: usize) {
        let carried = self.reset_count;
        let (layer, mode, detail, zoom, pan) =
            (self.layer, self.mode, self.detail, self.zoom, self.pan);
        *self = Self::new(seed_index);
        self.reset_count = carried;
        self.layer = layer;
        self.mode = mode;
        self.detail = detail;
        self.zoom = zoom;
        self.pan = pan;
    }

    /// The direction the fixed isometric camera looks along.
    #[must_use]
    pub fn view_direction(&self) -> Vec3 {
        self.base_frame.0.forward().as_vec3()
    }

    /// How many cells of each archetype the current solve placed.
    #[must_use]
    pub fn archetype_census(&self) -> BTreeMap<&'static str, usize> {
        let mut census = BTreeMap::new();
        for placement in self.world.placements.values() {
            if placement.archetype == HexArchetype::Void {
                continue;
            }
            *census
                .entry(archetype_label(placement.archetype))
                .or_insert(0) += 1;
        }
        census
    }

    /// How many cells fall into each composition, and how many connections join
    /// them. These are the numbers Arc O's composition phases are supposed to
    /// move, so they belong on screen rather than in a log.
    #[must_use]
    pub fn composition_census(&self) -> (BTreeMap<&'static str, usize>, usize, usize) {
        let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut lateral = 0;
        let mut vertical = 0;
        let grid = self.world.config.grid();
        for (coord, placement) in &self.world.placements {
            if placement.archetype == HexArchetype::Void {
                continue;
            }
            let role = sketch_role(
                placement.archetype,
                placement.space,
                self.world.room_id_at(*coord).is_some(),
            );
            *by_kind.entry(role.composition().label()).or_insert(0) += 1;
            for face in FORWARD_FACES {
                if placement.is_open(face)
                    && let Some(neighbour) = grid.neighbor(*coord, face)
                    && self
                        .world
                        .placements
                        .get(&neighbour)
                        .is_some_and(|other| other.is_open(face.opposite()))
                {
                    lateral += 1;
                }
            }
            if placement.up != PortClass::Sealed {
                let above = HexCoord {
                    q: coord.q,
                    r: coord.r,
                    level: coord.level.saturating_add(1),
                };
                if self
                    .world
                    .placements
                    .get(&above)
                    .is_some_and(|other| other.down != PortClass::Sealed)
                {
                    vertical += 1;
                }
            }
        }
        (by_kind, lateral, vertical)
    }

    /// How many cells each register owns, and how many disjoint regions those
    /// cells form. A district that is a *place* has few, large regions. Before
    /// Phase 106 this reported hundreds of singletons per register — bug backlog
    /// #14 made visible — and it is now the measurement that shows the spatial
    /// partition holding.
    #[must_use]
    pub fn district_census(&self) -> BTreeMap<ArchitectureRegister, (usize, usize)> {
        let mut cells: BTreeMap<ArchitectureRegister, BTreeSet<HexCoord>> = BTreeMap::new();
        for (coord, register) in &self.world.architecture {
            if self
                .world
                .placements
                .get(coord)
                .is_none_or(|p| p.archetype == HexArchetype::Void)
            {
                continue;
            }
            cells.entry(*register).or_default().insert(*coord);
        }
        cells
            .into_iter()
            .map(|(register, owned)| {
                let regions = count_regions(&owned);
                (register, (owned.len(), regions))
            })
            .collect()
    }
}

/// Flood-fill `cells` over lateral hex adjacency within a level, returning the
/// number of connected components.
fn count_regions(cells: &BTreeSet<HexCoord>) -> usize {
    let mut unvisited = cells.clone();
    let mut regions = 0;
    while let Some(&start) = unvisited.iter().next() {
        regions += 1;
        let mut frontier = vec![start];
        unvisited.remove(&start);
        while let Some(coord) = frontier.pop() {
            for (dq, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)] {
                let (Some(q), Some(r)) = (
                    coord.q.checked_add_signed(dq),
                    coord.r.checked_add_signed(dr),
                ) else {
                    continue;
                };
                let neighbour = HexCoord {
                    q,
                    r,
                    level: coord.level,
                };
                if unvisited.remove(&neighbour) {
                    frontier.push(neighbour);
                }
            }
        }
    }
    regions
}

/// Which style-owned sketch role a cell draws as.
///
/// The heights and widths live in `observed_style` so this lab and the in-game
/// survivor map cannot drift apart; only the mapping from the solver's
/// vocabulary is local. Blueprint membership wins over archetype, because the
/// blueprint is the authority on what composes a room.
#[must_use]
pub fn sketch_role(archetype: HexArchetype, space: HexSpace, in_blueprint: bool) -> HexSketchRole {
    if in_blueprint || space == HexSpace::Room {
        return HexSketchRole::Room;
    }
    match archetype {
        HexArchetype::Void => HexSketchRole::Void,
        HexArchetype::Straight | HexArchetype::Corner => HexSketchRole::Corridor,
        HexArchetype::Junction => HexSketchRole::Junction,
        HexArchetype::RampUp => HexSketchRole::Ramp,
        HexArchetype::RampHead => HexSketchRole::RampHead,
        HexArchetype::Shaft => HexSketchRole::Shaft,
        HexArchetype::Expanse => HexSketchRole::Expanse,
        HexArchetype::Room => HexSketchRole::Room,
    }
}

/// How a cell of this world is drawn: slab height and footprint fill.
#[must_use]
pub fn cell_sketch(world: &HexWfcWorld, coord: HexCoord) -> Option<HexSketch> {
    let placement = world.placements.get(&coord)?;
    Some(hex_sketch(sketch_role(
        placement.archetype,
        placement.space,
        world.room_id_at(coord).is_some(),
    )))
}

#[must_use]
pub fn archetype_label(archetype: HexArchetype) -> &'static str {
    match archetype {
        HexArchetype::Void => "void",
        HexArchetype::Straight => "straight",
        HexArchetype::Corner => "corner",
        HexArchetype::Junction => "junction",
        HexArchetype::Room => "room",
        HexArchetype::RampUp => "ramp",
        HexArchetype::RampHead => "ramp head",
        HexArchetype::Shaft => "shaft",
        HexArchetype::Expanse => "expanse",
    }
}

fn register_label(register: ArchitectureRegister) -> &'static str {
    match register {
        ArchitectureRegister::ShadowScreen => "shadow screen",
        ArchitectureRegister::Monolith => "monolith",
        ArchitectureRegister::OverlitGrid => "overlit grid",
        ArchitectureRegister::Institutional => "institutional",
        ArchitectureRegister::FacetMonument => "facet monument",
        ArchitectureRegister::Megastructure => "megastructure",
        ArchitectureRegister::Wellshaft => "wellshaft",
        ArchitectureRegister::InfiniteGallery => "infinite gallery",
        ArchitectureRegister::Thinning => "thinning",
        ArchitectureRegister::LiminalGrid => "liminal grid",
    }
}

/// Frame the drawn cells in an orthographic isometric view, returning
/// (transform, world-units-per-pixel, far plane).
#[must_use]
pub fn frame_camera(min: Vec3, max: Vec3) -> (Transform, f32, f32) {
    let rotation = Quat::from_euler(EulerRot::YXZ, FRAC_PI_4, ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        extent = extent.max((inverse * (corner - centre)).truncate().abs());
    }
    let scale = (extent.x * 2.0 / WINDOW_WIDTH)
        .max(extent.y * 2.0 / WINDOW_HEIGHT)
        .max(f32::MIN_POSITIVE)
        * 1.08;
    let diagonal = (max - min).length().max(1.0);
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * diagonal).with_rotation(rotation);
    (transform, scale, diagonal * 2.0)
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(schematic_screen()))
        .insert_resource(LabState::new(0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Isometric Observer Lab".to_string(),
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_input,
                handle_click,
                rebuild,
                sync_camera,
                update_status,
            )
                .chain(),
        );

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(capture::CaptureRun::new(dir))
            .add_systems(Update, capture::capture_progress.after(rebuild));
    }
    app.run();
}

fn setup(mut commands: Commands, state: Res<LabState>) {
    let (min, max) =
        draw::drawn_bounds(&state.world, state.layer).unwrap_or((Vec3::ZERO, Vec3::splat(64.0)));
    let (transform, scale, far) = frame_camera(min, max);
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        // The phosphor lines run hot on purpose; HDR plus bloom is what turns
        // them from flat strokes into something that reads as a lit screen.
        Hdr,
        Bloom {
            intensity: 0.62,
            low_frequency_boost: 1.0,
            low_frequency_boost_curvature: 0.9,
            ..Bloom::NATURAL
        },
        Projection::Orthographic(OrthographicProjection {
            scale: scale * DEFAULT_ZOOM,
            near: 0.1,
            far,
            ..OrthographicProjection::default_3d()
        }),
        transform,
        Name::new("Isometric observer camera"),
    ));

    // Studio fill for the dev solid view only; the schematic's lines are unlit
    // and light themselves.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 220.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 2_600.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.95, 0.0)),
        Name::new("Isometric observer key"),
    ));

    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(observed_style::schematic(SchematicRole::Pinned).base_color),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(16),
            padding: UiRect::all(px(8)),
            ..default()
        },
        // The schematic fills the frame, so the console needs its own screen
        // behind it or the numbers read through the geometry.
        BackgroundColor(schematic_screen().with_alpha(0.86)),
    ));
    commands.spawn((
        LabDiagnostics,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(observed_style::schematic(SchematicRole::Selected).base_color),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(16),
            left: px(16),
            padding: UiRect::all(px(8)),
            ..default()
        },
        BackgroundColor(schematic_screen().with_alpha(0.86)),
    ));
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    let levels = state.world.config.levels;
    if keyboard.just_pressed(KeyCode::KeyR) {
        let index = state.seed_index;
        let count = state.reset_count + 1;
        state.reload(index);
        state.reset_count = count;
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let next = (state.seed_index + 1) % PRESET_SEEDS.len();
        state.reload(next);
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let next = (state.seed_index + PRESET_SEEDS.len() - 1) % PRESET_SEEDS.len();
        state.reload(next);
        return;
    }
    if keyboard.just_pressed(KeyCode::Tab) || keyboard.just_pressed(KeyCode::PageUp) {
        state.layer = state.layer.next(levels);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::PageDown) {
        state.layer = state.layer.previous(levels);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyD) {
        state.mode = match state.mode {
            RenderMode::Schematic => RenderMode::Solid,
            RenderMode::Solid => RenderMode::Schematic,
        };
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        state.detail = !state.detail;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::Digit0) {
        state.zoom = DEFAULT_ZOOM;
        state.pan = Vec2::ZERO;
    }
    if keyboard.just_pressed(KeyCode::Escape) && state.selected.is_some() {
        state.selected = None;
        state.dirty = true;
    }
}

fn handle_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<LabCamera>>,
    mut state: ResMut<LabState>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let (Ok(window), Ok((camera, transform))) = (windows.single(), camera.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let picked = inspect::pick(&state.world, state.layer, camera, transform, cursor);
    if picked.is_some() && picked != state.selected {
        state.selected = picked;
        state.dirty = true;
    }
}

fn rebuild(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<LabVisual>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    if let Some((min, max)) = draw::drawn_bounds(&state.world, state.layer) {
        state.base_frame = frame_camera(min, max);
    }
    let report = match state.mode {
        RenderMode::Schematic => {
            draw::schematic_view(&mut commands, &mut state, &mut meshes, &mut materials)
        }
        RenderMode::Solid => draw::solid_view(&mut commands, &state, &mut meshes, &mut materials),
    };
    state.report = report;
}

/// Zoom and pan are applied to the camera every frame rather than folded into
/// the rebuild, so dragging around a layer never re-meshes it.
fn sync_camera(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut state: ResMut<LabState>,
    mut camera: Query<(&mut Projection, &mut Transform), With<LabCamera>>,
) {
    if scroll.delta.y.abs() > f32::EPSILON {
        state.zoom = (state.zoom * 1.14_f32.powf(-scroll.delta.y)).clamp(MIN_ZOOM, MAX_ZOOM);
    }
    // Left drag is reserved for selecting a hex, so panning is the other buttons.
    if mouse.pressed(MouseButton::Right) || mouse.pressed(MouseButton::Middle) {
        let scale = state.base_frame.1 * state.zoom;
        state.pan += Vec2::new(-motion.delta.x, motion.delta.y) * scale;
    }

    let Ok((mut projection, mut transform)) = camera.single_mut() else {
        return;
    };
    let (base, scale, far) = state.base_frame;
    let offset = base.rotation * Vec3::new(state.pan.x, state.pan.y, 0.0);
    *transform =
        Transform::from_translation(base.translation + offset).with_rotation(base.rotation);
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: scale * state.zoom,
        near: 0.1,
        far,
        ..OrthographicProjection::default_3d()
    });
}

fn update_status(
    mut state: ResMut<LabState>,
    mut status: Query<&mut Text, (With<LabStatus>, Without<LabDiagnostics>)>,
    mut diagnostics: Query<&mut Text, (With<LabDiagnostics>, Without<LabStatus>)>,
) {
    if let Ok(mut text) = diagnostics.single_mut() {
        **text = inspect::diagnostics(&state);
    }
    let Ok(mut text) = status.single_mut() else {
        return;
    };

    let census = state.archetype_census();
    let total: usize = census.values().sum();
    let archetypes = census
        .iter()
        .map(|(label, count)| {
            #[allow(clippy::cast_precision_loss)]
            let share = *count as f32 * 100.0 / total.max(1) as f32;
            format!("{label} {count} ({share:.0}%)")
        })
        .collect::<Vec<_>>()
        .join("  ");

    let (by_kind, lateral, vertical) = state.composition_census();
    let composed = by_kind
        .iter()
        .map(|(label, count)| format!("{label} {count}"))
        .collect::<Vec<_>>()
        .join("  ");

    let mut district_lines = state
        .district_census()
        .iter()
        .map(|(register, (cells, regions))| {
            #[allow(clippy::cast_precision_loss)]
            let mean = *cells as f32 / (*regions).max(1) as f32;
            format!(
                "  {:<17} {cells:>5} cells in {regions:>4} regions (mean {mean:.1})",
                register_label(*register)
            )
        })
        .collect::<Vec<_>>();
    district_lines.sort();

    let geometry = if state.report.real_geometry {
        format!(
            "real authored hulls ({} tiles wired)",
            state.report.tiles_wired
        )
    } else if state.geometry.is_some() {
        "symbolic: walls are lines, doorways are gaps  (G for real hulls)".to_string()
    } else {
        "symbolic (tile catalog unavailable)".to_string()
    };
    let mode = match state.mode {
        RenderMode::Schematic => "schematic",
        RenderMode::Solid => "solid (dev)",
    };

    state.status = format!(
        "seed {:#018x}  ({} of {})   {}   {mode}   attempts {}\n\
         green will not rewire    red the solver can rewire    amber selected\n\
         drawing     {geometry}   {} cells, {} lines, {} wall panels\n\
         archetypes  {archetypes}\n\
         composes    {composed}      connects {lateral} lateral | {vertical} vertical\n\
         districts (cells / disjoint regions; a district that is a place has few, large regions):\n\
         {}\n\
         scroll zoom {:.0}%   right-drag pan   Home reset view\n\
         Tab layer   G hull detail   D solid dev   click a hex   Esc clear   [ ] seed   R resolve",
        state.world.seed,
        state.seed_index + 1,
        PRESET_SEEDS.len(),
        state.layer.label(),
        state.world.last_attempts,
        state.report.cells,
        state.report.segments,
        state.report.walls,
        district_lines.join("\n"),
        100.0 / state.zoom,
    );
    **text = state.status.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(LabState::new(0))
            .add_systems(Update, rebuild);
        app.update();
        app
    }

    fn count_visuals(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<LabVisual>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn every_preset_seed_solves_at_production_scale() {
        for (index, seed) in PRESET_SEEDS.iter().enumerate() {
            let state = LabState::new(index);
            assert_eq!(state.world.seed, *seed);
            assert!(!state.world.placements.is_empty());
        }
    }

    #[test]
    fn the_layer_cycle_visits_every_level_then_the_whole_stack() {
        let mut layer = Layer::Single(0);
        let mut seen = Vec::new();
        for _ in 0..12 {
            seen.push(layer);
            layer = layer.next(10);
        }
        for level in 0..10u8 {
            assert!(
                seen.contains(&Layer::Single(level)),
                "layer {level} is in the cycle"
            );
        }
        assert!(
            seen.contains(&Layer::All),
            "the all-layers state is in the cycle"
        );
        assert_eq!(layer, Layer::Single(1), "the cycle wraps");
    }

    #[test]
    fn the_layer_cycle_reverses_exactly() {
        for start in [Layer::Single(0), Layer::Single(5), Layer::All] {
            assert_eq!(start.next(10).previous(10), start, "{start:?} round-trips");
        }
    }

    #[test]
    fn the_schematic_batches_line_work_instead_of_spawning_per_edge() {
        let mut app = test_app();
        let drawn = count_visuals(&mut app);
        let (cells, segments) = {
            let state = app.world().resource::<LabState>();
            (state.report.cells, state.report.segments)
        };
        assert!(cells > 0, "a layer must draw cells");
        assert!(segments > cells, "each cell contributes several segments");
        // At most one line mesh and one wall mesh per role. The whole point of
        // batching is that this count does not grow with the facility.
        assert!(
            drawn <= 2 * 3,
            "the schematic batches per role, saw {drawn} entities for {cells} cells \
             and {segments} segments"
        );
    }

    #[test]
    fn reset_rebuilds_the_projection_without_leaking_entities() {
        let mut app = test_app();
        let baseline = count_visuals(&mut app);
        assert!(baseline > 0);
        for expected in 1..=3 {
            {
                let mut state = app.world_mut().resource_mut::<LabState>();
                let index = state.seed_index;
                let count = state.reset_count + 1;
                state.reload(index);
                state.reset_count = count;
            }
            app.update();
            assert_eq!(count_visuals(&mut app), baseline, "reset {expected} leaked");
            assert_eq!(app.world().resource::<LabState>().reset_count, expected);
        }
    }

    #[test]
    fn reloading_a_seed_keeps_the_view_you_were_using() {
        let mut state = LabState::new(0);
        state.layer = Layer::Single(4);
        state.mode = RenderMode::Solid;
        state.reload(1);
        assert_eq!(state.layer, Layer::Single(4), "the layer survives a reseed");
        assert_eq!(state.mode, RenderMode::Solid, "the view mode survives too");
    }

    #[test]
    fn the_district_census_counts_every_drawn_cell_exactly_once() {
        let state = LabState::new(1);
        let drawn = state
            .world
            .placements
            .values()
            .filter(|p| p.archetype != HexArchetype::Void)
            .count();
        let counted: usize = state
            .district_census()
            .values()
            .map(|(cells, _)| cells)
            .sum();
        assert_eq!(counted, drawn);
    }

    #[test]
    fn the_composition_census_covers_every_drawn_cell_and_finds_connections() {
        let state = LabState::new(0);
        let drawn = state
            .world
            .placements
            .values()
            .filter(|p| p.archetype != HexArchetype::Void)
            .count();
        let (by_kind, lateral, vertical) = state.composition_census();
        assert_eq!(by_kind.values().sum::<usize>(), drawn);
        assert!(lateral > 0 && vertical > 0);
    }

    #[test]
    fn the_lab_and_the_game_share_one_sketch_table() {
        assert_eq!(
            sketch_role(HexArchetype::Shaft, HexSpace::Hall, false),
            HexSketchRole::Shaft
        );
        assert_eq!(
            sketch_role(HexArchetype::Junction, HexSpace::Hall, true),
            HexSketchRole::Room,
            "blueprint membership wins over archetype"
        );
        let room = hex_sketch(HexSketchRole::Room);
        assert!((room.inset - 1.0).abs() < f32::EPSILON);
        assert!(hex_sketch(HexSketchRole::Corridor).inset < room.inset);
    }

    #[test]
    fn every_district_is_a_place_rather_than_confetti() {
        // The inverse of the test this replaced. Before Phase 106 the per-cell
        // lottery gave every register a mean region size under 1.5 — hundreds of
        // singletons scattered across the floor. A district is now a
        // neighbourhood you can cross, so the mean climbs by orders of
        // magnitude, and it must hold for *every* register rather than on
        // average across them.
        let state = LabState::new(0);
        let census = state.district_census();
        assert_eq!(
            census.len(),
            ArchitectureRegister::ALL.len(),
            "every register is somewhere in the facility"
        );
        for (register, (cells, regions)) in &census {
            #[allow(clippy::cast_precision_loss)]
            let mean = *cells as f32 / (*regions).max(1) as f32;
            assert!(
                mean > 8.0,
                "{register:?} averages {mean:.1} cells per region across {cells} cells"
            );
        }
    }
}
