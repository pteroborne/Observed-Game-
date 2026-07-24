//! Hex tile & room lab — authoring preview with a legibility-first UX.
//!
//! Design rules (fixing the prior overlapping-controls / murky-lighting pass):
//! - **One render mode enum** (`Tab`): Lit / Clay / X-ray / Colliders. No
//!   overlapping dev/wireframe/collider booleans.
//! - **Movement keys never double as toggles.** All hotkeys are gated off
//!   while the `F2` menu is open; the menu owns the keyboard when visible.
//! - **Three-tier lighting** in Lit mode: an ambient legibility floor,
//!   interior practical pool lights per occupied cell/level (so sealed tiles
//!   are lit from *inside*, not by a roof-blocked key light alone), and a
//!   shadow-casting district key spot for cutaway/orbit drama.
//! - **Deduplicated composition list**: one entry per authored module
//!   (rotation 0); the register (1-9) re-skins the current composition
//!   instead of multiplying the list by 54.
//!
//! Controls: WASD/mouse move (first person), M camera mode, Tab render mode,
//! X cutaway, O auto-orbit, V volumetrics, B bloom, R respawn, H hot reload,
//! `[`/`]` cycle compositions, 1-9 register, F1 HUD, F2 menu.

mod capture;
mod lab_menu;
pub mod script_runner;

use std::f32::consts::TAU;

use bevy::asset::{AssetPlugin, RenderAssetUsages};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::VolumetricFog;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution};
use lab_menu::{FilterCategory, LabMenuState, MenuTab};
use observed_authoring::{CompiledTileCatalog, Manifest, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{blueprint_cell_archetype, blueprint_for_role};
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style as style;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{ColliderSpec, FpsBody, FpsConfig};
use player_input::PlayerIntent;
use rapier3d::prelude::{SharedShape, Vector as RapierVector};

pub use capture::CaptureRun;
use capture::capture_progress;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    FirstPerson,
    Orbit,
    FreeLook,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::FirstPerson => "First Person",
            Self::Orbit => "Orbit",
            Self::FreeLook => "Free Look",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::FirstPerson => Self::Orbit,
            Self::Orbit => Self::FreeLook,
            Self::FreeLook => Self::FirstPerson,
        }
    }
}

/// The single render mode axis. Replaces the old overlapping
/// `dev_mode` / `strong_wireframe` / `collider_view` booleans.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderMode {
    /// District palette lighting: ambient floor + interior pools + key spot.
    #[default]
    Lit,
    /// Neutral matte study lighting with a subtle wireframe — the authoring
    /// default for reading geometry.
    Clay,
    /// Translucent surfaces + bright wireframe for seeing through structure.
    Xray,
    /// Collision hulls as translucent cyan shells.
    Colliders,
}

impl RenderMode {
    pub const ALL: [Self; 4] = [Self::Lit, Self::Clay, Self::Xray, Self::Colliders];

    pub fn label(self) -> &'static str {
        match self {
            Self::Lit => "Lit (district)",
            Self::Clay => "Clay (study)",
            Self::Xray => "X-ray",
            Self::Colliders => "Colliders",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Lit => Self::Clay,
            Self::Clay => Self::Xray,
            Self::Xray => Self::Colliders,
            Self::Colliders => Self::Lit,
        }
    }
}

/// Room blueprint roles shown in the BROWSE list.
const ROOM_ROLES: [RoomRole; 7] = [
    RoomRole::DecoherenceFork,
    RoomRole::Decision,
    RoomRole::DualStation,
    RoomRole::AnchorCheckpoint,
    RoomRole::GuardianControl,
    RoomRole::Start,
    RoomRole::Exit,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Composition {
    /// 7-hex silo: solid core, helical ring ramp, one bridge per level.
    SiloWellshaft,
    Room(RoomRole),
    /// One authored module at rotation 0; the active register re-skins it.
    SingleTile {
        archetype: String,
        variant: u16,
    },
}

/// Levels of the silo wellshaft showcase composition.
const SILO_LEVELS: usize = 4;
/// Ring cell plan offsets (meters) in climb order: each tile's local
/// north_west face leads to the next entry, so the helix ascends
/// E -> NE -> NW -> W -> SW -> SE and returns to E one level up.
const SILO_RING_ORDER: [(f32, f32); 6] = [
    (14.0, 0.0),
    (7.0, -12.0),
    (-7.0, -12.0),
    (-14.0, 0.0),
    (-7.0, 12.0),
    (7.0, 12.0),
];

/// Placement list for the silo wellshaft: `(tile, origin, rotation)` per
/// instance. The canonical ring tile is authored for the E position (core at
/// local west); position j uses turn (6 - j) % 6 with the catalog's
/// clockwise-rotation convention. One ring tile per level (staggered around
/// the shaft) is the bridge variant.
fn silo_placements(tiles: &[TilePrototype], register: &str) -> Vec<(TilePrototype, Vec3, Quat)> {
    let core = resolve_tile(tiles, "silo_core", 0, register);
    let ring = resolve_tile(tiles, "silo_ring", 0, register);
    let bridge = resolve_tile(tiles, "silo_ring_bridge", 0, register);
    let (Some(core), Some(ring), Some(bridge)) = (core, ring, bridge) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for level in 0..SILO_LEVELS {
        out.push((
            core.clone(),
            Vec3::Y * (level as f32 * TILE_LEVEL_HEIGHT),
            Quat::IDENTITY,
        ));
    }
    let rise = TILE_LEVEL_HEIGHT / 6.0;
    for k in 0..SILO_LEVELS * 6 {
        let j = k % 6;
        let level = k / 6;
        let turn = ((6 - j) % 6) as f32;
        let rotation = Quat::from_rotation_y(-turn * TAU / 6.0);
        let (x, z) = SILO_RING_ORDER[j];
        let origin = Vec3::new(x, k as f32 * rise, z);
        let tile = if j == level % 6 { bridge } else { ring };
        out.push((tile.clone(), origin, rotation));
    }
    out
}

impl Composition {
    pub fn title(&self, tiles: &[TilePrototype], register: &str) -> String {
        match self {
            Self::SiloWellshaft => format!(
                "SILO WELLSHAFT: 7-hex, {SILO_LEVELS} levels — solid core, helical ring ramp, bridge per level"
            ),
            Self::Room(role) => format!("ROOM BLUEPRINT: {role:?}"),
            Self::SingleTile { archetype, variant } => {
                match resolve_tile(tiles, archetype, *variant, register) {
                    Some(tile) => format!(
                        "TILE: {} v{} [{}] — {} level(s), {} hulls",
                        tile.key.archetype,
                        tile.key.variant / 6,
                        tile.key.register,
                        tile.levels,
                        tile.hulls.len()
                    ),
                    None => format!("TILE: {archetype} v{variant} (unresolved)"),
                }
            }
        }
    }

    pub fn slug(&self) -> String {
        match self {
            Self::SiloWellshaft => "silo_wellshaft".to_string(),
            Self::Room(role) => format!("room_{role:?}").to_lowercase(),
            Self::SingleTile { archetype, variant } => format!("{archetype}_v{variant}"),
        }
    }

    pub fn matches_filter(&self, filter: FilterCategory) -> bool {
        match filter {
            FilterCategory::All => true,
            FilterCategory::Blueprints => matches!(self, Self::Room(_)),
            FilterCategory::Chambers => matches!(
                self,
                Self::SingleTile { archetype, .. } if archetype == "sanctuary"
            ),
            FilterCategory::Halls => matches!(
                self,
                Self::SingleTile { archetype, .. }
                    if archetype.starts_with("hall") && !archetype.contains("ramp")
            ),
            FilterCategory::Ramps => matches!(
                self,
                Self::SingleTile { archetype, .. } if archetype.contains("ramp")
            ),
            FilterCategory::Shafts => match self {
                Self::SiloWellshaft => true,
                Self::SingleTile { archetype, .. } => archetype.contains("silo"),
                _ => false,
            },
        }
    }
}

/// Resolve a composition tile to the best catalog entry for `register`:
/// exact register+variant, then any register at that variant, then the
/// archetype's lowest variant.
fn resolve_tile<'a>(
    tiles: &'a [TilePrototype],
    archetype: &str,
    variant: u16,
    register: &str,
) -> Option<&'a TilePrototype> {
    tiles
        .iter()
        .find(|t| {
            t.key.archetype == archetype && t.key.register == register && t.key.variant == variant
        })
        .or_else(|| {
            tiles
                .iter()
                .find(|t| t.key.archetype == archetype && t.key.variant == variant)
        })
        .or_else(|| {
            tiles
                .iter()
                .filter(|t| t.key.archetype == archetype)
                .min_by_key(|t| t.key.variant)
        })
}

#[derive(Component)]
struct TileVisual;

#[derive(Component)]
struct LabStatus;

#[derive(Component)]
struct MenuOverlayRoot;

#[derive(Component)]
struct MenuText;

#[derive(Component)]
struct EyeCamera;

#[derive(Component)]
struct Headlamp;

#[derive(Resource)]
pub struct LabState {
    pub tiles: Vec<TilePrototype>,
    pub current_composition: usize,
    pub compositions: Vec<Composition>,
    pub register_index: usize,
    pub view_mode: ViewMode,
    pub render_mode: RenderMode,
    pub cross_section: bool,
    pub volumetrics: bool,
    pub bloom: bool,
    pub overlay: bool,

    // Simulation & geometry state
    pub scene: RapierTraversalScene,
    pub body: FpsBody,
    pub config: FpsConfig,

    // Camera framing & flight state
    pub center: Vec3,
    pub radius: f32,
    pub height: f32,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub free_fly_pos: Vec3,
    pub free_fly_yaw: f32,
    pub free_fly_pitch: f32,
    pub look_delta: Vec2,
    pub auto_orbit: bool,

    // Scripted walk for capture
    pub scripted_walk: bool,
    pub last_reload: String,
    pub dirty: bool,
}

fn tile_source_dir() -> std::path::PathBuf {
    let root = std::path::PathBuf::from("assets/tiles");
    if root.exists() {
        root
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
    }
}

fn load_corpus_tiles() -> Vec<TilePrototype> {
    let base = tile_source_dir();
    let mut cells = Manifest::load(&base.join("manifest.ron"))
        .expect("tile manifest loads")
        .load_tiles(&base)
        .expect("tile prototypes validate");
    let text = std::fs::read_to_string(base.join("compiled_catalog.ron"))
        .expect("compiled tile catalog is committed");
    let compiled =
        CompiledTileCatalog::from_ron(&text).expect("compiled tile catalog schema loads");
    let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
    let strict = compiled
        .runtime_catalog(&slugs)
        .expect("compiled catalog expands into runtime tiles");
    cells.extend(strict.cells);
    cells
}

fn face_plan_dir(face: observed_hex::HexFace) -> Vec2 {
    let [a, b] = observed_hex::face_edge(face);
    Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5).normalize()
}

fn first_lateral_door(tile: &TilePrototype) -> Option<observed_hex::HexFace> {
    observed_hex::HexFace::LATERAL
        .into_iter()
        .find(|&face| tile.signature.port(face) == observed_hex::PortClass::Door)
}

fn yaw_toward(f: Vec2) -> f32 {
    f.x.atan2(-f.y)
}

fn tile_spawn_pose(tile: &TilePrototype) -> (Vec3, f32, f32) {
    let archetype = tile.key.archetype.as_str();
    if archetype.contains("ramp") {
        let door = first_lateral_door(tile).unwrap_or(observed_hex::HexFace::West);
        let dir = face_plan_dir(door);
        return (
            Vec3::new(dir.x * 6.3, 0.95, dir.y * 6.3),
            yaw_toward(-dir),
            0.42,
        );
    }
    match first_lateral_door(tile) {
        Some(door) => {
            let dir = face_plan_dir(door);
            (
                Vec3::new(dir.x * 5.2, 0.5, dir.y * 5.2),
                yaw_toward(-dir),
                0.0,
            )
        }
        None => (Vec3::new(-4.5, 0.5, 0.0), std::f32::consts::FRAC_PI_2, 0.0),
    }
}

const ANCHOR: HexCoord = HexCoord {
    q: 24,
    r: 24,
    level: 0,
};

/// One list entry per authored module: the rotation-0 representative of each
/// `(archetype, variant)` pair, regardless of register.
fn build_compositions_list(tiles: &[TilePrototype]) -> Vec<Composition> {
    let mut list = Vec::new();
    if tiles.iter().any(|t| t.key.archetype == "silo_ring") {
        list.push(Composition::SiloWellshaft);
    }
    for role in ROOM_ROLES {
        list.push(Composition::Room(role));
    }
    let mut singles: Vec<(String, u16)> = tiles
        .iter()
        .filter(|t| t.key.variant.is_multiple_of(6))
        .map(|t| (t.key.archetype.clone(), t.key.variant))
        .collect();
    singles.sort();
    singles.dedup();
    for (archetype, variant) in singles {
        list.push(Composition::SingleTile { archetype, variant });
    }
    list
}

impl LabState {
    pub fn load() -> Self {
        let tiles = load_corpus_tiles();
        assert!(!tiles.is_empty(), "corpus has tiles");
        let compositions = build_compositions_list(&tiles);
        let register_index = ArchitectureRegister::ALL
            .iter()
            .position(|r| r.slug() == "institutional")
            .unwrap_or(0);
        let mut state = Self {
            scene: RapierTraversalScene::from_arena_spec(&tiles[0].arena_spec()),
            tiles,
            current_composition: 0,
            compositions,
            register_index,
            view_mode: ViewMode::FirstPerson,
            render_mode: RenderMode::default(),
            cross_section: false,
            volumetrics: false,
            bloom: true,
            overlay: true,

            body: FpsBody::spawned(Vec3::ZERO, 0.0),
            config: FpsConfig::default(),

            center: Vec3::ZERO,
            radius: 35.0,
            height: 16.0,
            orbit_yaw: 0.0,
            orbit_pitch: 0.35,
            free_fly_pos: Vec3::new(0.0, 15.0, 30.0),
            free_fly_yaw: 0.0,
            free_fly_pitch: -0.2,
            look_delta: Vec2::ZERO,
            auto_orbit: false,

            scripted_walk: false,
            last_reload: "tiles loaded".to_string(),
            dirty: true,
        };
        state.switch(0);
        state
    }

    pub fn register(&self) -> ArchitectureRegister {
        ArchitectureRegister::ALL[self.register_index % ArchitectureRegister::ALL.len()]
    }

    pub fn composition(&self) -> &Composition {
        &self.compositions[self.current_composition % self.compositions.len()]
    }

    pub fn filtered_compositions(&self, filter: FilterCategory) -> Vec<usize> {
        self.compositions
            .iter()
            .enumerate()
            .filter(|(_, comp)| comp.matches_filter(filter))
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn cycle_filtered(&mut self, forward: bool, filter: FilterCategory) {
        let filtered = self.filtered_compositions(filter);
        if filtered.is_empty() {
            return;
        }
        let pos = filtered
            .iter()
            .position(|&idx| idx == self.current_composition)
            .unwrap_or(0);
        let next_pos = if forward {
            (pos + 1) % filtered.len()
        } else {
            (pos + filtered.len() - 1) % filtered.len()
        };
        self.switch(filtered[next_pos]);
    }

    /// Jump to the first single-tile composition whose archetype contains
    /// `needle` (menu actions, view scripts).
    pub fn jump_to_archetype(&mut self, needle: &str) {
        if let Some(pos) = self.compositions.iter().position(|c| {
            matches!(c, Composition::SingleTile { archetype, .. } if archetype.contains(needle))
        }) {
            self.switch(pos);
        }
    }

    pub fn jump_to_silo_wellshaft(&mut self) {
        if let Some(pos) = self
            .compositions
            .iter()
            .position(|c| *c == Composition::SiloWellshaft)
        {
            self.switch(pos);
        }
    }

    pub fn switch(&mut self, index: usize) {
        self.current_composition = index % self.compositions.len();
        let composition = self.composition().clone();
        let register = self.register();

        match composition {
            Composition::SingleTile { archetype, variant } => {
                if let Some(tile) =
                    resolve_tile(&self.tiles, &archetype, variant, register.slug()).cloned()
                {
                    self.scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
                    let (feet, yaw, pitch) = tile_spawn_pose(&tile);
                    self.body = FpsBody::spawned(feet + Vec3::Y * self.config.half_height, yaw);
                    self.body.pitch = pitch;
                    let height = f32::from(tile.levels) * TILE_LEVEL_HEIGHT;
                    self.center = Vec3::Y * (height * 0.45);
                    self.radius = 22.0 + height * 0.5;
                    self.height = 8.0 + height * 0.75;
                }
            }
            Composition::Room(role) => {
                let blueprint = blueprint_for_role(role);
                let mut collider_specs: Vec<ColliderSpec> = Vec::new();
                let mut origins = Vec::new();
                for (idx, &offset) in blueprint.cells.iter().enumerate() {
                    let coord = HexCoord {
                        q: (i32::from(ANCHOR.q) + offset.0) as u16,
                        r: (i32::from(ANCHOR.r) + offset.1) as u16,
                        level: (i32::from(ANCHOR.level) + offset.2) as u8,
                    };
                    let origin = Vec3::from_array(hex_origin(coord));
                    origins.push(origin);
                    if let Some(archetype) = blueprint_cell_archetype(role, idx)
                        && let Some(tile) = resolve_tile(&self.tiles, archetype, 0, register.slug())
                    {
                        let specs = tile.collider_specs(collider_specs.len() as u32, origin);
                        collider_specs.extend(specs);
                    }
                }
                let count = origins.len().max(1) as f32;
                let center = origins.iter().copied().sum::<Vec3>() / count
                    + Vec3::Y * (TILE_LEVEL_HEIGHT * 0.5);
                let extent = origins
                    .iter()
                    .map(|o| (*o - center).length())
                    .fold(0.0_f32, f32::max);
                self.center = center;
                self.radius = extent + 26.0;
                self.height = (extent + 26.0) * 0.65;
                let spec = observed_traversal::ArenaSpec {
                    colliders: collider_specs,
                    floor_y: 0.0,
                    safety_center: center,
                    safety_half: Vec3::new(extent + 20.0, 24.0, extent + 20.0),
                };
                self.scene = RapierTraversalScene::from_arena_spec(&spec);
                self.body = FpsBody::spawned(center + Vec3::Y * self.config.half_height, 0.0);
            }
            Composition::SiloWellshaft => {
                let mut collider_specs: Vec<ColliderSpec> = Vec::new();
                for (tile, origin, rotation) in silo_placements(&self.tiles, register.slug()) {
                    let specs = tile.collider_specs_with_transform(
                        collider_specs.len() as u32,
                        origin,
                        rotation,
                    );
                    collider_specs.extend(specs);
                }
                let height = SILO_LEVELS as f32 * TILE_LEVEL_HEIGHT;
                self.center = Vec3::Y * (height * 0.5);
                self.radius = 46.0;
                self.height = height * 0.8;
                let spec = observed_traversal::ArenaSpec {
                    colliders: collider_specs,
                    floor_y: 0.0,
                    safety_center: self.center,
                    safety_half: Vec3::new(30.0, height + 12.0, 30.0),
                };
                self.scene = RapierTraversalScene::from_arena_spec(&spec);
                // Spawn on the E-position ring tile, on the ramp, facing up
                // the helix (toward the tile's north_west exit edge).
                self.body = FpsBody::spawned(
                    Vec3::new(14.0, 1.5 + self.config.half_height, 0.0),
                    yaw_toward(Vec2::new(-0.5, -0.86)),
                );
            }
        }
        self.free_fly_pos = self.center + Vec3::new(0.0, self.height, self.radius);
        self.dirty = true;
    }

    pub fn respawn(&mut self) {
        self.switch(self.current_composition);
    }

    pub fn reload_authored_sources(&mut self) -> Result<(), String> {
        let tiles = load_corpus_tiles();
        if tiles.is_empty() {
            return Err("corpus contains no tiles".to_string());
        }
        self.tiles = tiles;
        self.compositions = build_compositions_list(&self.tiles);
        self.switch(self.current_composition);
        self.last_reload = format!("hot reload OK: {} tiles loaded", self.tiles.len());
        Ok(())
    }

    pub fn tile(&self) -> Option<&TilePrototype> {
        match self.composition() {
            Composition::SingleTile { archetype, variant } => {
                resolve_tile(&self.tiles, archetype, *variant, self.register().slug())
            }
            _ => None,
        }
    }

    pub fn feet_height(&self) -> f32 {
        self.body.position.y - self.config.half_height
    }
}

fn sync_wireframe_config(state: Res<LabState>, mut wireframe_config: ResMut<WireframeConfig>) {
    wireframe_config.global = state.render_mode != RenderMode::Lit;
    wireframe_config.default_color = match state.render_mode {
        RenderMode::Clay => Color::srgba(0.10, 0.22, 0.30, 0.8),
        _ => Color::srgb(0.0, 0.94, 1.0),
    };
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.024)))
        .insert_resource(LabState::load())
        .init_resource::<LabMenuState>()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — Hex Tile Lab".to_string(),
                        resolution: WindowResolution::new(1500, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin::default()),
            WireframePlugin::default(),
        ))
        .add_systems(Startup, (setup, spawn_menu_overlay))
        .add_systems(FixedUpdate, step_body)
        .add_systems(
            Update,
            (
                handle_input,
                handle_menu_navigation,
                rebuild_visuals,
                sync_wireframe_config,
                sync_camera,
                sync_render_env,
                update_status,
                update_menu_ui,
            )
                .chain(),
        );

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRun::new(path))
            .add_systems(Update, capture_progress.after(sync_camera));
    }

    if let Some(script_path) = script_runner::ScriptExecution::detect_script() {
        match script_runner::ViewScript::load_from_file(&script_path) {
            Ok(script) => {
                println!(
                    "hex_tile_lab: loaded view script -> {}",
                    script_path.display()
                );
                app.insert_resource(script_runner::ScriptExecution {
                    script: Some(script),
                    script_path: Some(script_path),
                    configured: false,
                    captured: false,
                    timer: 0.0,
                })
                .add_systems(
                    Update,
                    (script_runner::run_script_system,).after(sync_camera),
                );
            }
            Err(error) => {
                eprintln!(
                    "hex_tile_lab: failed to load script {}: {error}",
                    script_path.display()
                );
            }
        }
    }

    app.run();
}

fn setup(mut commands: Commands, mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    commands
        .spawn((
            EyeCamera,
            Camera3d::default(),
            Hdr,
            Bloom {
                intensity: 0.06,
                ..Bloom::NATURAL
            },
            Transform::from_xyz(0.0, 1.6, 0.0),
            Name::new("Lab Camera"),
        ))
        .with_children(|eye| {
            eye.spawn((
                Headlamp,
                PointLight {
                    intensity: 90_000.0,
                    range: 16.0,
                    color: Color::srgb(0.92, 0.95, 1.0),
                    shadows_enabled: false,
                    ..default()
                },
                Name::new("Headlamp"),
            ));
        });

    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));

    if std::env::var("OBSERVED2_CAPTURE").is_err()
        && let Ok(mut cursor) = cursors.single_mut()
    {
        cursor.grab_mode = CursorGrabMode::Confined;
        cursor.visible = false;
    }
}

fn spawn_menu_overlay(mut commands: Commands) {
    commands
        .spawn((
            MenuOverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                max_width: Val::Px(620.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.94)),
            BorderColor::all(Color::srgba(0.0, 0.9, 1.0, 0.85)),
            GlobalZIndex(100),
            Visibility::Hidden,
            Name::new("Lab Menu"),
        ))
        .with_children(|root| {
            root.spawn((
                MenuText,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.94, 1.0)),
            ));
        });
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motions: MessageReader<MouseMotion>,
    mut scroll: MessageReader<MouseWheel>,
    mut state: ResMut<LabState>,
    mut menu_state: ResMut<LabMenuState>,
) {
    let mut delta = Vec2::ZERO;
    for motion in motions.read() {
        delta += motion.delta;
    }
    state.look_delta = delta;

    let mut wheel = 0.0;
    for ev in scroll.read() {
        wheel += ev.y;
    }

    if keyboard.just_pressed(KeyCode::F2) {
        menu_state.is_open = !menu_state.is_open;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        state.overlay = !state.overlay;
    }

    // The menu owns the keyboard while open: no lab hotkeys, no camera input.
    if menu_state.is_open {
        if keyboard.just_pressed(KeyCode::Escape) {
            menu_state.is_open = false;
        }
        state.look_delta = Vec2::ZERO;
        return;
    }

    if keyboard.just_pressed(KeyCode::Tab) {
        state.render_mode = state.render_mode.next();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        state.view_mode = state.view_mode.next();
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        state.cross_section = !state.cross_section;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyO) {
        state.auto_orbit = !state.auto_orbit;
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        state.volumetrics = !state.volumetrics;
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        state.bloom = !state.bloom;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.respawn();
    }
    if keyboard.just_pressed(KeyCode::KeyH)
        && let Err(error) = state.reload_authored_sources()
    {
        state.last_reload = format!("hot reload FAILED: {error}");
    }

    const DIGITS: [KeyCode; 9] = [
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
    for (index, key) in DIGITS.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            state.register_index = index;
            let comp = state.current_composition;
            state.switch(comp);
            state.dirty = true;
        }
    }

    if keyboard.just_pressed(KeyCode::BracketRight) {
        let filter = menu_state.active_filter;
        state.cycle_filtered(true, filter);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let filter = menu_state.active_filter;
        state.cycle_filtered(false, filter);
    }

    match state.view_mode {
        ViewMode::Orbit => {
            if mouse_buttons.pressed(MouseButton::Left) {
                state.orbit_yaw += delta.x * 0.005;
                state.orbit_pitch = (state.orbit_pitch - delta.y * 0.005).clamp(0.05, 1.4);
            }
            if wheel != 0.0 {
                state.radius = (state.radius - wheel * 2.0).clamp(5.0, 100.0);
            }
        }
        ViewMode::FreeLook => {
            if mouse_buttons.pressed(MouseButton::Right) {
                state.free_fly_yaw -= delta.x * 0.003;
                state.free_fly_pitch = (state.free_fly_pitch - delta.y * 0.003).clamp(-1.4, 1.4);
            }
            let forward = Quat::from_rotation_y(state.free_fly_yaw)
                * Quat::from_rotation_x(state.free_fly_pitch)
                * -Vec3::Z;
            let right = Quat::from_rotation_y(state.free_fly_yaw) * Vec3::X;
            let mut move_vec = Vec3::ZERO;
            if keyboard.pressed(KeyCode::KeyW) {
                move_vec += forward;
            }
            if keyboard.pressed(KeyCode::KeyS) {
                move_vec -= forward;
            }
            if keyboard.pressed(KeyCode::KeyD) {
                move_vec += right;
            }
            if keyboard.pressed(KeyCode::KeyA) {
                move_vec -= right;
            }
            if keyboard.pressed(KeyCode::KeyE) {
                move_vec += Vec3::Y;
            }
            if keyboard.pressed(KeyCode::KeyQ) {
                move_vec -= Vec3::Y;
            }
            let speed = if keyboard.pressed(KeyCode::ShiftLeft) {
                18.0
            } else {
                8.0
            };
            state.free_fly_pos += move_vec * speed * 0.016;
        }
        ViewMode::FirstPerson => {}
    }
}

fn handle_menu_navigation(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    mut menu_state: ResMut<LabMenuState>,
) {
    if !menu_state.is_open {
        return;
    }

    if keyboard.just_pressed(KeyCode::ArrowRight) {
        menu_state.next_tab();
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        menu_state.prev_tab();
    }

    let tab = menu_state.tab();
    let max_items = match tab {
        MenuTab::Browse => FilterCategory::ALL.len(),
        MenuTab::Registers => ArchitectureRegister::ALL.len(),
        MenuTab::Render => 8,
        MenuTab::Actions => 6,
    };

    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu_state.selected_item = (menu_state.selected_item + 1) % max_items;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu_state.selected_item = (menu_state.selected_item + max_items - 1) % max_items;
    }

    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        let sel = menu_state.selected_item;
        match tab {
            MenuTab::Browse => {
                let filter = FilterCategory::ALL[sel % FilterCategory::ALL.len()];
                menu_state.active_filter = filter;
                let filtered = state.filtered_compositions(filter);
                if !filtered.is_empty() {
                    state.switch(filtered[0]);
                }
            }
            MenuTab::Registers => {
                state.register_index = sel % ArchitectureRegister::ALL.len();
                let comp = state.current_composition;
                state.switch(comp);
                state.dirty = true;
            }
            MenuTab::Render => match sel {
                0..=3 => {
                    state.render_mode = RenderMode::ALL[sel];
                    state.dirty = true;
                }
                4 => {
                    state.cross_section = !state.cross_section;
                    state.dirty = true;
                }
                5 => state.volumetrics = !state.volumetrics,
                6 => state.bloom = !state.bloom,
                7 => state.auto_orbit = !state.auto_orbit,
                _ => {}
            },
            MenuTab::Actions => match sel {
                0 => state.jump_to_archetype("sanctuary"),
                1 => state.jump_to_silo_wellshaft(),
                2 => state.jump_to_archetype("ramp"),
                3 => {
                    let _ = state.reload_authored_sources();
                }
                4 => state.respawn(),
                _ => {}
            },
        }
    }
}

fn intent_from_keys(keyboard: &ButtonInput<KeyCode>, look: Vec2) -> PlayerIntent {
    let mut movement = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        movement.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        movement.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        movement.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        movement.x -= 1.0;
    }
    PlayerIntent {
        movement,
        look,
        jump_pressed: keyboard.just_pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft),
        ..PlayerIntent::default()
    }
}

fn step_body(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu_state: Res<LabMenuState>,
    mut state: ResMut<LabState>,
) {
    if state.view_mode != ViewMode::FirstPerson {
        return;
    }
    // The menu owns the keyboard while open.
    if menu_state.is_open && !state.scripted_walk {
        return;
    }
    let intent = if state.scripted_walk {
        PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..PlayerIntent::default()
        }
    } else {
        intent_from_keys(&keyboard, state.look_delta * 0.06)
    };
    state.look_delta = Vec2::ZERO;

    let LabState {
        scene,
        body,
        config,
        ..
    } = &mut *state;
    step_character(scene, body, intent, config, 1.0 / 60.0);
    if state.body.position.y < -15.0 {
        state.respawn();
    }
}

type ShowcaseCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        Has<Bloom>,
        Has<VolumetricFog>,
        Has<DistanceFog>,
    ),
    With<EyeCamera>,
>;

fn sync_camera(time: Res<Time>, mut state: ResMut<LabState>, mut camera: ShowcaseCameraQuery) {
    let Ok((_, mut transform, _, _, _)) = camera.single_mut() else {
        return;
    };

    match state.view_mode {
        ViewMode::FirstPerson => {
            let eye = state.body.position
                + Vec3::Y * (state.config.eye_height - state.config.half_height);
            transform.translation = eye;
            transform.rotation =
                Quat::from_rotation_y(-state.body.yaw) * Quat::from_rotation_x(state.body.pitch);
        }
        ViewMode::Orbit => {
            if state.auto_orbit {
                state.orbit_yaw = (state.orbit_yaw + time.delta_secs() * 0.35) % TAU;
            }
            let pos = state.center
                + Vec3::new(
                    state.radius * state.orbit_yaw.cos() * state.orbit_pitch.cos(),
                    state.height + state.radius * state.orbit_pitch.sin(),
                    state.radius * state.orbit_yaw.sin() * state.orbit_pitch.cos(),
                );
            *transform = Transform::from_translation(pos).looking_at(state.center, Vec3::Y);
        }
        ViewMode::FreeLook => {
            transform.translation = state.free_fly_pos;
            transform.rotation = Quat::from_rotation_y(state.free_fly_yaw)
                * Quat::from_rotation_x(state.free_fly_pitch);
        }
    }
}

/// Keep camera post effects and the headlamp consistent with the render mode:
/// bloom / volumetrics / distance fog are Lit-mode effects, and the headlamp
/// is a Lit-mode safety light (Clay/X-ray/Colliders are self-illuminating).
fn sync_render_env(
    state: Res<LabState>,
    mut camera: ShowcaseCameraQuery,
    mut headlamps: Query<&mut PointLight, With<Headlamp>>,
    mut commands: Commands,
) {
    let Ok((cam_entity, _, has_bloom, has_vol, has_fog)) = camera.single_mut() else {
        return;
    };
    let lit = state.render_mode == RenderMode::Lit;

    let want_bloom = state.bloom && lit;
    if want_bloom && !has_bloom {
        commands.entity(cam_entity).insert(Bloom {
            intensity: 0.06,
            ..Bloom::NATURAL
        });
    } else if !want_bloom && has_bloom {
        commands.entity(cam_entity).remove::<Bloom>();
    }

    let want_vol = state.volumetrics && lit;
    if want_vol && !has_vol {
        commands.entity(cam_entity).insert(VolumetricFog {
            ambient_color: Color::srgb(0.32, 0.50, 0.78),
            ambient_intensity: 0.45,
            step_count: 64,
            ..default()
        });
    } else if !want_vol && has_vol {
        commands.entity(cam_entity).remove::<VolumetricFog>();
    }

    if lit && !has_fog {
        let palette = style::architecture(state.register());
        commands.entity(cam_entity).insert(DistanceFog {
            color: palette.fog_color,
            falloff: FogFalloff::Linear {
                start: 60.0,
                end: 170.0,
            },
            ..default()
        });
    } else if !lit && has_fog {
        commands.entity(cam_entity).remove::<DistanceFog>();
    }

    if let Ok(mut lamp) = headlamps.single_mut() {
        lamp.intensity = match state.render_mode {
            RenderMode::Lit | RenderMode::Clay => 90_000.0,
            RenderMode::Xray | RenderMode::Colliders => 0.0,
        };
    }
}

/// A hull counts as ceiling for the cutaway when it sits entirely in the top
/// band of the composition's vertical extent.
fn is_ceiling(hull: &[Vec3], top_y: f32) -> bool {
    hull.iter().all(|point| point.y >= top_y - 1.5)
}

fn is_cutaway_wall(hull: &[Vec3], center: Vec3) -> bool {
    hull.iter()
        .any(|p| p.z > center.z + 1.5 || p.x > center.x + 3.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceKind {
    Floor,
    Wall,
    Trim,
    Ceiling,
}

fn hull_surface_kind(hull: &[Vec3], top_y: f32) -> SurfaceKind {
    if is_ceiling(hull, top_y) {
        SurfaceKind::Ceiling
    } else if hull.iter().all(|p| p.y <= 0.4) {
        SurfaceKind::Floor
    } else if hull.iter().all(|p| p.y <= 4.0 && p.y >= 0.2) {
        SurfaceKind::Trim
    } else {
        SurfaceKind::Wall
    }
}

/// Faces meeting at less than this dihedral angle shade as one smooth
/// surface; sharper edges stay crisp. Chamfers and slope transitions
/// (~20-45 degrees) blend; hex corners and wall-floor joints (60-90) do not.
const SMOOTH_CREASE_COS: f32 = 0.70; // cos(~45.5 degrees)

/// Build a render mesh from a convex hull with crease-angle normal smoothing
/// and per-face box-projected UVs. Replaces the old forced flat shading
/// (every facet hard-edged: the "assembled blocks" look) and the old planar
/// UV projection (streaky textures on off-axis walls).
fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let points: Vec<_> = hull
        .iter()
        .map(|v| RapierVector::new(v.x, v.y, v.z))
        .collect();
    let shape = SharedShape::convex_hull(&points)?;
    let (vertices, indices) = shape.as_convex_polyhedron()?.to_trimesh();

    // Face normals per triangle.
    let tri_normal = |tri: &[u32; 3]| -> Vec3 {
        let a = vertices[tri[0] as usize];
        let b = vertices[tri[1] as usize];
        let c = vertices[tri[2] as usize];
        let a = Vec3::new(a.x, a.y, a.z);
        let b = Vec3::new(b.x, b.y, b.z);
        let c = Vec3::new(c.x, c.y, c.z);
        (b - a).cross(c - a).normalize_or_zero()
    };
    let face_normals: Vec<Vec3> = indices.iter().map(tri_normal).collect();

    // Group triangles by shared (quantized) vertex position so smoothing can
    // average across coincident corners from different triangles.
    let key = |p: &RapierVector| -> (i64, i64, i64) {
        (
            (p.x * 1024.0).round() as i64,
            (p.y * 1024.0).round() as i64,
            (p.z * 1024.0).round() as i64,
        )
    };
    let mut adjacent: std::collections::HashMap<(i64, i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (tri_idx, tri) in indices.iter().enumerate() {
        for &vert in tri {
            adjacent
                .entry(key(&vertices[vert as usize]))
                .or_default()
                .push(tri_idx);
        }
    }

    // One output vertex per triangle corner: position, smoothed normal, and a
    // UV projected along the face's dominant axis (box mapping).
    let mut positions = Vec::with_capacity(indices.len() * 3);
    let mut normals = Vec::with_capacity(indices.len() * 3);
    let mut uvs = Vec::with_capacity(indices.len() * 3);
    for (tri_idx, tri) in indices.iter().enumerate() {
        let face_n = face_normals[tri_idx];
        for &vert in tri {
            let p = vertices[vert as usize];
            let mut n = Vec3::ZERO;
            for &other in &adjacent[&key(&p)] {
                let other_n = face_normals[other];
                if face_n.dot(other_n) >= SMOOTH_CREASE_COS {
                    n += other_n;
                }
            }
            let n = n.normalize_or(face_n);
            positions.push([p.x, p.y, p.z]);
            normals.push([n.x, n.y, n.z]);
            let abs = face_n.abs();
            let uv = if abs.y >= abs.x && abs.y >= abs.z {
                [p.x * 0.25, p.z * 0.25]
            } else if abs.x >= abs.z {
                [p.z * 0.25, p.y * 0.25]
            } else {
                [p.x * 0.25, p.y * 0.25]
            };
            uvs.push(uv);
        }
    }
    let index_list: Vec<u32> = (0..positions.len() as u32).collect();

    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(index_list)),
    )
}

fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    visuals: Query<Entity, With<TileVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }

    let register = state.register();
    let palette = style::architecture(register);
    let mode = state.render_mode;

    let floor_tex: Handle<Image> = asset_server.load("textures/floor.png");
    let wall_tex: Handle<Image> = asset_server.load("textures/wall.png");
    let ceiling_tex: Handle<Image> = asset_server.load("textures/ceiling.png");

    // Tier 1: the ambient legibility floor. District palettes may be moody in
    // the shipped game, but in the lab geometry legibility wins: Lit clamps
    // ambient up to a floor; the study modes use bright neutral ambient.
    match mode {
        RenderMode::Lit => {
            // Cutaway views are inspection views: open roofs read as caves
            // without a stronger fill, so the ambient floor rises with it.
            let floor = if state.cross_section { 340.0 } else { 200.0 };
            commands.insert_resource(GlobalAmbientLight {
                color: palette.ambient_color,
                brightness: palette.ambient_brightness.max(floor),
                ..default()
            });
            commands.insert_resource(ClearColor(palette.fog_color));
        }
        RenderMode::Clay => {
            commands.insert_resource(GlobalAmbientLight {
                color: Color::srgb(1.0, 0.98, 0.94),
                brightness: 900.0,
                ..default()
            });
            commands.insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.06)));
        }
        RenderMode::Xray | RenderMode::Colliders => {
            commands.insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 300.0,
                ..default()
            });
            commands.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.024)));
        }
    }

    let mut get_surface_material = |kind: SurfaceKind| -> Handle<StandardMaterial> {
        match mode {
            RenderMode::Xray => {
                let color = match kind {
                    SurfaceKind::Floor => Color::srgba(1.0, 0.72, 0.10, 0.45),
                    SurfaceKind::Wall => Color::srgba(0.0, 0.75, 0.95, 0.20),
                    SurfaceKind::Trim => Color::srgba(0.05, 0.12, 0.35, 0.65),
                    SurfaceKind::Ceiling => Color::srgba(0.0, 0.50, 0.70, 0.15),
                };
                materials.add(StandardMaterial {
                    base_color: color,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })
            }
            RenderMode::Colliders => materials.add(StandardMaterial {
                base_color: Color::srgba(0.3, 0.9, 1.0, 0.35),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            }),
            RenderMode::Clay => {
                // Neutral matte clay: no emissive, shading carries the form.
                let (color, tex) = match kind {
                    SurfaceKind::Floor => (Color::srgb(0.60, 0.59, 0.57), Some(floor_tex.clone())),
                    SurfaceKind::Wall => (Color::srgb(0.55, 0.55, 0.54), Some(wall_tex.clone())),
                    SurfaceKind::Trim => (Color::srgb(0.42, 0.43, 0.45), Some(wall_tex.clone())),
                    SurfaceKind::Ceiling => {
                        (Color::srgb(0.50, 0.50, 0.52), Some(ceiling_tex.clone()))
                    }
                };
                materials.add(StandardMaterial {
                    base_color: color,
                    base_color_texture: tex,
                    perceptual_roughness: 0.92,
                    ..default()
                })
            }
            RenderMode::Lit => {
                // District materials: near-zero emissive — light, not glow,
                // shapes the space. Trim keeps a whisper of accent.
                let (color, tex, emissive) = if register == ArchitectureRegister::Monolith {
                    let (color, tex) = match kind {
                        SurfaceKind::Floor => {
                            (Color::srgb(0.26, 0.255, 0.245), Some(floor_tex.clone()))
                        }
                        SurfaceKind::Wall => {
                            (Color::srgb(0.24, 0.235, 0.225), Some(wall_tex.clone()))
                        }
                        SurfaceKind::Trim => {
                            (Color::srgb(0.18, 0.175, 0.17), Some(wall_tex.clone()))
                        }
                        SurfaceKind::Ceiling => {
                            (Color::srgb(0.16, 0.155, 0.15), Some(ceiling_tex.clone()))
                        }
                    };
                    let emissive = if kind == SurfaceKind::Trim {
                        palette.accent * 0.08
                    } else {
                        LinearRgba::BLACK
                    };
                    (color, tex, emissive)
                } else {
                    let (color, tex) = match kind {
                        SurfaceKind::Floor => {
                            (Color::srgb(0.50, 0.52, 0.55), Some(floor_tex.clone()))
                        }
                        SurfaceKind::Wall => {
                            (Color::srgb(0.45, 0.47, 0.50), Some(wall_tex.clone()))
                        }
                        SurfaceKind::Trim => {
                            (Color::srgb(0.35, 0.37, 0.40), Some(wall_tex.clone()))
                        }
                        SurfaceKind::Ceiling => {
                            (Color::srgb(0.30, 0.32, 0.35), Some(ceiling_tex.clone()))
                        }
                    };
                    let emissive = if kind == SurfaceKind::Trim {
                        palette.accent * 0.10
                    } else {
                        LinearRgba::BLACK
                    };
                    (color, tex, emissive)
                };
                materials.add(StandardMaterial {
                    base_color: color,
                    base_color_texture: tex,
                    emissive,
                    perceptual_roughness: 0.90,
                    ..default()
                })
            }
        }
    };

    let composition = state.composition().clone();
    let cross_section = state.cross_section;
    let center = state.center;

    // Authored practical positions accumulate with the exact geometry
    // transforms. Only source data without explicit lights uses a centered fallback.
    let mut pool_origins: Vec<Vec3> = Vec::new();

    let mut spawn_hull_entity = |commands: &mut Commands,
                                 meshes: &mut ResMut<Assets<Mesh>>,
                                 hull: &[Vec3],
                                 transform: Transform,
                                 top_y: f32,
                                 name: String| {
        let kind = hull_surface_kind(hull, top_y);
        let mat = get_surface_material(kind);
        if let Some(mesh) = hull_mesh(hull) {
            commands.spawn((
                TileVisual,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                transform,
                Name::new(name),
            ));
        }
    };

    match composition {
        Composition::SingleTile {
            ref archetype,
            variant,
        } => {
            if let Some(tile) = resolve_tile(&state.tiles, archetype, variant, register.slug()) {
                let tile = tile.clone();
                let top_y = f32::from(tile.levels) * TILE_LEVEL_HEIGHT;
                if tile.lights.is_empty() {
                    for level in 0..tile.levels {
                        pool_origins.push(Vec3::Y * (f32::from(level) * TILE_LEVEL_HEIGHT + 5.4));
                    }
                } else {
                    pool_origins.extend(tile.lights.iter().map(|light| light.position));
                }
                for hull in &tile.hulls {
                    if cross_section && is_ceiling(hull, top_y) {
                        continue;
                    }
                    spawn_hull_entity(
                        &mut commands,
                        &mut meshes,
                        hull,
                        Transform::IDENTITY,
                        top_y,
                        "Tile hull".to_string(),
                    );
                }
            }
        }
        Composition::Room(role) => {
            let blueprint = blueprint_for_role(role);
            for (idx, &offset) in blueprint.cells.iter().enumerate() {
                let coord = HexCoord {
                    q: (i32::from(ANCHOR.q) + offset.0) as u16,
                    r: (i32::from(ANCHOR.r) + offset.1) as u16,
                    level: (i32::from(ANCHOR.level) + offset.2) as u8,
                };
                let origin = Vec3::from_array(hex_origin(coord));
                if let Some(archetype) = blueprint_cell_archetype(role, idx)
                    && let Some(tile) = resolve_tile(&state.tiles, archetype, 0, register.slug())
                {
                    let tile = tile.clone();
                    let top_y = f32::from(tile.levels) * TILE_LEVEL_HEIGHT;
                    if tile.lights.is_empty() {
                        pool_origins.push(origin + Vec3::Y * 5.4);
                    } else {
                        pool_origins
                            .extend(tile.lights.iter().map(|light| origin + light.position));
                    }
                    for hull in &tile.hulls {
                        if cross_section
                            && (is_ceiling(hull, top_y) || is_cutaway_wall(hull, center - origin))
                        {
                            continue;
                        }
                        spawn_hull_entity(
                            &mut commands,
                            &mut meshes,
                            hull,
                            Transform::from_translation(origin),
                            top_y,
                            format!("Room cell {archetype}"),
                        );
                    }
                }
            }
        }
        Composition::SiloWellshaft => {
            for (tile, origin, rotation) in silo_placements(&state.tiles, register.slug()) {
                let top_y = f32::from(tile.levels) * TILE_LEVEL_HEIGHT;
                let transform = Transform::from_translation(origin).with_rotation(rotation);
                pool_origins.extend(
                    tile.lights
                        .iter()
                        .map(|light| origin + rotation * light.position),
                );
                for hull in &tile.hulls {
                    // Dollhouse section: drop only tall wall shells on the
                    // south (camera-default) side. Ramps, landings, and the
                    // core stay, so the helix stays readable.
                    if cross_section && tile.key.archetype != "silo_core" {
                        let min_y = hull.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                        let max_y = hull.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                        let is_wall_shell = max_y - min_y > 4.0;
                        let south_side = hull.iter().any(|p| (origin + rotation * *p).z > 2.0);
                        if is_wall_shell && south_side {
                            continue;
                        }
                    }
                    spawn_hull_entity(
                        &mut commands,
                        &mut meshes,
                        hull,
                        transform,
                        top_y,
                        format!("Silo {}", tile.key.archetype),
                    );
                }
            }
        }
    }

    if mode == RenderMode::Clay {
        // Studio rig: two shadowless directionals. Without shadows they light
        // every face by orientation alone — roofs and walls never occlude, so
        // form reads everywhere. This is the guaranteed-legibility mode.
        commands.spawn((
            TileVisual,
            DirectionalLight {
                illuminance: 5_500.0,
                color: Color::srgb(1.0, 0.98, 0.92),
                shadows_enabled: false,
                ..default()
            },
            Transform::default().looking_to(Vec3::new(-0.5, -1.0, -0.35).normalize(), Vec3::Y),
            Name::new("Clay key"),
        ));
        commands.spawn((
            TileVisual,
            DirectionalLight {
                illuminance: 1_800.0,
                color: Color::srgb(0.82, 0.88, 1.0),
                shadows_enabled: false,
                ..default()
            },
            Transform::default().looking_to(Vec3::new(0.55, -0.35, 0.5).normalize(), Vec3::Y),
            Name::new("Clay rim"),
        ));
    }

    if mode == RenderMode::Lit && cross_section {
        // Inspection fill for open-roof views: soft, shadowless, just enough
        // to keep backfaces out of pure black without flattening the mood.
        // The second, lower-angle fill catches vertical faces (pylons).
        for (illuminance, dir) in [
            (2_400.0, Vec3::new(-0.3, -1.0, 0.25)),
            (1_200.0, Vec3::new(0.7, -0.35, 0.55)),
        ] {
            commands.spawn((
                TileVisual,
                DirectionalLight {
                    illuminance,
                    color: Color::srgb(0.85, 0.90, 1.0),
                    shadows_enabled: false,
                    ..default()
                },
                Transform::default().looking_to(dir.normalize(), Vec3::Y),
                Name::new("Cutaway inspection fill"),
            ));
        }
    }

    if mode == RenderMode::Lit {
        // Tier 3: exact authored practicals, backed by fixture geometry.
        for origin in &pool_origins {
            commands.spawn((
                TileVisual,
                PointLight {
                    color: palette.light_color,
                    intensity: 900_000.0,
                    range: 14.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(*origin),
                Name::new("Authored practical"),
            ));
        }

        // Tier 2: the district key spot — sealed-view mood only. In cutaway
        // the inspection fills own the frame: a high-intensity shadow-casting
        // spot enclosed with a tall central occluder renders the occluder
        // pitch black behind a tall central occluder, and inspection is
        // the point of a cutaway anyway.
        let key_height = pool_origins.iter().map(|o| o.y).fold(0.0_f32, f32::max) + 14.0;
        if !cross_section {
            if register == ArchitectureRegister::Monolith {
                commands.spawn((
                    TileVisual,
                    SpotLight {
                        color: Color::srgb(0.92, 0.95, 1.0),
                        intensity: 30_000_000.0,
                        range: 65.0,
                        radius: 0.02,
                        inner_angle: 0.16,
                        outer_angle: 0.22,
                        shadows_enabled: true,
                        ..default()
                    },
                    Transform::from_translation(center + Vec3::new(-12.0, key_height, -2.0))
                        .looking_at(center + Vec3::new(3.0, 0.0, 2.5), Vec3::Y),
                    Name::new("Monolith hard key light"),
                ));
            } else {
                commands.spawn((
                    TileVisual,
                    SpotLight {
                        color: palette.key_color,
                        intensity: palette.key_intensity * 0.65,
                        range: palette.key_range,
                        radius: palette.key_radius,
                        inner_angle: palette.key_inner_angle,
                        outer_angle: palette.key_outer_angle,
                        shadows_enabled: palette.key_shadows_enabled,
                        ..default()
                    },
                    Transform::from_translation(center + Vec3::new(3.0, key_height, 3.0))
                        .looking_at(center, Vec3::Z),
                    Name::new("District key light"),
                ));
            }
        }
    }

    state.dirty = false;
}

fn update_status(
    state: Res<LabState>,
    menu_state: Res<LabMenuState>,
    mut status: Query<&mut Text, With<LabStatus>>,
) {
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    if !state.overlay {
        **text = String::new();
        return;
    }
    let register = state.register();
    let filter = menu_state.active_filter;
    let filtered = state.filtered_compositions(filter);
    let position = filtered
        .iter()
        .position(|&idx| idx == state.current_composition)
        .map(|p| p + 1)
        .unwrap_or(0);
    **text = format!(
        "{}\n\
         Render: {}  |  View: {}  |  Register {}/9: {}  |  {}{}\n\
         [{}] {}/{} in filter \"{}\"  —  Tab render · M view · X cutaway · 1-9 register · F2 menu · F1 hide",
        state.composition().title(&state.tiles, register.slug()),
        state.render_mode.label(),
        state.view_mode.label(),
        state.register_index + 1,
        register.slug(),
        if state.cross_section { "CUTAWAY  " } else { "" },
        if state.auto_orbit { "AUTO-ORBIT" } else { "" },
        state.last_reload,
        position,
        filtered.len(),
        filter.label(),
    );
}

fn update_menu_ui(
    state: Res<LabState>,
    menu_state: Res<LabMenuState>,
    mut root_vis: Query<&mut Visibility, With<MenuOverlayRoot>>,
    mut text: Query<&mut Text, With<MenuText>>,
) {
    let Ok(mut vis) = root_vis.single_mut() else {
        return;
    };
    if !menu_state.is_open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let active_tab = menu_state.tab();
    let sel = menu_state.selected_item;

    let mut content = String::from("HEX TILE LAB — TOOL MENU\n  ");
    for (idx, tab) in MenuTab::ALL.iter().enumerate() {
        if idx == menu_state.active_tab {
            content.push_str(&format!("[ {} ]  ", tab.label()));
        } else {
            content.push_str(&format!("  {}    ", tab.label()));
        }
    }
    content.push_str("\n\n");

    let mut push_items = |items: &[String], marked: Option<usize>| {
        for (idx, item) in items.iter().enumerate() {
            let cursor = if sel == idx { ">" } else { " " };
            let mark = match marked {
                Some(m) if m == idx => "[*]",
                Some(_) => "[ ]",
                None => "   ",
            };
            content.push_str(&format!(" {cursor} {mark} {item}\n"));
        }
    };

    match active_tab {
        MenuTab::Browse => {
            let items: Vec<String> = FilterCategory::ALL
                .iter()
                .map(|f| format!("{} ({})", f.label(), state.filtered_compositions(*f).len()))
                .collect();
            let marked = FilterCategory::ALL
                .iter()
                .position(|f| *f == menu_state.active_filter);
            push_items(&items, marked);
        }
        MenuTab::Registers => {
            let items: Vec<String> = ArchitectureRegister::ALL
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {}", i + 1, r.slug()))
                .collect();
            push_items(&items, Some(state.register_index));
        }
        MenuTab::Render => {
            let mode_pos = RenderMode::ALL
                .iter()
                .position(|m| *m == state.render_mode)
                .unwrap_or(0);
            let mut items: Vec<String> = RenderMode::ALL
                .iter()
                .map(|m| format!("Mode: {}", m.label()))
                .collect();
            items.push(format!(
                "Cutaway cross-section: {}",
                if state.cross_section { "ON" } else { "OFF" }
            ));
            items.push(format!(
                "Volumetric fog (Lit): {}",
                if state.volumetrics { "ON" } else { "OFF" }
            ));
            items.push(format!(
                "Bloom (Lit): {}",
                if state.bloom { "ON" } else { "OFF" }
            ));
            items.push(format!(
                "Auto-orbit turntable: {}",
                if state.auto_orbit { "ON" } else { "OFF" }
            ));
            // Radio mark only applies to the four mode rows.
            for (idx, item) in items.iter().enumerate() {
                let cursor = if sel == idx { ">" } else { " " };
                let mark = if idx < 4 {
                    if idx == mode_pos { "[*]" } else { "[ ]" }
                } else {
                    "   "
                };
                content.push_str(&format!(" {cursor} {mark} {item}\n"));
            }
        }
        MenuTab::Actions => {
            let items = [
                "Jump to grounded sanctuary hub".to_string(),
                "Jump to silo wellshaft (7-hex helix)".to_string(),
                "Jump to hall_ramp".to_string(),
                "Hot reload authored maps (H)".to_string(),
                "Respawn body (R)".to_string(),
            ];
            push_items(&items, None);
        }
    }

    content.push_str("\n  Left/Right tabs · Up/Down select · Enter apply · Esc close");
    **text = content;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_state_loads_and_compositions_are_deduplicated() {
        let state = LabState::load();
        assert!(!state.tiles.is_empty());
        // One SingleTile entry per (archetype, rotation-0 variant) — far
        // fewer than the raw runtime cell count (registers x rotations).
        let singles = state
            .compositions
            .iter()
            .filter(|c| matches!(c, Composition::SingleTile { .. }))
            .count();
        assert!(singles > 0);
        assert!(singles * 10 < state.tiles.len().max(10) * 6);
    }

    #[test]
    fn filters_partition_the_composition_list() {
        let state = LabState::load();
        assert!(
            !state
                .filtered_compositions(FilterCategory::Halls)
                .is_empty()
        );
        assert!(
            !state
                .filtered_compositions(FilterCategory::Chambers)
                .is_empty()
        );
        assert_eq!(
            state.filtered_compositions(FilterCategory::All).len(),
            state.compositions.len()
        );
    }

    #[test]
    fn jump_helpers_land_on_matching_compositions() {
        let mut state = LabState::load();
        state.jump_to_archetype("sanctuary");
        let register = state.register();
        let title = state
            .composition()
            .title(&state.tiles, register.slug())
            .to_lowercase();
        assert!(title.contains("sanctuary"), "got {title}");
        state.jump_to_archetype("ramp");
        let title = state
            .composition()
            .title(&state.tiles, register.slug())
            .to_lowercase();
        assert!(title.contains("ramp"), "got {title}");
    }

    #[test]
    fn register_switch_reskins_the_same_composition() {
        let mut state = LabState::load();
        state.jump_to_archetype("sanctuary");
        let before = state.current_composition;
        for (idx, reg) in ArchitectureRegister::ALL.iter().enumerate() {
            state.register_index = idx;
            state.switch(before);
            assert_eq!(state.current_composition, before);
            let tile = state.tile().expect("tile resolves in every register");
            assert_eq!(tile.key.register, reg.slug());
        }
    }

    #[test]
    fn all_registers_resolve_every_composition_without_fallback() {
        let mut state = LabState::load();
        for (reg_idx, reg) in ArchitectureRegister::ALL.iter().enumerate() {
            state.register_index = reg_idx;
            for comp_idx in 0..state.compositions.len() {
                state.switch(comp_idx);
                let comp = &state.compositions[comp_idx];
                if let Composition::SingleTile { archetype, variant } = comp {
                    let resolved = resolve_tile(&state.tiles, archetype, *variant, reg.slug())
                        .expect("single tile resolves");
                    assert_eq!(
                        resolved.key.register,
                        reg.slug(),
                        "archetype {archetype} v{variant} missing for register {}",
                        reg.slug()
                    );
                }
            }
        }
    }
}
