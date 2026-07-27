# Phase 110 — District-Exclusive Tilesets

**Status:** `[x]` — landed 2026-07-27. Closes bug backlog #20 and the remainder
of #13.

Nine of the ten districts were built out of the tenth's geometry.

## What was actually wrong

`compatibility_cells()` generated **one** library — from `institutional` — and
relabelled every tile `generic`. `Catalogue::select` tries the exact
`(archetype, register, signature)` first and falls back to `generic`, and since
only Liminal Grid had authored `.map` modules, the fallback was not a safety net
under a per-district kit: it *was* the kit, in nine districts out of ten.

So a district could be lit differently (Arc I), composed differently (Phase 107),
and stacked with different stair towers (Phase 109), and the walls, the dado
rail, the junction pylons and the doorways were still institutional everywhere.
That is why districts kept reading as one place.

`register_style` had carried per-register trim heights and pylon radii since Arc
L — nine distinct pairs, from Thinning's no trim at all to Megastructure's 3 m
dado band. None of it had ever reached the game.

## What changed

Every register generates its own library and keeps its own key. The `generic`
copy stays underneath as a net for a register nothing was generated for, and
authored `.map` modules still outrank both — the layering is unchanged; what
changed is that the middle layer now exists.

Liminal Grid joins `REGISTERS`. It had been left out for two arcs on the
reasoning that the one district with hand-authored modules needs no generated
kit — which held exactly as long as the authored corpus covered every demand the
solver could make. It stopped holding the moment `Expanse` arrived: the district
whose identity *is* open space had no exact tiles for it and fell through to a
fallback drawn in another district's style. That was backlog #20. The generated
kit sits *under* the authored modules, not in place of them, and a test pins that
both authored Liminal layouts are still reachable.

Cost: 302 tiles to roughly 1 500, and 0.3 s to 1.4 s of parsing in a debug build.
It is a `OnceLock` filled once per process, and no `.map` files are committed —
the generated library has always been in-memory, which is the reason it exists
rather than hundreds of mechanically derived files in the repository.

## Both exemptions are gone

`merged_authoring_corpus_covers_every_wfc_geometry_demand_exactly` carried two,
and both were load-bearing:

- **`stair_tower`**, because no authored tower existed anywhere. It hid backlog
  #13 for an entire arc — half the facility was one procedural switchback and the
  coverage gate said nothing.
- **`expanse`**, added in Phase 108 for Liminal Grid, and recorded as backlog #20
  precisely so it could not repeat the first one's silence.

The assertion is now unconditional, and it asks for something stronger than it
used to. It no longer accepts `generic` as coverage for a register: **every
district must have exact tiles of its own for every demand.** A tile keyed
`generic` satisfies the solver but is drawn in another district's style, so a
facility can be fully covered and still read as one place — which is exactly what
was happening.

The test is renamed to say what it now checks:
`every_district_covers_every_wfc_geometry_demand_with_its_own_geometry`.

Its hull budget is now `COLLIDER_STRIDE` rather than a hand-picked 28. That is
the real ceiling — the projector reserves a fixed collider ID range per cell and
refuses a tile that overruns it — and the old number was a Liminal-only stylistic
bound that a six-door expanse legitimately exceeds.

## Two gates the phase turns on

**Exclusivity is a property of the selector, not a convention.**
`a_district_exclusive_tile_never_answers_for_another_district` asks every foreign
register for every Liminal-Grid tile's signature, across sixteen variation keys
each (selection is weighted, so a single probe can miss a leak that only shows on
some rolls), and checks none of them is handed back. A widened fallback or a
stray `generic` relabel is the way this breaks.

**And the selector is actually reached.**
`every_placed_cell_is_built_from_its_own_district` projects a full production
solve and asserts that **no** collider is drawn from another district's kit.
Catalog-side coverage proves the tiles exist; this proves the facility uses them.
Stair towers are exempt by design and covered separately — Phase 109 chooses them
for the whole column from its base cell's register, so they need not match the
cell they stand in.

## Measured

Seed `0xa11ce3d000000008`, production `28 x 20 x 10`, 5 473 cells.

The archetype census is **identical** to Phase 109 — 17.7 % shaft, 23.3 % corner,
21.5 % expanse — and that is the correct result. This phase changes which tile
fills a cell, not which cell the solver chooses; a census that moved would mean
something had leaked into the solver.

What moved is the thing the census cannot see:

| | before | after |
|---|---|---|
| districts with an exact kit of their own | 1 of 10 | **10 of 10** |
| colliders drawn from a foreign district's kit | nearly all | **0** |
| generic-fallback coverage exemptions | 2 | **0** |

## What a player will actually notice

Trim height and pylon radius, layered over what the earlier phases already
delivered. Thinning has no dado at all; Overlit Grid's is 0.25 m; Megastructure's
is 3 m. Junction pylons run from 0.5 m to 1 m in radius. On its own that is
subtle. Against Phase 106's contiguous regions, Phase 107's composition profiles
(45 % corner in Overlit Grid against 29 % expanse in Liminal Grid), Phase 109's
handed towers and the per-district lighting, it is the layer that was missing.

**Honest limit:** this is one generated kit *parameterised* per district, not ten
authored kits. The differences are real and they are per-district, but they are
two numbers wide. Deeper geometric identity — different interior motifs, district
landmarks — is authoring work, and the `register_scope` mechanism and the
exclusivity gate are now in place for it. It is not scheduled; it belongs with
the post-MVP landmark-archetype item.

## Evidence

`docs/evidence/arc_o/phase_110/`.

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_110"; cargo dev-run -p iso_observer_lab
```

## Hand-off to Phase 111

- `GATE_SEED` was re-pinned to `0xd9c1_e6e5_fd29_f054`. Every register's geometry
  changed, so the old seed's route lost its ramps. Expect to re-run
  `scan_gate_seeds` after any phase that touches geometry or weighting; the gate
  asserts a bot can walk such a route, not that one seed produces one.
- Rooms are next, and they have the same shape of problem one layer up:
  `blueprint_cell_archetype` returns `sanctuary` for every role and every cell
  (backlog #15), which is the room-scale version of "one kit for everywhere".
