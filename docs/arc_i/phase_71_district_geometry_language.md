# Phase 71 — District Geometry Language

**Objective:** the "Forerunner" idea, scoped and shipped: districts get a
geometric grammar — obtuse-angle wall breaks, slat/screen occluders, verticality
accents — applied to room shells and hallway dressing, so architecture reads as
belonging to a district the way Halo CE's architecture reads as belonging to a
civilization: by committing to a constraint. Read [README.md](README.md) first.

## Read first

- `docs/light_and_line_arc_plan.md` — reference table; the geometry-relevant
  registers (Halo obtuse angles, Japanese slats, Brutalist mass/reveals, Silo
  verticality) and Phase 68's diorama captures of each.
- `game/src/screens/place/shell.rs` + `game/src/teleport/geom.rs` — how room
  shells and hallway interiors are built today; where the polygon room support
  and gap/threshold machinery live.
- `docs/arc_h/phase_64_threshold_integrity.md` — the corpus-wide threshold gate
  that MUST stay green; geometry grammar is decoration and wall articulation,
  never threshold movement.
- `game/src/map_validation.rs` — the permanent gates.
- Phase 70's as-landed note — which district has which light register, so
  geometry and light reinforce the same identity.

## Design rulings (already decided)

- **Grammar, not layouts.** The sim's graph, room footprints, thresholds, and
  nav are untouched. The grammar articulates *within* the existing shell: wall
  faces may break into obtuse-angle facets, slat/screen occluders may stand
  proud of walls (clear of thresholds and walkable approaches, same rule as
  props), ceilings may step or coffer, vertical accent shafts may recess.
- **Occluders follow the prop rules** (Arc F Phase 60): never cover a threshold,
  never steal a signal color, always dimmer than interactables — plus the new
  one: never block the key light from reaching the floor entirely (the luminance
  corridor enforces this automatically).
- **Deterministic from seed + district + geometry**, like every spawner.
- **One grammar per district this arc** (matched to its light register); the
  system is data-extensible but the arc ships committed examples, not an editor.
- **Bot routing and threshold integrity stay green corpus-wide** — the Phase-64
  gate and routing tests are the hard constraint; if a grammar element cannot
  satisfy them it is cut, not special-cased.

## Files you may edit

`game/src/screens/place/shell.rs`, `game/src/screens/place/` (new grammar
module), `crates/observed_style` (grammar data per district),
`game/src/tests.rs`. Do NOT touch `game/src/teleport/geom.rs` gap/threshold
placement, `sim/`, or map generation in `crates/observed_facility`.

## Success criteria

- Captures: the same room archetype in each district showing geometry + light
  identity together (this arc's money shot), **viewed**; the as-landed note
  names each district's grammar in one sentence.
- The full map-validation suite (incl. Phase-64 threshold integrity, all
  decoherence versions) passes corpus-wide; bot routing tests green.
- Luminance corridor green for all new captures.
- A leak/teardown check covers the new grammar entities (state-scoped like all
  place geometry).
- Full verification recipe green.
