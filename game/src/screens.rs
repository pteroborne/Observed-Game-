//! The assembled game's UX: a Bevy state machine that strings the proven systems
//! into one cohesive loop — Splash → Main Menu → Loadout → Lobby → Match → Results
//! → Menu — with a persistent career, a unified visual theme, keyboard/controller navigation,
//! a first-person 3D match with an in-match HUD and pause, and strict state-scoped
//! cleanup (every screen's entities despawn on exit, so transitions never leak).
//!
//! Each screen reuses a proven model: the **career/profile** ([`crate::flow`] +
//! `progression_lab`), the **lobby/matchmaking** (`session_lab`), and the **match** —
//! the live, first-person, networked hybrid match (`net_match_lab`'s `LiveNetMatch`
//! over `fps_hybrid_match_lab`). The state machine and presentation are the only new
//! code; the game logic is the labs.
//!
//! This module is split by runtime responsibility into submodules; it keeps only the
//! shared theme/components/resources/UI helpers plus the two composition plugins, and
//! re-exports each submodule so the rest of the crate keeps the flat `screens::*` path.
//! The submodules are descendants of this module, so they reach the shared items
//! (including `MatchAssets`'s private fields) through `use super::*`.

use std::path::PathBuf;

use bevy::input::InputSystems;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::facility::TEAM_COUNT;
use observed_match::maze::{CORRIDOR_RADIUS, TILE_SIZE};
use observed_net::netmatch::LiveNetMatch;
use observed_progression::session::SessionLabWorld;
use observed_style as style;
use observed_traversal::{FpsArena, FpsBody, FpsConfig};
use player_input::PlayerIntent;

use crate::GameState;
use crate::teleport::{self, Place};

mod audio;
mod hud;
mod input;
mod loadout;
mod lobby;
mod match_runtime;
mod menu;
mod place;

pub(crate) use audio::*;
pub(crate) use hud::*;
pub(crate) use input::*;
pub(crate) use loadout::*;
pub(crate) use lobby::*;
pub(crate) use match_runtime::*;
pub(crate) use menu::*;
pub(crate) use place::*;

const WALL_HEIGHT: f32 = 3.4;

/// How long (seconds) the first-person decoherence feedback — the route-shift flash,
/// camera jolt, and door slam — lasts after a reroute commits. Shared so the camera
/// jolt ([`place::present_match_camera`]) and the feedback driver
/// ([`match_runtime::sync_decohere_fx`]) agree.
pub(crate) const ROUTE_SHIFT_FLASH_SECS: f32 = 0.7;

// Tac-map overlay panel size (pixels); the per-element layout lives in `hud`.
const TAC_MAP_SIZE: f32 = 300.0;

// Drop-in asset slots — the paths are *not* re-declared here; they come from the
// shared `observed_assets` manifest (the single source of truth `asset_lab` also
// reads). Drop a file at a slot's path and the match uses it; leave it absent and the
// match falls back procedurally.
const CEILING_TEX: &str = observed_assets::CEILING.path;
const EXIT_PANEL_TEX: &str = observed_assets::EXIT_PANEL.path;
// Present in the asset inventory but not rendered: proper HDRI image-based lighting
// needs a `.ktx2` cubemap, not an equirectangular `.hdr` (see assets/SOURCES.md).
const ENVIRONMENT_HDR: &str = observed_assets::ENVIRONMENT.path;
const LIGHT_FIXTURE_MODEL: &str = observed_assets::LIGHT_FIXTURE.path;
const EXIT_GATE_MODEL: &str = observed_assets::EXIT_GATE.path;
const PLAYER_MODEL: &str = observed_assets::PLAYER.path;
const BOT_MODEL: &str = observed_assets::BOT.path;
const EQUIPMENT_MODEL: &str = observed_assets::EQUIPMENT.path;
const HAZARD_MODEL: &str = observed_assets::HAZARD.path;
const FOOTSTEP_SOUND: &str = observed_assets::FOOTSTEP.path;
const REROUTE_SOUND: &str = observed_assets::REROUTE.path;
const ESCAPE_SOUND: &str = observed_assets::ESCAPE.path;
const AMBIENCE_SOUND: &str = observed_assets::AMBIENCE.path;
// Optional (not in the required asset plan): a door open/close thunk on entering or
// leaving a place. Silent until a file is dropped here.
const DOOR_SOUND: &str = observed_assets::DOOR.path;

const FIXTURE_LIGHT_INTENSITY: f32 = 2_800.0;
const FOOTSTEP_STRIDE: f32 = 1.8;

// Procedural neon doorways (code-as-art; no GLB). A closed leaf hides the corridor
// beyond (mystery) and slides up into the lintel as the player approaches.
// The frame spans the FULL hall width so doorways and hallways line up by design —
// corridors are carved to `CORRIDOR_RADIUS` (1 ⇒ 3 tiles wide), not one tile.
const HALL_WIDTH: f32 = (2 * CORRIDOR_RADIUS + 1) as f32 * TILE_SIZE;
const DOOR_POST_W: f32 = 0.22;
const DOOR_POST_D: f32 = 0.5;
const DOOR_LINTEL_H: f32 = 0.34;
const DOOR_LEAF_D: f32 = 0.14;

/// The workspace `assets/` directory (where `cargo run` resolves Bevy's asset root).
pub(crate) fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("game crate lives directly under the workspace root")
        .join("assets")
}

fn asset_present(relative: &str) -> bool {
    assets_dir().join(relative).is_file()
}

/// Build a neon-noir `StandardMaterial` from the shared visual language's treatment,
/// so the match never invents ad-hoc surface colours (see `observed_style`).
fn neon_material(t: &style::Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: t.base_color,
        emissive: t.emissive,
        perceptual_roughness: 0.85,
        ..default()
    }
}

pub(crate) const PLANNED_ASSET_PATHS: [&str; 13] = [
    CEILING_TEX,
    LIGHT_FIXTURE_MODEL,
    EXIT_GATE_MODEL,
    EXIT_PANEL_TEX,
    PLAYER_MODEL,
    BOT_MODEL,
    EQUIPMENT_MODEL,
    HAZARD_MODEL,
    FOOTSTEP_SOUND,
    REROUTE_SOUND,
    ESCAPE_SOUND,
    AMBIENCE_SOUND,
    ENVIRONMENT_HDR,
];

fn all_planned_assets_present() -> bool {
    PLANNED_ASSET_PATHS.iter().all(|path| asset_present(path))
}

// --- theme -----------------------------------------------------------------
const TITLE: Color = Color::srgb(0.95, 0.97, 1.0);
const ACCENT: Color = Color::srgb(0.40, 0.92, 1.0);
const DIM: Color = Color::srgb(0.58, 0.64, 0.74);
const PANEL: Color = Color::srgba(0.04, 0.06, 0.10, 0.92);
const BORDER: Color = Color::srgba(0.40, 0.92, 1.0, 0.5);

const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34),
    Color::srgb(0.32, 0.62, 1.0),
    Color::srgb(0.72, 0.46, 1.0),
    Color::srgb(1.0, 0.62, 0.20),
];

// --- shared components / resources -----------------------------------------
#[derive(Component)]
pub(crate) struct ScreenRoot;

#[derive(Clone, Copy)]
pub(crate) enum MenuAction {
    Goto(GameState),
    StartRun,
    Launch,
    Equip(u16),
    QuitApp,
}

#[derive(Component)]
pub(crate) struct MenuButton {
    index: usize,
    action: MenuAction,
}

#[derive(Component)]
pub(crate) struct MenuBanner;

#[derive(Component)]
pub(crate) struct LoadoutHeader;

#[derive(Component)]
pub(crate) struct MatchHud;

#[derive(Component)]
pub(crate) struct PausePanel;

#[derive(Resource, Default)]
pub struct MenuCursor(pub usize);

#[derive(Resource)]
pub struct SplashTimer(pub Timer);

#[derive(Resource)]
pub struct MatchRuntime {
    /// The live, host-authoritative networked first-person match: the host is the
    /// locally-played match, replicated over the lockstep transport to a remote.
    pub live: LiveNetMatch,
    /// Resolves a round when the local team is no longer an active runner so the
    /// remaining teams can finish.
    pub wait_timer: Timer,
    pub done: bool,
}

/// The player's first-person intent for the current frame, consumed by the
/// fixed-timestep controller.
#[derive(Resource, Default)]
pub struct MatchIntent(pub PlayerIntent);

/// One-frame item actions sampled from hardware input and consumed by the item systems.
/// Kept separate from [`PlayerIntent`] because these are game-local tool actions, not
/// movement/controller intent.
#[derive(Resource, Default)]
pub struct ItemIntent {
    pub(crate) torch_action: bool,
    pub(crate) pad_action: bool,
    pub(crate) activate_pad: bool,
}

/// Marks the shared 3D camera (first-person during the Match).
#[derive(Component)]
pub(crate) struct GameCam;

/// Shared meshes/materials for the solid maze, resolved with the drop-in convention:
/// textured/models/audio if present, procedural fallbacks otherwise.
#[derive(Resource)]
// Some drop-in slots (GLB scenes, panel/halo meshes, team materials) are retained for
// the asset plan and future per-place props but aren't read by the teleport renderer.
#[allow(dead_code)]
pub struct MatchAssets {
    floor_mesh: Handle<Mesh>,
    wall_mesh: Handle<Mesh>,
    ceiling_mesh: Handle<Mesh>,
    panel_mesh: Handle<Mesh>,
    placeholder_mesh: Handle<Mesh>,
    halo_mesh: Handle<Mesh>,
    door_post_mesh: Handle<Mesh>,
    door_lintel_mesh: Handle<Mesh>,
    door_leaf_mesh: Handle<Mesh>,
    objective_beam_mesh: Handle<Mesh>,
    rival_body_mesh: Handle<Mesh>,
    floor_material: Handle<StandardMaterial>,
    spine_floor_material: Handle<StandardMaterial>,
    safe_floor_material: Handle<StandardMaterial>,
    trap_active_material: Handle<StandardMaterial>,
    trap_idle_material: Handle<StandardMaterial>,
    wall_material: Handle<StandardMaterial>,
    ceiling_material: Handle<StandardMaterial>,
    exit_panel_material: Handle<StandardMaterial>,
    fixture_glow_material: Handle<StandardMaterial>,
    placeholder_material: Handle<StandardMaterial>,
    doorframe_material: Handle<StandardMaterial>,
    spine_doorframe_material: Handle<StandardMaterial>,
    door_leaf_material: Handle<StandardMaterial>,
    objective_material: Handle<StandardMaterial>,
    rival_material: Handle<StandardMaterial>,
    anchor_torch_material: Handle<StandardMaterial>,
    teleport_pad_material: Handle<StandardMaterial>,
    team_materials: [Handle<StandardMaterial>; TEAM_COUNT],
    light_fixture: Option<Handle<Scene>>,
    exit_gate: Option<Handle<Scene>>,
    player: Option<Handle<Scene>>,
    bot: Option<Handle<Scene>>,
    equipment: Option<Handle<Scene>>,
    hazard: Option<Handle<Scene>>,
    footstep: Option<Handle<AudioSource>>,
    reroute: Option<Handle<AudioSource>>,
    escape: Option<Handle<AudioSource>>,
    ambience: Option<Handle<AudioSource>>,
    door: Option<Handle<AudioSource>>,
}

/// Marks the spawned geometry of the *current place* (room or hallway) so it can be
/// torn down and rebuilt when the player teleports to the next place.
#[derive(Component)]
pub(crate) struct PlaceGeometry;

/// An animated door leaf filling a doorway gap. A passage leaf (`openable`) slides up
/// into the lintel as the player nears — restoring the "the corridor is hidden until
/// you commit to it" mystery — and slams shut again when they leave or the structure
/// decoheres. Sealed leaves (side doors, the locked exit) never open. `closed_y`/
/// `open_y` are the leaf's local-Y at each extreme; `center` is the gap centre (XZ) for
/// the proximity test. Presentation-only — driven entirely by `animate_doors`.
#[derive(Component)]
pub(crate) struct DoorLeaf {
    pub center: Vec2,
    pub closed_y: f32,
    pub open_y: f32,
    pub openable: bool,
}

/// A keystone pickup item sitting in a room; collected on contact.
#[derive(Component)]
pub(crate) struct KeystoneItem(pub RoomId);

/// A droppable single-player tool visible in the current place.
#[derive(Component)]
pub(crate) struct DroppedItemVisual;

/// A rival team's avatar, walking the player's current room while that team's clump
/// shares it. Holds the rival team index; managed entirely by `sync_rival_avatars`
/// (presentation-only — reads the brain, never writes it).
#[derive(Component)]
pub(crate) struct RivalAvatar {
    pub team: usize,
}

/// Full-screen reroute-flash overlay (shown briefly when the layout shifts).
#[derive(Component)]
pub(crate) struct RouteShiftPanel;

/// Geometry rendered behind a room's open doorway as a preview of the actual hallway
/// you'll teleport into (aligned to the opening). Also tagged [`PlaceGeometry`] so it is
/// torn down on the next teleport; this marker just lets it be queried/tested.
#[derive(Component)]
pub(crate) struct PassagePreview;

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) enum MatchAudioCue {
    Ambience,
    Footstep,
    Door,
    Escape,
    Reroute,
}

/// First-person feedback for graph **decoherence** (a committed reroute): when the
/// brain's `reroute_commits` advances — the unobserved structure has rewired — the game
/// fires a brief route-shift flash, an audio sting, a camera jolt, and slams the current
/// place's doors shut (the "re-hide"). Presentation-only; driven by
/// [`match_runtime::sync_decohere_fx`]. Initialised to the live commit count on entering
/// the Match so it never flashes on the first frame.
#[derive(Resource, Default)]
pub struct DecohereFx {
    /// The reroute-commit count we last reacted to.
    pub last_commits: u32,
    /// Seconds remaining on the active route-shift feedback (0 = idle).
    pub flash: f32,
}

#[derive(Resource)]
pub struct MatchAudioState {
    last_position: Vec3,
    stride_distance: f32,
    last_place: Place,
    escaped_count: usize,
}

#[derive(Resource, Default)]
pub struct MatchPaused(pub bool);

/// Whether the tac-map overlay is currently shown (toggled with Tab).
#[derive(Resource, Default)]
pub struct TacMapState(pub bool);

/// The root node of the tac-map overlay (Visibility toggled with Tab).
#[derive(Component)]
pub(crate) struct TacMapPanel;

/// A dynamic child of the tac-map (room/bar/marker), rebuilt each frame while shown.
#[derive(Component)]
pub(crate) struct TacMapElement;

/// The live teleport-place state: which discrete place the player occupies, the
/// controller body + its collision arena for that place, and what the renderer last
/// built (so it rebuilds only on a teleport).
#[derive(Resource)]
pub struct TeleportState {
    pub place: teleport::Place,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub arena: FpsArena,
    /// The current place's footprint + doorway gaps + interior (maze) walls. Cached so
    /// a labyrinth is generated once per teleport, not every fixed step.
    pub geom: teleport::PlaceGeom,
    pub prev_xz: Vec2,
    /// Latched once the body crosses a hallway's exit, until the round commits.
    pub crossed_exit: bool,
    /// The place the geometry currently reflects.
    pub rendered: Option<teleport::Place>,
}

#[derive(Resource)]
pub struct LobbyRuntime {
    /// The formed session is retained for the duration of the lobby/match so the
    /// matchmaking state stays live; the screen renders from it at spawn time.
    #[allow(dead_code)]
    pub world: SessionLabWorld,
}

// --- ui helpers ------------------------------------------------------------
fn screen_root(state: GameState) -> impl Bundle {
    (
        ScreenRoot,
        DespawnOnExit(state),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(10),
            ..default()
        },
    )
}

fn panel() -> impl Bundle {
    (
        Node {
            min_width: px(560),
            padding: UiRect::all(px(26)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(8),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s.into()),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

fn menu_button(index: usize, action: MenuAction, label: impl Into<String>) -> impl Bundle {
    (
        MenuButton { index, action },
        Text::new(label.into()),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(DIM),
    )
}

// --- composition -----------------------------------------------------------
/// The menu flow: every menu-like screen (splash/main-menu/loadout/lobby/results) and
/// the shared keyboard/controller navigation. Inert where there are no buttons.
pub(crate) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuCursor>()
            .add_systems(OnEnter(GameState::Splash), setup_splash)
            .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(OnEnter(GameState::Loadout), setup_loadout)
            .add_systems(OnEnter(GameState::Lobby), setup_lobby)
            .add_systems(OnEnter(GameState::Results), setup_results)
            .add_systems(
                Update,
                (
                    // Menu navigation is shared across every menu-like screen and is
                    // inert where there are no buttons (Splash/Match).
                    menu_navigate.after(InputSystems),
                    menu_highlight,
                    menu_activate,
                    menu_escape,
                    splash_advance.run_if(in_state(GameState::Splash)),
                    main_menu_banner.run_if(in_state(GameState::MainMenu)),
                    loadout_header.run_if(in_state(GameState::Loadout)),
                )
                    .chain(),
            );
    }
}

/// The first-person match: lifecycle (setup/atmosphere/cursor/cleanup), the fixed-step
/// teleport controller, place rendering, audio, HUD, and the tac-map.
pub(crate) struct MatchPlugin;

impl Plugin for MatchPlugin {
    fn build(&self, app: &mut App) {
        let in_match = || in_state(GameState::Match).and(resource_exists::<MatchRuntime>);
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(
                OnEnter(GameState::Match),
                (
                    setup_match,
                    spawn_match_setpieces,
                    grab_match_cursor,
                    apply_match_atmosphere,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(GameState::Match),
                (
                    release_match_cursor,
                    cleanup_match_resources,
                    clear_match_atmosphere,
                ),
            )
            .add_systems(FixedUpdate, teleport_sim.run_if(in_match()))
            .add_systems(
                Update,
                (
                    match_input.after(InputSystems),
                    toggle_tac_map,
                    match_pump,
                    item_actions,
                    rebuild_place,
                    sync_decohere_fx,
                    keystone_pickup,
                    sync_rival_avatars,
                    sync_match_audio,
                    animate_doors,
                    present_match_camera,
                    match_draw,
                    draw_tac_map,
                )
                    .chain()
                    .run_if(in_match()),
            );
    }
}
