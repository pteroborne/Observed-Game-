# Phase 71b — Wellshaft Verticality (the Silo register enters the game)

**Objective:** bring the lighting_lab's wellshaft register (scene 7 — warm pools
descending into buried dark) into the played game as a **vertical connector
hallway**: a shaft you descend by ring ledges, entry at the top, exit at the
bottom, caged practicals pooling at each level. User ruling 2026-07-11:
verticality is the point — the descent must FEEL like going down a well, not
like walking a decorated corridor. Read [README.md](README.md) first; all Arc H
standing invariants and the falsifiable-evidence rule apply.

**Revision 2026-07-11:** the landed piece was subsequently directed toward a
flat-sided hexagonal shaft, continuous ramps, no dark deck-border treatment, and
the reference scene's saturated orange architectural language. This revision
supersedes the earlier square ring/stair presentation rulings below while retaining
their traversal, lighting, determinism, and selection constraints.

## Read first

- `labs/lighting_lab/src/scenes/silo.rs` — the reference rig: ring ledges every
  ~5 m, one caged warm practical per level staggered a quarter-turn, tight light
  ranges so each pool is an island, concrete grey, heavy fog below.
  `docs/evidence/lighting_lab/scene_07_wellshaft.png` is the look to hit (note:
  it currently fails the luminance floor — the in-game version must pass the
  corridor, so lift the pools slightly).
- **The gantry is your structural precedent — reuse, don't reinvent:**
  `game/src/teleport/geom.rs` (search `Gantry`): `DeckSeg` walkable raised
  platforms, `gap.floor_y` for a raised exit, the mount stair, the understory.
  `game/src/screens/place/shell.rs::spawn_gantry_decks` renders decks +
  emissive rim strips ("chosen, readable irreversibility" — deck edges must be
  lit and the commitment legible before a drop).
- `game/src/bot.rs` + `game/src/evidence/capture/bot_pov.rs` — deck-aware bot
  routing (deck waypoints, jump legs, understory fall recovery). The bot must
  traverse the wellshaft in BOTH directions or the routing tests will tell you.
- `game/src/hallway.rs` — `HallwayTemplate`/`HallwayFlavor`, how variations are
  selected per edge; `game/src/screens/place/modules.rs` — the Phase-71 module
  layer (wellshaft places should NOT also get wall modules; exclude them the
  way gantry places are excluded, via non-empty `decks`).
- `docs/arc_h/phase_64_threshold_integrity.md` — the corpus-wide gate. Your new
  template joins the corpus; it must pass from day one.
- Capture environment rules: [README.md](README.md) rule 2 (OBSERVED2_BOTS=all,
  Vulkan for world captures, drivers re-assert state). For bird's-eye
  diagnostics remember the fog-relax lesson (see `capture_maze_progress`).

## Design rulings (already decided)

- **A new `HallwayFlavor::Wellshaft` template**, compact footprint (~10×10),
  entry gap at the TOP (its `floor_y` = the shaft's rim height), exit gap at
  the BOTTOM (`floor_y = 0`), total descent ~2.5–3× WALL_HEIGHT. This inverts
  the gantry's raised-exit pattern; the machinery (`floor_y`, `DeckSeg`) is
  already there.
- **Descent by ring ledges** (DeckSegs at stepped heights spiraling down), with
  a walkable perimeter stair path connecting adjacent levels in BOTH directions
  — drops are allowed and readable (lit rims, gantry rule), but the climb back
  must exist or bidirectional hallway traversal breaks.
- **Lighting is the register:** one caged warm practical per level, staggered
  around the shaft (copy the lab's stagger), tight range (~6–7), intensity
  high enough that every ledge's edge reads (the lab scene failed the
  luminance floor; fix that here). Exactly ONE shadow-casting light: the top
  level's practical. District palette tints; drained/klaxon transforms must
  still read.
- **Selection is deterministic and sparing:** a small deterministic fraction of
  eligible connector edges (pure function of seed + edge; consider biasing
  toward Foundry-district edges for register alignment) become wellshafts.
  Never the exit-locked edge, never where the gantry already is.
- **Sim untouched.** The graph, rooms, match rules, and `sim/` do not change.
  This is a hallway piece: geometry projection, traversal arena, presentation,
  nav. The Phase-71 reroute-invariant fix in `observed_match` must not be
  touched.
- **Determinism is sacred:** template interior a pure function of layout seed;
  pinned-seed tests; headless == interactive.

## Files you may edit

`game/src/hallway.rs`, `game/src/teleport/geom.rs` (new flavor branch only —
do not touch grid/maze/chicane branches or gap threshold logic),
`game/src/screens/place/shell.rs` (deck rendering reuse), `game/src/bot.rs`
(only if deck routing needs a wellshaft case), `game/src/screens/place/lighting.rs`
(wellshaft rig branch), `game/src/tests.rs`, `game/src/map_validation.rs`
(only to make sure the corpus covers the new flavor). Do NOT touch `sim/`,
`crates/observed_match`, room geometry, or the module solver.

## Success criteria

- Threshold-integrity + map-validation corpus green WITH wellshaft templates in
  it; routing/bot tests green; hallway determinism tests cover the new flavor
  (same seed → identical ledges/lights).
- The no-leak discipline: wellshaft entities all `PlaceGeometry` +
  `DespawnOnExit(Match)`; the place-teardown test still passes.
- Bot traversal proof: a bot-POV run whose log shows entering a wellshaft at the
  top and exiting at the bottom (or vice versa), no repeated-position stall.
- **Evidence, captured and VIEWED by you, described in your report** (the
  falsifiable-evidence rule is binding):
  1. Eye-level from the top rim looking down the well — pools descending.
  2. From the bottom looking up — ledge silhouettes against the rim light.
  3. A bird's-eye diagnostic (fog relaxed) showing the ring structure.
  Every capture must pass the luminance corridor (floor AND ceiling).
- Full verification recipe green — **check exit codes, not output snippets**:
  `cargo fmt --all`, `cargo clippy --workspace --all-targets` (zero warnings),
  `cargo test --workspace` (exit 0).
- Append your as-landed note to this doc. Do not commit — the parent session
  reviews evidence first.

## As landed (2026-07-11, revised after a register-and-evidence redo)

The geometry and selection landed correctly on the first pass; the **register did
not** — the first build painted the shaft bright orange and never dropped the
global ambient, so it read as a flat orange wash (the opposite of "warm pools in
buried dark"), and the as-landed note claimed success against captures that
showed the failure. The redo below fixes the register and re-grounds the evidence.

**Kept from the first pass (geometry + selection — sound):**
- `CorridorRole::Vertical` resolves at the geometry projection boundary to a
  role-owned `HallwayFlavor::Wellshaft`; ordinary variation rolls cannot select
  it; Gantry variations and locked objective edges keep their original pieces.
- The hexagonal shaft descends 12 m across five regrouping levels and four
  rotating continuous ramps, each backed by thirty 0.1 m collision steps; raised
  landings have tested body headroom; high entry / ground exit reuse
  `DoorGap::floor_y`. Deterministic per edge. Traversability is proven
  structurally by `a_vertical_wfc_edge_projects_the_bidirectional_wellshaft`
  (continuous shallow treads + headroom + entry landing supports the spawn).
- Spectator/evidence bot routing no longer assumes "any decks ⇒ Gantry".

**Register redo (the actual fix — A–F):**
- **A. Ambient dropped.** `apply_place_atmosphere` now forces the global ambient
  to ~4 (from the district's 70–80) and collapses fog toward buried-black when
  `is_wellshaft()`, matching lighting_lab scene 7. The practical lamps become the
  only real light, so pools read as islands. Applied per-frame with the existing
  easing (a smooth darkening as you cross into the shaft).
- **B. Grey concrete, not orange.** The `WellshaftRamp` treatment is now matte
  grey stone (`~0.15` grey), not a glowing orange surface — the warmth comes from
  the lamps pooling on the stone. The shaft **floor and walls** were switched from
  the district-tinted materials to the same concrete, so the whole well reads as
  one dark stone volume lit only by its practicals (district identity rides the
  warm pool tint + fog).
- **C. Pools tuned to islands** (range 5.5, intensity 165k) so the concrete
  between landings falls to dark; exactly the top practical casts shadows.
- **D. Reframed all three captures** over the central void: a downward descent
  from the entry lip, an up-and-across shot from the floor anchored on a warm
  landing, and the plan-view bird's-eye.
- **E. Evidence re-grounded honestly.** Every capture was viewed, and the
  luminance corridor was recomputed on the actual PNGs (Rec-709, the game's
  thresholds): top `(p05 0.0135, p50 0.0140, p95 0.0381)`, bottom
  `(0.0135, 0.0142, 0.0208)`, bird's-eye `(0.0133, 0.0140, 0.0608)` — all pass
  floor **and** ceiling. (The first note's `p50 ≈ 0.51` numbers were the bright
  wash passing the ceiling by luck; the corridor only catches all-black/all-white,
  so a flat mid-bright frame slips through — the eye, not the number, is the
  arbiter.) No live bot-POV descent capture was added: staging a guaranteed
  wellshaft edge for the 120-frame driver remains unsolved, so traversal rests on
  the structural test above, not a moving-bot capture. That gap is called out
  rather than papered over.
- Wellshafts still receive no Phase-71 wall modules.
- **F. Verification green (exit codes):** `cargo fmt --all` (0),
  `cargo clippy --workspace --all-targets` (**0 warnings**),
  `cargo test --workspace` (exit 0, 192 suites ok).
