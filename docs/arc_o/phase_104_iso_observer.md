# Phase 104 — Arc M Closeout & Isometric Observer Lab

**Status:** `[x]` — landed 2026-07-26.

Two deliverables: land the outstanding Arc M working tree, and build the
instrument the rest of Arc O is measured with.

## Arc M closeout

The 2026-07-26 per-district FPS investigation was sitting uncommitted across 18
files. It landed as six reviewed commits, ordered so each one compiles on its
predecessor:

| commit | what |
|---|---|
| `perf(facility)` | `route_within_cost` — cost-bounded A* with an admissible-heuristic early abandon, plus a 12-seed exactness corpus |
| `perf(sim)` | the guardian guard reorder, the bot route de-duplication, and the cached spawn-to-exit cost |
| `perf(sim)` | `derive_trim_for` — seam trim scoped to the cells a relayout changed |
| `fix(style)` | shadow-casting keys inheriting a degenerate zero-width cone from Hollow |
| `feat(perf)` | per-register, per-system and GPU-pass timing buckets; report schema v3 |
| `perf(view)` | the unconditional `Visibility` write that marked every cell changed every frame |

Evidence landed as measurements only. `FINDINGS.md` plus 26 `timings.json` is
350 KB; the five accompanying screenshot sets were 77 MB of near-identical
startup frames making an argument the timing tables already make, so they were
left out and `FINDINGS.md` records how to regenerate them.

## The lab

`labs/iso_observer_lab` solves a production-scale `HexWfcWorld` and renders it
orthographically from a true isometric angle, on two orthogonal channels:

- **Colour is the district.** Tinted by the district accent from
  `observed_style::architecture`.
- **Height is the archetype.** Corridors are thin slabs, junctions thicker,
  rooms thicker still, ramps a wedge-height block, shafts a tall column.

Stack and slice views, `[`/`]` seed cycling, `PageUp`/`PageDown` level focus, `R`
resolve, and a live census. `OBSERVED2_CAPTURE=<dir>` walks all five pinned seeds
and writes overviews, per-level slices and a census manifest.

### Two things the build surfaced

**`architecture_surface(register, Floor)` is register-blind.** Every base
register falls through to `surface(SurfaceRole::Plain)`, so the first render
painted the entire facility one grey. Only `LiminalGrid` has its own structural
family. The style crate's actual per-neighbourhood structural colour is the
district `accent` — the channel `PracticalFixture` already reads — so the lab
uses that. Consequence worth knowing before Phase 106: ten registers collapse
onto **seven** accent families (`ShadowScreen`/`Institutional` and
`FacetMonument`/`OverlitGrid` and `Monolith`/`Wellshaft` each share one), so
colour separates districts, not registers.

**The 3D orthographic default far plane is 1000 m.** A production facility's
diagonal exceeds that on its own, so the first capture clipped most of the map
away. `frame_camera` now returns an explicit far plane derived from the bounds.

## Revised after Phase 105

The lab was upgraded alongside the in-game map so it stays the arc's showcase
surface rather than falling behind it. It now draws what the tiles *compose*:
footprint width separates room from hallway from vertical, and bars mark every
open port pair, with risers joining floors. The heights and widths moved into
`observed_style::hex_sketch` — the lab and the game each only map their
archetypes onto its roles, so a change lands in both at once instead of drifting.

The baseline captures below were re-taken with that renderer so before/after
comparisons across Phases 106–113 stay like-for-like. The solve is byte-identical;
only the drawing changed. The census gained two lines, and the first one is a
sharper statement of backlog #13 than the shaft figure was:

- **61 % of the facility is vertical circulation** (ramps, ramp heads and shafts
  together, 3 368 of 5 495 cells), against 38 % hallway.
- **Rooms are 0 %** — 14 cells, rounding to zero.
- 5 297 lateral connections and 2 041 vertical ones.

## Baseline measurements

All five pinned seeds, production `28 x 20 x 10`, one solve attempt each:

| seed | cells | shaft | straight | room cells | rooms | base register mean region | liminal grid mean region |
|---|---|---|---|---|---|---|---|
| `0xa11ce3d000000008` | 5495 | 2608 (47.5 %) | 212 (3.9 %) | 14 | 9 | 1.39 | 48.0 |
| `0x00000000000c0ffe` | 5483 | 2595 (47.3 %) | 236 (4.3 %) | 14 | 9 | 1.42 | 47.9 |
| `0x0000000000000b0b` | 5483 | 2552 (46.5 %) | 215 (3.9 %) | 14 | 9 | 1.39 | 48.1 |
| `0x00000000000d00d0` | 5472 | 2543 (46.5 %) | 230 (4.2 %) | 14 | 9 | 1.38 | 47.8 |
| `0x5eed000000000001` | 5495 | 2561 (46.6 %) | 223 (4.1 %) | 14 | 9 | 1.41 | 47.8 |

Three findings, and the numbers are tighter than the estimates that opened the
arc:

1. **Shaft is 46.5–47.5 % of every placed cell.** The planning survey estimated
   ~39 % from the static weight table; measured placement is worse. Flat
   corridor is 3.9–4.3 %. Bug backlog #13 understated the problem.
2. **Base registers average 1.38–1.42 cells per disjoint region.** That is not a
   district, it is per-cell noise — around 400 fragments per register per solve.
   `LiminalGrid`, the one register with a real zone, averages ~48. Backlog #14,
   measured.
3. **14 room cells out of ~5490.** Nine blueprints stamp, but rooms occupy
   0.26 % of the facility.

The isometric slices make (2) unarguable: one olive Liminal Grid island sits in a
field of confetti, identically on every seed.

## Evidence

`docs/evidence/arc_o/phase_104/` — five stacked overviews, one mid-stack level
slice per seed, and `manifest.json` carrying the full census. The remaining 45
per-level slices are regenerable and were not committed:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_104"; cargo dev-run -p iso_observer_lab
```

## Hand-off to Phase 105

- The renderer to promote is `rebuild` plus `frame_camera` in
  `labs/iso_observer_lab/src/lib.rs`. The in-game version must read
  `HexPlayerMapKnowledge`, never `world.placements`.
- `PRESET_SEEDS` is part of the evidence contract. Changing it invalidates every
  before/after comparison in this arc.
- `todays_registers_fragment_into_many_regions` pins the pre-Phase-106 state and
  is **meant to be inverted** when spatial districts land — it asserts mean base
  region size is under 4.0, which is exactly what Phase 106 must break.
