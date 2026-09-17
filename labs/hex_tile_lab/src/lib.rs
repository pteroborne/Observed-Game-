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
//!   (rotation 0); the register (1-9, then 0 for Liminal Grid) re-skins the current composition
//!   instead of multiplying the list by 54.
//!
//! Controls: WASD/mouse move (first person), M camera mode, Tab render mode,
//! X cutaway, O auto-orbit, V volumetrics, B bloom, R respawn, H hot reload,
//! `[`/`]` cycle compositions, 1-9/0 register, F1 HUD, F2 menu.

mod capture;
mod lab_menu;
pub mod script_runner;

use std::f32::consts::TAU;

use bevy::asset::{AssetPlugin, RenderAssetUsages};
use bevy::camera::Hdr;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::VolumetricFog;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution};
use lab_menu::{FilterCategory, LabMenuState, MenuTab};
use observed_authoring::{RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{blueprint_cell_archetype, blueprint_for_role};
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style as style;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{ColliderSpec, ConvexRenderMesh, FpsBody, FpsConfig};
use player_input::PlayerIntent;

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

/// How much of a composition a section view takes away.
///
/// Named after what a drawing of each is called, because that is what they
/// are: `Plan` is the roof lifted off, `Half` is a cut on a vertical plane,
/// `Quarter` is the dollhouse that keeps two elevations standing so the
/// composition still reads as a solid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionCut {
    /// Sealed. What a body inside actually sees.
    #[default]
    None,
    /// Roof off. Every wall stands and nothing is above you.
    Plan,
    /// Roof off, one quadrant of walls gone. Two elevations stay standing, so
    /// the composition still reads as a solid while you see inside it.
    Quarter,
    /// Roof off, the near half of the walls gone. Storeys stack and you see
    /// into all of them at once - the only view that shows a shaft as a shaft.
    Half,
    /// Slice a quadrant through all geometry, including shelves and floor slabs.
    QuarterVolume,
    /// Slice through all geometry on the near side of a vertical plane.
    HalfVolume,
}

impl SectionCut {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Plan,
        Self::Quarter,
        Self::Half,
        Self::QuarterVolume,
        Self::HalfVolume,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Sealed",
            Self::Plan => "Plan (no roof)",
            Self::Quarter => "Quarter (dollhouse)",
            Self::Half => "Half (section)",
            Self::QuarterVolume => "Quarter volume",
            Self::HalfVolume => "Half volume",
        }
    }

    /// True for anything that opens the composition up. Inspection lighting
    /// and the HUD both key off this rather than off a particular cut.
    pub fn is_open(self) -> bool {
        self != Self::None
    }

    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Plan,
            Self::Plan => Self::Quarter,
            Self::Quarter => Self::Half,
            Self::Half => Self::QuarterVolume,
            Self::QuarterVolume => Self::HalfVolume,
            Self::HalfVolume => Self::None,
        }
    }

    /// Parse a view script's `section` field.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "none" | "sealed" | "off" => Some(Self::None),
            "plan" | "no_ceiling" | "noceiling" => Some(Self::Plan),
            "quarter" | "dollhouse" => Some(Self::Quarter),
            "half" | "section" => Some(Self::Half),
            "quarter_volume" => Some(Self::QuarterVolume),
            "half_volume" => Some(Self::HalfVolume),
            _ => None,
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
    /// A hand-chosen sequence of tiles laid end to end, each rotated so its
    /// entry door meets the previous tile's exit.
    ///
    /// The point of this one is composition. Every other entry in this list
    /// shows a module by itself, which answers whether a tile is *correct* and
    /// says nothing about whether a run of them is a place - and "somewhere to
    /// go" is a claim about the run.
    Run {
        steps: Vec<(String, u16)>,
    },
    /// Tiles placed at explicit lattice coordinates and turns.
    ///
    /// [`Composition::Run`] auto-mates a chain and can only walk laterally,
    /// which is the wrong shape for the identities that are *vertical* - a
    /// shaft you live in, galleries stacked over a light well. This one makes
    /// no decisions at all: an author says where every cell goes, including its
    /// level, and the lattice supplies the world position.
    ///
    /// Composing by hand first is deliberate. What the solver should be made to
    /// produce is a question that cannot be answered until somebody has seen
    /// the thing standing up.
    Layout {
        cells: Vec<LayoutCell>,
    },
}

/// One explicitly placed cell of a [`Composition::Layout`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutCell {
    pub archetype: String,
    pub variant: u16,
    pub coord: HexCoord,
    /// Sixths of a turn, applied the same way a run's mating rotation is.
    pub turn: u8,
    /// The register this one cell resolves in, overriding the composition's.
    ///
    /// Every other view in this lab is one district at a time, which is right
    /// for reading a tile and wrong for the only question a *facility* poses:
    /// what does the boundary between two districts look like from inside. A
    /// composition that cannot cross a register cannot ask it.
    pub register: Option<String>,
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
/// World placements for an explicit layout. No mating, no inference: the
/// author's coordinates and turns, resolved against the active register.
fn layout_placements(
    tiles: &[TilePrototype],
    register: &str,
    cells: &[LayoutCell],
) -> Vec<(TilePrototype, Vec3, Quat)> {
    cells
        .iter()
        .filter_map(|cell| {
            let scope = cell.register.as_deref().unwrap_or(register);
            let tile = resolve_tile(tiles, &cell.archetype, cell.variant, scope).cloned()?;
            #[allow(clippy::cast_precision_loss)]
            let rotation =
                Quat::from_rotation_y(-f32::from(cell.turn % 6) * std::f32::consts::TAU / 6.0);
            Some((tile, Vec3::from_array(hex_origin(cell.coord)), rotation))
        })
        .collect()
}

/// Lay a chosen sequence of tiles end to end, each rotated so its entry door
/// meets the previous tile's exit.
///
/// The lattice does the arithmetic. Stepping with `HexGridSize::neighbor` and
/// placing at `hex_origin` means the run cannot drift out of alignment the way
/// a hand-written table of plan offsets can - the silo composition above is
/// exactly such a table, and it is right only because its six offsets were
/// checked once and never touched again.
///
/// **Rotation is the whole trick.** A tile is authored with its doors on
/// particular faces, so laying two of them next to each other only mates if the
/// second one is turned to receive the first. `turn` is how many sixths carry
/// the tile's own entry door round to the face we are arriving through, and the
/// same turn carries its exit door to wherever the run goes next.
///
/// Tiles with fewer than two lateral doors end the run rather than being
/// skipped: a `hall_cap` is a dead end and pretending otherwise would lay the
/// next tile inside it. A ramp ends it too, for a subtler reason. Its second
/// opening is a *vertical* port belonging to the cell above, so continuing
/// through one means changing level, and a lateral walk is all this composition
/// claims to build.
fn run_placements(
    tiles: &[TilePrototype],
    register: &str,
    steps: &[(String, u16)],
) -> Vec<(TilePrototype, Vec3, Quat)> {
    let grid = HexGridSize {
        cols: 32,
        rows: 32,
        levels: 4,
    };
    let mut coord = HexCoord {
        q: 12,
        r: 12,
        level: 0,
    };
    let mut entry = HexFace::West;
    let mut out = Vec::new();
    for (archetype, variant) in steps {
        let Some(tile) = resolve_tile(tiles, archetype, *variant, register).cloned() else {
            continue;
        };
        let doors: Vec<HexFace> = HexFace::LATERAL
            .into_iter()
            .filter(|face| tile.signature.port(*face) == PortClass::Door)
            .collect();
        let (Some(&own_entry), Some(&own_exit)) = (doors.first(), doors.get(1)) else {
            out.push((tile, Vec3::from_array(hex_origin(coord)), Quat::IDENTITY));
            break;
        };
        let turn = (entry.index() + 6 - own_entry.index()) % 6;
        #[allow(clippy::cast_precision_loss)]
        let rotation = Quat::from_rotation_y(-(turn as f32) * std::f32::consts::TAU / 6.0);
        out.push((tile, Vec3::from_array(hex_origin(coord)), rotation));

        let exit = HexFace::LATERAL[(own_exit.index() + turn) % 6];
        let Some(next) = grid.neighbor(coord, exit) else {
            break;
        };
        coord = next;
        entry = exit.opposite();
    }
    out
}

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
            Self::Layout { cells } => format!("LAYOUT: {} cells, placed by hand", cells.len()),
            Self::Run { steps } => {
                let names: Vec<String> = steps
                    .iter()
                    .map(|(archetype, variant)| format!("{archetype} v{variant}"))
                    .collect();
                format!("RUN ({} tiles): {}", steps.len(), names.join(" -> "))
            }
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
            Self::Run { steps } => format!("run_{}", steps.len()),
            Self::Layout { cells } => format!("layout_{}", cells.len()),
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

/// Resolve a composition tile without borrowing another register's authored
/// geometry. Generic compatibility remains a valid fallback for signatures
/// that do not have register-specific production art.
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
            tiles.iter().find(|t| {
                t.key.archetype == archetype
                    && t.key.register == "generic"
                    && t.key.variant == variant
            })
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

/// The district key spotlight, spawned only under facility lighting.
#[derive(Component)]
struct FacilityKeyLight;

#[derive(Resource)]
pub struct LabState {
    pub tiles: Vec<TilePrototype>,
    pub current_composition: usize,
    pub compositions: Vec<Composition>,
    pub register_index: usize,
    pub view_mode: ViewMode,
    pub render_mode: RenderMode,
    pub section: SectionCut,
    /// Which way the cut opens, in radians about Y. The default opens the
    /// quadrant the orbit camera starts in.
    pub section_axis: f32,
    roof_section: RoofSection,
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

    /// Light the scene exactly as the shipped facility lights it, instead of
    /// with the lab's inspection fill. See `apply_facility_lighting`.
    pub facility_lighting: bool,
    /// Use the existing inspection rig without cutting away authored roofs.
    pub inspection_fill: bool,
    pub inspection_shadows: bool,
    // Scripted walk for capture
    pub scripted_walk: bool,
    /// Cell centres of the current run, in order, for the scripted walk to
    /// steer along. Empty for every composition that is not a run.
    pub walk_path: Vec<Vec3>,
    pub walk_index: usize,
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
    let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
    RuntimeHexCatalog::load(&base, &slugs)
        .expect("canonical runtime tile catalog loads")
        .cells
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

/// Which composition the shipped facility would call this tile.
///
/// Mirrors `composition_at` in `game/src/hex_wfc/view/lighting.rs`, which is
/// the authority. It works from a solved world - blueprint membership first,
/// then the placement's `HexArchetype` - and a lab has neither, so this reads
/// the tile's own archetype string instead.
///
/// The one place the two can disagree is `Room`, and it is unavoidable rather
/// than sloppy: in a facility a room cell is one the blueprint stamped, and in
/// a lab it is whichever composition the author selected. Every other arm is
/// the same rule on the same names.
#[must_use]
fn facility_composition(archetype: &str) -> style::HexComposition {
    match archetype {
        "expanse" | "sanctuary" => style::HexComposition::Room,
        a if a.starts_with("room_") => style::HexComposition::Room,
        "stair_tower" | "hall_ramp" => style::HexComposition::Vertical,
        _ => style::HexComposition::Hall,
    }
}

/// The palette the shipped facility would light this scene with.
fn facility_palette(state: &LabState) -> style::DistrictPalette {
    let composition = match state.composition() {
        Composition::Room(_) => style::HexComposition::Room,
        Composition::SiloWellshaft => style::HexComposition::Vertical,
        Composition::SingleTile { archetype, .. } => facility_composition(archetype),
        // A run is lit for the tile the body is standing in at the start; a run
        // that crosses compositions is exactly the case a single still cannot
        // report, and the walk capture is the answer to it.
        Composition::Run { steps } => {
            steps.first().map_or(style::HexComposition::Hall, |(a, _)| {
                facility_composition(a)
            })
        }
        Composition::Layout { cells } => {
            cells.first().map_or(style::HexComposition::Hall, |cell| {
                facility_composition(&cell.archetype)
            })
        }
    };
    style::architecture_for_composition(state.register(), composition)
}

/// Load a surface texture the way the facility loads one: **tiling**.
///
/// A plain `asset_server.load` leaves the sampler on its default
/// `ClampToEdge`, and every hull face here is far larger than one texture, so
/// the UVs run well past 1.0 and Bevy smears the edge row of pixels across the
/// rest of the surface. That is why lab captures showed long horizontal streaks
/// where the game shows fine mottling - the same PNG, sampled two different
/// ways.
///
/// Not a lighting-mode difference and not a matter of taste, so it is fixed for
/// every render mode rather than only under `facility_lighting`.
///
/// Duplicated from `load_repeating_texture` in `game/src/view/environment.rs`
/// rather than shared: it is a Bevy sampler descriptor, and the crate that owns
/// the asset paths (`observed_assets`) is deliberately Bevy-free so it can be
/// unit-tested without an app.
fn repeating_texture(asset_server: &AssetServer, path: &'static str) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
            settings.sampler =
                bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    ..default()
                });
        })
        .load(path)
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
            section: SectionCut::default(),
            section_axis: std::f32::consts::FRAC_PI_4,
            roof_section: RoofSection::default(),
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

            facility_lighting: false,
            inspection_fill: false,
            inspection_shadows: false,
            scripted_walk: false,
            walk_path: Vec::new(),
            walk_index: 0,
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
            .filter(|(index, comp)| {
                comp.matches_filter(filter) && self.composition_available(*index)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn composition_available(&self, index: usize) -> bool {
        let register = self.register().slug();
        match &self.compositions[index % self.compositions.len()] {
            Composition::SingleTile { archetype, variant } => {
                resolve_tile(&self.tiles, archetype, *variant, register).is_some()
            }
            Composition::Room(role) => blueprint_for_role(*role)
                .cells
                .iter()
                .enumerate()
                .filter_map(|(cell, _)| blueprint_cell_archetype(*role, cell))
                .all(|archetype| resolve_tile(&self.tiles, archetype, 0, register).is_some()),
            Composition::SiloWellshaft => ["silo_core", "silo_ring", "silo_ring_bridge"]
                .into_iter()
                .all(|archetype| resolve_tile(&self.tiles, archetype, 0, register).is_some()),
            // A run is available when its *first* tile is: the rest are reported
            // by the run coming out short, which is more use to an author than
            // the whole composition vanishing from the list.
            Composition::Run { steps } => steps.first().is_some_and(|(archetype, variant)| {
                resolve_tile(&self.tiles, archetype, *variant, register).is_some()
            }),
            // A layout's first cell may name its own register, and a
            // cross-district composition's first cell usually does. Asking
            // whether it resolves in the *composition's* register is asking
            // the wrong question, and answering it wrongly is silent: `switch`
            // quietly falls back to another composition and the capture comes
            // out empty with nothing logged.
            Composition::Layout { cells } => cells.first().is_some_and(|cell| {
                let scope = cell.register.as_deref().unwrap_or(register);
                resolve_tile(&self.tiles, &cell.archetype, cell.variant, scope).is_some()
            }),
        }
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
        let requested = index % self.compositions.len();
        self.current_composition = if self.composition_available(requested) {
            requested
        } else {
            let requested_archetype = match &self.compositions[requested] {
                Composition::SingleTile { archetype, .. } => Some(archetype.as_str()),
                _ => None,
            };
            self.compositions
                .iter()
                .enumerate()
                .find(|(candidate, composition)| {
                    self.composition_available(*candidate)
                        && requested_archetype.is_none_or(|archetype| {
                            matches!(composition, Composition::SingleTile { archetype: candidate, .. } if candidate == archetype)
                        })
                })
                .map_or(0, |(candidate, _)| candidate)
        };
        let composition = self.composition().clone();
        let register = self.register();
        self.walk_path.clear();
        self.walk_index = 0;

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
            Composition::Layout { ref cells } => {
                let placements = layout_placements(&self.tiles, register.slug(), cells);
                self.build_from_placements(&placements);
            }
            Composition::Run { ref steps } => {
                let placements = run_placements(&self.tiles, register.slug(), steps);
                self.build_from_placements(&placements);
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

    /// Build the scene, colliders, camera framing and spawn pose from a list of
    /// placed tiles.
    ///
    /// Shared by [`Composition::Run`] and [`Composition::Layout`] because the
    /// only thing that differs between them is *how the list was decided* - a
    /// chain that mates itself, or an author's explicit coordinates. Everything
    /// downstream is the same work, and two copies of it would drift.
    fn build_from_placements(&mut self, placements: &[(TilePrototype, Vec3, Quat)]) {
        let mut collider_specs: Vec<ColliderSpec> = Vec::new();
        for (tile, origin, rotation) in placements {
            let specs =
                tile.collider_specs_with_transform(collider_specs.len() as u32, *origin, *rotation);
            collider_specs.extend(specs);
        }
        let count = placements.len().max(1) as f32;
        let center = placements.iter().map(|(_, o, _)| *o).sum::<Vec3>() / count
            + Vec3::Y * (TILE_LEVEL_HEIGHT * 0.5);
        let extent = placements
            .iter()
            .map(|(_, o, _)| (*o - center).length())
            .fold(0.0_f32, f32::max);
        self.center = center;
        self.radius = extent + 30.0;
        self.height = (extent + 30.0) * 0.7;
        let spec = observed_traversal::ArenaSpec {
            colliders: collider_specs,
            floor_y: 0.0,
            safety_center: center,
            safety_half: Vec3::new(extent + 26.0, 30.0, extent + 26.0),
        };
        self.scene = RapierTraversalScene::from_arena_spec(&spec);
        // Standing in the first tile, facing the way the run goes.
        let (spawn, facing) = match (placements.first(), placements.get(1)) {
            (Some((_, first, _)), Some((_, second, _))) => {
                (*first, (*second - *first).normalize_or_zero())
            }
            (Some((_, first, _)), None) => (*first, Vec3::X),
            _ => (center, Vec3::X),
        };
        self.body = FpsBody::spawned(
            spawn + Vec3::Y * self.config.half_height,
            yaw_toward(Vec2::new(facing.x, facing.z)),
        );
        self.walk_path = placements.iter().map(|(_, o, _)| *o).collect();
        self.walk_index = 1;
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
                    shot: 0,
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
                    shadow_maps_enabled: false,
                    ..default()
                },
                Name::new("Headlamp"),
            ));
        });

    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
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
                    font_size: FontSize::Px(13.0),
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
        state.section = state.section.next();
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

    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
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
                    state.section = state.section.next();
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
        // Steer along the run rather than simply holding forward. Forward alone
        // walks a straight and stops dead at the first turn, which would make
        // every captured sequence a test of the first tile only.
        //
        // This is the production character controller being driven toward a
        // waypoint, not a camera flying a path: it collides, it is stopped by a
        // seam that does not mate, and it falls if the floor is not there. That
        // is the whole reason to walk a run instead of photographing it.
        let mut movement = Vec2::new(0.0, 1.0);
        if let Some(&target) = state.walk_path.get(state.walk_index) {
            let here = state.body.position;
            let to = Vec3::new(target.x - here.x, 0.0, target.z - here.z);
            if to.length() < 2.5 {
                state.walk_index += 1;
            }
            let desired = yaw_toward(Vec2::new(to.x, to.z));
            let error = (desired - state.body.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            state.body.yaw += error.clamp(-0.05, 0.05);
        } else if !state.walk_path.is_empty() {
            // Past the last waypoint: stand still rather than walk into the end
            // wall, so the tail of a sequence is the run and not a scuffle.
            movement = Vec2::ZERO;
        }
        PlayerIntent {
            movement,
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

/// The district key light. `Without<EyeCamera>` is load-bearing rather than
/// tidy: the camera query beside it also takes `Transform` mutably, and Bevy
/// rejects the overlap at startup rather than at the call site.
type FacilityKeyQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut SpotLight, &'static mut Transform),
    (With<FacilityKeyLight>, Without<EyeCamera>),
>;

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
    mut key: FacilityKeyQuery,
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

    if lit {
        // Facility fog is tuned for a body in a corridor - roughly 10 to 28 m -
        // and the lab's 60..170 was chosen so an orbit camera standing well
        // back could still see the tile. At tile scale the lab's fog never
        // engages at all, which is most of why a Lit capture looked flat: the
        // depth cue the district is built around was simply absent.
        let (start, end) = if state.facility_lighting {
            let palette = facility_palette(&state);
            (palette.fog_start, palette.fog_end)
        } else {
            (60.0, 170.0)
        };
        let color = if state.facility_lighting {
            facility_palette(&state).fog_color
        } else {
            style::architecture(state.register()).fog_color
        };
        commands.entity(cam_entity).insert(DistanceFog {
            color,
            falloff: FogFalloff::Linear { start, end },
            ..default()
        });
    } else if has_fog {
        commands.entity(cam_entity).remove::<DistanceFog>();
    }

    if let Ok(mut lamp) = headlamps.single_mut() {
        // The facility ships **no headlamp**, and deliberately: the rig's own
        // note says a flat player-locked fill "washed out the very shadows this
        // rig exists to cast". The lab keeps one as a Lit-mode safety light, and
        // it is the single biggest reason a lab capture reads flatter than the
        // game - so facility lighting turns it off rather than dimming it.
        lamp.intensity = match state.render_mode {
            RenderMode::Lit if state.facility_lighting => 0.0,
            RenderMode::Lit | RenderMode::Clay => 90_000.0,
            RenderMode::Xray | RenderMode::Colliders => 0.0,
        };
    }

    // The district key: a shadow-casting spot over the body, which is what
    // gives each register its directional read. Nothing in the lab had one.
    let want_key = lit && state.facility_lighting;
    match (want_key, key.single_mut()) {
        (true, Ok((mut light, mut transform))) => {
            let palette = facility_palette(&state);
            let origin = Vec3::new(state.body.position.x, 0.0, state.body.position.z);
            *transform = Transform::from_translation(origin + Vec3::new(2.6, 6.4, 2.6))
                .looking_at(origin + Vec3::new(-1.0, 0.2, -1.0), Vec3::Y);
            light.color = palette.key_color;
            light.intensity = palette.key_intensity * style::HEX_KEY_INTENSITY_SCALE;
            light.range = palette.key_range;
            light.radius = palette.key_radius;
            light.inner_angle = palette.key_inner_angle;
            light.outer_angle = palette.key_outer_angle;
            light.shadow_maps_enabled = palette.key_shadows_enabled;
        }
        (true, Err(_)) => {
            let palette = facility_palette(&state);
            let origin = Vec3::new(state.body.position.x, 0.0, state.body.position.z);
            commands.spawn((
                FacilityKeyLight,
                SpotLight {
                    color: palette.key_color,
                    intensity: palette.key_intensity * style::HEX_KEY_INTENSITY_SCALE,
                    range: palette.key_range,
                    radius: palette.key_radius,
                    inner_angle: palette.key_inner_angle,
                    outer_angle: palette.key_outer_angle,
                    shadow_maps_enabled: palette.key_shadows_enabled,
                    ..default()
                },
                Transform::from_translation(origin + Vec3::new(2.6, 6.4, 2.6))
                    .looking_at(origin + Vec3::new(-1.0, 0.2, -1.0), Vec3::Y),
                Name::new("facility key light"),
            ));
        }
        (false, Ok((mut light, _))) => light.intensity = 0.0,
        (false, Err(_)) => {}
    }
}

/// A capped module can reserve more height than its visible envelope: a tower
/// head reserves two cells for compatibility but its lid is one storey high.
/// Open climbs retain their reservation height so a landing is never cut as a lid.
fn tile_ceiling_height(tile: &TilePrototype) -> f32 {
    let reserved = f32::from(tile.levels) * TILE_LEVEL_HEIGHT;
    if tile.signature.port(HexFace::Up) != PortClass::Sealed {
        return reserved;
    }
    let geometry_top = tile.hulls.iter().flatten().map(|p| p.y).fold(0.0, f32::max);
    reserved.min((geometry_top / TILE_LEVEL_HEIGHT).ceil() * TILE_LEVEL_HEIGHT)
}

/// A hull counts as ceiling for the cutaway when it sits entirely in the top
/// band of the composition's vertical extent.
fn is_ceiling(hull: &[Vec3], top_y: f32) -> bool {
    hull.iter().all(|point| point.y >= top_y - 0.75)
}

/// Optional photographic removal of a low suspended ceiling. Coordinates are
/// local to each module; front-only uses the existing section axis and centre.
#[derive(Clone, Copy, Debug, Default)]
struct RoofSection {
    height: Option<f32>,
    upper: Option<f32>,
    front_only: bool,
}

impl RoofSection {
    fn hides(
        self,
        hull: &[Vec3],
        top_y: f32,
        transform: &Transform,
        center: Vec3,
        cut: SectionCut,
        axis: f32,
    ) -> bool {
        if cut != SectionCut::None
            && self
                .upper
                .is_some_and(|height| hull.iter().all(|p| p.y >= height))
        {
            return true;
        }
        let mut ceiling_top = self.height.map_or(top_y, |height| height + 0.75);
        if self.front_only
            && !section_point_hidden(transform.translation, center, SectionCut::Half, axis)
        {
            ceiling_top = f32::INFINITY;
        }
        section_hides(hull, ceiling_top, transform, center, cut, axis)
    }

    fn keeps_practical(self, position: Vec3, center: Vec3, cut: SectionCut, axis: f32) -> bool {
        self.height.is_none()
            || cut == SectionCut::None
            || (self.front_only && !section_point_hidden(position, center, SectionCut::Half, axis))
    }
}

/// A hull tall enough to be a wall shell rather than a slab, in metres.
const WALL_SHELL_MIN: f32 = 4.0;
/// How far past the cut plane a hull's middle must sit before it is removed.
/// A slab centred on the plane straddles it and stays.
const SECTION_EPS: f32 = 1.5;

/// Whether a section view removes this hull.
///
/// One rule for every composition. There used to be four: a single tile
/// dropped ceilings, a room dropped ceilings and any wall past the centre, the
/// silo dropped tall shells on its south side, and a layout — the composition
/// that actually stacks — dropped ceilings only. That is why a four-storey
/// shaft photographed as a closed box: the one view that needed a wall opened
/// was the one view that never opened one.
///
/// Ordinary sections preserve floors and landings. QuarterVolume deliberately
/// clips all geometry at the quadrant planes, so fittings do not float after their
/// backing walls disappear. It is a display cut; collision geometry is untouched.
fn section_hides(
    hull: &[Vec3],
    top_y: f32,
    transform: &Transform,
    center: Vec3,
    cut: SectionCut,
    axis: f32,
) -> bool {
    if cut == SectionCut::None {
        return false;
    }
    if is_ceiling(hull, top_y) {
        return true;
    }
    if cut == SectionCut::Plan {
        return false;
    }
    let (min_y, max_y) = hull.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    if !matches!(cut, SectionCut::QuarterVolume | SectionCut::HalfVolume)
        && max_y - min_y < WALL_SHELL_MIN
    {
        return false;
    }
    #[allow(clippy::cast_precision_loss)]
    let mid = hull
        .iter()
        .map(|p| transform.transform_point(*p))
        .sum::<Vec3>()
        / hull.len() as f32;
    if matches!(cut, SectionCut::QuarterVolume | SectionCut::HalfVolume) {
        hull.iter()
            .all(|p| section_point_hidden(transform.transform_point(*p), center, cut, axis))
    } else {
        section_point_hidden(mid, center, cut, axis)
    }
}

fn section_point_hidden(point: Vec3, center: Vec3, cut: SectionCut, axis: f32) -> bool {
    let delta = point - center;
    let (sin, cos) = axis.sin_cos();
    let toward = delta.x * cos + delta.z * sin;
    let across = delta.z * cos - delta.x * sin;
    match cut {
        SectionCut::Half | SectionCut::HalfVolume => toward > SECTION_EPS,
        SectionCut::Quarter | SectionCut::QuarterVolume => {
            toward > SECTION_EPS && across > SECTION_EPS
        }
        SectionCut::None | SectionCut::Plan => false,
    }
}

/// Clip convex points to a half-space. Pairwise crossings include every true
/// edge intersection; extra crossings lie inside the same convex result.
fn clipped_hull(hull: &[Vec3], distance: impl Fn(Vec3) -> f32) -> Vec<Vec3> {
    let distances: Vec<_> = hull.iter().map(|p| distance(*p)).collect();
    let mut points: Vec<_> = hull
        .iter()
        .zip(&distances)
        .filter_map(|(p, d)| (*d <= 0.0).then_some(*p))
        .collect();
    for (i, &a) in hull.iter().enumerate() {
        for (j, &b) in hull.iter().enumerate().skip(i + 1) {
            if (distances[i] < 0.0 && distances[j] > 0.0)
                || (distances[j] < 0.0 && distances[i] > 0.0)
            {
                let p = a.lerp(b, distances[i] / (distances[i] - distances[j]));
                if points
                    .iter()
                    .all(|other| other.distance_squared(p) > 0.000_001)
                {
                    points.push(p);
                }
            }
        }
    }
    points
}

fn half_volume_hull(hull: &[Vec3], transform: &Transform, center: Vec3, axis: f32) -> Vec<Vec3> {
    let (sin, cos) = axis.sin_cos();
    clipped_hull(hull, |p| {
        let p = transform.transform_point(p) - center;
        p.x * cos + p.z * sin - SECTION_EPS
    })
}

fn volume_section_pieces(
    hull: &[Vec3],
    transform: &Transform,
    center: Vec3,
    axis: f32,
) -> [Vec<Vec3>; 2] {
    let (sin, cos) = axis.sin_cos();
    let toward = |p: Vec3| {
        let p = transform.transform_point(p) - center;
        p.x * cos + p.z * sin - SECTION_EPS
    };
    let across = |p: Vec3| {
        let p = transform.transform_point(p) - center;
        p.z * cos - p.x * sin - SECTION_EPS
    };
    let far = clipped_hull(hull, toward);
    let near = clipped_hull(hull, |p| -toward(p));
    [far, clipped_hull(&near, across)]
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
    } else if hull.iter().all(|p| p.y <= 0.5001)
        || observed_traversal::render_mesh::is_horizontal_slab(hull)
    {
        SurfaceKind::Floor
    } else if hull.iter().all(|p| p.y <= 4.0 && p.y >= 0.2) {
        SurfaceKind::Trim
    } else {
        SurfaceKind::Wall
    }
}

/// Build a render mesh from a convex hull with crease-angle normal smoothing
/// and per-face box-projected UVs through the shared Bevy-free policy used by
/// the assembled game.
fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let data = ConvexRenderMesh::from_convex_hull(hull)?;
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
        .with_inserted_indices(Indices::U32(data.indices)),
    )
}

struct PreviewPractical {
    position: Vec3,
    module_origin: Vec3,
    light: style::HexPracticalLight,
}

fn preview_practicals(
    tile: &TilePrototype,
    transform: Transform,
    register: ArchitectureRegister,
    composition: style::HexComposition,
) -> Vec<PreviewPractical> {
    let positions: Vec<Vec3> = if tile.lights.is_empty() {
        (0..tile.levels)
            .map(|level| Vec3::Y * (f32::from(level) * TILE_LEVEL_HEIGHT + 5.6))
            .collect()
    } else {
        tile.lights.iter().map(|light| light.position).collect()
    };
    let light = style::hex_practical_light(register, composition, positions.len());
    positions
        .into_iter()
        .map(|position| PreviewPractical {
            position: transform.transform_point(position),
            module_origin: transform.translation,
            light,
        })
        .collect()
}

fn district_weave_texture(
    images: &mut Assets<Image>,
    register: ArchitectureRegister,
) -> Option<Handle<Image>> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let data = style::surface_weave_rgba(style::architecture_weave(register))?;
    let n = style::SURFACE_WEAVE_SIZE;
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    Some(images.add(image))
}

fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    visuals: Query<Entity, With<TileVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }

    let roof_section = state.roof_section;
    let register = state.register();
    let palette = style::architecture(register);
    let mode = state.render_mode;

    let floor_tex = repeating_texture(&asset_server, observed_assets::FLOOR.path);
    let wall_tex = repeating_texture(&asset_server, observed_assets::WALL.path);
    let ceiling_tex = repeating_texture(&asset_server, observed_assets::CEILING.path);
    let wall_weave =
        district_weave_texture(&mut images, register).unwrap_or_else(|| wall_tex.clone());

    // Tier 1: the ambient legibility floor. District palettes may be moody in
    // the shipped game, but in the lab geometry legibility wins: Lit clamps
    // ambient up to a floor; the study modes use bright neutral ambient.
    match mode {
        RenderMode::Lit if state.facility_lighting => {
            // No floor, no lift, no clamp: the shipped rig's own numbers, so a
            // capture answers "how does this read in the game" rather than "how
            // does this read in the lab". The two are different questions and
            // the lab's fill exists for the other one.
            let palette = facility_palette(&state);
            commands.insert_resource(GlobalAmbientLight {
                color: palette.ambient_color,
                brightness: palette.ambient_brightness,
                ..default()
            });
            commands.insert_resource(ClearColor(palette.fog_color));
        }
        RenderMode::Lit => {
            // Cutaway views are inspection views: open roofs read as caves
            // without a stronger fill, so the ambient floor rises with it.
            let floor = if state.section.is_open() || state.inspection_fill {
                340.0
            } else {
                200.0
            };
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

    let facility_lighting = state.facility_lighting;
    let inspection_fill = state.inspection_fill && !facility_lighting;
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
            RenderMode::Lit
                if facility_lighting
                    || inspection_fill
                    || register == ArchitectureRegister::OverlitGrid =>
            {
                // The facility's own shell look, from the one function the game
                // paints with. The branch below hardcodes neutral greys for
                // eight of the ten registers, so a Lit capture said nothing
                // whatever about the district it claimed to be showing.
                //
                // Ceilings take the **wall** texture, because that is what the
                // facility does: `ceiling.png` is loaded by this lab and by
                // nothing in the shipped game.
                let role = match kind {
                    SurfaceKind::Floor => style::ArchitectureSurfaceRole::Floor,
                    SurfaceKind::Wall | SurfaceKind::Trim => style::ArchitectureSurfaceRole::Wall,
                    SurfaceKind::Ceiling => style::ArchitectureSurfaceRole::Ceiling,
                };
                let look = style::hex_shell_surface(register, role);
                let tex = match kind {
                    SurfaceKind::Floor => floor_tex.clone(),
                    SurfaceKind::Wall | SurfaceKind::Trim => wall_weave.clone(),
                    SurfaceKind::Ceiling => wall_tex.clone(),
                };
                materials.add(StandardMaterial {
                    base_color: look.base_color,
                    emissive_texture: (register == ArchitectureRegister::ShadowScreen
                        && role == style::ArchitectureSurfaceRole::Wall)
                        .then_some(tex.clone()),
                    base_color_texture: look.textured.then_some(tex),
                    emissive: look.emissive,
                    unlit: look.unlit,
                    perceptual_roughness: palette.surface_roughness,
                    ..default()
                })
            }
            RenderMode::Lit => {
                // District materials: near-zero emissive — light, not glow,
                // shapes the space. Trim keeps a whisper of accent.
                let (color, tex, emissive) = if register == ArchitectureRegister::LiminalGrid {
                    let role = match kind {
                        SurfaceKind::Floor => style::ArchitectureSurfaceRole::Floor,
                        SurfaceKind::Wall | SurfaceKind::Trim => {
                            style::ArchitectureSurfaceRole::Wall
                        }
                        SurfaceKind::Ceiling => style::ArchitectureSurfaceRole::Ceiling,
                    };
                    let treatment = style::architecture_surface(register, role);
                    let texture = match kind {
                        SurfaceKind::Floor => Some(floor_tex.clone()),
                        SurfaceKind::Wall | SurfaceKind::Trim => Some(wall_tex.clone()),
                        SurfaceKind::Ceiling => Some(ceiling_tex.clone()),
                    };
                    (treatment.base_color, texture, treatment.emissive)
                } else if register == ArchitectureRegister::Monolith {
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
    let section = state.section;
    let section_axis = state.section_axis;
    let cross_section = section.is_open();
    let center = state.center;

    // Authored practical positions accumulate with the exact geometry
    // transforms. Only source data without explicit lights uses a centered fallback.
    let mut pool_origins: Vec<PreviewPractical> = Vec::new();

    let mut spawn_hull_entity = |commands: &mut Commands,
                                 meshes: &mut ResMut<Assets<Mesh>>,
                                 hull: &[Vec3],
                                 transform: Transform,
                                 top_y: f32,
                                 name: String| {
        let kind = if register == ArchitectureRegister::OverlitGrid
            && observed_traversal::render_mesh::is_overhead_slab(hull)
        {
            SurfaceKind::Ceiling
        } else {
            hull_surface_kind(hull, top_y)
        };
        let mat = get_surface_material(kind);
        let pieces = if section == SectionCut::HalfVolume {
            vec![half_volume_hull(hull, &transform, center, section_axis)]
        } else if section == SectionCut::QuarterVolume {
            volume_section_pieces(hull, &transform, center, section_axis).to_vec()
        } else {
            vec![hull.to_vec()]
        };
        for piece in pieces {
            if piece.len() >= 4
                && let Some(mesh) = hull_mesh(&piece)
            {
                commands.spawn((
                    TileVisual,
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mat.clone()),
                    transform,
                    Name::new(name.clone()),
                ));
            }
        }
    };

    match composition {
        Composition::SingleTile {
            ref archetype,
            variant,
        } => {
            if let Some(tile) = resolve_tile(&state.tiles, archetype, variant, register.slug()) {
                let tile = tile.clone();
                let top_y = tile_ceiling_height(&tile);
                pool_origins.extend(preview_practicals(
                    &tile,
                    Transform::IDENTITY,
                    register,
                    facility_composition(archetype),
                ));
                for hull in &tile.hulls {
                    if roof_section.hides(
                        hull,
                        top_y,
                        &Transform::IDENTITY,
                        Vec3::ZERO,
                        section,
                        section_axis,
                    ) {
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
                    let top_y = tile_ceiling_height(&tile);
                    pool_origins.extend(preview_practicals(
                        &tile,
                        Transform::from_translation(origin),
                        register,
                        style::HexComposition::Room,
                    ));
                    for hull in &tile.hulls {
                        if roof_section.hides(
                            hull,
                            top_y,
                            &Transform::from_translation(origin),
                            center,
                            section,
                            section_axis,
                        ) {
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
        Composition::Layout { ref cells } => {
            for (tile, origin, rotation) in layout_placements(&state.tiles, register.slug(), cells)
            {
                let top_y = tile_ceiling_height(&tile);
                let transform = Transform::from_translation(origin).with_rotation(rotation);
                pool_origins.extend(preview_practicals(
                    &tile,
                    transform,
                    register,
                    facility_composition(&tile.key.archetype),
                ));
                for hull in &tile.hulls {
                    if roof_section.hides(hull, top_y, &transform, center, section, section_axis) {
                        continue;
                    }
                    spawn_hull_entity(
                        &mut commands,
                        &mut meshes,
                        hull,
                        transform,
                        top_y,
                        format!("Layout {}", tile.key.archetype),
                    );
                }
            }
        }
        Composition::Run { ref steps } => {
            for (tile, origin, rotation) in run_placements(&state.tiles, register.slug(), steps) {
                let top_y = tile_ceiling_height(&tile);
                let transform = Transform::from_translation(origin).with_rotation(rotation);
                pool_origins.extend(preview_practicals(
                    &tile,
                    transform,
                    register,
                    facility_composition(&tile.key.archetype),
                ));
                for hull in &tile.hulls {
                    if roof_section.hides(hull, top_y, &transform, center, section, section_axis) {
                        continue;
                    }
                    spawn_hull_entity(
                        &mut commands,
                        &mut meshes,
                        hull,
                        transform,
                        top_y,
                        format!("Run {}", tile.key.archetype),
                    );
                }
            }
        }
        Composition::SiloWellshaft => {
            for (tile, origin, rotation) in silo_placements(&state.tiles, register.slug()) {
                let top_y = tile_ceiling_height(&tile);
                let transform = Transform::from_translation(origin).with_rotation(rotation);
                pool_origins.extend(preview_practicals(
                    &tile,
                    transform,
                    register,
                    facility_composition(&tile.key.archetype),
                ));
                for hull in &tile.hulls {
                    // The core is the subject of this composition, so it is
                    // never cut; everything round it obeys the same rule as
                    // every other composition.
                    if tile.key.archetype != "silo_core"
                        && roof_section.hides(
                            hull,
                            top_y,
                            &transform,
                            center,
                            section,
                            section_axis,
                        )
                    {
                        continue;
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

    if matches!(section, SectionCut::QuarterVolume | SectionCut::HalfVolume) {
        pool_origins
            .retain(|point| !section_point_hidden(point.position, center, section, section_axis));
    }

    pool_origins.retain(|point| {
        roof_section.keeps_practical(point.module_origin, center, section, section_axis)
    });

    if mode == RenderMode::Clay {
        // Studio rig: two shadowless directionals. Without shadows they light
        // every face by orientation alone — roofs and walls never occlude, so
        // form reads everywhere. This is the guaranteed-legibility mode.
        commands.spawn((
            TileVisual,
            DirectionalLight {
                illuminance: 5_500.0,
                color: Color::srgb(1.0, 0.98, 0.92),
                shadow_maps_enabled: false,
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
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default().looking_to(Vec3::new(0.55, -0.35, 0.5).normalize(), Vec3::Y),
            Name::new("Clay rim"),
        ));
    }

    if mode == RenderMode::Lit && (cross_section || inspection_fill) {
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
                    color: if register == ArchitectureRegister::OverlitGrid {
                        palette.light_color
                    } else {
                        Color::srgb(0.85, 0.90, 1.0)
                    },
                    shadow_maps_enabled: state.inspection_shadows && illuminance > 2_000.0,
                    ..default()
                },
                Transform::default().looking_to(dir.normalize(), Vec3::Y),
                Name::new("Cutaway inspection fill"),
            ));
        }
    }

    if mode == RenderMode::Lit {
        // Tier 3: exact authored practicals, backed by fixture geometry.
        let fixture = style::architecture_practical_fixture(register);
        let fixture_mesh = meshes.add(Cuboid::new(2.2, 0.08, 0.48));
        let fixture_material = materials.add(StandardMaterial {
            base_color: fixture.base_color,
            emissive: fixture.emissive,
            perceptual_roughness: 0.72,
            ..default()
        });
        for practical in &pool_origins {
            let origin = practical.position;
            let light = practical.light;
            commands.spawn((
                TileVisual,
                Mesh3d(fixture_mesh.clone()),
                MeshMaterial3d(fixture_material.clone()),
                Transform::from_translation(origin - Vec3::Y * 0.06),
                Name::new("Style-owned practical diffuser"),
            ));
            commands.spawn((
                TileVisual,
                PointLight {
                    color: light.color,
                    intensity: if facility_lighting || register == ArchitectureRegister::OverlitGrid
                    {
                        light.intensity
                    } else {
                        900_000.0
                    },
                    range: light.range,
                    radius: light.radius,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(origin),
                Name::new("Authored practical"),
            ));
        }

        // Tier 2: the district key spot — sealed-view mood only. In cutaway
        // the inspection fills own the frame: a high-intensity shadow-casting
        // spot enclosed with a tall central occluder renders the occluder
        // pitch black behind a tall central occluder, and inspection is
        // the point of a cutaway anyway.
        let key_height = pool_origins
            .iter()
            .map(|o| o.position.y)
            .fold(0.0_f32, f32::max)
            + 14.0;
        if !cross_section && !inspection_fill {
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
                        shadow_maps_enabled: true,
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
                        shadow_maps_enabled: palette.key_shadows_enabled,
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
         Render: {}  |  View: {}  |  Register {}/{}: {}  |  {}{}\n\
         [{}] {}/{} in filter \"{}\"  —  Tab render · M view · X cutaway · 1-9/0 register · F2 menu · F1 hide",
        state.composition().title(&state.tiles, register.slug()),
        state.render_mode.label(),
        state.view_mode.label(),
        state.register_index + 1,
        ArchitectureRegister::ALL.len(),
        register.slug(),
        if state.section.is_open() {
            state.section.label()
        } else {
            ""
        },
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
            items.push(format!("Section: {}", state.section.label()));
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
    fn noon_practicals_and_overhead_materials_survive_visual_resets() {
        let mut state = LabState::load();
        state.register_index = 2;
        state.compositions.push(Composition::SingleTile {
            archetype: "hall_straight".to_string(),
            variant: 900,
        });
        state.current_composition = state.compositions.len() - 1;
        state.render_mode = RenderMode::Lit;
        state.facility_lighting = true;
        state.dirty = true;
        let tile = state.tile().expect("Noon source");
        let panels: Vec<_> = tile
            .hulls
            .iter()
            .filter(|h| observed_traversal::render_mesh::is_overhead_slab(h))
            .collect();
        assert_eq!(panels.len(), 2, "outer roof and suspended raft");
        assert_eq!(
            panels.iter().filter(|h| is_ceiling(h, 8.0)).count(),
            1,
            "roof cutting must preserve the raft"
        );
        let expected = preview_practicals(
            tile,
            Transform::IDENTITY,
            ArchitectureRegister::OverlitGrid,
            style::HexComposition::Hall,
        );
        assert_eq!(expected.len(), 4);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            },
            bevy::image::ImagePlugin::default(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(state)
        .add_systems(Update, rebuild_visuals);
        let mut count = None;
        for _ in 0..3 {
            app.world_mut().resource_mut::<LabState>().dirty = true;
            app.update();
            let world = app.world_mut();
            let actual = world
                .query_filtered::<Entity, With<TileVisual>>()
                .iter(world)
                .count();
            assert_eq!(
                *count.get_or_insert(actual),
                actual,
                "visuals accumulate on reset"
            );
            let lights: Vec<_> = world.query::<&PointLight>().iter(world).collect();
            assert_eq!(lights.len(), 4);
            for light in lights {
                assert_eq!(light.intensity, expected[0].light.intensity);
                assert_eq!(light.radius, expected[0].light.radius);
                assert!(!light.shadow_maps_enabled);
            }
        }
    }

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
            (0..state.compositions.len())
                .filter(|&index| state.composition_available(index))
                .count()
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
    fn every_visible_composition_resolves_without_cross_register_fallback() {
        let mut state = LabState::load();
        for (reg_idx, reg) in ArchitectureRegister::ALL.iter().enumerate() {
            state.register_index = reg_idx;
            for comp_idx in state.filtered_compositions(FilterCategory::All) {
                state.switch(comp_idx);
                let comp = &state.compositions[comp_idx];
                if let Composition::SingleTile { archetype, variant } = comp {
                    let resolved = resolve_tile(&state.tiles, archetype, *variant, reg.slug())
                        .expect("single tile resolves");
                    assert!(
                        resolved.key.register == reg.slug() || resolved.key.register == "generic",
                        "archetype {archetype} v{variant} borrowed {} while browsing {}",
                        resolved.key.register,
                        reg.slug(),
                    );
                }
            }
        }
    }

    #[test]
    fn liminal_layout_variants_are_hidden_from_other_registers() {
        let mut state = LabState::load();
        let liminal_only = state
            .compositions
            .iter()
            .enumerate()
            .find(|(_, composition)| {
                matches!(composition, Composition::SingleTile { archetype, variant }
                    if archetype == "hall_cap" && *variant == 6)
            })
            .map(|(index, _)| index)
            .expect("sparse Liminal cap composition exists");

        state.register_index = ArchitectureRegister::ALL
            .iter()
            .position(|register| *register == ArchitectureRegister::Institutional)
            .expect("institutional register");
        assert!(!state.composition_available(liminal_only));

        state.register_index = ArchitectureRegister::ALL
            .iter()
            .position(|register| *register == ArchitectureRegister::LiminalGrid)
            .expect("liminal register");
        assert!(state.composition_available(liminal_only));
    }

    fn assert_layout_resolves_mates_connects_and_resets(
        source: &str,
        register_index: usize,
        register: &str,
        count: usize,
    ) {
        use observed_authoring::rotation::rotate_signature;
        use observed_hex::{PortClass, PortSignature, ports_compatible};
        use std::collections::{BTreeMap, BTreeSet};

        let script: script_runner::ViewScript =
            serde_json::from_str(source).expect("benchmark script parses");
        let cells: Vec<LayoutCell> = script
            .layout
            .expect("explicit benchmark layout")
            .into_iter()
            .map(|entry| {
                let (archetype, variant) = entry.tile.split_once(':').expect("explicit variant");
                LayoutCell {
                    archetype: archetype.to_string(),
                    variant: variant.parse().expect("variant"),
                    coord: HexCoord {
                        q: entry.q,
                        r: entry.r,
                        level: entry.level,
                    },
                    turn: entry.turn,
                    register: entry.register,
                }
            })
            .collect();
        let mut state = LabState::load();
        state.register_index = register_index;
        let placements = layout_placements(&state.tiles, register, &cells);
        assert_eq!(
            placements.len(),
            count,
            "the complete benchmark must resolve"
        );
        assert_eq!(placements.len(), cells.len());
        let mut signatures = BTreeMap::new();
        for (cell, (tile, _, _)) in cells.iter().zip(&placements) {
            assert!(
                signatures
                    .insert(cell.coord, rotate_signature(tile.signature, cell.turn))
                    .is_none()
            );
            if tile.key.archetype == "hall_ramp" {
                // The solver represents the upper half separately as RampHead;
                // the lab renders both halves using the lower prefab.
                let mut ports = [PortClass::Sealed; 8];
                ports[HexFace::East.index()] = PortClass::Door;
                ports[HexFace::Down.index()] = PortClass::RampOpen;
                let head = PortSignature::try_from_ports(ports).expect("ramp head");
                let upper = HexCoord {
                    level: cell.coord.level + 1,
                    ..cell.coord
                };
                assert!(
                    signatures
                        .insert(upper, rotate_signature(head, cell.turn))
                        .is_none()
                );
            }
        }
        let grid = HexGridSize {
            cols: 10,
            rows: 10,
            levels: 4,
        };
        for (&cell, signature) in &signatures {
            for face in HexFace::ALL {
                if let Some(neighbor) = grid.neighbor(cell, face)
                    && let Some(other) = signatures.get(&neighbor)
                {
                    assert!(
                        ports_compatible(signature.port(face), other.port(face.opposite())),
                        "unmatched {cell:?} {face:?} -> {neighbor:?}"
                    );
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![cells[0].coord];
        while let Some(cell) = pending.pop() {
            if !seen.insert(cell) {
                continue;
            }
            for face in HexFace::ALL {
                if signatures[&cell].port(face) != PortClass::Sealed
                    && let Some(next) = grid.neighbor(cell, face)
                    && signatures.contains_key(&next)
                {
                    pending.push(next);
                }
            }
        }
        assert_eq!(
            seen.len(),
            signatures.len(),
            "all tiers must be reachable through authored ports"
        );
        state.compositions.push(Composition::Layout { cells });
        let index = state.compositions.len() - 1;
        state.switch(index);
        let colliders = state.scene.collider_count();
        let spawn = state.body.position;
        for _ in 0..3 {
            state.body.position = Vec3::ZERO;
            state.switch(index);
            assert_eq!(state.scene.collider_count(), colliders);
            assert_eq!(state.body.position, spawn);
        }
    }
    #[test]
    fn witness_exchange_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/witness_exchange/hero.json"),
            6,
            "wellshaft",
            24,
        );
    }

    #[test]
    fn last_courtyard_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/last_courtyard/hero.json"),
            8,
            "thinning",
            9,
        );
    }
    #[test]
    fn empty_audience_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/empty_audience/hero.json"),
            4,
            "facet_monument",
            11,
        );
    }
    #[test]
    fn missing_rooms_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/missing_rooms/hero.json"),
            7,
            "infinite_gallery",
            14,
        );
    }

    #[test]
    fn a_capped_tower_loses_its_lid_in_section_but_an_open_climb_keeps_its_landing() {
        let state = LabState::load();
        let cap =
            resolve_tile(&state.tiles, "stair_tower", 240, "infinite_gallery").expect("shaft head");
        let base =
            resolve_tile(&state.tiles, "stair_tower", 234, "infinite_gallery").expect("shaft foot");
        assert_eq!(tile_ceiling_height(cap), 8.0);
        assert_eq!(tile_ceiling_height(base), 16.0);
        let cap_roofs = cap
            .hulls
            .iter()
            .filter(|h| {
                section_hides(
                    h,
                    tile_ceiling_height(cap),
                    &Transform::IDENTITY,
                    Vec3::ZERO,
                    SectionCut::Plan,
                    0.0,
                )
            })
            .count();
        assert_eq!(cap_roofs, 1, "only the actual lid should disappear");
        assert!(base.hulls.iter().all(|h| !section_hides(
            h,
            tile_ceiling_height(base),
            &Transform::IDENTITY,
            Vec3::ZERO,
            SectionCut::Plan,
            0.0
        )));
    }
    #[test]
    fn volume_section_removes_shelves_and_their_lights_with_the_cut_quadrant() {
        let shelf = vec![Vec3::new(3.0, 2.0, 3.0), Vec3::new(5.0, 2.2, 5.0)];
        assert!(!section_hides(
            &shelf,
            8.0,
            &Transform::IDENTITY,
            Vec3::ZERO,
            SectionCut::Quarter,
            0.0
        ));
        assert!(section_hides(
            &shelf,
            8.0,
            &Transform::IDENTITY,
            Vec3::ZERO,
            SectionCut::QuarterVolume,
            0.0
        ));
        assert!(section_point_hidden(
            Vec3::new(4.0, 3.0, 4.0),
            Vec3::ZERO,
            SectionCut::QuarterVolume,
            0.0
        ));
        assert!(!section_point_hidden(
            Vec3::new(-4.0, 3.0, 4.0),
            Vec3::ZERO,
            SectionCut::QuarterVolume,
            0.0
        ));
        assert_eq!(
            SectionCut::parse("quarter_volume"),
            Some(SectionCut::QuarterVolume)
        );
    }
    #[test]
    fn volume_section_clips_a_crossing_floor_instead_of_discarding_the_whole_slab() {
        let mut cube = Vec::new();
        for x in [-4.0, 4.0] {
            for y in [0.0, 0.5] {
                for z in [-4.0, 4.0] {
                    cube.push(Vec3::new(x, y, z));
                }
            }
        }
        let half = half_volume_hull(&cube, &Transform::IDENTITY, Vec3::ZERO, 0.0);
        assert!(half.iter().all(|p| p.x <= SECTION_EPS + 0.001));
        assert!(hull_mesh(&half).is_some());
        let pieces = volume_section_pieces(&cube, &Transform::IDENTITY, Vec3::ZERO, 0.0);
        assert!(
            pieces
                .iter()
                .all(|piece| piece.len() >= 8 && hull_mesh(piece).is_some())
        );
        assert!(pieces[0].iter().all(|p| p.x <= SECTION_EPS + 0.001));
        assert!(
            pieces[1]
                .iter()
                .all(|p| p.x >= SECTION_EPS - 0.001 && p.z <= SECTION_EPS + 0.001)
        );
        assert!(
            pieces
                .iter()
                .flatten()
                .any(|p| (p.x - SECTION_EPS).abs() < 0.001)
        );
    }
    #[test]
    fn roof_removal_preserves_all_twenty_reading_room_shelves() {
        let state = LabState::load();
        let tile = resolve_tile(&state.tiles, "hall_turn_120", 840, "infinite_gallery")
            .expect("reading room");
        let hidden = tile
            .hulls
            .iter()
            .filter(|h| {
                section_hides(
                    h,
                    tile_ceiling_height(tile),
                    &Transform::IDENTITY,
                    Vec3::ZERO,
                    SectionCut::Plan,
                    0.0,
                )
            })
            .count();
        assert_eq!(hidden, 1, "remove the roof, not the upper shelf courses");
    }
    #[test]
    fn a_roof_cut_removes_offset_practicals_with_their_parent_module() {
        let tile = observed_authoring::parse_authored_module(
            &observed_authoring::forge::borrowed::layered(),
        )
        .expect("valid screen source")
        .prototype;
        let practicals = preview_practicals(
            &tile,
            Transform::from_xyz(0.0, 0.0, 3.0),
            ArchitectureRegister::ShadowScreen,
            style::HexComposition::Hall,
        );
        assert!(practicals.iter().any(|light| light.position.z < 0.0));
        let roof = RoofSection {
            height: Some(7.25),
            front_only: true,
            ..default()
        };
        assert!(practicals.iter().all(|light| !roof.keeps_practical(
            light.module_origin,
            Vec3::ZERO,
            SectionCut::Plan,
            std::f32::consts::FRAC_PI_2
        )));
    }

    #[test]
    fn uncalled_number_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/uncalled_number/hero.json"),
            3,
            "institutional",
            15,
        );
    }

    #[test]
    fn weight_between_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/weight_between/hero.json"),
            1,
            "monolith",
            13,
        );
    }

    #[test]
    fn borrowed_view_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/borrowed_view/hero.json"),
            0,
            "shadow_screen",
            11,
        );
    }

    #[test]
    fn third_light_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/third_light/hero.json"),
            9,
            "liminal_grid",
            19,
        );
    }

    #[test]
    fn low_ceiling_sections_keep_rear_roofs_and_remove_front_practicals() {
        assert_eq!(
            hull_surface_kind(&[Vec3::ZERO, Vec3::new(7.0, 0.5, 4.0)], 8.0),
            SurfaceKind::Floor
        );
        let roof = RoofSection {
            height: Some(2.95),
            upper: Some(4.5),
            front_only: true,
        };
        let slab = vec![Vec3::new(-2.0, 3.5, -2.0), Vec3::new(2.0, 3.75, 2.0)];
        let front = Transform::from_xyz(8.0, 0.0, 0.0);
        let rear = Transform::from_xyz(-8.0, 0.0, 0.0);
        assert!(roof.hides(&slab, 8.0, &front, Vec3::ZERO, SectionCut::Half, 0.0));
        assert!(!roof.hides(&slab, 8.0, &rear, Vec3::ZERO, SectionCut::Half, 0.0));
        assert!(!roof.hides(&slab, 8.0, &front, Vec3::ZERO, SectionCut::None, 0.0));
        assert!(!roof.keeps_practical(front.translation, Vec3::ZERO, SectionCut::Half, 0.0));
        assert!(roof.keeps_practical(rear.translation, Vec3::ZERO, SectionCut::Half, 0.0));
        assert!(roof.keeps_practical(front.translation, Vec3::ZERO, SectionCut::None, 0.0));
        let envelope = vec![Vec3::new(-2.0, 7.5, -2.0), Vec3::new(2.0, 8.0, 2.0)];
        assert!(roof.hides(&envelope, 8.0, &rear, Vec3::ZERO, SectionCut::Plan, 0.0));
        assert!(!roof.hides(&envelope, 8.0, &rear, Vec3::ZERO, SectionCut::None, 0.0));
        let full = RoofSection {
            front_only: false,
            ..roof
        };
        assert!(full.hides(&slab, 8.0, &rear, Vec3::ZERO, SectionCut::Plan, 0.0));
        let floor = vec![Vec3::ZERO, Vec3::new(2.0, 0.5, 2.0)];
        assert!(!full.hides(&floor, 8.0, &front, Vec3::ZERO, SectionCut::Plan, 0.0));
    }

    #[test]
    fn unfinished_crossing_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/unfinished_crossing/hero.json"),
            5,
            "megastructure",
            17,
        );
    }

    #[test]
    fn same_door_twice_resolves_mates_connects_and_resets() {
        assert_layout_resolves_mates_connects_and_resets(
            include_str!("../../../docs/compositions/same_door_twice/hero.json"),
            2,
            "overlit_grid",
            15,
        );
    }
}
