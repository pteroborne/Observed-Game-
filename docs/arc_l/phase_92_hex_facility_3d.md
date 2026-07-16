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

## Evidence

Agent-viewed captures: 3D walkthrough shots (hall, ramp ascent mid-slope, the
look down a wellshaft), plus one 2D-mode capture proving it still works. Describe
scale/readability honestly — 8 m levels should read as tall.
