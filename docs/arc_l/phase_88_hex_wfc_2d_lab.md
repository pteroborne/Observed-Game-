# Phase 88 — 2D Hex WFC Solver + Animated Step Lab

**Wave 2, parallel with Phase 89.** You own
`crates/observed_facility/src/hex_wfc/` and `labs/hex_wfc_lab/`. Do not touch
`crates/observed_authoring/`, `labs/hex_tile_lab/`, `assets/tiles/`, or anything
under `full_wfc`. Arc context: [../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

The hex WFC solver at `levels = 1` (lateral faces only), plus `labs/hex_wfc_lab`
with an **animated step mode** that showcases the WFC connecting rooms — the
user-mandated first deliverable of the arc.

## Deliverables

### Solver — `crates/observed_facility/src/hex_wfc/`

New sibling module of `full_wfc` (which is frozen): `mod.rs`, `config.rs`,
`catalog.rs`, `collapse.rs`, `constraints.rs`, `topology.rs`, `tests.rs` (split up
front for the 600-line ratchet; `blueprint.rs` and `relayout.rs` arrive in later
phases). Port the proven `full_wfc` architecture onto `observed_hex` types:

- `HexModulePlacement { coord: HexCoord, space: ModuleSpace, ports: PortSignature,
  ... }` — reuse `ModuleSpace { Void, Room, Hall }` semantics; archetype grammar
  may start minimal (Room, Straight, Corner, Junction).
- Variant domain over the 6 lateral faces with `PortClass::{Sealed, Door}` only
  (verticality is Phase 90). Do NOT mask-enumerate into a `u8`; build the variant
  set programmatically over lateral port combinations, weighted by degree like
  `full_wfc::collapse::catalogue`.
- AC-3 propagation, min-entropy weighted collapse, SplitMix determinism keyed
  `(seed, generation, attempt)` — mirror `full_wfc/collapse.rs` structure.
- Forced-route constraint (spawn→exit hex path), `prune_disconnected`, validation:
  spawn/exit rooms, single component, room count range, `min_room_distance` in
  **hex distance**, edge matching, hall components touching 2–4 rooms.
- Rooms are single hexes in this phase (blueprints arrive in Phase 90).
- **Solve tracing for the lab**: the collapse must be able to emit an ordered
  `Vec<SolveStep>` (cell collapsed, variant chosen, cells re-propagated,
  contradiction/retry events) so the lab can replay it step by step. Keep the
  trace behind a flag so headless solves pay nothing.

### Lab — `labs/hex_wfc_lab/`

Standard conventions (thin `main.rs` + `lib.rs`, tag + dirty-flag reset,
`OBSERVED2_CAPTURE` state-machine writing `manifest.json`, F1-hidden overlay HUD,
colors only via `observed_style`). Model the crate layout on `lighting_lab`, the
step/pulse UI on `full_wfc_lab`.

- Top-down 2D rendering of the hex grid (flat meshes or gizmo polygons):
  uncollapsed cells dim with entropy count, rooms/halls/void distinct via
  `observed_style` treatments, forced route and spawn/exit marked per the
  Legibility Contract (documented legend meanings).
- **Animated step mode**: Space = play/pause, N = single step, +/- = speed,
  R = new seed, digits = preset seeds. Playback walks the `SolveStep` trace so
  the user watches propagation fronts, collapse order, and halls snaking between
  rooms. An instant-solve toggle shows the final layout.
- Capture mode records a stepped solve as sequential frames (GIF-able via the
  CLAUDE.md ffmpeg recipe).

## Success criteria

1. Determinism: same `(seed, generation)` ⇒ byte-identical placement list AND
   identical `SolveStep` trace (test).
2. Corpus: ≥100 seeds solve within retry budget on the default config; validation
   invariants hold on all (test).
3. Lab resets without leaking entities/resources (test, per lab discipline).
4. Animated mode visibly shows rooms being connected by halls.

## Evidence

Agent-viewed GIF of a full animated solve connecting rooms (describe what you
see: collapse order, hall growth), plus a final-layout capture. Note test counts.
