# Phase 92 — 3D Hex Facility Lab

**Wave 4, parallel with Phase 93.** You own
`crates/observed_match/src/hex_wfc/` (geometry) and the 3D modules of
`labs/hex_wfc_lab/`. Phase 93 owns `hex_wfc/{relayout,topology}.rs` in the
facility crate and the lab's 2D relayout mode; the shared facility
`hex_wfc/tests.rs` is **append-only** for both. Do not touch anything under
`full_wfc`. Arc context: [../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Project a solved hex facility into physical space — authored tile prefabs placed
per cell, colliders in one `ArenaSpec` — and grow `hex_wfc_lab` into a 3D
walkthrough. The user requirement: **both** modes stay testable — the 2D animated
step mode remains as a debug view alongside the new 3D mode.

## Source material

- `crates/observed_match/src/full_wfc/geometry.rs` — the projection pattern being
  replaced: `project(world) -> { generation, pieces, arena }`; `ArenaSpec` is the
  seam (`crates/observed_traversal/src/world.rs`).
- `crates/observed_authoring` — `TilePrototype` (colliders + render hints) and the
  manifest loader (Phases 89/91).
- `observed_hex::hex_origin` — the only lattice→world mapping.

## Deliverables

- `crates/observed_match/src/hex_wfc/geometry.rs`: solved placements + manifest →
  per-cell prefab instancing (translate the prototype's colliders/render pieces
  by `hex_origin`; ramp prefabs anchor at the low cell, `RampHead` cells emit
  nothing), stable collider IDs per cell/piece (mirror `StableColliderId`
  discipline), and an arena boundary shell following the **rhombus** footprint.
- Room blueprints instance their authored footprint geometry once per blueprint
  anchor, not per cell.
- **Collider budget measurement** (an arc-listed risk): report collider counts
  and Rapier scene build/step time on the default 28×20×10 config. If counts
  swamp the scene, implement the cheapest sufficient mitigation (per-tile hull
  merging at import is the expected first resort) and record the decision in
  this doc's as-landed notes.
- `labs/hex_wfc_lab` 3D mode: solve → spawn geometry via `observed_style`
  materials → free-fly camera + FPS walk mode (shared `observed_traversal`
  controller). Mode toggle keeps the 2D step/slice views fully functional.

## Success criteria

1. Every non-void cell of a solved facility has geometry and colliders; render
   and collision agree (spot-check via the lab's collider debug view).
2. First-person: walk a hall chain between two rooms, ascend a ramp pair, and
   stand at a wellshaft rim looking down ≥4 levels.
3. Determinism: geometry projection is a pure function of the solved world +
   manifest (test: identical piece lists for identical inputs).
4. Collider budget numbers recorded; scene steps at 60 Hz on the default config.
5. Lab resets cleanly in both modes; workspace green.

## As landed -- 2026-07-17

Phase 92 is complete. `observed_match::hex_wfc` now projects a solved
`HexWfcWorld` plus the authored tile manifest into one deterministic geometry
snapshot. Prototype selection is exact on semantic archetype, full eight-face
port signature, and the simulation-owned variation key. `RampUp` owns its
two-level prefab and `RampHead` emits nothing; room blueprint geometry is
instanced once from its stamped anchor; and the boundary shell follows the
rhombic grid outline rather than an axis-aligned box. Render meshes and the
Rapier `ArenaSpec` are produced from the same stable piece list.

Collider IDs reserve 128 IDs per source cell, keep the boundary shell in a
separate high range, and reject an oversized grid before projection instead of
truncating. Tests cover deterministic projection, every non-void cell, unique
stable IDs, the boundary-room exception, rhombic shell shape, a controller walk
up a projected ramp, full-width `u64` variation modulo, oversized-ID failure,
and byte-identical pinned pieces across a committed relayout.

The lab retains the accepted 12x9x4 Phase 90/93 plan fixture and adds F3 3D
walk/free-fly mode, shared-controller traversal, semantic `observed_style`
materials, live collider-facet debug, reset/respawn, and a targeted status
legend. Shared R/digit resets cancel Phase 93 work in the same update, including
reselecting the current seed. The evidence runner uses a separate deterministic
12x9x5 world so its top-rim-to-bottom-floor wellshaft proof is a real 32 m drop
without changing the shared fixture.

### Collider budget decision

The ignored production measurement used seed `0xA11C930000000001` and the real
28x20x10 (`5,600` cell) `HexWfcConfig::arc_default()`:

- `5,488` non-void cells and `108,332` authored colliders.
- WFC solve: `622,817 ms`; projection: `1,132 ms`; Rapier scene build:
  `2,164 ms`.
- Eight independently offset characters, all sprinting/moving/looking through
  the same scene for 600 ticks at a requested 60 Hz: `470 us` mean batch frame,
  or `58 us` per character query.

The moving eight-body batch is comfortably below the `16,667 us` CPU query
budget, so Phase 92 preserves exact authored collision instead of merging hulls.
This benchmark excludes the WFC solve, projection, scene construction, Bevy
entity spawning, and GPU rendering from the per-frame number. In particular,
the roughly 10.4-minute production WFC solve is an explicit remaining generation
cost, and full-scale Bevy spawning/rendering of 108k pieces has not been claimed
as measured.

### Evidence and verification

The accepted evidence set and machine-readable counts are in
[`../evidence/hex_wfc_lab_p92/`](../evidence/hex_wfc_lab_p92/). The hall and ramp
frames are shared-controller `Walk` captures with `grounded true`; the shaft is
an actual free-fly view from the top landing rim, with the manifest and test
pinning five cells, 40 m of structure, and 32 m between floor levels. The
collider frame alternates the style-owned edge/base tones by stable collider ID
and reports `collider debug true`, making adjacent exact hulls visible. The 2D
frame proves the animated solve/slice view remains functional. These are
diagnostic collider-mesh visuals, not a claim of final decorative art.

Verification run:

- `cargo fmt -p observed_match -p hex_wfc_lab`
- `cargo test -p observed_match`
- `cargo test -p hex_wfc_lab`
- `cargo clippy -p observed_match -p hex_wfc_lab --all-targets -- -D warnings`
- `cargo test -p observed_match report_arc_default_collider_build_and_step_budget -- --ignored --nocapture`
- `OBSERVED2_HEX_3D_CAPTURE=docs/evidence/hex_wfc_lab_p92 cargo dev-run -p hex_wfc_lab`

## Evidence

Agent-viewed captures: 3D walkthrough shots (hall, ramp ascent mid-slope, the
look down a wellshaft), plus one 2D-mode capture proving it still works. Describe
scale/readability honestly — 8 m levels should read as tall.
