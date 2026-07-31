# Slice 1 — `composition_studio` shell + solve controls

Detailed, self-contained execution plan. Written 2026-07-31 immediately after
Slice 0 landed, so a fresh session can resume without re-deriving context.

- Arc plan: `C:\Users\comma\.claude\plans\lets-plan-out-a-swirling-kernighan.md`
- Slice order: 0 substrate ✅ · **1 shell + solve controls** · 2 coverage ·
  3 pinning *(the first-class line)* · 4 candidate scoring · 5 FGD + module
  validator · 6a tileforge→Rust · 6b parametric builder

---

## 0. Where things stand

**Slice 0 is complete, verified, and UNCOMMITTED.** `cargo fmt --all`,
`cargo dev-clippy`, and `cargo dev-test` (246 suites, 1538 tests, 0 failures)
all pass. Working tree:

```
M  Cargo.lock  Catalogue.md  docs/tile_authoring.md
M  crates/observed_authoring/{Cargo.toml,src/bin/tilec.rs,src/catalog.rs,src/lib.rs}
M  crates/observed_facility/{Cargo.toml,src/hex_wfc/{collapse,context,mod,score,tests}.rs}
M  crates/observed_match/src/hex_wfc/model.rs
M  game/src/hex_wfc/launch.rs   server/src/lib.rs
?? assets/tiles/composition_profile.ron   assets/tiles/composition_profile.sha256
?? crates/observed_authoring/src/composition.rs
?? crates/observed_facility/src/hex_wfc/profile.rs
```

**Commit Slice 0 before starting Slice 1.** It is a coherent, self-contained
change and the one-time content-hash move (`398e3689…` → `b21cb320…`) must not
be entangled with tool work. Suggested subject:
`feat(wfc): make composition authorable and fold it into the content hash`.

### API surface Slice 0 already gives you

```rust
// crates/observed_facility/src/hex_wfc/profile.rs   (pub mod profile)
HexCompositionProfile { version, label, tendencies, archetype_bias,
                        district_bias, score, search, pin_sets }
  ::baseline() · .validate() -> Result<(), Vec<ProfileDefect>>
  .is_baseline() · .bias_for(HexArchetype) · .district_bias_for(register, archetype)
CompositionTendencies { enabled, vertical_center_boost, vertical_edge_falloff,
                        room_low_level, room_high_level }  ::baseline() .fields()->[(&str,f64);4]
ArchetypeBias { void..expanse }  ::neutral() .get(a) .with(a,f) .fields()->[(&str,f64);9]
DistrictBias { register: String, bias: ArchetypeBias }
ScoreWeights { connectivity, elevation, room_wholeness, variety, rhythm }
                                   ::baseline() .fields()->[(&str,f64);5]
SearchPolicy { candidates: u32, retry_budget_override: Option<u32> }
PinSet / HexPin / PinIntent / PinPortClass        // defined, unused until Slice 3
ProfileDefect (impl Display)
consts: COMPOSITION_PROFILE_VERSION=1, SCORE_WEIGHT_MAX=16.0, MAX_SEARCH_CANDIDATES=8

// validation band — REUSE, do not invent new bounds
context::PROFILE_MIN = 0.25   context::PROFILE_MAX = 4.0     (pub(super))

// solver entry points
HexWfcWorld::generate_with_profile(seed, config, Option<HexRoomQuotas>, &profile)
HexWfcWorld::generate_traced_with_profile(seed, config, &profile) -> (world, Vec<SolveStep>)
score::score_layout_with(&world, ScoreWeights) -> LayoutScore

// crates/observed_authoring/src/composition.rs
CompositionBuild { profile, content_hash: String }  ::new(profile) ::baseline()
parse_profile(&str) · to_pretty_ron(&p) · to_canonical_ron(&p) · profile_content_hash(&p)
load_profile(&Path) -> CompositionBuild · write_profile_build(&build, &Path)
fold_simulation_content_hash(catalog_hex, profile_hex) -> [u8; 32]
CompositionError { Ron, Defects(Vec<ProfileDefect>), Io, SidecarDisagreement }
consts: COMPOSITION_PROFILE_FILE, COMPOSITION_PROFILE_SHA_FILE, SIMULATION_CONTENT_DOMAIN

// RuntimeHexCatalog now carries `composition` and a FOLDED simulation_content_hash
```

`observed_facility` needs `features = ["serde"]` for the profile to serialize.

---

## 1. Goal and the done test

Launch a tool, load the committed profile, edit tuning live, re-solve, watch the
layout and the score change, save a validated profile + sidecar.

**Done when** a person can: run `cargo dev-run -p composition_studio`, press a
key to open the tuning panel, drag `shaft` bias from 1.0 to 2.5, see the layout
re-solve and the score delta appear, press `Ctrl+S`, and find a changed
`composition_profile.ron` whose sidecar agrees — with the status bar having shown
the whole time that the simulation hash moved.

**Not in this slice:** coverage/demand tables (Slice 2), pin painting (Slice 3),
candidate ladder (Slice 4), any geometry authoring (Slice 5).

---

## 2. Crate placement — `tools/composition_studio`

Not `labs/`. The `agents.md` lab contract ("test one primary technical
question", disposable, reset-on-R) describes an experiment; this is a durable
artifact-producing editor, and it would be lab #69. `tools/` is already a
workspace location (`tools/content_baker`).

Add `"tools/composition_studio"` to the `members` list in `O:\Observed 2\Cargo.toml`.

Voluntarily keep two lab rules because they are good and `cargo dev-test`
enforces the spirit anyway: **reset without restarting** (Ctrl+R) and
**observable success/failure conditions** (the status line).

### `tools/composition_studio/Cargo.toml`

Modelled directly on `labs/iso_observer_lab/Cargo.toml` (read it — it is the
closest sibling and its comments explain each dep):

```toml
[package]
name = "composition_studio"
version = "0.1.0"
edition = "2024"
publish = false

[[bin]]
name = "composition-studio"
path = "src/main.rs"

[dependencies]
bevy = { version = "0.18.1", default-features = false, features = [
    "2d", "3d", "png", "experimental_bevy_ui_widgets",
] }
# The profile type and the solver it drives. `serde` carries the profile;
# hex_wfc is unconditional, so no `wfc` feature is needed.
observed_facility = { path = "../../crates/observed_facility", features = ["serde"] }
# Profile RON/sidecar I/O and the content-hash fold.
observed_authoring = { path = "../../crates/observed_authoring" }
observed_content = { path = "../../crates/observed_content" }
observed_hex = { path = "../../crates/observed_hex" }
# The projector: the studio must show what a match would actually build.
observed_match = { path = "../../crates/observed_match", default-features = false }
# Colours come from the production style crate, never invented locally.
observed_style = { path = "../../crates/observed_style" }
# `ConvexRenderMesh` turns a hull point cloud into edges.
observed_traversal = { path = "../../crates/observed_traversal", default-features = false }
serde = { version = "1", features = ["derive"] }
serde_json = "1.0"
```

Check `bevy` feature-name spelling against `game/Cargo.toml`, which already
enables `experimental_bevy_ui_widgets`.

---

## 3. UI decision — Bevy UI, settled

Do not reopen this without writing an R11-style evaluation document first.

1. **The Legibility Contract is a hard rule** (`agents.md`): the visual language
   lives in one shared tested module and presentation never invents colours.
   egui ships its own `Visuals`/`Style`, which would immediately become a second
   source of visual truth for panel chrome and selection highlight.
2. **Written, recent precedent.** `docs/refactor_r11_evaluation.md` records a
   formal evaluation — one dependency accepted (feature-gated), five deferred.
   `labs/lab_observability_lab/Cargo.toml` explicitly rejects a dep that
   force-pulls `bevy_egui`, saying the lab "renders its own neon-noir overlay
   rather than adopting an egui UI stack."
3. The heavy interaction is a 3D viewport with picking — Bevy-native.

**Widget layer: build lab-local, modelled on `labs/hex_tile_lab/src/lab_menu.rs`.**
Do NOT extract `game/src/screens/widgets/` — it is `pub(crate)`, its
`FocusScope`/`restore_focus` model is menu-navigation-shaped, and extracting a
shared crate for one consumer is the "speculative abstraction" `agents.md`
forbids. Extract later, from whichever proved out.

`lab_menu.rs` already solves this tool's hardest chrome problem, quoted from its
own module doc: *"While the menu is open it owns the keyboard: lab hotkeys and
character movement are gated off so a key never means two things at once."*
A tool with tuning keys, camera keys and a save key needs that from hour one.

Its shape to copy:

```rust
#[derive(Resource)]
pub struct LabMenuState { pub is_open: bool, pub active_tab: usize,
                          pub selected_item: usize, pub active_filter: FilterCategory }
pub enum MenuTab { Browse, Registers, Render, Actions }   // ::ALL, .label()
```

---

## 4. Files to create

```
tools/composition_studio/Cargo.toml
tools/composition_studio/README.md          # incl. the "trim.rs is deliberately dead" note
tools/composition_studio/src/main.rs        # thin: App::new().add_plugins(StudioPlugin).run()
tools/composition_studio/src/lib.rs         # StudioPlugin, StudioState, SolveResult  (~250 lines)
tools/composition_studio/src/chrome.rs      # StudioMenu, tabs, keyboard ownership
tools/composition_studio/src/viewport.rs    # iso camera, frame_camera, zoom/pan
tools/composition_studio/src/pick.rs        # screen-space picking
tools/composition_studio/src/draw.rs        # schematic mesh emission from HexWfcWorld
tools/composition_studio/src/solve.rs       # dirty-flag re-solve + timing
tools/composition_studio/src/tunables.rs    # TUNABLE_FIELDS table
tools/composition_studio/src/panels/mod.rs
tools/composition_studio/src/panels/tuning.rs
tools/composition_studio/src/panels/score.rs
tools/composition_studio/src/persist.rs     # load/save + working-path vs corpus promotion
tools/composition_studio/src/capture.rs     # OBSERVED2_CAPTURE staged screenshots
tools/composition_studio/src/script.rs      # StudioScript headless driver
```

Keep every file under 600 lines. The `arch_check` ratchet
(`game/src/arch_check.rs:228`) does not cover `tools/`, but its 600-line rule is
the house norm and `collapse.rs` at 989 lines is the cautionary example.

---

## 5. Core types

```rust
#[derive(Resource)]
pub struct StudioState {
    /// The profile being edited, in memory.
    pub profile: HexCompositionProfile,
    /// Immutable reference point for the A/B delta readout.
    pub baseline: HexCompositionProfile,
    /// What is currently on disk, for the dirty marker.
    pub saved: HexCompositionProfile,
    pub saved_hash: String,
    pub catalog_hash: String,

    pub config: HexWfcConfig,
    pub quotas: Option<HexRoomQuotas>,
    pub seed_index: usize,

    pub solved: Option<SolveResult>,
    /// Score of `baseline` at the same seed, for the delta column.
    pub baseline_score: Option<LayoutScore>,
    pub dirty: bool,
    pub reset_count: u32,

    // Carried across a reload — see §8.
    pub zoom: f32,
    pub pan: Vec2,
    pub layer: Layer,
    pub status: String,
}

pub struct SolveResult {
    pub world: HexWfcWorld,
    pub steps: Vec<SolveStep>,
    pub score: LayoutScore,
    pub attempts: u32,
    pub elapsed_ms: u32,
    /// `None` when the catalog could not be projected; the viewport then falls
    /// back to cell shells and the status line says so.
    pub geometry: Option<HexWfcGeometrySnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudioTab { Solve, Tuning, Districts, Diagnostics }
// Coverage and Pins tabs arrive in Slices 2 and 3.
```

### The tunable table (`tunables.rs`) — the anti-drift device

```rust
pub struct TunableField {
    pub label: &'static str,
    pub tab: StudioTab,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub get: fn(&HexCompositionProfile) -> f64,
    pub set: fn(&mut HexCompositionProfile, f64),
}
pub const TUNABLE_FIELDS: &[TunableField] = &[ /* 4 tendencies + 9 archetype + 5 score */ ];
```

18 entries. Bounds: tendencies and archetype bias use
`context::PROFILE_MIN..PROFILE_MAX` (0.25..4.0); score weights use
`0.0..SCORE_WEIGHT_MAX`. **`PROFILE_MIN`/`PROFILE_MAX` are `pub(super)` today —
promote them to `pub` in `context.rs` and re-export from `hex_wfc::mod` so the
studio can read them instead of hardcoding 0.25/4.0.**

Per-district biases (10 registers × 9 archetypes) are *not* in this flat table —
they are a separate generated grid on the Districts tab, built by iterating
`ArchitectureRegister::ALL` and `ArchetypeBias::fields()`.

---

## 6. What to reuse, verbatim, and from where

| Need | Source | Notes |
| --- | --- | --- |
| Screen-space picking | `labs/iso_observer_lab/src/inspect.rs:24` `pick()` | `camera.world_to_viewport`, `PICK_RADIUS = 26.0`, ties by `distance + depth * 0.001`. No raycasts, no `bevy_mod_picking`. |
| Iso camera framing | `labs/iso_observer_lab/src/lib.rs:440` `frame_camera(min,max) -> (Transform, units_per_px, far)` | `ISO_PITCH = atan(1/sqrt(2))`, yaw `FRAC_PI_4`. |
| Zoom / pan | same file, `sync_camera` ~line 678 | Applied to the camera every frame so dragging never re-meshes. `DEFAULT_ZOOM 0.34`, `MIN_ZOOM 0.04`, `MAX_ZOOM 2.0`. **Left-drag stays reserved for selection; pan is right/middle** — this matters enormously for Slice 3 painting. |
| Dirty-flag rebuild | `iso_observer_lab` `LabVisual` marker + despawn-all-and-re-emit | Batches to ≤2 meshes per role; a test asserts `drawn <= 2*3` regardless of facility size. |
| Reset carrying view state | `iso_observer_lab` `LabState::reload` (~line 229) | Carries `reset_count, layer, mode, detail, zoom, pan` across a re-solve. |
| Layer cycling | `iso_observer_lab` `enum Layer { Single(u8), All }` | Cycle is `0,1,…N-1,All`, wrapping. |
| Colours | `observed_style::{hex_sketch, HexSketchRole, SchematicRole, schematic, schematic_screen}` | `schematic_screen()` = `ClearColor` and 0.86-alpha panel background. **Never `game/src/view/theme.rs`** — menu chrome, inside `game`. |
| Edge meshes | `observed_traversal::ConvexRenderMesh` | Hull cloud → edges without re-triangulating. |
| Modal menu | `labs/hex_tile_lab/src/lab_menu.rs` | Keyboard-ownership rule. |
| Capture | `labs/iso_observer_lab/src/capture.rs` | `Stage` walker; phased 0.8 s settle → `Screenshot::primary_window()` + `save_to_disk` → 1.6 s `AppExit`. **The split is load-bearing — `save_to_disk` writes on a later frame.** |
| Headless script | `labs/hex_tile_lab/src/script_runner.rs:22` `ViewScript` | `--script <path>` or `OBSERVED2_SCRIPT`; phases t≥0.1 configure, t≥0.8 shoot, t≥1.6 exit. |
| Preset seeds | `iso_observer_lab::PRESET_SEEDS` (5 seeds, an evidence contract) | Reuse the same five so studio captures are like-for-like with Arc O evidence. Import rather than copy if the dep is acceptable; otherwise copy with a comment naming the source. |

---

## 7. Behaviour spec

### Solve loop (`solve.rs`)

- Any tunable edit sets `dirty`. Re-solve on a debounce (~250 ms after the last
  edit) so dragging a slider does not queue 60 production solves.
- Use `HexWfcConfig::default()` (12×9×1, the compact lab fixture) as the **default**
  working scale; offer `arc_default()` (28×20×10, 5 600 cells) behind a toggle.
  A production solve is far too slow to drag a slider against — measure and show
  `elapsed_ms` so this is visible rather than mysterious.
- Solve via `generate_traced_with_profile` so the step log is available for the
  Diagnostics tab and Slice 4's ladder.
- Score with `score_layout_with(&world, profile.score)`; compute
  `baseline_score` once per seed with `ScoreWeights::baseline()` on a baseline
  solve, for the delta column.
- On `HexWfcError`, keep the previous `solved` on screen and put the error in the
  status line. A failed solve must never blank the viewport — that reads as a
  crash.

### Save discipline (`persist.rs`) — implements arc-plan Pushback #3

Every pin or tuning edit changes the folded hash, which locks out any peer that
has not taken the same edit. So:

- **`Ctrl+S` writes to a working path, not `assets/tiles/`.** Default
  `scratch/composition/composition_profile.ron`.
- **Promotion to the corpus is a separate, explicit action** (`Ctrl+Shift+P` or
  an ACTIONS-tab entry) with a confirmation naming the before/after simulation
  hash.
- The status bar **permanently** shows the folded simulation hash (first 12 hex
  chars is enough) plus a dirty marker when in-memory ≠ on-disk.
- Save path is `validate()` → `to_pretty_ron()` → `write_profile_build()`. On
  `Err(Vec<ProfileDefect>)`, list every defect in the panel — they already
  `impl Display`.

### Tuning panel copy — implements arc-plan Pushback #2

Someone will want "no shafts" and will drag a slider to zero. The invariant
forbids it, correctly: a zero weight silently degrades solvability with no check,
whereas a domain filter is verified.

- Slider floor is `PROFILE_MIN` (0.25), never 0.
- The control is labelled **"bias"**, not "weight" or "amount".
- When a slider is held at its floor, the panel shows:
  *"Bias only — never removes an archetype. To forbid one, pin it (Slice 3)."*

Design this into the copy, not just the validator.

### Keys (provisional; put the legend on screen — Legibility Contract)

```
F2            open/close the tool menu (menu owns the keyboard while open)
Tab           cycle layer   PageUp/PageDown  layer step
[ / ]         previous / next preset seed
R             re-solve current seed        Ctrl+R  reload profile from disk + re-solve
G             real authored hulls on/off
Home / 0      reset view (zoom + pan)
Ctrl+S        save to working path         Ctrl+Shift+P  promote to corpus
Ctrl+Z        revert in-memory profile to `saved`
B             toggle A/B: show baseline solve vs authored solve
right/middle drag  pan          scroll  zoom          left drag  reserved (Slice 3)
```

---

## 8. Tests

Put unit tests in-module; put the Bevy lifecycle ones in a `tests` module that
builds a headless `App` (see how `iso_observer_lab` does it — `MinimalPlugins`
plus the studio plugin, stepping `app.update()`).

| Test | Pins |
| --- | --- |
| `baseline_studio_solve_matches_generate` | The studio's solve path equals `HexWfcWorld::generate` across `PRESET_SEEDS`. |
| `reset_rebuilds_the_projection_without_leaking_entities` | The house reset contract. Copy the shape from `iso_observer_lab`. |
| `reloading_a_seed_keeps_the_view_you_were_using` | `zoom`, `pan`, `layer` survive a re-solve. |
| `every_tunable_field_round_trips` | For each `TUNABLE_FIELDS` entry: `set(x)` then `get()` returns `x`; and `set(min)`/`set(max)` both `validate()` clean. |
| `every_profile_scalar_has_a_tunable_entry` | `TUNABLE_FIELDS.len() == 4 + 9 + 5`, asserted against `CompositionTendencies::fields().len() + ArchetypeBias::neutral().fields().len() + ScoreWeights::baseline().fields().len()` so a new profile field cannot ship without UI. |
| `no_tunable_can_produce_an_invalid_profile` | Sweep every field to both bounds; `validate()` stays `Ok`. |
| `menu_open_gates_viewport_hotkeys` | The `lab_menu.rs` keyboard-ownership rule. |
| `a_failed_solve_keeps_the_previous_layout_on_screen` | Force an unsolvable config; assert `solved` is unchanged and `status` names the error. |
| `saving_writes_a_loadable_profile_and_sidecar` | `write_profile_build` → `load_profile` round trip through a temp dir. |
| `rendered_studio_strings_stay_ascii` | Mirrors `game/src/arch_check.rs:200-222`. **The tool ships no font asset either** — Bevy's default ASCII subset is all you get; a non-ASCII glyph renders as a blank box. This has already bitten this repo once (a diamond glyph vanished from the Play hub). |
| `no_locally_invented_colours` | Source-scan `tools/composition_studio/src` for `Color::srgb`, `Color::rgb`, `Srgba`, `LinearRgba` literals; assert zero. The Legibility Contract as a ratchet. |

---

## 9. Verification

```bash
cargo fmt --all && cargo dev-clippy && cargo dev-test
```

Warnings resolved, not suppressed.

```bash
cargo dev-run -p composition_studio
```

Then, per the falsifiable-evidence rule in `docs/tile_authoring.md`, capture and
**look at** the result:

```bash
OBSERVED2_CAPTURE=scratch/studio cargo dev-run -p composition_studio
```

Manual pass to actually perform:

1. Open the tuning panel, raise `shaft` bias to 2.5 — the layout visibly gains verticals.
2. Confirm the score delta column moves.
3. `Ctrl+S`, then confirm the working-path file loads back via
   `cargo run -p observed_authoring --bin tilec -- profile-validate scratch/composition`.
4. `Ctrl+R` — the profile reloads from disk and the view keeps its zoom/pan.
5. Drag a bias to the slider floor and confirm the "bias only, never removes"
   copy appears.

---

## 10. Traps, gathered

1. **PowerShell pipeline exit codes.** `cargo dev-test 2>&1 | Select-Object -First N`
   reports exit 255 because `-First` kills the upstream pipeline — *not* a test
   failure. Pipe to a file, then `Select-String` it, and read `$LASTEXITCODE`
   immediately after the cargo call. This cost time in the Slice 0 session.
2. **PS 5.1 mangles BOM-less UTF-8.** Never bulk-edit `.rs`/`.ron`/`.map` with
   `Get-Content`/`Set-Content`. Use the Edit tool.
3. **Bevy UI camera trap.** Bevy assigns UI to the highest-order camera on the
   primary window and *ignores* `is_active`. If the studio ever gains a second
   camera, exactly one must claim `IsDefaultUiCamera` and other UI must name its
   camera via `UiTargetCamera` — or panels silently vanish. Headless tests cannot
   catch this; they assert entities exist, not where they render.
4. **ASCII-only rendered strings.** See the test table above.
5. **Capture timing.** `save_to_disk` writes on a *later* frame than the
   `Screenshot` spawn — exiting early loses the PNG. Keep the 0.8 s / 1.6 s split.
   For a ~13 k-entity production view, `iso_observer_lab` uses a 2.0 s settle.
6. **Window opens maximized**, so a logical resize is silently ignored — relevant
   if a capture needs an exact viewport size.
7. **Test fixtures inheriting the dev's real settings** has bitten this repo
   before. The studio must read the profile from an explicit path, never from
   user config.
8. **`observed_facility` needs `features = ["serde"]`** or the profile will not
   serialize and the error is a confusing missing-trait message.
9. **Production-scale solves are slow.** Do not wire a slider directly to an
   `arc_default()` solve; debounce, and default to the compact config.

---

## 11. Out of scope, deliberately

- Coverage/demand tables, seam findings, distribution — **Slice 2**.
- Pin painting and `PinIntent` — **Slice 3**. `PinSet` already exists in the
  profile and serializes; leave `pin_sets` untouched and empty here.
- `search.candidates > 1` — **Slice 4**. Show the field read-only for now, or
  clamp the editor to 1 and label it "Slice 4".
- Any `.map`/geometry authoring — **Slice 5**, and it is a *separate binary* in
  this same crate (arc-plan Pushback #1).
- `trim.rs` activation — deliberately unscheduled. **Say so in the studio
  README** so the next reader does not think it was forgotten.
