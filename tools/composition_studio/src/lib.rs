//! `composition_studio` — author the WFC composition profile and watch the
//! facility answer.
//!
//! Tiles say what the solver *may* build; the composition profile says what it
//! *tends* to build. Slice 0 made that profile authorable content and folded it
//! into the simulation content hash. This tool is where a person actually turns
//! the knobs and sees the layout move.
//!
//! Two rules this tool inherits and must not soften:
//!
//! - **Never display a hash it did not compute.** The status line shows the
//!   folded simulation hash because editing a profile locks out LAN peers who
//!   have not taken the same edit. A placeholder there would be worse than a
//!   blank, so an unreadable catalog reports *unavailable* rather than a
//!   plausible-looking wrong value.
//! - **Never silently fall back to the baseline profile.** Slice 0 made a
//!   missing profile a hard error precisely because a quiet fallback is what the
//!   content hash exists to catch. If the corpus profile cannot be read, the
//!   tool says so, keeps saying so, and refuses to promote over it.

pub mod actionbar;
pub mod brush;
pub mod capture;
pub mod chrome;
pub mod coverage;
pub mod detail;
pub mod draw;
pub mod field_widgets;
pub mod input;
pub mod layer;
pub mod module;
pub mod panels;
pub mod persist;
pub mod pick;
pub mod script;
pub mod solve;
pub mod tunables;
pub mod viewport;
pub mod viewport_input;

/// Pin and coverage gates, split out so neither test file outgrows the
/// 600-line review budget the rest of the WFC path lives under.
#[cfg(test)]
pub mod authoring_tests;
#[cfg(test)]
pub mod tests;
#[cfg(test)]
pub mod widget_tests;

use std::sync::OnceLock;

use bevy::prelude::*;
use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::profile::HexCompositionProfile;
use observed_facility::hex_wfc::score::LayoutScore;
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld, SolveStep};
use observed_hex::HexCoord;
use observed_match::hex_wfc::HexWfcGeometrySnapshot;
use observed_style::schematic_screen;

pub use chrome::{LabMenuState, StudioTab};
pub use draw::DrawReport;
pub use layer::Layer;
pub use viewport::StudioCamera;

/// The same five seeds `iso_observer_lab` pins, so a studio capture and an Arc O
/// capture are comparing the same facilities. Treat as an evidence contract.
pub const PRESET_SEEDS: [u64; 5] = [
    0xa11c_e3d0_0000_0008,
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
];

/// How long after the last edit the solver waits before re-solving. Dragging a
/// value through ten steps should cost one solve, not ten.
pub const SOLVE_DEBOUNCE_SECONDS: f32 = 0.25;

/// Width of the docked panel, in logical pixels. The 3D camera's viewport is
/// inset by this, so the panel sits *beside* the facility rather than on top of
/// it — nothing is ever hidden behind chrome.
pub const PANEL_WIDTH: f32 = 560.0;

/// Which region the keyboard is talking to.
///
/// The studio used to be modal: opening the panel froze the viewport. That is
/// the right trade in a lab where a stray keypress moves your character, and
/// the wrong one in a design tool, where the whole loop is *change a value and
/// watch the facility answer*. You cannot watch something you have frozen.
///
/// So the rule "a key never means two things at once" is kept, but enforced by
/// **ownership** rather than by blocking the world: whichever region you last
/// clicked receives keys, both stay live, and the owner is drawn with a border
/// so it is never a guess.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KeyboardOwner {
    #[default]
    Panel,
    Viewport,
}

impl KeyboardOwner {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Viewport => "viewport",
        }
    }
}

pub struct SolveResult {
    pub world: HexWfcWorld,
    pub steps: Vec<SolveStep>,
    pub score: LayoutScore,
    pub attempts: u32,
    pub elapsed_ms: u32,
    /// `None` when the authored catalog could not be projected. The schematic
    /// still draws; the status line says the projection is unavailable.
    pub geometry: Option<HexWfcGeometrySnapshot>,
    /// What this layout asks the catalog for, and whether it is there.
    pub coverage: crate::coverage::CoverageReport,
    /// Every candidate the search considered, winner included. One entry when
    /// the profile asks for a single candidate, so the Solve tab never has to
    /// special-case "no search happened".
    pub candidates: Vec<observed_facility::hex_wfc::CandidateOutcome>,
}

/// Where the in-memory profile came from, so the status line can say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileOrigin {
    Working,
    Corpus,
    /// The corpus profile could not be read. The tool runs on the baseline so a
    /// person can still look around, but it must not pretend this is normal and
    /// must refuse to promote over a corpus it failed to load.
    Unreadable(String),
}

impl ProfileOrigin {
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Unreadable(_))
    }
}

/// The catalog half of the folded simulation hash.
///
/// Deliberately an enum rather than a `String` default: there is no honest
/// placeholder for a content hash, and a zeroed one folds into a plausible
/// wrong answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogHash {
    Known(String),
    Unavailable(String),
}

#[derive(Resource)]
pub struct StudioState {
    /// The profile being edited.
    pub profile: HexCompositionProfile,
    /// Fixed reference point for the A/B comparison.
    pub baseline: HexCompositionProfile,
    /// What is currently on disk, for the dirty marker and `Ctrl+Z`.
    pub saved: HexCompositionProfile,
    pub saved_hash: String,
    pub origin: ProfileOrigin,
    pub catalog_hash: CatalogHash,

    pub config: HexWfcConfig,
    pub seed_index: usize,

    pub solved: Option<SolveResult>,
    /// The same seed solved at the baseline profile, for the compare overlay
    /// and the score delta. Invalidated whenever the seed or config moves.
    pub baseline_world: Option<HexWfcWorld>,
    pub baseline_score: Option<LayoutScore>,

    /// The profile changed; the facility must be solved again.
    pub solve_dirty: bool,
    /// Drawn geometry changed — layer, detail mode, detent, cutaway, or a new
    /// solve. Triggers a full re-emit, including hull triangulation.
    pub geometry_dirty: bool,
    /// Only the selection or hover ring changed.
    ///
    /// Kept separate because hover fires on every mouse move to a new cell, and
    /// moving a ring has no business re-triangulating the facility. At a
    /// hundred cells that distinction is invisible; at production scale it is
    /// the difference between usable and not.
    pub overlay_dirty: bool,
    /// Seconds on the app clock when the last edit landed, for debouncing.
    pub last_edit: Option<f32>,
    pub reset_count: u32,

    pub zoom: f32,
    pub pan: Vec2,
    pub layer: Layer,
    pub selected: Option<HexCoord>,
    /// The cell under the cursor. Drawn as a ring and echoed by the cursor
    /// shape, because "wasn't sure where to click" is a discoverability bug and
    /// a 3D viewport with no hover state gives you nothing to aim at.
    pub hovered: Option<HexCoord>,
    pub show_walls: bool,
    pub show_baseline_compare: bool,
    /// Which cells draw their real authored geometry.
    pub detail_mode: detail::DetailMode,
    /// Whether the ceiling and near walls are cut away in detail mode.
    pub cutaway: bool,
    /// View azimuth, in 60-degree detents anchored at the historical default.
    pub detent: usize,
    pub detail_report: detail::DetailReport,
    /// Whether the docked panel is expanded. Collapsing gives the facility the
    /// whole window; it never changes what the keyboard can reach.
    pub panel_open: bool,
    /// Which region receives keys. See [`KeyboardOwner`].
    pub keyboard_owner: KeyboardOwner,
    /// What left-drag paints.
    pub brush: brush::Brush,
    /// Diagnostics for the current pin set, refreshed on every pin edit.
    pub pin_diagnostics: Vec<observed_facility::hex_wfc::PinDiagnostic>,
    pub status: String,
    pub report: DrawReport,
    /// The whole-catalog seam audit. On demand, because it recompiles every
    /// authored `.map` and would make the tuning loop unusable if it ran per
    /// solve.
    pub seam_audit: panels::coverage::SeamAudit,
    pub base_frame: (Transform, f32, f32),
}

fn tile_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("assets/tiles");
    if cwd_relative.exists() {
        return cwd_relative;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
}

/// The authored tile corpus: per-cell prototypes and whole-room prototypes.
pub type Corpus = (Vec<TilePrototype>, Vec<RoomPrototype>);

/// The authored corpus, loaded once. The `Err` arm is carried rather than
/// discarded so the status line can name the failure.
pub fn corpus() -> &'static Result<Corpus, String> {
    static CORPUS: OnceLock<Result<Corpus, String>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        RuntimeHexCatalog::load(&tile_dir(), &slugs)
            .map(|loaded| (loaded.cells, loaded.rooms))
            .map_err(|error| format!("authored catalog unavailable: {error}"))
    })
}

/// Read the compiled catalog's committed digest — the other half of the fold.
fn load_catalog_hash() -> CatalogHash {
    let path = persist::corpus_dir().join("compiled_catalog.sha256");
    match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().len() == 64 => CatalogHash::Known(text.trim().to_string()),
        Ok(_) => CatalogHash::Unavailable(format!("{} is not a 64-char digest", path.display())),
        Err(error) => CatalogHash::Unavailable(format!("{}: {error}", path.display())),
    }
}

/// Load the profile to edit: the working copy if one exists, else the corpus.
fn load_startup_profile() -> (HexCompositionProfile, String, ProfileOrigin) {
    if let Ok(build) = observed_authoring::composition::load_profile(&persist::working_dir()) {
        return (build.profile, build.content_hash, ProfileOrigin::Working);
    }
    match observed_authoring::composition::load_profile(&persist::corpus_dir()) {
        Ok(build) => (build.profile, build.content_hash, ProfileOrigin::Corpus),
        Err(error) => {
            let baseline = HexCompositionProfile::baseline();
            let hash = observed_authoring::composition::profile_content_hash(&baseline)
                .unwrap_or_else(|_| String::from("unavailable"));
            (
                baseline,
                hash,
                ProfileOrigin::Unreadable(format!("corpus profile unreadable: {error}")),
            )
        }
    }
}

impl Default for StudioState {
    fn default() -> Self {
        let (profile, saved_hash, origin) = load_startup_profile();
        let catalog_hash = load_catalog_hash();

        let mut status = match &origin {
            ProfileOrigin::Working => String::from("loaded working profile"),
            ProfileOrigin::Corpus => String::from("loaded corpus profile"),
            ProfileOrigin::Unreadable(detail) => format!("ERROR: {detail}"),
        };
        if let CatalogHash::Unavailable(detail) = &catalog_hash {
            status.push_str(&format!("  |  ERROR: {detail}"));
        }
        if let Err(detail) = corpus() {
            status.push_str(&format!("  |  {detail}"));
        }

        Self {
            profile: profile.clone(),
            baseline: HexCompositionProfile::baseline(),
            saved: profile,
            saved_hash,
            origin,
            catalog_hash,
            config: HexWfcConfig::default(),
            seed_index: 0,
            solved: None,
            baseline_world: None,
            baseline_score: None,
            solve_dirty: true,
            geometry_dirty: true,
            overlay_dirty: true,
            last_edit: None,
            reset_count: 0,
            zoom: viewport::DEFAULT_ZOOM,
            pan: Vec2::ZERO,
            layer: Layer::All,
            selected: None,
            hovered: None,
            show_walls: true,
            show_baseline_compare: false,
            detail_mode: detail::DetailMode::default(),
            cutaway: true,
            detent: 0,
            detail_report: detail::DetailReport::default(),
            panel_open: true,
            keyboard_owner: KeyboardOwner::default(),
            brush: brush::Brush::default(),
            pin_diagnostics: Vec::new(),
            status,
            report: DrawReport::default(),
            seam_audit: panels::coverage::SeamAudit::default(),
            base_frame: (Transform::IDENTITY, 1.0, 1000.0),
        }
    }
}

impl StudioState {
    /// The seed currently being solved.
    #[must_use]
    pub fn seed(&self) -> u64 {
        PRESET_SEEDS[self.seed_index % PRESET_SEEDS.len()]
    }

    /// Mark the profile edited: re-solve after the debounce, and redraw.
    pub fn touch_profile(&mut self, now: f32) {
        self.solve_dirty = true;
        self.last_edit = Some(now);
    }

    /// Drawn geometry changed: re-emit everything, but do not re-solve.
    pub fn touch_view(&mut self) {
        self.geometry_dirty = true;
        self.overlay_dirty = true;
    }

    /// Only a ring moved. Cheap: no hull work.
    pub fn touch_overlay(&mut self) {
        self.overlay_dirty = true;
    }

    /// The seed or config moved, so the cached baseline solve no longer
    /// describes the same facility.
    pub fn invalidate_baseline(&mut self) {
        self.baseline_world = None;
        self.baseline_score = None;
    }

    /// Whether the in-memory profile differs from what is on disk.
    #[must_use]
    pub fn is_unsaved(&self) -> bool {
        self.profile != self.saved
    }

    /// Where the facility is drawn, in logical window pixels: the whole window
    /// minus the docked panel.
    #[must_use]
    pub fn viewport_origin(&self) -> f32 {
        if self.panel_open { PANEL_WIDTH } else { 0.0 }
    }

    /// Whether a window-space cursor position is over the facility.
    #[must_use]
    pub fn cursor_in_viewport(&self, cursor: Vec2) -> bool {
        cursor.x >= self.viewport_origin()
    }

    /// Convert a window-space cursor position into the camera's viewport space.
    ///
    /// The camera's viewport is inset by the panel, and `world_to_viewport`
    /// returns coordinates relative to *that* rect while `cursor_position` is
    /// relative to the window. Picking compares the two, so one of them has to
    /// move — and getting this wrong offsets every pick by the panel width,
    /// which reads as "clicking selects the wrong cell".
    #[must_use]
    pub fn cursor_to_viewport(&self, cursor: Vec2) -> Vec2 {
        Vec2::new(cursor.x - self.viewport_origin(), cursor.y)
    }

    /// Re-check the pin set. Cheap for the isolation checks, and it runs the
    /// attribution probe only when there is something to attribute.
    pub fn refresh_pin_diagnostics(&mut self) {
        self.pin_diagnostics = if self.profile.pin_sets.is_empty() {
            Vec::new()
        } else {
            observed_facility::hex_wfc::diagnose_pins(self.config, &self.profile)
        };
    }

    /// Re-read the profile from disk and re-solve, carrying the view.
    ///
    /// The house lab rule is that a reset must not require restarting the
    /// application, and must not throw away where you were looking.
    pub fn reload(&mut self, now: f32) {
        let (profile, saved_hash, origin) = load_startup_profile();
        self.status = match &origin {
            ProfileOrigin::Working => String::from("reloaded working profile"),
            ProfileOrigin::Corpus => String::from("reloaded corpus profile"),
            ProfileOrigin::Unreadable(detail) => format!("ERROR: {detail}"),
        };
        self.profile = profile.clone();
        self.saved = profile;
        self.saved_hash = saved_hash;
        self.origin = origin;
        self.catalog_hash = load_catalog_hash();
        self.selected = None;
        self.reset_count += 1;
        self.invalidate_baseline();
        self.refresh_pin_diagnostics();
        self.touch_profile(now);
        self.touch_view();
        // zoom, pan and layer are deliberately NOT touched.
    }
}

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        // Not part of `DefaultPlugins`: without it the sliders spawn, lay out,
        // and draw exactly as they should while ignoring every drag.
        //
        // `SliderPlugin` alone, not the whole `UiWidgetsPlugins` group. The
        // group's menu plugin runs `Update` systems against `InputFocus`, a
        // resource owned by a plugin this tool does not install - which turns
        // every headless test into a panic. Taking only the widget in use also
        // keeps the focus story straight: this tool routes the keyboard through
        // `KeyboardOwner`, and never through Bevy's focus resource.
        app.add_plugins(bevy::ui_widgets::SliderPlugin)
            .insert_resource(ClearColor(schematic_screen()))
            .init_resource::<StudioState>()
            .init_resource::<detail::TileMeshCache>()
            .init_resource::<LabMenuState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Startup, setup_studio)
            .add_systems(
                Update,
                (
                    input::handle_chrome_input,
                    update_studio_solve.after(input::handle_chrome_input),
                    draw::rebuild_visuals.after(update_studio_solve),
                    draw::rebuild_overlay.after(draw::rebuild_visuals),
                    viewport::sync_camera.after(draw::rebuild_visuals),
                    chrome::update_chrome_ui.after(update_studio_solve),
                    field_widgets::sync_field_rows.after(update_studio_solve),
                    viewport_input::handle_viewport_painting,
                    viewport_input::update_hover_and_cursor,
                    viewport::sync_camera_viewport,
                ),
            )
            .add_observer(field_widgets::apply_slider_change);

        if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
            app.insert_resource(capture::CaptureState {
                dir,
                name: String::from("studio_capture"),
                timer: 0.0,
                step: 0,
            })
            .add_systems(Update, capture::capture_system);
        }

        if let Some(path) = script::script_path() {
            match script::load_script(&path) {
                Ok(script) => {
                    app.insert_resource(script::ScriptRun {
                        script,
                        timer: 0.0,
                        phase: 0,
                    })
                    .add_systems(Update, script::script_system);
                }
                // Refusing to run beats photographing the wrong view under a
                // name that claims to be evidence.
                Err(detail) => panic!("composition studio script: {detail}"),
            }
        }
    }
}

impl Default for StudioPlugin {
    fn default() -> Self {
        Self
    }
}

fn setup_studio(mut commands: Commands) {
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        std::f32::consts::FRAC_PI_4,
        viewport::ISO_PITCH,
        0.0,
    );
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: viewport::DEFAULT_ZOOM,
            far: 1000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(Vec3::new(10.0, 20.0, 10.0)).with_rotation(rotation),
        // Fill for the detail pass, so a cut-open interior is shaded rather
        // than a black hole. Low enough to stay atmosphere: the schematic's
        // emissive lines are the signal and must not be competed with.
        AmbientLight {
            color: Color::WHITE,
            brightness: draw::AMBIENT_BRIGHTNESS,
            ..default()
        },
        StudioCamera,
    ));

    // The chrome gets its own full-window camera, and this is load-bearing.
    //
    // Bevy lays UI out inside its target camera's viewport. The studio camera's
    // viewport is inset by the panel width so the facility renders beside the
    // panel - so if the UI also targeted it, the entire chrome tree would be
    // laid out from the panel's far edge and shifted right by exactly its own
    // width. That presents as "the panel renders in the wrong place", which
    // reads as a text or alignment fault and sends you looking at justification.
    //
    // Splitting the cameras makes the two coordinate spaces independent: the 3D
    // camera's viewport describes where the *world* draws, and never moves the
    // chrome. `IsDefaultUiCamera` is claimed exactly once, here - a second
    // claimant on the same window silently swallows every UI node.
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            // The 3D pass has already drawn; clearing would erase the facility.
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
    ));

    chrome::setup_chrome(commands);
}

/// Re-solve once the edit has settled. Debounced so dragging a value through a
/// run of steps costs one production solve rather than one per frame.
fn update_studio_solve(time: Res<Time>, mut state: ResMut<StudioState>) {
    if !state.solve_dirty {
        return;
    }
    let now = time.elapsed_secs();
    if let Some(edited) = state.last_edit
        && now - edited < SOLVE_DEBOUNCE_SECONDS
    {
        return;
    }
    state.last_edit = None;
    solve::run_solve(&mut state);
    state.geometry_dirty = true;
}
