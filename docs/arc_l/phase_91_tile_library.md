# Phase 91 — Tile Library Authoring

**Wave 3, parallel with Phase 90 — content-only.** You own `assets/tiles/**` and
validator additions in `crates/observed_authoring/`. Do not touch solver code
(`crates/observed_facility/`), and do not change the manifest **schema** — if the
schema can't express a tile you need, stop and escalate to the parent. Arc
context: [../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Author the real tile library against the Phase 89 schema so the 3D facility
(Phase 92) has a complete, varied vocabulary: every port signature the solver can
demand has at least one tile, and the register/archetype spread supports the
game's visual language.

## Source material

- `assets/tiles/template.map` — the locked authoring template; start every tile
  from it. Footprints must snap exactly (the importer hard-fails otherwise).
- `crates/observed_facility/src/map_spec.rs` — `TraversalArchetype` (10) and
  `RoomRole` (11) vocabularies; `crates/observed_content/src/lib.rs` —
  `ArchitectureRegister` (9). Tiles are keyed
  `TileKey { archetype, register, variant }`.
- Interior-form inspiration: the wellshaft silo geometry
  (`docs/arc_i/phase_71b_wellshaft_verticality.md`), gantry expanse decks
  (`crates/observed_traversal/src/gantry.rs`), and the comfort design language
  (wide/tall halls, pillared pseudo-rooms).

## Deliverables

- **Hall tiles**: straights, corners, junctions (3- and 4-way) with authored
  interior variety per relevant archetype reading (incl. Colonnade pillars,
  Pressure narrowing, GantryExpanse openness within a tile).
- **Ramp prefabs for all 6 lateral directions** (or a canonical prefab + the
  rotation story if `observed_authoring` supports yaw-rotation of tiles — check
  with the parent before building rotation; authoring 6 explicit variants is the
  safe default).
- **Shaft tiles**: segments (ShaftOpen Up+Down), bottom caps, top caps, and at
  least one landing variant with a lateral Door — enough to compose a Silo well
  with entries at multiple levels.
- **Room blueprint footprints**: multi-hex room shapes (2–4 hexes; at least one
  2-level-tall atrium) with named exterior ports, covering the RoomRole needs
  (start, exit, keystone, monitor, control at minimum).
- Register variants where they matter most (at minimum: Megastructure, Wellshaft,
  Institutional readings for halls); the rest may share geometry and differ in
  style layer only.
- Manifest entries for everything; coverage validator extended if needed
  (validator code additions are in-scope, schema changes are not).

## Success criteria

1. Coverage validator green: every demandable port signature × archetype has ≥1
   tile; no manifest entry dangles.
2. Every tile loads in `labs/hex_tile_lab`, renders, and its walkable surfaces
   are traversable first-person (spot-check each family; walk every ramp
   direction).
3. Importer exact-snap passes on all tiles; workspace green.

## Evidence

Agent-viewed capture montage from `hex_tile_lab`: one capture per tile family
(hall straight/corner/junction, each ramp direction at least once, shaft
segment/cap/landing, each room blueprint). Describe interior variety honestly —
flag any tile that reads as a bare box, since the whole point of authoring is
deliberate interior form.
