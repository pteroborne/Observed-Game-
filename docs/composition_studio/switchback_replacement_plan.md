# Replacing the switchback

Research and plan for removing `tile_source::verticals`' procedural switchback
entirely and authoring the facility's vertical circulation.

Two earlier attempts failed. Both are worth reading before starting, because the
reasons were structural rather than fiddly, and neither is obvious from the
code.

---

## 1. What exists today

### The generator

`crates/observed_authoring/src/tile_source/verticals.rs`

| item | what it is |
|---|---|
| `supported_switchback(hand)` | the flight geometry: two flights, a turn cantilever, piers, brackets, rails. Returns `(brushes, spine, deck)`. |
| `TowerHand` / `register_tower_hand(register)` | mirrors the whole tower about `x = 0`. **Per register**, which is the load-bearing detail - see §3. |
| `StairVertical` | `UpOnly` / `DownOnly` / `Through` - which vertical ports the cell carries. |
| `stair_access_map(register, doors, vertical, variant)` | wraps the switchback in walls, cuts doors, caps the floor or ceiling, emits ports, spine and deck. |
| `stair_segment_map` / `stair_top_cap_map` / `stair_bottom_cap_map` / `stair_landing_map` | the four public entry points. |

### The enumeration

`tile_source/catalog.rs:168-270` pushes **66 variants per register**:

- 3 with no doors - one per vertical connectivity
- 18 with one door - 3 verticals x 6 faces
- 45 with two doors - 3 verticals x 15 unordered pairs

At 11 registers that is **726 prototypes**, which matches what
`no_generated_stair_tower_strands_a_body` walks.

### The naming bridge

Source archetypes stay unique for manifest keys - `stair_segment`, `stair_top`,
`stair_bottom`, `stair_landing` - and `compatibility_archetype`
(`tile_source/mod.rs:104`) flattens all four to `stair_tower`. **That rewrite
runs only inside `compatibility_cells`.** Authored `.map` modules never pass
through it and keep the archetype they declare, so an authored tower declares
`stair_tower` directly.

---

## 2. The contracts a replacement must satisfy

Each is already a test. This is the acceptance list.

| test | contract |
|---|---|
| `the_switchback_stair_lands_flush_on_the_deck_above` | the climb tops out at exactly `TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP` (8.5 m). Short is a gap; proud is a lip past the 0.42 m autostep. |
| `every_generated_stair_tower_ships_a_followable_spine` | a spine exists, does not self-cross, starts on this cell's deck, and either ends on the deck above or - if capped - clears the lid by a body's height. |
| `every_generated_stair_tower_ships_a_walkable_deck` | **a deck path exists and every node sits at `FLOOR_SLAB_TOP`.** |
| `every_generated_stair_tower_is_physically_climbable` | the Rapier controller reaches the deck above within 0.35 m. |
| `no_generated_stair_tower_strands_a_body` | the geometric walk probe clears every tower in the library. |
| `every_switchback_support_meets_the_span_it_carries` | switchback-specific; retires with the switchback. |

**The deck contract is the one the last attempt missed.** The fifteen authored
towers shipped no deck path at all. `every_generated_stair_tower_ships_a_walkable_deck`
filters on `archetype.starts_with("stair_")`, so it never saw them - but the
objective bot consults the deck whenever it is off the climb, and without one it
is steered straight at the spine through whatever stands between.

---

## 3. The invariant that killed the last attempt

`tile_for` (`observed_match/src/hex_wfc/geometry.rs:758`) resolves a stair
tower's **register** from the column's base cell - but the *signature* and
`tile_variation_key` stay per cell. Its own comment says why the register is
pinned: *"give two cells in one column towers of different shapes and the lower
flight tops out under the upper cell's solid deck."*

So:

> **A tower's climb geometry may depend on the register and on nothing else.**
> Not on its doors, not on its vertical connectivity, not on its variant.

The switchback satisfies this by construction: `supported_switchback(hand)`
takes only the hand, and the hand comes from `register_tower_hand(register)`.
Doors are openings cut in walls that never touch the climb.

The fifteen authored towers violated it. `arc()` chose the sweep from the door
pattern, so two cells in one column with different doors got different flights.
**23 of 24 seeds stalled.** Forcing the arc to ignore the doors instead put
doors into the mass of the flight: **24 of 24**.

---

## 4. Why the switchback gets away with it, and what that implies

The switchback's flights sit at `x -80..60` - **inset**, well clear of the rim -
ringed by a grounded circulation deck that every door opens onto. The doors
never touch the climb, so the climb never has to know about them.

A perimeter helix hugging the wall cannot do that. Hence:

> **The replacement is an inset helix with a walkable perimeter ring**, not a
> variant of `forge::perimeter`'s wall-hugging one.

### Geometry, checked

At `outer = 0.75` of the hexagon with the band's inner edge at `0.55` of that,
sweeping a **fixed four faces** (240 degrees):

| quantity | value |
|---|---|
| run | 298 TB (18.6 m) |
| slope | 0.43 - under the validator's 0.65 |
| angle | 23 degrees - under the controller's 36 |
| ring width | 1.88 m - a body is 0.76 m wide |
| band width | 2.53 m |

The sweep is **fixed**, not chosen per door pattern. That is what makes it
column-constant. Handedness stays per register, as now, so districts still stack
their towers differently.

---

## 5. What must be removed

Only after the replacement passes §2, and in this order:

1. `verticals.rs`: `supported_switchback`, `stair_access_map`, and the four
   `stair_*_map` entry points. `ramp_map` **stays** - it is the straight
   `hall_ramp`, unrelated.
2. `catalog.rs:168-270`: the 66-variant enumeration.
3. `tile_source/mod.rs`: the four re-exports.
4. `compatibility_archetype`: the `stair_segment | stair_top | stair_bottom |
   stair_landing => "stair_tower"` arm, once nothing emits those names.
5. `tests.rs`: `the_switchback_stair_lands_flush_on_the_deck_above` and
   `every_switchback_support_meets_the_span_it_carries` - both name switchback
   internals. Their *contracts* move to the authored family, not their bodies.
6. `TowerHand` / `register_tower_hand` **stay** if the authored family keeps
   per-register handedness, which it should.

---

## 6. Slices

Each ships and is verifiable alone.

**A. The inset tower, one shape.** `forge::tower` rewritten: fixed four-face
sweep at `outer = 0.75`, perimeter ring at floor level, spine, **and a deck path
around the ring**. One source, no doors, `Through` only. Verify with the module
studio's walk probe and `every_authored_spine_can_be_walked_by_the_controller`.

**B. The three connectivities.** `Bottom` caps the floor, `Top` caps the ceiling
*and stops the climb below the lid* - the fault that shipped in the generated
family and stalled the soak bots. Same flight in all three.

**C. Doors.** Five orbits under sixfold rotation - none, one, two adjacent, two
apart, two opposite - times three connectivities = **15 sources**, compiling to
90 prototypes. Doors are wall openings and an apron onto the ring; the climb is
untouched. Base variants from 11 so compiled variants clear the generated 0..62.

**D. Prove column-constancy.** A test that two towers differing only in doors
produce **byte-identical flight brushes**. This is the invariant that broke the
last attempt, and it should fail loudly rather than as a stalled spectator.

**E. Weight up, then remove.** Raise the authored weight so facilities render
helices, run `survey_spectator_routes_across_seeds` (24 seeds) and the full
gate, then delete §5. Watch a bot climb one in the iso spectator view before
deleting anything.

---

## 7. Risks

- **The deck path needs three nodes.** The importer rejects two as "a straight
  line, which is the case that needs no path at all". A ring route genuinely
  bends, so this is satisfied honestly rather than by adding a waypoint.
- **`authoring_version 2` is stricter than the generated tiles.** The generated
  towers declare an `up` port at level 0 on a two-level cell, which is
  `PortOnInternalFace`; they pass only because version 1 skips the strict port
  checks. Authored towers put it at level 1.
- **`floor: "open"`, not `"ramp"`.** The ramp policy wants a surface over the
  cell centre; an inset helix leaves the centre a shaft.
- **Retiring 726 prototypes changes the catalog hash**, and with it LAN
  compatibility. Expected, and worth landing alone.
- **The 24-seed sweep is the gate that matters.** One seed proves one facility;
  the last attempt passed its own unit tests and failed 23 of 24 seeds.
