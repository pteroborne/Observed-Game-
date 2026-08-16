//! Everything the studio holds while it is running, and where it came from.
//!
//! Split out of `lib.rs` when that file crossed the 600-line review budget the
//! rest of the WFC path lives under. The division is by kind rather than by
//! size: `lib.rs` is now the plugin — what systems run, in what order, on what
//! camera — and this is the state those systems read and write, plus the
//! loading that establishes it.

use std::collections::VecDeque;

use bevy::prelude::*;
use observed_facility::hex_wfc::profile::HexCompositionProfile;
use observed_facility::hex_wfc::score::LayoutScore;
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld, SolveStep};
use observed_hex::HexCoord;
use observed_match::hex_wfc::HexWfcGeometrySnapshot;

use crate::load::{self, corpus};
use crate::{
    DrawReport, KeyboardOwner, Layer, PANEL_WIDTH, PRESET_SEEDS, brush, detail, neighbors, panels,
    timeline, viewport,
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
    /// The seed being solved. A real value rather than an index, so the studio
    /// is not confined to the five pinned evidence seeds — `PRESET_SEEDS` stays
    /// exactly what it was and becomes a set of bookmarks into a whole space.
    pub seed: u64,
    /// Which preset this seed came from, when it came from one.
    pub seed_preset: Option<usize>,

    /// Profiles as they were before each edit, most recent last.
    ///
    /// Bounded, because a session can edit for hours and an unbounded history
    /// of a struct this size is a leak with a nice name.
    pub undo_stack: VecDeque<HexCompositionProfile>,
    pub redo_stack: Vec<HexCompositionProfile>,
    /// The profile as of the last history entry, so an edit can be detected
    /// without every call site having to announce one. Bookkeeping for
    /// [`StudioState::undo`]; nothing outside it should write this.
    pub last_committed: HexCompositionProfile,
    /// When the last history entry was pushed, for coalescing a burst.
    pub last_undo_push: Option<f32>,

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
    /// How much of each cell's authored walls the deck draws.
    pub walls: detail::WallMode,
    /// Whether the solver's schematic is laid over the authored deck.
    ///
    /// Off by default — it is a diagnostic, not the view. `draw.rs` forces it on
    /// when there is no authored geometry to draw and while a replay runs, and
    /// says which in the status line.
    pub show_schematic: bool,
    /// Why the schematic is on when it was not asked for, set by the drawer.
    /// `None` when the overlay reflects what you configured.
    pub schematic_forced: Option<String>,
    /// View azimuth, in 60-degree detents anchored at the historical default.
    pub detent: usize,
    pub detail_report: detail::DetailReport,
    /// Whether the docked panel is expanded. Collapsing gives the facility the
    /// whole window; it never changes what the keyboard can reach.
    pub panel_open: bool,
    /// How the panel and the facility share the window. Driven by window width
    /// in `sync_layout_mode`; never set by hand.
    pub layout: crate::LayoutMode,
    /// Which region receives keys. See [`KeyboardOwner`].
    pub keyboard_owner: KeyboardOwner,
    /// What left-drag paints.
    pub brush: brush::Brush,
    /// Diagnostics for the current pin set, refreshed on every pin edit.
    pub pin_diagnostics: Vec<observed_facility::hex_wfc::PinDiagnostic>,
    /// What could stand around the selected cell, and which of those options
    /// is currently being previewed. See [`neighbors`].
    pub neighbors: neighbors::NeighborView,
    /// Where the solve replay is. Settled at the end of the trace unless a
    /// person has asked to watch the solve happen.
    pub timeline: timeline::Timeline,
    pub status: String,
    pub report: DrawReport,
    /// The whole-catalog seam audit. On demand, because it recompiles every
    /// authored `.map` and would make the tuning loop unusable if it ran per
    /// solve.
    pub seam_audit: panels::coverage::SeamAudit,
    pub base_frame: (Transform, f32, f32),
}

/// A seed nobody chose.
///
/// The clock's entropy is all in its low bits, so it is spread across all
/// sixty-four with the standard SplitMix64 finaliser — otherwise every rolled
/// seed would share a high half and look like a family. This is a convenience
/// for an author browsing facilities; it is not a simulation RNG and nothing
/// deterministic depends on it.
///
/// The clock is read through `web_time`, not `std::time`: the browser has no
/// `SystemTime`, and `std`'s panics there rather than failing a call the
/// `map_or` below would have absorbed.
fn rolled_seed() -> u64 {
    let nanos = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64);
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Default for StudioState {
    fn default() -> Self {
        let (profile, saved_hash, origin) = load::startup_profile();
        let catalog_hash = load::catalog_hash();

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

        let profile_for_history = profile.clone();
        Self {
            profile: profile.clone(),
            baseline: HexCompositionProfile::baseline(),
            saved: profile,
            saved_hash,
            origin,
            catalog_hash,
            config: HexWfcConfig::default(),
            seed: PRESET_SEEDS[0],
            seed_preset: Some(0),
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            last_committed: profile_for_history,
            last_undo_push: None,
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
            // The authored deck, not the schematic. The tool's subject is the
            // facility a profile builds, and it should open on it.
            detail_mode: detail::DetailMode::Layer,
            walls: detail::WallMode::default(),
            show_schematic: false,
            schematic_forced: None,
            detent: 0,
            detail_report: detail::DetailReport::default(),
            panel_open: true,
            layout: crate::LayoutMode::default(),
            keyboard_owner: KeyboardOwner::default(),
            brush: brush::Brush::default(),
            pin_diagnostics: Vec::new(),
            neighbors: neighbors::NeighborView::default(),
            timeline: timeline::Timeline::default(),
            status,
            report: DrawReport::default(),
            seam_audit: panels::coverage::SeamAudit::default(),
            base_frame: (Transform::IDENTITY, 1.0, 1000.0),
        }
    }
}

/// How many edits back the history reaches.
pub const UNDO_DEPTH: usize = 64;

impl StudioState {
    /// The seed currently being solved.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// How the current seed should be named on screen.
    #[must_use]
    pub fn seed_label(&self) -> String {
        match self.seed_preset {
            Some(index) => format!("{:#018x} (preset {index})", self.seed),
            None => format!("{:#018x} (free)", self.seed),
        }
    }

    /// Move to a pinned preset seed, wrapping.
    pub fn step_preset_seed(&mut self, delta: isize, now: f32) {
        let count = PRESET_SEEDS.len();
        #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
        let next = match self.seed_preset {
            Some(index) => (index as isize + delta).rem_euclid(count as isize) as usize,
            // Coming back from a free seed lands on the first preset rather
            // than guessing which one a rolled seed was "near".
            None => 0,
        };
        self.set_seed(PRESET_SEEDS[next], Some(next), now);
    }

    /// Solve a seed that is not in the pinned set.
    ///
    /// The five presets are an evidence contract shared with `iso_observer_lab`
    /// and are not touched by this; they simply stop being the only reachable
    /// facilities. A seed is not part of the composition profile, so rolling one
    /// does not move the simulation hash and cannot lock out a LAN peer.
    pub fn roll_seed(&mut self, now: f32) {
        self.set_seed(rolled_seed(), None, now);
    }

    fn set_seed(&mut self, seed: u64, preset: Option<usize>, now: f32) {
        self.seed = seed;
        self.seed_preset = preset;
        // A different seed is a different facility, so the cached baseline solve
        // no longer describes anything the compare overlay should trust.
        self.invalidate_baseline();
        self.status = format!("seed is now {}", self.seed_label());
        self.touch_profile(now);
    }

    /// Mark the profile edited: re-solve after the debounce, and redraw.
    ///
    /// This is also where edit history is taken. Every profile edit in the tool
    /// already routes through here, so the alternative — asking each call site
    /// to announce itself — would only mean a future edit path that silently
    /// cannot be undone.
    pub fn touch_profile(&mut self, now: f32) {
        self.record_edit(now);
        self.solve_dirty = true;
        self.last_edit = Some(now);
    }

    fn record_edit(&mut self, now: f32) {
        if self.profile == self.last_committed {
            return;
        }
        // A drag across thirty cells, or a slider swept through ten steps, is
        // one edit to the person doing it. Coalescing on the same window the
        // solver debounces on keeps "one gesture, one undo" true without
        // inventing a second notion of what an edit is.
        let coalesce = self
            .last_undo_push
            .is_some_and(|pushed| now - pushed < crate::SOLVE_DEBOUNCE_SECONDS);
        if !coalesce {
            self.undo_stack.push_back(self.last_committed.clone());
            if self.undo_stack.len() > UNDO_DEPTH {
                self.undo_stack.pop_front();
            }
            self.redo_stack.clear();
            self.last_undo_push = Some(now);
        }
        self.last_committed = self.profile.clone();
    }

    /// Step back one edit. Returns whether there was one.
    pub fn undo(&mut self, now: f32) -> bool {
        let Some(previous) = self.undo_stack.pop_back() else {
            return false;
        };
        self.redo_stack.push(self.profile.clone());
        self.apply_history(previous, now);
        self.status = format!("undo ({} left)", self.undo_stack.len());
        true
    }

    /// Step forward again. Returns whether there was something to redo.
    pub fn redo(&mut self, now: f32) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push_back(self.profile.clone());
        self.apply_history(next, now);
        self.status = format!("redo ({} left)", self.redo_stack.len());
        true
    }

    fn apply_history(&mut self, profile: HexCompositionProfile, now: f32) {
        self.profile = profile;
        // The history move is not itself an edit to record, and the next real
        // edit must start a fresh entry rather than coalescing into this one.
        self.last_committed = self.profile.clone();
        self.last_undo_push = None;
        self.refresh_pin_diagnostics();
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
    ///
    /// Zero in a compact layout, where the panel is below the facility rather
    /// than beside it. See [`Self::viewport_bottom_inset`].
    #[must_use]
    pub fn viewport_origin(&self) -> f32 {
        if self.panel_open && !self.layout.is_compact() {
            PANEL_WIDTH
        } else {
            0.0
        }
    }

    /// How much of the window's foot the panel occupies, in logical pixels.
    ///
    /// The compact counterpart to [`Self::viewport_origin`]: one of the two is
    /// always zero, because the panel is docked to exactly one edge.
    #[must_use]
    pub fn viewport_bottom_inset(&self, window_height: f32) -> f32 {
        if self.panel_open && self.layout.is_compact() {
            (window_height * crate::COMPACT_PANEL_FRACTION).floor()
        } else {
            0.0
        }
    }

    /// Whether a window-space cursor position is over the facility.
    #[must_use]
    pub fn cursor_in_viewport(&self, cursor: Vec2) -> bool {
        cursor.x >= self.viewport_origin()
    }

    /// [`Self::cursor_in_viewport`] for a window whose height is known.
    ///
    /// A compact layout puts the panel under the facility, so "is this over the
    /// facility" stops being a question about x alone. Callers with a window to
    /// hand should prefer this; the x-only form stays correct for wide layouts
    /// and for the tests that pin them.
    #[must_use]
    pub fn cursor_in_viewport_within(&self, cursor: Vec2, window_height: f32) -> bool {
        cursor.x >= self.viewport_origin()
            && cursor.y < window_height - self.viewport_bottom_inset(window_height)
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
        let (profile, saved_hash, origin) = load::startup_profile();
        self.status = match &origin {
            ProfileOrigin::Working => String::from("reloaded working profile"),
            ProfileOrigin::Corpus => String::from("reloaded corpus profile"),
            ProfileOrigin::Unreadable(detail) => format!("ERROR: {detail}"),
        };
        self.profile = profile.clone();
        self.saved = profile;
        self.saved_hash = saved_hash;
        self.origin = origin;
        self.catalog_hash = load::catalog_hash();
        self.selected = None;
        self.reset_count += 1;
        self.invalidate_baseline();
        self.refresh_pin_diagnostics();
        self.touch_profile(now);
        self.touch_view();
        // zoom, pan and layer are deliberately NOT touched.
    }
}
