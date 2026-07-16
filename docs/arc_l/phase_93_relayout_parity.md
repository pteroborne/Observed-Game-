# Phase 93 — Relayout / Decoherence Parity

**Wave 4, parallel with Phase 92.** You own
`crates/observed_facility/src/hex_wfc/{relayout,topology}.rs` and the 2D relayout
mode of `labs/hex_wfc_lab/`. Phase 92 owns `crates/observed_match/src/hex_wfc/`
and the lab's 3D modules; the shared facility `hex_wfc/tests.rs` is
**append-only** for both. Do not touch anything under `full_wfc`. Arc context:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Full relayout/decoherence parity on hex — a locked user ruling: the
changes-when-unobserved identity must work before game integration. Port the
square lattice's continuous-mutation machinery at **blueprint granularity**.

## Source material

- `crates/observed_facility/src/full_wfc/relayout.rs` — `begin_relayout` /
  `advance_relayout` / `propose_relayout`, `RelayoutWork` / `RelayoutProgress`,
  the one-attempt-per-tick discipline, `fallback_geometry_relayout`.
- `crates/observed_facility/src/full_wfc/topology.rs` — `pinned_cells`,
  `change_is_safe`, route re-validation at commit.
- `crates/observed_facility/src/full_wfc/mod.rs` — `commit_relayout` (route
  checks for players/landmarks before accepting).

## Deliverables

- Multi-tick relayout on the hex solver: begin/advance/propose over the
  `(seed, generation, attempt)` streams; incremental, cancellable, one
  deterministic attempt per tick.
- **Pin semantics at blueprint granularity**: observed/occupied/landmark cells
  pin their whole blueprint footprint plus attached threshold cells; hall pins
  stay per-cell. Ramp pairs pin as a unit (never split a `RampUp` from its
  `RampHead`). Shaft columns pin per-cell.
- Identity across relayout: `RoomId` (via blueprint `generation_key`) and
  `CorridorId` survive relayouts that keep them in place; threshold attachments
  re-derive stably.
- Commit-time validation: every occupied-player and remaining-objective route
  re-verified; pinned geometry byte-identical across the commit.
- Fallback path: when the topological retry budget exhausts, mutate one unseen
  cell's architecture register with identical ports (mirror
  `fallback_geometry_relayout`).
- **Decoherence-yield measurement** (arc-listed risk): with N rooms observed
  (blueprint pins), measure how many cells remain free to re-collapse on the
  default config; record the curve in as-landed notes so the parent can judge
  whether blueprint pins starve relayout.
- Lab: 2D animated relayout demo — pin a room (click/key), trigger a pulse,
  watch unpinned regions decohere and re-collapse in the step view.

## Success criteria

1. Replay determinism test: same pin set + seed + tick schedule ⇒ identical
   proposal, twice in a row.
2. Pin invariants over a corpus (≥100 seeds × ≥20 relayouts): pinned blueprints
   and ramp pairs never change; all routes survive every commit.
3. Stable-ID test: an observed room keeps its `RoomId` across ≥20 relayouts.
4. Decoherence yield recorded.
5. Workspace green.

## Evidence

Agent-viewed GIF of the lab relayout demo (pinned room stays, surroundings
re-form) + corpus/replay test output. Describe what visibly changed and what
visibly held still.
