# Arc L — Hex Tile Facility

**Status:** active (opened 2026-07-16).
**Baseline:** `main` at the Arc K closeout commit (Phase 86).
**Goal:** replace the square full-WFC lattice with a TrenchBroom-authored hex-prism
tile system — taller cells, a larger grid in all three axes, multi-tile rooms, halls
as long as they need to be, walkable ramps and true shafts — landed through a lab
sequence (2D hex step lab → tile lab → 3D hex lab) and culminating in full game
integration, with relayout/decoherence/determinism parity before the swap.

Hand-off docs: [arc_l/README.md](arc_l/README.md) (wave model, phase briefs).

## The thesis

The square lattice proved the game: continuous facility, observation pins, relayout,
stable identity, bots, a full match. But its cells are stubby (12 m wide, 5 m tall),
its rooms are single cells, and its geometry is procedural cuboids — the facility
reads as corridors of boxes, and verticality is a teleport-flavored climb rather
than architecture. Hex prisms with authored tiles invert that: six lateral
neighbors give halls organic, snaking routes; taller cells (8 m) make ramps
walkable architecture and let shaft stacks read as genuine wells; TrenchBroom
authorship gives every tile deliberate interior form while WFC still owns topology,
identity, and the shifts-when-unobserved core. Rooms become composed places — a few
tiles with authored footprints — and halls become as many tiles as the route needs.

## Locked rulings (user, 2026-07-16)

- **Hex replaces square at integration.** The hex system is built in parallel
  (sibling modules, new labs); at Phase 95 it becomes the canonical map and the
  square lattice is demoted to regression-only. No long-term dual maintenance.
- **Full relayout parity in-arc.** Observation pins, multi-tick relayout, and
  `(seed, generation)` determinism must work on hex before game integration; the
  game never regresses the changes-when-unobserved identity.
- **Ramps and shafts.** Walkable authored ramp tiles connect level N to N+1 through
  lateral faces; Up/Down shaft faces remain for wellshaft/silo formations. Both are
  first-class in the solver's port vocabulary.
- **Mandated arc sequence:** 2D hex WFC lab with an animated step mode showcasing
  the WFC connecting rooms → TrenchBroom tile lab → 3D expansion of the hex lab
  (both modes kept testable) → full game integration.

## Design decisions

### The quantized hexagon

Pointy-top hex prisms in plan view, so East/West faces are flat axis-aligned walls.
A regular hexagon cannot sit on an integer grid (√3), so the canonical footprint is
a **quantized hexagon** with integer corners in meters:

```
(7,-4) (7,4) (0,8) (-7,4) (-7,-4) (0,-8)
```

Flat faces are 8 m wide at x = ±7; diagonal edges are √65 ≈ 8.06 m (0.8%
irregularity, imperceptible). Neighbor pitch: E = (+14, 0), NE = (+7, −12),
SE = (+7, +12). Every corner is an integer in TrenchBroom units at any integer
units-per-meter scale, so brushes snap exactly and the importer validates
footprints against the canonical corners with zero tolerance.

### Coordinates, faces, ports

- `HexCoord { q: u16, r: u16, level: u8 }` — axial storage, rhombic bounds
  (parallelogram world footprint; no offset-parity branches; indexing stays
  `level*cols*rows + r*cols + q`). Hex distance `(|dq|+|dr|+|dq+dr|)/2` plus a
  weighted level term replaces Manhattan distance everywhere.
- `HexFace { East, NorthEast, NorthWest, West, SouthWest, SouthEast, Up, Down }` —
  8 faces; lateral `opposite() = (f+3) % 6`.
- The boolean `openings: u8` mask is replaced by **port classes**:
  `PortClass { Sealed, Door, RampOpen, ShaftOpen }`, 2 bits × 8 faces =
  `PortSignature(u16)`. Compatibility stays a symmetric per-face relation
  (Door↔Door lateral only, ShaftOpen↔ShaftOpen vertical only, RampOpen↔RampOpen
  vertical only) — AC-3 propagation is structurally unchanged.

### Ramp encoding — paired vertical variants

A full-level ramp exits at the top of its cell, where the doorway's headroom lies
in a *diagonal* cell — which would demand diagonal constraints that break face
propagation. Instead, two paired variants connected by an ordinary vertical face:

- **`RampUp(D)`** at level N: ramp geometry rising floor N → N+1; `opposite(D)` =
  `Door` (low entrance); `Up` = `RampOpen`.
- **`RampHead(D)`** at level N+1: open headroom, no floor (the ramp below is the
  walking surface); `Down` = `RampOpen`; `D` = `Door`, connecting to the lateral
  neighbor at N+1 as a completely normal connection.

The pair self-assembles through `RampOpen` compatibility during collapse; every
constraint is pure face adjacency. Geometrically the pair is **one two-level-tall
authored prefab** anchored at the low cell; the head cell contributes no geometry.

### Dimensions and scale

| constant | square (today) | hex (Arc L) |
| --- | --- | --- |
| cell across flats (E–W) | 12.0 | **14.0** |
| cell across corners (N–S) | 12.0 | **16.0** |
| level height | 5.0 | **8.0** |
| grid | 20 × 12 × 5 (1,200 cells) | **28 × 20 × 10 (5,600 cells)** |

Ramp slope ≈ 29.7° (rise 8 over run 14), walkable by the shared controller
(verified at Phase 89's gate). Ten 8 m levels make silo shafts genuinely deep.

### Multi-tile rooms — blueprint stamping

Rooms are authored **blueprints**: 1–4 hexes laterally, optionally 2 levels tall,
with fixed exterior `PortSignature`s and named ports `(cell_offset, face)`. A
deterministic pre-collapse seeding stage (same SplitMix `(seed, generation,
attempt)` stream, same pipeline slot as the square lattice's forced-route seeding)
stamps blueprints against `RoomRole` quotas; WFC collapses halls around them.
Rooms are already special-cased everywhere (Room↔Room forbidden, quotas,
`min_room_distance`, forced spawn→exit route) — stamping preserves all of it.

- Identity: `generation_key = hash(blueprint_id, anchor_cell)` (the
  `corridor_generation_key` construction), so `RoomId` survives relayout.
- Pins operate at **blueprint granularity**: observing any cell pins the whole
  footprint plus attached threshold cells.
- `ThresholdKey { room, face }` generalizes to `{ room, port }` with the
  blueprint's named exterior ports; `ThresholdId` derivation keeps the ordered-list
  construction.

### The tile manifest — authored catalog, not mask enumeration

The square catalog enumerated 64 opening masks; the hex catalog is **driven by the
authored tile set**: `assets/tiles/manifest.ron` maps `TileKey { archetype,
register, variant } → { map_path, port_signature, footprint }`. The solver crate
consumes only parsed pure manifest data (`observed_facility` stays Bevy-free), and
a coverage validator (test + CLI) proves every port signature the solver can
demand has at least one tile — a missing variant would otherwise mean unsolvable
seeds.

### New units

| unit | path | contents |
| --- | --- | --- |
| `observed_hex` | `crates/observed_hex/` | coords, faces, ports, quantized metrics, world mapping; no Bevy/Rapier |
| `hex_wfc` solver | `crates/observed_facility/src/hex_wfc/` | pure hex solver, sibling of untouched `full_wfc`; files split up front for the 600-line ratchet |
| `observed_authoring` | `crates/observed_authoring/` | .map import (importer-only, per the A9 ruling), tile schema, footprint validation, brush → `ColliderSpec::ConvexHull`; promotes `trenchbroom_lab/src/project.rs` + `rapier_authoring_lab/src/import.rs` — the A9 adopt-when trigger ("design pivots to hand-authored maps") has fired |
| tile assets | `assets/tiles/` | `manifest.ron` + authored `.map` tiles + the locked template map |
| `hex_wfc` match | `crates/observed_match/src/hex_wfc/` | prefab placement → `ArenaSpec` (the existing seam), hex A* costs, ramp-aware routing; movement shares `step_character` |
| labs | `labs/hex_wfc_lab/`, `labs/hex_tile_lab/` | standard lab conventions; `hex_wfc_lab` grows from 2D (Wave 2) into 3D (Wave 4), keeping the 2D step mode as a debug view |
| game | `game/src/hex_wfc/` | Bevy shell; canonical at Phase 95 |

## Design principles

1. **Lab before game.** No hex system code touches `game/` until Phases 88–94 have
   proven it in `hex_wfc_lab` / `hex_tile_lab` with viewed evidence.
2. **The square lattice is untouchable until Phase 95.** `observed_facility::full_wfc`,
   `observed_match::full_wfc`, and `game/src/full_wfc/` receive no edits during
   Waves 1–5; hex work lands only in new files/modules.
3. **Sim/presentation separation holds.** The solver and match layers stay
   Bevy-free; authored content reaches the solver only as parsed pure data.
4. **Falsifiable evidence** (Arc H standing rule): every phase ends with captures
   the implementing agent has viewed and described; the parent rejects phases
   whose evidence doesn't visibly show the claim.
5. **Determinism is inviolable:** SplitMix streams keyed by
   `(seed, generation, attempt)`; same inputs, same layout, headless == interactive.
6. **One source of truth for hex metrics:** solver, projector, importer, and the
   TrenchBroom template all read `observed_hex` constants.

## Landing sequence

1. **Phase 86 — Arc K Closeout & Clean Baseline.** Commit the dirty tree, triage
   `labs/map_observer_lab`, formally carry the Phase 85 playtest gate into Phase
   95, open the arc docs.
2. **Phase 87 — `observed_hex` Foundation.** The shared crate: coords, faces,
   ports, quantized metrics, world mapping, property tests.
3. **Phase 88 — 2D Hex WFC Solver + Animated Step Lab.** Levels=1 solver; the
   animated step mode showcasing propagation, collapse order, and halls snaking
   between rooms.
4. **Phase 89 — TrenchBroom Tile Pipeline + Tile Lab.** `observed_authoring`,
   tile schema + manifest, seed tiles including a full ramp prefab; first-person
   ramp ascent proven.
5. **Phase 90 — Solver Verticality & Room Blueprints.** Levels>1, shaft stacks,
   ramp pairing, blueprint stamping, tall-formation validation.
6. **Phase 91 — Tile Library Authoring.** The full tile set against the schema;
   coverage validator green.
7. **Phase 92 — 3D Hex Facility Lab.** Solved world + manifest → prefab placement
   → `ArenaSpec`; FPS walkthrough of a solved facility; collider budget measured.
8. **Phase 93 — Relayout / Decoherence Parity.** Multi-tick relayout with
   blueprint-granularity pins, stable IDs, replay determinism tests, animated
   relayout demo.
9. **Phase 94 — Match-Layer Parity.** Hex/ramp/shaft routing costs, bots
   physically traversing ramps and shafts, thresholds on hex ports.
10. **Phase 95 — Game Integration & Square Demotion.** `game/src/hex_wfc/` shell,
    hex becomes the Play flow, square demoted to regression-only, docs/evidence
    refresh, hands-on playtest gate (absorbs the deferred Phase 85 playtest).

## Hard acceptance gates

- Stable domain IDs only; `generation_key` identity survives relayout on hex
  exactly as it does on square.
- Simulation never imports presentation; the solver never links Bevy; authored
  tiles reach it as data.
- Same `(seed, generation)` + input frames ⇒ same canonical digest, headless and
  interactive, on the hex facility.
- Every manifest tile has render/collision agreement and two-way traversal
  coverage, including ramp pairs and shaft stacks.
- The importer rejects any tile whose footprint deviates from the canonical
  quantized corners, with a precise diagnostic.
- Relayout preserves all occupied-player and remaining-objective routes and never
  changes pinned blueprints.
- `cargo fmt` / `cargo clippy --workspace --all-targets` (zero warnings) /
  `cargo test --workspace` green at every phase commit; files under the 600-line
  ratchet; falsifiable evidence viewed at every phase; user playtest at Phase 95.

## Risks under watch

1. **Rhombus domain.** Axial-rectangular bounds make a parallelogram footprint —
   hex distance everywhere, arena shell follows the skew; corner cells may be
   masked to Void for silhouette.
2. **TrenchBroom precision.** Authors will place off-template vertices; the
   importer hard-fails with diagnostics, and a locked tile-template `.map` ships
   as the authoring starting point.
3. **Collider count.** 5,600 cells × multi-brush convex hulls; Phase 92 measures
   and picks mitigation (per-tile hull merge, static compounds, or streaming)
   under the enhanced-determinism constraint.
4. **Contradiction rate.** Ramp pairing + blueprint stamping constrain harder than
   the square catalog; retry budgets and weights need tuning; the coverage
   validator prevents unsolvable-by-missing-tile seeds.
5. **Ramp traversal.** `step_character` at 29.7° and ramp A* costs are unproven
   until the Phase 89/94 gates; watch for lips at prefab boundaries.
6. **Pin granularity.** Blueprint pins freeze more lattice than cell pins; Phase
   93 measures decoherence yield under heavy observation.

## Out of scope

- LAN/online transport (still the next arc after this one).
- New gameplay mechanics: roles, objectives, Guardian, equipment all port as-is.
- Retiring the square-lattice code (it stays as the regression fixture).
- BSP compilation / lightmaps — the importer remains parse-only.
