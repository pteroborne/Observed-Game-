# Authoring a stair tower

`stair_tower` is the most-walked element in the facility — ~64 demands in a
production solve — and until now it has had **zero authored coverage**. Every
shaft renders from `tile_source::verticals`, the procedural switchback whose
spine "stalled all four soak bots on the first run of this code".

This is the plan for giving it authored geometry, and the traps that shape it.

## The naming trap, and the way through

`placement_tile_archetype` returns the string `"stair_tower"` for every
`HexArchetype::Shaft` cell, and `Catalogue::select` looks that up directly —
`(archetype, register, signature)`, falling back only to
`(archetype, "generic", signature)`. There is no aliasing at that layer.

But no module is *called* `stair_tower`. The bridge is
`tile_source/mod.rs:104`:

```rust
"stair_segment" | "stair_top" | "stair_bottom" | "stair_landing" => "stair_tower"
```

`tile_source/catalog.rs` states the intent: *"Source archetypes stay unique for
manifest keys; runtime compatibility maps them to `stair_tower`."*

**But that mapping applies only to the generated library.**
`compatibility_cells` rewrites `tile.key.archetype` through
`compatibility_archetype` as it converts; authored `.map` modules never pass
through it and keep whatever archetype they declare.

So an authored tower declares **`archetype: "stair_tower"` directly**. The four
source names exist to keep the *generated* library's manifest keys unique before
they are flattened; they are not the authoring interface.

## Why this is an addition, not a replacement

`tile_for` picks a tower **per column, from the base cell's register** — not per
cell. Its own comment says why: *"give two cells in one column towers of
different shapes and the lower flight tops out under the upper cell's solid
deck… a body climbing meets the underside of the floor above and stops."* That
makes *partial* coverage the dangerous case.

It does not arise here. `Catalogue::new` keys
`(archetype, register, signature)` to a **`Vec`** of prototypes, and `select`
runs `weighted_select` over them by `weight`. An authored tower with the same
signature as a generated one joins the same bucket and is chosen by weight — so
every signature keeps a complete fallback and no column can be stranded. The
`weight` field is the dial for how often the authored shape appears.

## The contract

- **`levels: 2`**, not 1. The generated tower declares 2 because "the upper
  flight intersects the first metre of the cell above", which closes the
  standard floor-slab offset without a runtime pose rewrite.
- **The climb tops out at exactly 8.5 m (136 TB)** — `TILE_LEVEL_HEIGHT + 0.5`,
  pinned by `the_switchback_stair_lands_flush_on_the_deck_above`. Proud of it is
  a lip past the 0.42 m autostep, which physically stops a body stepping back
  onto the flight.
- **Open through the ceiling.** A perimeter helix cannot run under a solid deck:
  at 2.2 m headroom a body only reaches 5.3 m of the 8 m needed. `tile_for`
  already assumes the opening — *"a tower's stairwell opening is the hole the
  flight below arrives through"*.
- **`floor: "open"`.** The `ramp` policy wants a surface over the cell centre
  spanning most of a level, which assumes a full-width ramp. A perimeter climb
  leaves the centre open — the same declaration `silo_ring` makes.
- The floor is an **aperture slab** where the cell connects `down` (the hole the
  flight below arrives through) and solid where it does not (`stair_bottom`).

## What is already built

`forge::perimeter` produces a validated, walkable helix with a stair spine:
two triangles per face so the radial seams stay level, a landing spanning the
exit seam to the centre, and nodes from the deck at the foot to the landing at
the head. The tower reuses that machinery — the differences are the aperture
floor, no ceiling cap, and the vertical-only signature.

## Verification

The walk probe can prove a tower climbs before it ever reaches a match:
`module-studio` reports `WALK clear — N samples, 8.0 m climbed`, and the
corpus-wide test asserts every generated module parses and validates. That is
the check the switchback never had.

## Blocked: the bot cannot walk a perimeter climb

Attempted twice, 2026-08-02. Recorded so it is not re-derived a third time.

### The diagnostic lied, twice

`hex_spectator_route_is_physically_completable_without_guardian_pressure`
printed `first_step` = `spawn_route.cells[1]` **whatever happened** — a constant
with nothing to do with where the walk failed. Both investigations chased the
tile it names (`stair_tower` variant 18) before noticing it never changes.
Fixed in `7b60d53`; it now reports the cell the bot is actually stuck in.

### What is actually wrong

The stalling tile is `hall_ramp_perimeter_180` — **one of ours**, in the corpus
since it was authored. Adding the towers only reshuffled `weighted_select` onto
it. Three faults, each found by fixing the one in front of it:

1. `vertical_command` consulted the spine only for `ShaftOpen` ports, so every
   `RampOpen` cell went to `ramp_walk_dir`, which assumes `hall_ramp` variant
   0's straight shape. Fixed in `7b60d53`.
2. `finish_stair_command` bailed unless `archetype == Shaft`, so a body part-way
   up a ramp was never "still climbing". Fixed in `7b60d53`.
3. **Still open.** The bot now climbs to 7.67 m of the 8.5 m and stops. Whatever
   is between there and the landing has not been found.

Progress is measurable, which is the useful part: stuck at spawn (never moved
at all, bit-identically across three catalogs) → 7.0 m → 7.2 m → 7.67 m.

### The general hazard

Sharing a port signature is what lets the solver treat two tiles as
alternatives for one demand. **It is not a promise they are the same shape.**
Anything that reads geometry from an archetype rather than from the tile breaks
the moment a second shape appears — and the whole point of an authored family
is to be that second shape. Worth auditing for before authoring more variety.

### The tower family, when it can land

Fifteen sources: five door orbits under sixfold rotation (none; one; two
adjacent, one apart, opposite) times three vertical connectivities. The sweep
starts at whichever corner begins the longest door-free run, capped at four
faces — two opposite doors are the tight case, leaving runs of two, which is
still 120 degrees at slope 0.50. Base variants from 11, so compiled variants
start at 66 and clear the generated family's 0..62; overlapping keys are not an
error but they cost the ability to tell which tower a diagnostic is about,
which is exactly what was needed here.

All fifteen validate and walk. They are out of the corpus only because the
perimeter ramps they would stand beside are not bot-traversable yet.

The three no-door authored towers (commit `ed48e22`) remain in the corpus and
are unaffected: they are `Shaft`, so the bot always followed their spine.
