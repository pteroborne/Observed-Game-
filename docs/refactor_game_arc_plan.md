# Refactor Arc G — Game-Layer Architecture Cleanup Plan

Successor to the R0–R11 arc ([refactor_r0_audit.md](refactor_r0_audit.md)), which fixed
the *inter-crate* dependency graph (game → crates only, no lab edges). This arc fixes
the *intra-game* architecture: `game/` (~15,800 lines) has grown a god-module hub,
dead code, duplicated utilities, four parallel match models, and pattern bloat from
many contributors. The `crates/` layer is healthy and is mostly untouched here.

Audited: 2026-07-02. Every finding below was verified against the working tree, not
inherited from Catalogue.md (which has partially regressed — see G6).

---

## Assessment

**What's good (protect it):**
- `crates/*` are pure, deterministic, `default-features = false`, well-tested, and the
  dependency direction from R0–R11 still holds. No production crate touches a lab.
- The game's simulation/presentation split *at the crate boundary* is real: the brain
  (`LiveNetMatch`/`HybridMatch`) is never written by presentation systems.
- Doc discipline (module headers explaining *why*) is genuinely strong.

**What's wrong (this arc):**
- Inside `game/`, everything flows through one flat namespace (`screens::*`), so the
  clean crate boundary dissolves into an any-to-any web at the module level.
- The match's *outcome* has two competing sources of truth ticking in parallel.
- Dead code, a dead dependency, and re-duplicated utilities the Catalogue claims were
  centralized.

---

## Findings (evidence)

### F1 — Dead module + dead dependency: `wfc_maze.rs`
`game/src/wfc_maze.rs` (367 lines): its only public entry `generate_wfc_interior_walls`
has **zero callers** anywhere in the workspace. It is the sole reason the game depends
on `ghx_proc_gen = "0.8.0"`. The teleport-hallway interior maze that actually ships is
`game/src/maze.rs` (randomized-DFS + braid).

### F2 — `screens.rs` god-module (the central coupling problem)
`game/src/screens.rs` (710 lines) declares 8 submodules and glob re-exports them all
(`pub(crate) use match_runtime::*;` …), and every submodule opens with `use super::*`.
Consequences:
- 147 `screens::` references across 19 files; nothing states what it actually depends on.
- `screens.rs` itself is a grab-bag: theme constants, asset-path constants, material
  helpers, UI bundle helpers, **and ~20 shared resources/components** — including
  simulation-side state (`MatchRuntime`, `TeleportState`, `SpectatorBot`, `SeriesRuntime`)
  living in a module named "screens".
- `MatchAssets`'s private fields are reachable by all descendants by design (the module
  header says so) — privacy via module nesting instead of an API.
- Naming collision: `game/src/teleport/` (place geometry/crossing model) vs
  `game/src/screens/match_runtime/teleport.rs` (the Bevy sim driver). Two different
  "teleport" modules.

### F3 — Two sources of truth for the match outcome
The Match screen runs **four** match models simultaneously:
1. `MatchRuntime.live: LiveNetMatch` → `HybridMatch` → `CompetitiveFacility` — drives
   the player's rooms, thresholds, rivals, tac-map.
2. `SeriesRuntime.series: EliminationSeries` — autoplayed on a **wall-clock timer** in
   `match_pump` (`match_runtime/mod.rs:945-953`), read by the HUD.
3. `SpectatorBot.teamplay: TeamplayMatch` — spectator mode only; pumps the series
   instead of the timer.
4. `flow::play_match()` — headless careers run a *fresh* `EliminationSeries`, while
   `flow::play_networked_match()` runs a `NetMatch`; they can disagree with what the
   screen showed.

`match_pump` (`match_runtime/mod.rs:968-978`) ends the match when **either** the live
match or the series finishes, and resolves the result from whichever finished — the
HUD's series numbers and the world the player walked are only loosely correlated. This
is the deepest architectural incoherence in the game layer.

### F4 — SplitMix PRNG re-duplicated (game layer only) — RESOLVED in G1
Original finding overcounted: a workspace grep for the splitmix constants matched 17
files, but on file-by-file verification (G1, 2026-07-02) the production crates were
already centralized on `observed_core::prng` at HEAD. The real duplicates were in the
game: `game/src/maze.rs` and `guardian.rs` (converted to the shared `SplitMix` in G1,
streams bit-identical, all seeded tests green). Three intentional exceptions are NOT
duplicates and stay as-is: `game/src/hallway.rs` and `game/src/teleport/geom.rs` use
keyed one-shot *hash finalizers* (different state-advance, edge-symmetric semantics),
and `observed_style::district_for` is a stateless finalizer hash — rewriting any of
them over `SplitMix::next_u64()` would shift their outputs. Lab-local copies are out
of scope (labs are frozen archives).

### F5 — Constant-only dependency on an abandoned system
After the teleport pivot, the game no longer uses `observed_match::maze` (865 lines,
the walkable spatial labyrinth) — except to import **constants**: `TILE_SIZE`,
`CORRIDOR_RADIUS` (screens, place, audio, preview), and `GRID_W/GRID_H`
(`capture/tour.rs`). The game's hall width is defined via a dead system's tile grid.

### F6 — Pattern bloat in the place renderer
`screens/place/strategies.rs`: a `Box<dyn SpawningStrategy>` with exactly two impls,
selected by a `match tp.place` that already knows the variant (`place/mod.rs:99`) —
dynamic dispatch buying nothing. Same file: `GatewayPolicy` trait + three unit structs
(`PassagePolicy`/`LockedExitPolicy`/`SideDoorPolicy`) whose entire behavior is three
values — an enum or a 3-field struct. The `// STRATEGY PATTERN` / `// TEMPLATE METHOD
PATTERN` banners are decoration, not architecture.

### F7 — Oversized files remaining
- `game/src/diagnostics.rs` — 1,517 lines (scenario driver + snapshot builders +
  finding checks + screenshot plumbing in one file), and it reaches into `screens`
  internals (`diagnostics.rs:31-36`).
- `game/src/screens/place/mod.rs` — 1,288 lines even after factory/mesh/lighting/
  preview/items/strategies were split out.
- `game/src/lib.rs` — 1,101 lines, of which ~990 are one inline `#[cfg(test)]` module.
- `game/src/screens/match_runtime/mod.rs` — 1,007 lines (setup + teardown + pump +
  spectator-bot steering + nav derivation).
- Crates (lower priority): `observed_facility/room_world.rs` (981),
  `observed_interaction/equipment.rs` (941), `observed_style/lib.rs` (851).

### F8 — Duplicated evidence pipelines
`game/src/capture/` (~1,200 lines: scenarios, tour, bot_pov) and
`game/src/diagnostics.rs` (1,517) both script the match into staged states and take
screenshots, with separate scenario-stepping code.

### F9 — Manual resource lifecycle
`cleanup_match_resources` (`match_runtime/mod.rs:442-456`) hand-removes 13 resources.
Every new match resource must be remembered here or it leaks across states (the tests
guard some, not all).

### F10 — Stale root docs
`we-need-to-come-tidy-emerson.md` at the repo root is a *completed* feature plan (its
`gap_dests` freeze design is implemented in `TeleportState`). Root-level plan files
belong in `docs/` once done.

---

## The target shape

```text
game/src/
├── lib.rs              # plugin composition only (~100 lines)
├── flow.rs             # career + ONE headless match entry (same director as the screen)
├── match_director.rs   # F3 fix: single owner of live/series/teamplay + one outcome API
├── sim/                # simulation-side, Bevy-resource-holding, no rendering
│   ├── teleport/       #   (moved from src/teleport/, unchanged)
│   ├── state.rs        #   MatchRuntime, TeleportState, intents (from screens.rs)
│   ├── maze.rs, hallway.rs, items.rs, keystones.rs, guardian.rs, bot.rs, navmesh.rs
├── view/               # presentation: reads sim, never writes it
│   ├── theme.rs        #   colors/fonts/ui helpers (from screens.rs)
│   ├── assets.rs       #   MatchAssets + material builders (from screens.rs)
│   ├── place/          #   renderer (flattened strategies)
│   ├── hud.rs, tacmap view, audio.rs, ambience.rs, rivals sync
├── screens/            # state machine + menu screens only
└── evidence/           # capture + diagnostics behind one scenario driver
```

Module rule replacing the glob-hub: **explicit imports only** (no `pub use x::*`
between sibling modules), and `view/*` may import `sim/*` but never the reverse.

---

## Phases

Each phase is independently landable, ends green (`cargo fmt --all`,
`cargo clippy --workspace --all-targets`, `cargo test --workspace`), and re-captures
`OBSERVED2_CAPTURE` evidence when it touches rendering. Order is
risk-ascending so early wins de-risk the later surgery.

### G0 — Baseline & guardrails (half a day)
- Record baseline: test count, `cargo build -p observed_game` warnings, current
  evidence screenshots, and a saved `cargo metadata` module inventory.
- Add a tiny workspace test (or `xtask` check) that fails on `pub(crate) use .*::\*`
  inside `game/src` — ratcheted: allowed-list the current 8, shrink to 0 by G2.

### G1 — Deletions & mechanical dedup (1 day, low risk) — DONE 2026-07-02
1. ~~Delete~~ **Archive** `game/src/wfc_maze.rs` (decision: WFC may return for map
   generation): moved to `labs/wfc_proc_gen_lab/src/hallway_wfc.rs` behind a new lib
   target, with a local `WallSeg` and a connectivity smoke test so it keeps compiling.
   `ghx_proc_gen` dropped from `game/Cargo.toml`.
2. PRNG dedup — see F4 (resolved): game `maze.rs` + `guardian.rs` converted; crates
   were already clean; three intentional hash-finalizer exceptions documented there.
3. Layout constants now game-owned in `game/src/layout.rs` (`PLACE_TILE`,
   `HALL_WIDTH`); five presentation files re-pointed off `observed_match::maze`.
   `capture/tour.rs` deliberately keeps `GRID_W/H/TILE_SIZE` — it interprets the
   *brain's* room coordinates in the brain's own grid space.
4. `lib.rs`'s inline test module moved to `game/src/tests.rs` (lib.rs: 1,101 → 113
   lines; test content unchanged).
5. `we-need-to-come-tidy-emerson.md` → `docs/threshold_consistency_plan_2026-06.md`
   (it was a completed feature plan, archived under a descriptive name).

### G2 — Dissolve the `screens` hub — DONE 2026-07-02
1. Extract from `screens.rs` into new homes (types unchanged, imports explicit):
   theme/UI helpers → `view/theme.rs`; `MatchAssets` + material fns → `view/assets.rs`
   (give it real constructors/accessors instead of descendant-privacy);
   `MatchRuntime`, `TeleportState`, `FrozenDest`, intents, `SpectatorBot`,
   `SeriesRuntime` → `sim/state.rs`.
2. Kill the 8 glob re-exports; fix imports mechanically (compiler-driven).
3. Rename `screens/match_runtime/teleport.rs` → `crossing.rs` (or fold into
   `sim/teleport/`); rename `screens/place/items.rs` → `item_visuals.rs` to stop
   shadowing `src/items.rs`.
4. `screens.rs` shrinks to the state machine + `ScreensPlugin`/`MatchPlugin`
   composition. Ratchet check from G0 goes to zero.
- **Verify:** pure moves — full test suite + one capture screenshot diff.

**As landed:** `screens.rs` is 177 lines (was 710): menu domain + the two plugins,
with every system referenced by qualified path (`menu::`, `hud::`,
`match_runtime::ambience::`, `place::`…). All 8 submodule glob re-exports and all
`use super::*` (outside `#[cfg(test)]`) are gone; shared state lives in
`sim/state.rs`, `view/{theme,assets,components}.rs`, `layout.rs`; `MatchAssets`
gained a `load()` constructor (moved out of `setup_match`). The G0 ratchet landed
here instead as `game/src/arch_check.rs` — three source-scanning tests: no glob
re-exports, no non-test `use super::*`, and sim never imports view/screens.
Verified: 107 game tests, 670 workspace tests, clippy clean.

### G3 — One match brain — DONE 2026-07-02
1. Create `MatchDirector` (candidate promotion into `observed_match` as a pure
   orchestrator): owns the `LiveNetMatch`, the `EliminationSeries`, and (spectator)
   `TeamplayMatch`; exposes `tick(dt)`, `finished()`, `outcome() -> MatchResult`.
   Encapsulate today's coupling rules (series autoplay cadence, spectator pumping,
   live-finish → series fast-forward) behind that API, then **decide and document**
   the intended relationship — recommended: the series is *the* outcome authority
   and the hybrid live match is the round-in-play it aggregates; the HUD and the
   Results screen both read `outcome()`.
2. `match_pump` and `drive_spectator_bot` become thin adapters (input + director tick
   + state transition). Target: `match_runtime/mod.rs` < 400 lines.
3. Unify headless careers: `flow::play_match()` runs the same director to completion,
   deleting the second/`NetMatch` path or keeping it only as a transport test in
   `observed_net`. Headless result == on-screen result, asserted by a test.
- **Verify:** existing match/series/career tests + a new "HUD numbers come from the
  director" test; bot-POV GIF capture to eyeball behavior.

**As landed:** `game/src/sim/director.rs` (207 lines) — `MatchDirector` is the single
resource owning `live` + `series` + the cadence timers, replacing `MatchRuntime` and
`SeriesRuntime`. `tick(dt, spectator_driven)` encapsulates the whole per-frame rule
and yields the `MatchResult` exactly once; `run_to_completion()` is the headless
career path; `outcome()` is the one resolution rule (finished series is authority);
`pump_spectator()` and `record_local_keystone()` absorb the two out-of-band series
writers. `match_pump` is now pause-input + one `tick` call. `flow::play_match()` runs
the same director, and the pre-refactor characterization test
(`headless_and_interactive_matches_agree_on_the_result`, committed first as G3a)
pins headless == interactive. Behavior was preserved exactly — the current
either-model-ends rule is documented in the director; changing it (if ever) is now a
one-place, visible edit. `match_runtime/mod.rs` is 741 lines (from 1,007); the <400
target waits on G4 moving the nav derivation + spectator steering out.
`play_networked_match` stays as the transport-orthogonality test helper.
Verified: 671 workspace tests, workspace clippy clean.

### G4 — Flatten renderer patterns & split the remaining big files (1–2 days)
1. Replace `SpawningStrategy` `Box<dyn>` with two plain functions called from the
   existing `match tp.place`. Replace `GatewayPolicy` + three structs with a
   `ThresholdStyle { leaf_material: Option<...>, openable: bool, tethered: bool }`
   built by a 3-arm constructor. Delete the pattern banners.
2. `place/mod.rs` (1,288): move `rebuild_place` orchestration into `factory.rs`, the
   rival/keystone/tether systems into sibling files; `mod.rs` becomes declarations +
   the plugin systems list (< 200 lines).
3. Resource lifecycle (F9): group the 13 match resources under one
   `init_match_session`/`teardown_match_session` pair in a single file, with a test
   asserting the world has no match resources after `OnExit(Match)` — enumerated in
   one place so additions can't be forgotten.

### G5 — Evidence pipeline consolidation (1–2 days, optional but pays off for agents)
1. Split `diagnostics.rs` into `evidence/` modules: `scenarios.rs` (shared with
   capture), `snapshot.rs` (world → `observed_diagnostics` schema), `runner.rs`.
2. `capture/scenarios.rs` and the vis-audit drive scenarios through one
   `ScenarioDriver` (place the body, step N frames, screenshot) instead of two.
3. Diagnostics reads sim state via `sim/state.rs` types instead of screens internals.

### G6 — Docs & Catalogue truth-sync (half a day)
- Update `Catalogue.md`: correct the SplitMix "Centralized" claim (fixed for real in
  G1), refresh the bloated-files list, document the `sim`/`view`/`screens`/`evidence`
  layout and the explicit-imports rule.
- Add the module rule to `agents.md` Core Architectural Rules: *"Inside `game/`,
  presentation (`view`) reads simulation (`sim`), never the reverse; no glob
  re-exports between sibling modules."*
- Note in `ROADMAP.md` as the completed Arc G.

---

## Non-goals (per agents.md)
- No lab refactoring (the 50 labs stay as-is; they are archives of proven work).
- No Bevy 0.19 upgrade (rejected in R11; unchanged).
- No new abstractions beyond `MatchDirector` — everything else is moving, deleting,
  or flattening existing code.
- Crate-layer file splits (`room_world.rs`, `equipment.rs`, `observed_style`) are
  deferred; they are big but internally coherent and pure.

## Risk notes
- **G1.2 (PRNG)** is the only step that can silently change behavior (seed-stream
  drift). Mitigation: 1:1 call-site ports + the seeded determinism tests; do it as
  one reviewable commit per crate.
- **G3** changes when a match "ends". Pin the current behavior with a
  characterization test *before* refactoring, then change the rule (if at all) as a
  separate, visible commit.
- Everything else is compiler-checked moves.
