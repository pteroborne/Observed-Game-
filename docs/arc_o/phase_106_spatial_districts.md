# Phase 106 — Spatial Districts

**Status:** `[x]` — landed 2026-07-27. Closes bug backlog #14.

Districts are now places. `register_for` was a per-hex lottery; it is a lookup.

## What changed

`crates/observed_facility/src/hex_wfc/relayout.rs`. `LiminalGridZone` — the one
7x7 rhombus per level that made Liminal Grid the only real district — is
generalized into `DistrictSite`: one anchor per register per level, seeded, and a
cell belongs to the nearest anchor on its own level.

Nearest-anchor ownership is what makes a district contiguous, and it is
contiguous *by construction* rather than by tuning: every cell on the path
between a cell and its anchor is at least as close to that anchor, so a district
cannot be split.

Three deliberate choices inside that:

- **Ten anchors per level, one per register.** Every authored register is
  therefore exercised on every floor, which matters because the authoring
  coverage gate assumes it, and a production floor of 560 cells gives a district
  around fifty-six — big enough to be a neighbourhood, small enough that a floor
  holds ten of them.
- **A district's base anchor is level-independent, with a small per-level
  drift.** The same neighbourhood occupies roughly the same ground on every
  floor, so it can be navigated toward vertically as well as laterally, but it
  leans as you climb rather than making each floor a carbon copy.
- **Distance is lateral only.** Districts are defined per level; folding in a
  vertical term would let a cell several floors away win ownership over a
  neighbour on the same floor.

## The churn this fixed on the way past

The old `register_for` folded the relayout **generation** into its key. Every
committed relayout therefore re-rolled the register of every cell in the pocket —
32 to 64 cells — including cells whose placement had not moved at all. And
because `make_candidate` counts an architecture-only difference as a changed
cell, that churn flowed straight into `cell_revisions`, into
`HexPlayerMapKnowledge::last_confirmed_revision`, into `HexMatchSnapshot`, and
therefore into the digest the LAN wire compares. Cosmetic noise was reaching the
determinism surface.

No test caught it: the old drift test checked that the Liminal zone list and the
blueprint-anchor override survived a commit, not that unmoved cells kept their
register. `districts_do_not_drift_across_relayout_generations` now asserts that
**zero** cells are re-assigned across a committed relayout.

The geometry-only fallback relayout also used to step a cell to the next register
in an array, which would punch a hole in the middle of a district. It now hands
the cell to a **neighbouring** district, so the change reads as a boundary
shifting by one cell.

## Measured

Same five pinned seeds, same solve, production `28 x 20 x 10`. Cells per register
and the number of disjoint regions those cells form:

| register | before: cells / regions / mean | after: cells / regions / mean |
|---|---|---|
| Shadow Screen | 528 / 385 / **1.4** | 658 / 10 / **65.8** |
| Monolith | 571 / 412 / **1.4** | 415 / 10 / **41.5** |
| Overlit Grid | 550 / 401 / **1.4** | 693 / 10 / **69.3** |
| Institutional | 585 / 414 / **1.4** | 561 / 10 / **56.1** |
| Facet Monument | 538 / 383 / **1.4** | 564 / 10 / **56.4** |
| Megastructure | 566 / 415 / **1.4** | 619 / 10 / **61.9** |
| Wellshaft | 538 / 397 / **1.4** | 359 / 9 / **39.9** |
| Infinite Gallery | 578 / 396 / **1.5** | 291 / 10 / **29.1** |
| Thinning | 561 / 402 / **1.4** | 476 / 10 / **47.6** |
| Liminal Grid | 480 / 10 / 48.0 | 859 / 10 / **85.9** |

**Ten regions per register is exactly one contiguous region per level.** That is
the partition working perfectly, not approximately. Wellshaft's nine is a floor
where neighbouring anchors squeezed its district out entirely — legitimate, and
the reason the tests assert a mean rather than a fixed count.

Mean region size went from 1.4 to 29–86: a twenty- to sixty-fold change.

## Evidence

`docs/evidence/arc_o/phase_106/` — the district view for each of the five pinned
seeds, plus a schematic slice, a stacked overview, an inspector frame and the
census manifest.

The comparison to look at is `seed_0_districts.png` against Phase 104's
`seed_0_level_3.png`: the same floor, the same seed, confetti before and named
neighbourhoods after.

Note the district view is the **solid** mode. The console schematic spends its
colour channel on mutability — green will not rewire, red can — so it cannot also
carry district identity; the capture switches to the solid dev view for this one
frame, which is exactly the question that view exists to answer.

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_106"; cargo dev-run -p iso_observer_lab
```

## Hand-off to Phase 107

- Districts are now spatial, so per-district composition profiles have something
  coherent to act on: amplifying shafts in Wellshaft will produce a vertical
  *neighbourhood*, not scattered vertical cells.
- `district_sites` is `pub` and seed-stable. Phase 107 can key its weight
  profiles off the same anchors rather than re-deriving a partition.
- The register→district-palette mapping still collapses ten registers onto seven
  accent families, so two adjacent districts can share a colour. That is now
  visible rather than hidden, and Phase 110 has to decide whether the style
  mapping widens.
- `DISTRICTS_PER_LEVEL` is currently the full register list. If Phase 107 wants
  fewer, larger districts, that is the one constant to change — but the authoring
  coverage gate assumes every register appears.
