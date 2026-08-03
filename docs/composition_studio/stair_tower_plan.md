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

## Blocked: replacing the generated family stalls a spectator

Attempted 2026-08-02 and reverted. Recorded here so it is not re-attempted
blind.

Full replacement needs the door-bearing towers too - the generated family is 63
variants per register, covering each connectivity with no doors, one door, and
two. Fifteen authored sources cover the same ground (a door makes the tower
rotate sixfold, and two doors have only three orbits), and all fifteen validate
and walk.

**But adding them fails
`hex_spectator_route_is_physically_completable_without_guardian_pressure`.** A
solo spectator stalls climbing out of a `Shaft` cell with doors on faces 1 and 3.

The important detail is *which* tile it stalls on:

    TileKey { archetype: "stair_tower", register: "infinite_gallery", variant: 18 }

**Variant 18 is a generated tower, not an authored one.** Authored sources use
variant 0, and sixfold expansion gives them compiled variants 0-5; the generated
family is pushed with explicit variants up to 62. So the stalling tile is one of
the procedural switchbacks.

What appears to have happened: `Catalogue::new` collects prototypes into a `Vec`
per `(archetype, register, signature)` and `select` runs `weighted_select` over
it. Adding candidates changes which entry a given variation key lands on, so the
facility now renders a *different* generated tower than before - and that one
cannot be climbed. Lowering the authored weight to 6 does not help, which is
consistent with a reshuffle rather than a frequency effect.

If that reading is right there is a **latent unclimbable variant in the
generated stair family**, exposed rather than caused by this change, and it
would be reachable today under any perturbation of the catalog. That is worth
confirming before anything else: it is a shipped bug, not a blocker of our own
making.

Next step is not more geometry. It is to find which generated variant stalls -
bisect by adding a single dummy prototype to the bucket to force the same
reshuffle - and decide whether to fix or remove it. Only then does replacement
become safe.

The three no-door authored towers (commit `ed48e22`) are unaffected and remain
in the corpus.
