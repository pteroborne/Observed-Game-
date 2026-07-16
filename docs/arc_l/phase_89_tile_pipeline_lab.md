# Phase 89 — TrenchBroom Tile Pipeline + Tile Lab

**Wave 2, parallel with Phase 88.** You own `crates/observed_authoring/`,
`labs/hex_tile_lab/`, and `assets/tiles/`. Do not touch
`crates/observed_facility/`, `labs/hex_wfc_lab/`, or anything under `full_wfc`.
Arc context: [../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Promote the proven lab-side TrenchBroom import into `crates/observed_authoring`,
define the hex tile schema and manifest, author the first seed tiles (including a
full ramp prefab), and prove them walkable in `labs/hex_tile_lab` — first-person
ramp ascent included. This fires the A9 adopt-when trigger
([../asset_a9_decision.md](../asset_a9_decision.md)): the design has pivoted to
hand-authored tiles composed procedurally.

## Source material (read first)

- `labs/trenchbroom_lab/src/project.rs` — the pure .map → domain projection to
  generalize (importer-only stance: editor entities never become the game model).
- `labs/rapier_authoring_lab/src/import.rs` + `src/physics.rs` — brush → raw
  `rapier3d 0.34` convex-hull colliders; authored stairs already walk with the
  shared `observed_traversal` controller.
- `crates/observed_hex/src/metrics.rs` — canonical corners, pitch, level height.
  **Never copy these numbers; import them.**

## Deliverables

### `crates/observed_authoring/`

- .map parsing (reuse the `bevy_trenchbroom 0.13` / `quake-map` approach from the
  labs; keep any Bevy-coupled code behind a feature or in the lab — the core
  parse/project path must be pure so `observed_facility` and tests stay headless).
- **Tile schema** (point/brush entities in the map):
  `tile_meta { archetype, register, variant }`,
  `tile_port { face, class }` (class ∈ door/ramp_open/shaft_open), plus
  worldspawn brushes for solid geometry.
- **Footprint validation**: tile boundary must match the canonical quantized
  corners exactly (integer TrenchBroom units at the chosen units-per-meter
  scale) — hard fail with a precise diagnostic (which vertex, expected vs found).
- Brush → `observed_traversal::ColliderSpec::ConvexHull` conversion; a
  `TilePrototype { key, port_signature, footprint, colliders, render_hint }`
  output type.
- **Manifest**: `assets/tiles/manifest.ron` mapping
  `TileKey { archetype, register, variant } → { map_path, port_signature,
  footprint }`; loader + validator (manifest entries parse, signatures match the
  authored ports, footprints valid).

### `assets/tiles/`

- `template.map` — the locked authoring template: boundary reference brushes on
  the canonical corners at all levels, ready to copy per tile.
- 3–4 seed tiles: an open hall (Door on 2 lateral faces), a sealed cap (1 Door),
  **one full RampUp prefab** (two-level-tall: ramp rising a full 8 m level, low
  Door entrance, upper Door exit — authored as one map anchored at the low cell),
  and one shaft segment (ShaftOpen Up+Down with ladder/landing geometry).

### `labs/hex_tile_lab/`

Standard lab conventions. Load a tile by `TileKey`, render its geometry +
collider debug view, walk it first-person with the shared `observed_traversal`
controller. Keys to cycle tiles; overlay shows key/signature/collider count.

## Success criteria

1. Importer test: seed tiles parse; footprints snap exactly; a deliberately
   off-template fixture fails with the precise diagnostic.
2. Manifest coverage validator runs (green over the seed set).
3. **Ramp ascent proven**: first-person walk up the ramp prefab gains a full
   8 m level without jumping — the 29.7° slope is confirmed walkable by
   `step_character` (if it is not, stop and report; this invalidates a core
   arc assumption and the parent must re-plan).
4. Lab resets cleanly; workspace stays green.

## Evidence

Agent-viewed captures: each seed tile rendered, and a first-person sequence (or
GIF) ascending the ramp with visible level gain. Describe the slope feel/collider
behavior honestly.
