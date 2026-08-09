//! Everything the studio holds while it is running, and where it came from.
//!
//! Split out of `lib.rs` when that file crossed the 600-line review budget the
//! rest of the WFC path lives under. The division is by kind rather than by
//! size: `lib.rs` is now the plugin — what systems run, in what order, on what
//! camera — and this is the state those systems read and write, plus the
//! loading that establishes it.

use std::sync::OnceLock;

use bevy::prelude::*;
use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::profile::HexCompositionProfile;
use observed_facility::hex_wfc::score::LayoutScore;
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld, SolveStep};
use observed_hex::HexCoord;
use observed_match::hex_wfc::HexWfcGeometrySnapshot;

use crate::{
    DrawReport, KeyboardOwner, Layer, PANEL_WIDTH, PRESET_SEEDS, brush, detail, neighbors, panels,
    persist, viewport,
};

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
    /// What could stand around the selected cell, and which of those options
    /// is currently being previewed. See [`neighbors`].
    pub neighbors: neighbors::NeighborView,
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
            neighbors: neighbors::NeighborView::default(),
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

    /// Change how many floors the working facility has.
    ///
    /// The studio ran at one level for its whole life, which was fine while it
    /// only answered questions about lateral composition. The neighbourhood
    /// explorer made it a real gap: two of a cell's eight faces are up and
    /// down, and on a one-level lattice they are not merely empty but
    /// *unreachable*, so the tool could not be asked about ramps or shafts at
    /// all — the part of this grammar with the most rules in it.
    ///
    /// Clamped to `1..=MAX_WORKING_LEVELS`. Ten is the production figure, and a
    /// solve at that scale is seconds rather than milliseconds, which is why
    /// this is a deliberate keypress rather than a default.
    pub fn set_levels(&mut self, levels: u8, now: f32) {
        let levels = levels.clamp(1, MAX_WORKING_LEVELS);
        if levels == self.config.levels {
            return;
        }
        self.config.levels = levels;
        // The old selection may not exist at this scale, and the old layer may
        // name a floor that is gone. Both are cleared rather than clamped: a
        // selection silently landing on a different cell is worse than none.
        self.selected = None;
        self.layer = Layer::All;
        self.neighbors = neighbors::NeighborView::default();
        self.invalidate_baseline();
        self.status = format!("working scale is now {levels} level(s); re-solving");
        self.touch_profile(now);
        self.touch_view();
    }
}

/// The most floors the studio will solve at once.
///
/// `HexWfcConfig::arc_default` ships ten, so this is production scale and not
/// an invented ceiling.
pub const MAX_WORKING_LEVELS: u8 = 10;

impl StudioState {
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
