# Phase 90 — Solver Verticality & Room Blueprints

**Status: Complete (verified 2026-07-17).** Pinned seed
`0xa11ce3d000000008` contains a four-level wellshaft and a ramp chain climbing
three levels; the full solver corpus, workspace tests, Clippy, and refreshed
four-slice evidence are green.

**Wave 3, parallel with Phase 91.** You own `crates/observed_facility/src/hex_wfc/`
and `labs/hex_wfc_lab/`. Do not touch `assets/tiles/**` content (Phase 91 owns
it); the manifest **schema** is frozen — if you need a schema change, stop and
escalate to the parent. Arc context:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Take the 2D solver to full 3D: vertical port classes, ramp pairing, shaft stacks,
and multi-tile room **blueprints** — producing the arc's signature tall
formations (silo wellshafts, ramp chains).

## Deliverables

- **Levels > 1**: extend domains/propagation over `Up`/`Down` with
  `PortClass::{ShaftOpen, RampOpen}` (compatibility already defined in
  `observed_hex`).
- **Ramp pairing**: `RampUp(D)` low variant (lateral Door entrance at
  `opposite(D)`, `Up = RampOpen`) + `RampHead(D)` upper variant
  (`Down = RampOpen`, lateral Door exit at `D`). The pair self-assembles through
  face compatibility — no diagonal constraints. Validation: every `RampOpen`
  face is matched by its partner class; a `RampHead` never sits over anything
  but its matching `RampUp`.
- **Shaft stacks**: `ShaftOpen` columns compose N-level wells; weights tuned so
  multi-level shafts actually occur (a Silo = ≥4-level shaft column with a cap).
- **Blueprint stamping** (`blueprint.rs`): blueprint definitions (1–4 hexes
  laterally, optionally 2 levels tall; fixed exterior `PortSignature`s; named
  ports `(cell_offset, face)`); a deterministic pre-collapse seeding stage (same
  SplitMix stream discipline, same pipeline slot as the forced-route seeding)
  stamps blueprints against `RoomRole` quotas; interior cells pre-collapsed,
  exterior ports become constraints; WFC fills halls around them. Room↔Room
  adjacency rule now applies between blueprint footprints.
- **Identity**: `generation_key = hash(blueprint_id, anchor_cell)` (mirror
  `full_wfc::catalog::corridor_generation_key`); `ThresholdKey { room, port }`
  with the blueprint's named ports; catalog projection
  (rooms/corridors/attachments) over the new shapes.
- **Routing**: `topology` A* costs per class — ramps ≈ the Climb cost tier,
  shafts = the Shaft tier; heuristic uses hex distance + weighted levels.
- **Lab**: per-level slice view in `hex_wfc_lab` (PgUp/PgDn or [/] to step
  levels); the step mode visualizes blueprint stamping before hall collapse.

## Success criteria

1. Seeded corpus test: a solve on the default 3D config contains a ≥4-level
   wellshaft and a ramp chain climbing ≥3 levels (find seeds; pin them).
2. Ramp-pair invariant holds over the corpus (no orphan `RampHead`/`RampUp`).
3. Blueprint footprints never overlap; quotas met; `min_room_distance` in hex
   distance holds between blueprint anchors.
4. Determinism: identical placement lists and traces per `(seed, generation)`.
5. Route test: spawn→exit route exists and traverses at least one vertical
   element on the pinned seeds.

## Evidence

Agent-viewed lab captures: level-slice walkthrough of a solve showing a stamped
multi-hex room, a wellshaft column across slices, and a ramp chain. Describe them.
