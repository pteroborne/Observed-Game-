# Phase 105 — Full-Screen Isometric Tac Map

**Status:** `[x]` — landed 2026-07-26.

The Phase 104 renderer promoted into the game, and the bottom-right survivor
sketch deleted. Tab now opens a whole-facility isometric view.

## What changed

`game/src/hex_wfc/tacmap.rs` (392 lines of hand-built `Node` hex outlines) is
gone. `game/src/hex_wfc/view/map.rs` replaces it, wired into the same three
registration points and driven by the same toggle plumbing — `bindings.tac_map`
→ `intent.toggle_map` → `runtime.map_open` — which needed no changes at all.
`PageUp`/`PageDown` still browse floors through `browsed_level`, but the browsed
floor is now the *focus* level inside a stacked view rather than the only floor
drawn: off-focus levels stay visible at reduced brightness so a survivor can see
how the floors relate.

The map draws on its own render layer (2; layer 1 belongs to the portal
previews) through its own orthographic camera, activated only while the map is
open, and carries its own directional key. That last part is deliberate:
borrowing the world's lighting would make the sketch's brightness depend on
whichever district the runner happened to be standing in.

## The fog-of-war contract

This is the risk the phase exists to manage. The lab this renderer came from
draws ground truth, because its job is to audit what the WFC composed. The game's
version must draw only what the team has discovered.

The module reads `HexPlayerMapKnowledge` and indexes `world.placements` solely at
cells that knowledge already contains. Four gates hold it:

| gate | proves |
|---|---|
| `the_map_draws_no_cell_the_team_has_not_discovered` | solid count is bounded by known cells, and far below the placed-cell count |
| `every_solid_the_map_draws_stands_over_a_hex_the_team_knows` | inspects the spawned `Transform`s — every one stands over a known hex |
| `survivor_knowledge_is_keyed_per_team_not_per_player` | a player's map *is* its team's map; a rival on another team resolves elsewhere |
| `the_map_never_leaks_past_the_hex_state` | geometry, camera and projection cache are all gone after the state exits |

**Honest limits.** The rival half was originally asserted inside the second gate
and was silently vacuous — `drawn ⊆ known` already implies it. A fixture guard
caught that, and probing showed why: in the headless test app the bots never
leave spawn, so both teams know the same four cells and no rival-only knowledge
exists to test against. Rather than contrive a fixture, the rival property is
asserted structurally instead (third gate), and the plan-position comparison in
the second gate deliberately does not attempt to catch a same-column,
wrong-level leak — the count gate bounds that one.

## Two defects found by capturing

**Markers floated into the cell above.** The you/exit tokens were lifted a full
`TILE_LEVEL_HEIGHT` above their cell, which is exactly where the next level's
cell sits, so they rendered inside it and vanished. The fix was not a smaller
offset but a better design: the marked cell is now **recoloured** to its signal
treatment rather than having a token floated over it. A token is occludable by
the stack; a recoloured cell never is. This is also the honest 3D equivalent of
the corner sketch's `@` and `X` glyphs, and it removed a function.

**The map had no light of its own.** It was relying on whatever the world rig
happened to put on the default render layer. Fixed with a dedicated key on the
map layer.

## Shared geometry

`observed_hex::prism_hull` is new: the twelve corners of a cell-local hex prism,
the single source both the lab and the game now draw from. The quantized hexagon
is not regular, so a locally re-derived footprint drifts. The game turns those
corners into a mesh through the existing `ConvexRenderMesh` path — a prism is a
convex hull, and its 60-degree facets sit well outside the 45-degree smoothing
threshold, so the crisp faceting costs nothing. The lab keeps its own flat-shaded
assembly on purpose: a diagram wants hard facets unconditionally, not facets that
happen to survive a crease threshold.

## Evidence

`docs/evidence/arc_o/phase_105/survivor_map.png` — captured through the existing
`OBSERVED2_CAPTURE_HEX_WFC_MAP` driver, whose open/shoot frames were pushed from
900/950 to 2400/2460 because at frame 900 the spectator bot has barely left spawn
and the map showed five cells: a correct fog-of-war proof but a useless visual
gate.

The committed frame shows 7 of ~5490 cells known across three floors, traversed
bright against glimpsed dim, two districts distinguishable by colour, archetype
distinguishable by height, and the runner's own cell in signal cyan. Everything
undiscovered is absent.

```powershell
$env:OBSERVED2_CAPTURE_HEX_WFC_MAP = "docs/evidence/arc_o/phase_105/survivor_map.png"; cargo run -p observed_game
```

## Hand-off to Phase 106

- The map already tints by district, so the moment `register_for` becomes
  spatial the survivor's sketch shows contiguous neighbourhoods with no further
  presentation work. That is the intended Phase 106 evidence shot.
- Ten registers collapse onto seven accent families
  (`district_for_architecture` maps them onto six districts plus the LiminalGrid
  override), so the map separates *districts*, not registers. Phase 110 must
  decide whether that is sufficient.
- The map rebuilds on a content signature and keeps its geometry while closed,
  so reopening is free. If Phase 106 makes districts change under relayout, the
  signature must learn about it.
