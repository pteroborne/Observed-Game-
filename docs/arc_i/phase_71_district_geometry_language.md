# Phase 71 — WFC Register Composition (geometry + light as one language)

**Fast-track ruling 2026-07-11 (user):** implement all nine lab registers in the
game, with **WFC as the composition mechanism** — the geometry grammar and the
light staging are one system, delivered as a module layer the hallway WFC
solves. This supersedes the original "geometry grammar per district" scope by
merging it with the register elements. Read [README.md](README.md) first.

## Read first

- `docs/light_and_line_arc_plan.md` — the nine registers and the 2026-07-11 lab
  verdicts (user review): backrooms, lumon, megastructure, wellshaft, babel,
  thinning **nailed the vibe**; shoji works but is over-busy (thin the slats,
  blades on the floor — not every surface); monolith and forerunner **need
  rework** (monolith's shaft never lands in frame; forerunner's volumetric fog
  provably did not render — the vol-on/vol-off matrix captures are
  byte-identical). Rework the reference scenes as part of this phase; the lab
  stays the source of the rig parameters.
- `crates/observed_facility` WFC (the hallway interior solver, Phase 47
  convergence discipline) and `game/src/wfc_interior.rs` +
  `game/src/teleport/geom.rs` — where a solved grid becomes hallway geometry.
- `docs/arc_h/phase_64_threshold_integrity.md` — the corpus-wide gate that MUST
  stay green: modules are decorative and never move walls, gaps, or thresholds.
- `game/src/screens/place/lighting.rs` as landed by `12cfb53` — the key-light
  machinery modules feed (its ceiling-occlusion defect is fixed by Phase 69's
  completion pass).

## Design (the module layer)

- **Module vocabulary** — one WFC tile attribute per register element, solved
  alongside the existing interior-wall layer:
  `SlatScreen` (shoji — a wall slat run + one low warm spot raking the floor),
  `SeamStrip` (forerunner — vertical emissive junction line at wall breaks),
  `PanelGrid` (backrooms/lumon — emissive ceiling tiles + even unshadowed fill),
  `Practical` (wellshaft — caged warm lamp, tight range, a pool),
  `ShelfRun` (babel — horizontal rib bands),
  `Bare` (thinning — nothing; the decay register),
  `VoidEdge` (megastructure — a dark opening onto implied depth with one faint
  distant light).
- **Adjacency rules make it architecture, not confetti:** slat runs are
  contiguous (3–6 cells) and never abut a threshold cell; panel grids fill
  contiguous ceiling regions completely (no lone panels); practicals keep a
  minimum separation (pools, not strips); seams sit only at wall-angle breaks;
  `Bare` probability grows with distance from the hallway entry (the thinning
  gradient, diegetic); threshold cells always solve clear.
- **District weighting:** each district's register identity (Phase 70 data)
  biases module weights — Reactor leans slats, Hollow leans panel-grid overlit,
  Foundry leans practicals — while WFC composes a coherent mix inside one
  corridor. That mix is the point of the ruling: elements meet without hand
  authoring.
- **Geometry grammar rides the same modules:** obtuse wall facets host
  `SeamStrip`; slat screens ARE the occluder grammar; verticality accents come
  as `VoidEdge` cells. No separate grammar system.
- **Determinism:** the module layer is a pure function of seed + hallway
  identity; pinned-seed WFC tests extend to modules.
- **Light budget:** at most one shadow-casting light per hallway (the dominant
  module's); everything else emissive or unshadowed fill.
- **Prop rules apply** (Arc F Phase 60): modules never cover a threshold, never
  steal a signal color, never fully block the light path to the floor — the
  luminance corridor enforces the last automatically.

## Files you may edit

`crates/observed_facility` (WFC module layer), `game/src/wfc_interior.rs`,
`game/src/teleport/geom.rs` (module → spawn data only, no wall/gap changes),
`game/src/screens/place/` (module spawners), `crates/observed_style` (module
treatments), `labs/lighting_lab` (shoji simplification; monolith + forerunner
rework), `game/src/tests.rs`. Do NOT touch `sim/`, threshold placement, or nav.

## Success criteria

- Adjacency invariants unit-tested (slat contiguity, panel completeness,
  practical separation, threshold clearance, Bare gradient monotonicity);
  pinned-seed module solutions stable; threshold integrity + routing green
  corpus-wide.
- Reworked lab scenes: monolith's shaft visibly lands as a sharp quad, the
  forerunner matrix captures are provably different vol-on vs vol-off, shoji
  reads as architecture (blades on the floor, walls mostly calm) — all viewed.
- Captures of three hallways in three districts, **viewed**: composed modules
  visibly read (a slat run casting blades, a panel region overlit, practicals
  pooling), every capture inside the luminance corridor.
- Full verification recipe green.
