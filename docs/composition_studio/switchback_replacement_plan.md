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

## 5b. Settled: `levels: 2` is a reservation, and the exemption was too narrow

**Reading 1 confirmed by counting the solver's own output.** In a production
solve: 257 shaft columns, the tallest 10 levels, **543 adjacent placements
against 23 gapped**. A shaft column places a tower at *every* level, so each
tower climbs exactly one, and `levels: 2` is a reservation letting the flight
poke half a metre into the cell above to land flush.

So the `up` port belongs at the one-level boundary, 8 m - where the generated
tower puts it - and the authored tower's level-1 port at 16 m was wrong.

Moving it back exposed the real fault, which was not in the tower at all:

```rust
let prefab_ramp_handoff = module.kind == ModuleKind::Cell
    && port.face == HexFace::Up
    && port.class == PortClass::RampOpen;      // <- RampOpen alone
```

A two-level prefab handing off upward does so through what is, on its own
footprint, an internal face. `source.rs` exempts that - but only for
`RampOpen`, because the ramp prefab was the only two-level tile when the
exemption was written. **A stair tower is the other one**, in precisely the same
position, and its `ShaftOpen` handoff was rejected. The generated towers declare
the same port and pass only because `authoring_version 1` skips these checks -
the same exemption that let their capped variants ship a staircase into a
ceiling. The rule now covers both classes.

The gap that let this ship is closed by `the_climb_reaches_the_port_it_
advertises`. Writing it first was worth it twice over: it failed on the real
bug, and then failed again on my own wrong expectation - I asserted the climb
should meet the port, when the contract is that it lands one floor slab *above*
it, on the deck. The port marks the boundary; the deck is where a body stands.

## 5c. Slice C attempted and reverted: doors still break the spectator gates

Built and reverted. The tree is green at slice B; this is what was learned.

The implementation did what the design says: five door orbits times three
connectivities, doors cut as `door_wall` openings onto the ring, **the climb
untouched** - no `arc()`, a fixed sweep at a fixed `OUTER_SCALE`. All 10 tower
unit tests passed, including the new invariant.

**And `hex_spectator_physically_leaves_spawn` and
`hex_spectator_route_is_physically_completable_without_guardian_pressure` both
failed.**

What this rules out, and what it does not:

- **Not the column-constancy invariant.** `towers_differing_only_in_doors_share_
  one_climb` passes, and it is not vacuous: reintroducing the exact failure of
  the previous attempt - deriving `outer_scale` from the door count - fails it
  immediately. The climb is genuinely identical across all five orbits.
- **So the cause is something the doors bring that is not the flight.** The
  candidates, none yet tested: the ring deck is still door-independent and may
  not lead a body from a *door* to the foot; the doors' aprons may not meet the
  ring; the sixfold rotation now compiles 90 prototypes into buckets that were
  holding 3, which reshuffles `weighted_select` across the whole stair family
  the way adding towers did before.

The last one is worth suspecting first, because it has already happened twice in
this work: adding candidates to a bucket changes which *generated* tower renders
elsewhere, and the failure surfaces far from the change. The way to tell them
apart is the 24-seed sweep with the authored weight at zero - if seeds still
stall with the authored towers present but never selected, the reshuffle is the
cause and the doors are innocent.

**Do not retry slice C by adjusting geometry.** Run that experiment first.

## 5d. The real invariant: a column can mix authored and generated towers

The experiment that settles it needed no doors at all. Slice B's three towers
were switched from `rotation_policy: "none"` to `"sixfold"` - **the same
prototypes, 3 becoming 18** - so the bucket reshuffles exactly as adding doors
would, with no new geometry whatsoever.

Result: both named spectator gates pass, and **1 of 24 seeds stalls**, on

    TileKey { archetype: "stair_tower", register: "liminal_grid", variant: 4 }

Variant 4 is **generated** - authored towers compile to 66 and up. And
`no_generated_stair_tower_strands_a_body` passes, so that tower is perfectly
walkable on its own.

### Why

`tile_for` pins a column's **register**. It does not pin the *tile*:

```rust
catalogue.select(archetype, register, signature, world.tile_variation_key(coord))
```

`tile_variation_key` is **per cell**. So within one shaft column, one cell can
draw an authored tower and the next a generated one - and their climbs are
entirely different shapes, so the lower flight tops out under the upper cell's
deck and a body climbing stops dead. This is precisely the failure
`tile_for`'s own comment describes; the comment assumes the register is enough
to prevent it, which was true while every tower in a register had the same
shape.

### What this changes

**Slice E is wrong.** "Raise the authored weight, then remove the generated
family" cannot work: any weight strictly between none and total lets a column
mix the two. The authored family and the switchback cannot coexist in the same
bucket at all.

The replacement must be **atomic** - author the full 66-signature demand and
delete the generated enumeration in one change - or the mixing must be made
impossible first, by extending `tile_for` to resolve the *tile* per column
rather than only the register.

The second is smaller than it sounds and is worth pricing first: `tile_for`
already computes a column identity (`HexCoord { level: 0, ..coord }`) for the
register lookup, and passing that same identity as the variation key for
`stair_tower` would make the whole column draw one tile. That would also retire
the "climb depends only on the register" invariant, since the climb would then
depend on the column - which is what the geometry actually needs.

## 5e. A column-constant variation key is not enough

Tried and reverted. `tile_for` was changed to key `select` on the column
identity it already computes for the register lookup, instead of on the cell:

```rust
world.tile_variation_key(identity)   // was: (coord)
```

Run against the same experiment - slice B's towers at `sixfold`, 3 prototypes
becoming 18 - and the result was **identical**: the same seed, the same cell,
the same generated `variant: 4`.

The reason, which should have been seen before implementing it: **a column's
cells do not share a signature.** One has no doors, the next has one, the next
two. `select` is keyed `(archetype, register, signature)`, so those cells look
up **different buckets**, and a column-constant key just indexes different
`Vec`s holding different candidates. Making the *draw* column-constant does not
make the *family* column-constant.

Fixing it properly means the family has to be chosen per column and the tile
picked within it - a real change to `Catalogue`'s shape, not a key swap.

### Where that leaves the plan

Atomic replacement (§5d option 1) is the reliable path, and this is the second
measured argument for it. The two families cannot coexist in the bucket, and the
cheap ways to make them coexist do not work:

- weight tuning: any weight short of total lets a column mix
- column-constant key: different signatures, different buckets

So: author the full 66-signature demand, delete the generated enumeration, and
land both in one change. There is no testable intermediate state, which is
unwelcome but is what the measurements say. The 24-seed sweep is the gate.

## 5f. Candidate 1 tested: the ring deck did not lead a body in from a door

The first of §5c's three candidates, run as an experiment rather than argued.
**It was a real fault**, independent of the bucket reshuffle in §5d, and it
would have shipped with slice C.

`a_tower_climbs_from_every_face_a_door_could_be_on` places a body just inside
each of the six faces in turn - where a door on that face would put it - and
steers it with the bot's own rule against the production controller. Doors do
not exist yet, but the climb is fixed and door-blind by construction, so a face
that fails without a door fails with one.

**6 of 18 approaches failed**, the same two faces on all three towers: the
climb's own face (`start`) and the next one round (`start + 1`). Both bodies
came to rest 0.41 m outside the flight's outer edge - one capsule radius -
pinned against the side of the climb.

Two causes, both in `ring_deck`:

- **The path ended on the rim and never ran in to the foot**, though its own
  comment said "then in to its foot". `DeckPath::step_toward` returns the goal
  itself once the body and the goal are nearest the same leg, so wherever the
  path stops, the rest of the way is a straight line - and a straight line from
  the rim to the foot crosses the flight.
- **Two of the six faces carried no node**, so a body entering on one was
  handed a leg across the cell and cut the chord through the stairwell.

The fix is a full six-face ring, ordered away from the climb's own face, ending
at `Extent::foot()` - the same point `spine()` starts from, so the two ends of
the route come from one place instead of two calculations that can drift. The
inward leg comes off `start + 5`, which the sweep does not cover, so it runs
over open floor. A polyline is not a loop, so one face - the climb's own - walks
the long way round; that is paid in walking rather than in stalling.

### Why nothing caught it

`walk_spine_as_the_bot_does` starts a body just inside a lateral door - but only
for tiles that *have* one. A tower has none, so it fell to the `None` arm and
started the body **on the spine**, which is the exact mistake that harness's own
comment warns about ("Starting on the spine is what made the first version of
this harness pass everything: it never exercised the approach"). The ring deck,
the whole reason the climb is inset, had never been walked by anything.

### Left standing: the foot sits 7.6 cm inside the floor aperture

Found while measuring, not fixed. The spine's first node is at 0.2906 of the
corner ray; `hex_opening_slab` cuts the floor at 0.30. The capsule is 0.38 m in
radius so the lip carries it and nothing falls - but this is a margin that holds
by accident, which is the shape of fault slice B called out in the generated
family. It wants an assertion, and probably a foot moved outboard of the
aperture, before the doors land.

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
