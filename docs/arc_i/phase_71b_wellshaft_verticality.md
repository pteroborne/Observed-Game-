# Phase 71b — Wellshaft Verticality (the Silo register enters the game)

**Objective:** bring the lighting_lab's wellshaft register (scene 7 — warm pools
descending into buried dark) into the played game as a **vertical connector
hallway**: a shaft you descend from an elevated entry at the top to a ground exit
at the bottom, caged practicals pooling at each level. User ruling 2026-07-11:
verticality is the point — the descent must FEEL like going down a well, not like
walking a decorated corridor. All Arc H standing invariants and the
falsifiable-evidence rule apply.

## As built (current — supersedes all earlier iterations)

A WFC `CorridorRole::Vertical` edge resolves, at the simulation-to-geometry
projection boundary, to a role-owned `HallwayFlavor::Wellshaft`. Ordinary variation
rolls can never select it; Gantry variations and locked objective edges keep their
original pieces. The interior is a pure function of the edge (same edge → identical
shaft), and `sim/`, the graph, rooms, and match rules are untouched — this is a
hallway piece: geometry projection, traversal arena, presentation, nav.

**Geometry — a hex silo with a cantilevered spiral stair.**
- A central **hexagonal pillar** (visual `Cylinder` resolution 6, radius
  `WELL_SHAFT_PILLAR_RADIUS`) is the structural core. Collision is a conservative
  square core (`WELL_SHAFT_PILLAR_COLLISION_HALF`) held inboard of every tread, so
  the walkable band never catches on it.
- Six **landings** at the six hex directions, one **radial threshold bridge** each.
  Only the bottom (level 0) and top (level 5) bridges are live graph thresholds —
  elevated entry (`DoorGap::floor_y = WELL_SHAFT_HEIGHT`) and ground exit
  (`floor_y = 0`). The four middle bridge heads terminate in physical wall and
  render as rubble-dark X-braced **sealed service bays**, never cyan false exits.
- The descent is a **spiral stair cantilevered from the pillar**: per flight, eight
  radial-slab treads (`WELL_SHAFT_STEPS_PER_FLIGHT`) spread contiguously across the
  60° the spiral turns between adjacent landings, each closing onto the tread below
  (`WELL_SHAFT_TREAD_CLOSURE`, no open risers). Each tread's inner edge overlaps the
  pillar (it visibly springs from the core) and its outer edge overlaps the landing
  band (shared, continuous floor) — so the flight reads as one connected staircase,
  not a row of floating blocks. Seven legal risers per flight keep `STEP_RISE`
  within the controller's `step_height`.
- A short **guard rail** (`WELL_SHAFT_GUARD_HEIGHT`) lines the outward edge of every
  mid-flight tread so the descent cannot be walked off by accident. The first and
  last treads of each flight stay open, since they abut the landings and must
  connect to the threshold bridges.

**Lighting — buried dark, per-district undertone.** One caged warm practical per
level, staggered around the shaft, tight range so each pool is an island; exactly
the top level's practical casts shadows. `apply_place_atmosphere` crushes the global
ambient to ~4 and collapses fog toward black for `is_wellshaft()` places — but keeps
the **district's ambient and fog hue**, so a Foundry shaft's dark reads warmer than a
Reactor shaft's while neither leaves the buried-dark register. The practical pools
are tinted by the district `key_color`. Pillar, floor, walls, ledges, treads and
guards all use the one grey `SurfaceRole::WellshaftStone` treatment; district identity
rides the warm pool tint + ambient/fog hue, not a bright albedo.

**Verified:**
- Lab (`wellshaft_lab`): the production `observed_traversal` controller descends and
  ascends the spiral without jumping; treads are contiguous and spring from the
  pillar; determinism and no-leak reset pinned.
- Game: `production_controller_traverses_the_projected_wfc_wellshaft_both_ways`,
  `a_vertical_wfc_edge_projects_the_bidirectional_wellshaft`,
  `wellshaft_bot_route_follows_the_authored_spiral_in_both_directions`,
  `generated_wfc_vertical_roles_project_the_hex_pillar_wellshaft`, and the render
  test all green. Threshold-integrity + routing corpus green with the flavor in it.
- Evidence (viewed): `docs/evidence/wellshaft_lab/` (descent / ascent / plan) and
  `docs/evidence/wellshaft_game_hex/` (top / bottom / bird's-eye) show the connected
  spiral wrapping the pillar. Rec-709 luminance on the game stills sits in the
  buried-dark register with warm pools lifting the highlights — top
  `(p05 0.0101, p50 0.0140, p95 0.0177)`, bottom `(0.0071, 0.0135, 0.0199)`,
  bird's-eye `(0.0027, 0.0140, 0.0715)` — a mid-lit frame, never all-black or
  all-white, matching lighting_lab scene 7.
- **Live bot-POV descent (the previously-open gap, now closed):**
  `OBSERVED2_CAPTURE_WELLSHAFT_POV=<dir>` forces a bot-POV walkthrough to begin
  inside a wellshaft (`BotPovCaptureRequest::new_wellshaft`), so the earlier
  "staging a guaranteed wellshaft edge remains unsolved" gap is resolved. The bot
  spawns on the top landing and the existing descent piloting walks it down the
  spiral to the ground exit; `docs/evidence/wellshaft_pov/wellshaft_descent.gif`
  (frames 0–7) shows the descent, no repeated-position stall.

## Read first (source pointers)

- `labs/wellshaft_lab/src/shaft.rs` — the source of truth for the rig parameters and
  the deterministic geometry; the game mirrors its constants/derivation.
- `game/src/hallway.rs` — `WELL_SHAFT_*` constants and the tread/stair helpers.
- `game/src/teleport/geom.rs` (`HallwayFlavor::Wellshaft` branch) — projection into
  `PlaceGeom` decks/gaps; `game/src/teleport/transition.rs::place_arena` turns decks
  into collision solids.
- `game/src/screens/place/shell.rs::spawn_wellshaft_structure` — rendering.
- `game/src/screens/place/lighting.rs::spawn_wellshaft_lighting` +
  `game/src/screens/match_runtime/ambience.rs` (`is_wellshaft()` block) — the register.
- `docs/arc_h/phase_64_threshold_integrity.md` — the corpus-wide gate the flavor joins.

## Files owned by this piece

`game/src/hallway.rs`, `game/src/teleport/geom.rs` (Wellshaft branch only),
`game/src/screens/place/shell.rs`, `game/src/screens/place/lighting.rs`,
`game/src/screens/match_runtime/ambience.rs`, `crates/observed_style` (the
`WellshaftStone` treatment), `game/src/bot.rs` (wellshaft routing case),
`game/src/tests.rs` + `game/src/teleport/test.rs`, `labs/wellshaft_lab`. Do NOT touch
`sim/`, `crates/observed_match`, room geometry, the module solver, or threshold
placement.

## Revision history (superseded, kept for context)

1. **First pass (2026-07-11):** square ring ledges + perimeter stairs. Geometry and
   selection were sound but the register failed — a flat bright-orange wash instead
   of warm pools in dark.
2. **Register redo (2026-07-11):** dropped ambient, switched to grey concrete,
   tuned the pools to islands. Register fixed; geometry moved to a flat-sided hex
   shaft.
3. **Hex-pillar redesign (2026-07-12):** hex pillar + six radial bridges + discrete
   stair flights (replacing the "continuous ramp" language of the redo).
4. **Spiral grounding + guard rails (2026-07-12, current):** the flights were reading
   as free-floating disconnected blocks. Reworked to a spiral **cantilevered from the
   pillar** with **contiguous treads** and **guard rails**; added per-district
   undertone to the buried-dark register; renamed `WellshaftRamp` → `WellshaftStone`.
   The as-built section above is the single source of truth.
