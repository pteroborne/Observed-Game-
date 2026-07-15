# WFC Procedural Generation Lab

This lab is the proof surface for the versioned liminal-map generator. Its default
view runs `observed_facility::wfc::generate_liminal_map_v2` and renders both axes of
the procedural design catalogue:

- Room fill is the room's stable architecture register: Shadow Screen, Monolith,
  Overlit Grid, Institutional, Facet Monument, Megastructure, Wellshaft, Infinite
  Gallery, or Thinning.
- Corridor line color is its traversal archetype: Straight, Long, Pressure, Climb,
  Maze, Chicane, Gantry Expanse, Wellshaft, Colonnade, or Orthogonal.

All labels are production-safe. A white room border marks Start or Exit. Major
Gantry Expanse and Wellshaft courses use thicker lines. The on-screen legend states
every color/weight meaning so the debug visualization obeys the legibility contract.

## Controls

- `N` / `P` — next/previous deterministic map seed.
- `V` — toggle between catalogue v2 and the retained v1 role-only topology
  regression.
- `R` — reset to catalogue v2, seed 0.
- `M` — toggle the catalogue view and the archived abstract tile-WFC feasibility
  demo.
- `Space` — randomize the archived tile-WFC seed while that view is active.

Changing seed, revision, or mode despawns every owned visualization entity before
respawning. The lab therefore exposes reset behavior without leaking state between
proofs.

## Catalogue-v2 generation contract

`generate_liminal_map_v2` first preserves the validated v1 topology pipeline, then
adds explicit corridor identities and deterministic design assignments. Each map
has four to six architecture regions. Every region is connected in the room graph
and contains at least three rooms. Corridor designs carry a compatible traversal
archetype and a stable generation key, so later threshold rewiring changes
attachments without rerolling a Place's design.

The v1 generator remains available in the lab with `V`. This regression path makes
the version boundary visible and keeps the original role-only output testable.

## Corpus tests

`cargo test -p wfc_proc_gen_lab -- --nocapture` runs a deterministic 50-seed corpus.
The catalogue-v2 checks prove:

- regenerating a seed produces an identical `MapSpec`;
- every map contains four to six connected architecture regions; and
- the corpus exercises all nine architecture registers.

The retained v1 suite also checks deterministic generation, dense room IDs, full
`MapSpec::validate()` coverage, monitor paging capacity, competitive objective
coherence, and prints a room/edge/monitor summary table for manual review.

## Archived interior WFC adapter

`src/hallway_wfc.rs` remains a thin compatibility re-export of
`observed_facility::wfc::generate_interior_walls`. Its smoke test exercises the
production corridor-interior implementation rather than a second copy.
