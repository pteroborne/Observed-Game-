# Phase 95 — Game Integration & Square Demotion

**Wave 6, serial, parent-heavy, user playtest gate.** This phase ends the arc:
hex becomes the canonical game map; the square lattice is demoted to
regression-only. This is the one phase allowed to touch `game/src/full_wfc/`
(wiring only) and game-level docs. Arc context:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Ship the hex facility as the Play flow with the full match experience — visual
language, lighting, audio hooks, relayout in play — and land the arc's
documentation and evidence refresh. Absorbs the **Phase 85 playtest debt**: the
Arc K hands-on gate deferred at Phase 86 is discharged here, on the hex game.

## Source material

- `game/src/full_wfc/` — the shell being paralleled: `sim.rs` (driver),
  `view/shell.rs` (StructurePiece → mesh/material via `observed_style`),
  `view/lighting.rs` (deterministic local-light budget), `view/entities.rs`,
  `input.rs`.
- The `OBSERVED2_MAP` env precedent (map override) and the legacy-match
  regression pattern from Arc K Phase 85.

## Deliverables

- `game/src/hex_wfc/`: sim driver over `observed_match::hex_wfc`, view shell
  instancing authored tile render geometry with `observed_style` materials and
  register dressings, lighting within the deterministic budget, streaming by
  visibility (mirror the full_wfc chunk streaming), relayout warnings/feedback
  wired to the existing AudioDirector + visual cue tables.
- Menu/Play flow switches to hex; Spectate AI works on hex; tac-map renders the
  hex survivor sketch (hex outlines in the sketch style).
- **Square demotion**: `full_wfc` reachable only via regression tests and an env
  override (`OBSERVED2_MAP` style); no new features; its tests stay green.
- Docs: ROADMAP arc record, `Catalogue.md` (new crates/labs/modules),
  `agents.md` if any standing rule changed, as-landed notes across
  `docs/arc_l/`, evidence refresh under `docs/evidence/hex_wfc/`.
- arch_check ratchets extended to the new modules (hex sim never imports view;
  600-line ratchet covers `hex_wfc` paths).

## Success criteria (hard gate)

1. Full workspace green (fmt/clippy/test, zero warnings); square regression
   suite still green.
2. Determinism: same seed + inputs ⇒ same digest, headless and interactive, on
   hex.
3. Full-match soak on hex (mirror the Arc K 36,000-tick autonomous soak).
4. Evidence refresh: gameplay capture, tac-map capture, a wellshaft descent, a
   ramp chain — all agent-viewed.
5. **Hands-on user playtest**: ramps, a silo descent, an observed relayout, a
   complete match. The arc does not close without it. Tuning findings triaged
   into the backlog or fixed in-phase.

## Evidence

The refreshed evidence set plus the playtest verdict recorded in this doc's
as-landed notes.

---

## Implementation Plan (proposed 2026-07-18, pre-build)

Grounded in the current shipped-game seams. The strategy is a **structural
mirror** of `game/src/full_wfc/` into `game/src/hex_wfc/`, driven by
`observed_match::hex_wfc::HexWfcMatch`, then a flow repoint + square demotion.
`full_wfc` stays intact as the regression fixture — we parallel it, we do not
rewrite it.

### Reference seams (verified)
- State machine: `GameState` enum in `game/src/lib.rs` (`Match` already
  DEPRECATED-regression; `FullWfc` is today's Play target). Plugins registered in
  `ObservedGamePlugin::build`.
- Adapter shape: `full_wfc/sim.rs` — `FullWfcRuntime { match_state, local_player,
  pending_visual_changes, status, map_open, results_delay_frames }` with
  `setup_runtime` / `step_runtime` / `finish_runtime` / `cleanup_runtime`; bots
  cross `bot_command`; replay via `ReplayTape::new_full_wfc` / `record_full_wfc`.
- Plugin wiring: `full_wfc/mod.rs` — OnEnter setup chain (sim, view, hud, tacmap,
  feedback, audio, entities, input), FixedUpdate `step_runtime`, Update sync
  chain, OnExit cleanup.
- View: `full_wfc/view/shell.rs` instances `StructurePiece`/`StructureRole` →
  mesh/material via register assets. Hex equivalent already exists:
  `observed_match::hex_wfc` exports `HexWfcGeometrySnapshot`, `HexStructurePiece`,
  `HexStructureRole`.
- Flow: `flow.rs::resolve_full_wfc(&FullWfcMatch) -> MatchResult`; `ActiveMatchSeed`
  is mode-agnostic (reused as-is).
- Menu: `screens/menu.rs:454-484` routes play/rematch/spectate/full_wfc modes all
  to `GameState::FullWfc`.
- Replay: `sim/replay.rs` has `new_full_wfc`/`record_full_wfc` keyed by
  `FULL_WFC_INPUT_VERSION`; hex exports `HEX_INPUT_VERSION`.

### Work breakdown

**1. State + plugin registration** (`lib.rs`)
- Add `GameState::HexWfc`. Register `hex_wfc::HexWfcPlugin` in `ObservedGamePlugin`.
- `FullWfc` stays a registered state but is no longer a menu destination (reachable
  only via the env override below) — mirrors how `Match` was demoted.

**2. `game/src/hex_wfc/` module** (mirror `full_wfc/`, file-for-file)
- `sim.rs`: `HexWfcRuntime` wrapping `HexWfcMatch`; setup solves a nearby-solvable
  seed (reuse the 0..64 offset-search pattern), seeds `pending_visual_changes`
  from placements, builds the geometry snapshot; `step_runtime` threads local +
  bot commands and re-snapshots on generation change; `finish_runtime` →
  `flow::resolve_hex_wfc` → `Results`. Production config uses the real
  `HexWfcConfig::arc_default()` (28×20×10) — **now ~0.8s to solve**, so it can run
  on the setup path without the old offline-scale concern (see the stale-doc note
  below); tests still use a showcase config.
- `view/`: `shell.rs` (instance `HexStructurePiece` with `observed_style` register
  materials + register dressings), `lighting.rs` (deterministic local-light
  budget, port from full_wfc), `assets.rs`/`registers.rs` as needed. **Streaming by
  visibility** mirrors `sync_streamed_cells`.
- `input.rs`, `hud.rs`, `tacmap.rs` (hex survivor sketch — hex outlines in the
  sketch style), `feedback.rs` + `cues.rs` (relayout warnings → AudioDirector +
  visual cue tables), `audio.rs`, `entities.rs`, `mod.rs` (plugin).
- Capture-mode env hooks mirroring `OBSERVED2_CAPTURE_FULL_WFC*` →
  `OBSERVED2_CAPTURE_HEX_WFC*`.

**3. Flow + replay**
- `flow.rs`: add `resolve_hex_wfc(&HexWfcMatch) -> MatchResult`.
- `sim/replay.rs`: add `new_hex_wfc` / `record_hex_wfc` keyed by `HEX_INPUT_VERSION`.

**4. Menu / Play / Spectate / tac-map**
- Repoint `menu.rs` play/rematch/spectate/full_wfc arms to `GameState::HexWfc`.
- Spectate: `SpectatorBot` path works on hex (bot drives `bot_command`).
- Tac-map renders the hex survivor sketch.

**5. Square demotion**
- `full_wfc` reachable only via an `OBSERVED2_MAP=square`-style env override (mirror
  the Arc K precedent) + regression tests. No new features; its suite stays green.

**6. arch_check ratchets** (`game/src/arch_check.rs`)
- Extend: hex sim never imports view; 600-line ratchet covers `hex_wfc` paths.

**7. Docs + evidence**
- ROADMAP arc record, `Catalogue.md` (new module), `agents.md` if a standing rule
  changed, as-landed notes here, evidence refresh under `docs/evidence/hex_wfc/`.
- **Correct the stale perf note**: `phase_92_hex_facility_3d.md` and
  `phase_93_relayout_parity.md` still call the WFC solve "offline-scale ~10 min";
  it is now ~0.8s. Fix both as part of this phase's doc pass.

### Gates I will drive (1–4)
1. Full workspace green (fmt/clippy/test, zero warnings); square regression green.
2. Determinism: same seed+inputs ⇒ same digest, headless and interactive, on hex.
3. Full-match soak on hex (mirror the Arc K 36,000-tick autonomous soak).
4. Evidence refresh (gameplay, tac-map, wellshaft descent, ramp chain), agent-viewed.

### Gate 5 — yours
Hands-on playtest: ramps, a silo descent, an observed relayout, a complete match.
I hand you a build + a short test script; the arc does not close until you play it.
Tuning findings get triaged into the backlog or fixed in-phase.

### Open decisions for you
- **Polish bar**: match `full_wfc`'s current visual/lighting/audio fidelity (my
  default), or a lighter "playable-first" pass with polish deferred to a follow-up?
- **Execution**: once you approve this plan, build via one orchestrated subagent
  (like Phase 94, ~300k+ tokens) or inline stepwise?
- **Branch**: continue on `arc-l-phase-94`, or a fresh `arc-l-phase-95` (I'd
  recommend merging 94 to `main` first, then branching 95 off it).

---

## As landed (2026-07-18)

**Status: complete; hands-on user playtest passed.** The work landed in the
dedicated `arc-l-phase-95` worktree. The decisions above resolved to full
adapter parity, one orchestrated implementation pass, and a separate Phase 95
branch. The user approved closure on 2026-07-18 after running the integrated
hex game; the two tuning findings are recorded below and in the bug backlog.

### Canonical flow and simulation

- `GameState::HexWfc` and `HexWfcPlugin` now own Play, Rematch, and Spectate AI.
  `OBSERVED2_MAP=square` (also accepting `full_wfc` and `full-wfc`) is the
  explicit square regression override; the default and unknown values select
  hex. Regression tests ratchet that routing.
- `HexWfcRuntime` wraps the versioned `HexWfcMatch` fixed-step boundary, consumes
  only abstract `PlayerIntent`, records versioned replay frames, resolves match
  results, and keeps the survivor sketch's knowledge local to the current
  player. Normal play uses `HexWfcConfig::arc_default()` (`28 x 20 x 10`); compact
  configurations are restricted to tests, evidence, and the opt-in playtest
  fixtures below.
- Match-layer mutation now opens, advances, commits, rejects, and reschedules
  observation-safe relayout work on deterministic tick boundaries. Warning and
  commit events are ordinary authoritative match events, so headless and
  interactive schedules digest the same state.

### Presentation and feedback

- The new `game/src/hex_wfc/` adapter streams exact authored render/collider
  pieces, maps semantic register roles through `observed_style`, and applies the
  shared controller, bounded key/headlamp budget, fog, bloom, and camera rig.
  Stable actors and the exit are projected from domain IDs rather than Bevy
  entity identity.
- HUD, relayout warnings/commit banners, semantic one-shot audio, proximity
  pressure, and the Tab survivor sketch are wired. The sketch uses true
  pointy-top axial outlines across all ten production levels and draws only the
  local runner's visited cells; it never feeds simulation.
- Architecture ratchets now cover `hex_wfc`: simulation cannot import view or
  screen code, and canonical adapter files remain under 600 lines.

### Evidence reviewed

The refreshed set lives in [`docs/evidence/hex_wfc/`](../evidence/hex_wfc/):

- `gameplay.png` shows the authored first-person shell, legible cyan threshold,
  semantic HUD, and bounded headlamp in the production `28 x 20 x 10` facility.
- `tacmap.png` shows all ten level panels and true axial hex marks for private
  survivor knowledge. The capture completed without Bevy hierarchy warnings.
- `traversal/ramp_*.png` follows the pinned lower and upper ramp prefabs; the HUD
  records the corresponding level changes and the evidence-only external
  vantage keeps the local runner visible. `traversal/ramp.gif` packages the four
  reviewed stills into a palette-filtered loop.
- `traversal/wellshaft_*.png` records the runner at the upper landing, during
  descent, and on the lower level of an eight-metre shaft stack;
  `traversal/wellshaft.gif` is the loopable sequence.
- `relayout/relayout_1_before.png`, `relayout_2_warning.png`, and
  `relayout_3_after.png` show generation 0, the pre-commit STRUCTURE BREATHING
  warning, and the atomic generation-1 ROOMS MUTATED result; `relayout.gif`
  packages the sequence.
- `hex_wfc_000.png` through `hex_wfc_007.png` cover the semantic facility
  register capture sequence. The authored collider shell is deliberately
  flat-shaded and procedural; this evidence proves state/geometry legibility,
  not final art richness.

### Automated gates

The implementation was formatted and the following Phase 95 gates passed in
the worktree:

```text
cargo test -p observed_game
  316 passed; 0 failed; 2 ignored

cargo test -p observed_game \
  hex_wfc_gates::hex_headless_and_interactive_agree_across_relayout_commit \
  -- --ignored --nocapture
  passed through 1,810 ticks, including a relayout commit

cargo test -p observed_game \
  hex_wfc_gates::hex_full_match_soak -- --ignored --nocapture
  passed the 36,000-tick autonomous window
```

The final `cargo test --workspace` rerun passed every crate, lab, binary, and
doctest. (The first invocation spent 19m54s compiling the clean workspace and
then hit the command wrapper's 20-minute timeout while tests were starting; the
cached authoritative rerun completed in 118 seconds.)
`cargo clippy --workspace --all-targets -- -D warnings` then passed across the
same workspace in 1m54s. Gate 5 is not inferred from these automated results.

### Gate 5: hands-on playtest hand-off

The first fixture pins Phase 94's proven vertical route while running the real
game adapter. From PowerShell:

```powershell
$env:OBSERVED2_HEX_PLAYTEST='gate'
$env:OBSERVED2_SEED='0xa11c000000000000'
cargo run -p observed_game --bin observed
```

Choose **Play**. Follow the halls to the first ramp at `q0 r3 L0` and walk it
without jumping. At the shaft around `q2 r4 L1`, use Space to climb to L2, hold
Ctrl to descend back to L1, then climb again. Continue over the upper ramp at
`q4 r6 L3` to L4 and finish at `q11 r8 L4`. Toggle the private survivor sketch
with Tab. Record whether ramp movement, shaft ascent/descent, map readability,
and the complete match all feel correct.

The second fixture gives the known observation-safe relayout timeline:

```powershell
$env:OBSERVED2_HEX_PLAYTEST='relayout'
$env:OBSERVED2_SEED='0xcb8521b1f77dd0fc'
cargo run -p observed_game --bin observed
```

Choose **Spectate AI**. Around tick 1,620 (about 27 seconds), verify the visible
STRUCTURE BREATHING warning; around tick 1,800 (about 30 seconds), verify the
atomic generation `0 -> 1` commit and ROOMS MUTATED feedback. Visible/occupied
geometry must not change under the runners.

Clear the fixtures afterward:

```powershell
Remove-Item Env:OBSERVED2_HEX_PLAYTEST, Env:OBSERVED2_SEED -ErrorAction SilentlyContinue
```

### Gate 5 verdict (2026-07-18)

**Pass.** The user ran the integrated hex game and approved merging Phase 95 and
closing Arc L, noting that the facility had materially come together. The run
also exercised mutation strongly enough to expose the commit-time stall. Two
findings are deliberately carried into the next planning arc rather than
expanding this integration phase after its functional and deterministic gates:

1. The initial scene looks washed out before the hex-specific lighting rules
   take effect. Backlog #8 owns making the correct semantic lighting state
   present on the first visible frame.
2. A mutation commit freezes the whole game. Backlog #9 owns profiling the
   commit path and designing a bounded, incremental mutation/projection pipeline
   so small continuous work replaces a monolithic frame spike without weakening
   observation pins, atomic authoritative state, or replay determinism.

With those findings triaged, Gate 5, Phase 95, Arc L, and the carried Arc K
Phase 85 playtest debt are closed.
