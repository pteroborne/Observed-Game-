# District geometry: what the fiction is made of, and how to build it

The seven compositions in [docs/compositions/](compositions/) proved that
*arrangement* carries identity. They also showed the ceiling: composition can
only arrange what exists, and the corpus is one building in ten skirting boards.
This plan starts from the fiction — what geometry actually defines each place —
and works back to tiles the forge can emit under the seam contract.

Read it with [tile_authoring.md](tile_authoring.md) open: every recipe below is
named in the primitives that file documents.

---

## 0. Two capabilities everything waits on

Neither is a tile. Both gate several districts, and both are cheaper to decide
now than to work around seven times.

### 0.1 A ceiling cannot come below 80 units

`DOOR_TOP` is 72 units (4.5 m) and the storey is 128 (8 m). Any lid an author
drops has to clear the door aperture, so the lowest legal ceiling is about 80
units — 5 m. That is fine for Lumen and fatal for the Backrooms, whose entire
affect is a ceiling just above your head.

Three options, in increasing cost:

| option | what it costs |
| --- | --- |
| Accept 5 m. Author the dropped ceiling at 80 and let the Backrooms be roomy. | Nothing. Loses the defining move. |
| A second, lower aperture class — `PortClass::Crawl` at say 8..40 — usable only where both cells agree. | A port-class addition, solver compatibility work, and a catalogue-wide hash move. |
| Lower `DOOR_TOP` globally. | Every authored tile regenerates; every corridor in the game gets shorter. Not worth it for one district. |

**Recommendation: the middle one, but not yet.** Author the 80-unit lid first
and look at it. If 5 m reads as oppressive enough under the Liminal Grid
palette, the aperture class is never needed.

### 0.2 There is no way to author a glowing surface

`hull_surface_kind` classifies a hull by nothing but its height — under 0.4 m is
Floor, under 4 m is Trim, above is Wall. Nothing an author writes reaches the
material. Light exists only as `tile_light` point sources and the brushes
`ceiling_fixture` / `wall_fixture` emit beside them.

That blocks the defining move of **three** districts at once: the Forerunner
glowing seam, the Lumen cove, the Backrooms fluorescent run. All three are a
*line* of light, and today a line costs one fixture brush plus one point light
every few metres — which exhausts the 36-hull budget before the tile has any
architecture in it.

The fix is a per-brush tag in the `.map` source that survives compilation and
reaches `hull_surface_kind` as a fifth `SurfaceKind::Signal`. It is a contract
change and a hash move. It is also the single highest-leverage item in this
document: it turns "recessed channel" from twelve hulls into one.

**Do this before Wave 2.**

---

## 1. Halo / Forerunner — register 5, Facet Monument

### What the geometry is

Forerunner architecture is not "sci-fi temple". It is three moves:

1. **Everything steps back as it rises.** No mass is a plain box; each is a
   battered stack of slabs with a deep reveal between them. Scale reads from the
   number of steps, not from any human-sized object.
2. **Light comes out of the joints.** The reveal between two masses glows. There
   are no lamps.
3. **The axis terminates in a raised, centred object** you approach and then
   stand below. Symmetry is absolute and the terminus is always elevated.

### What we have

`rim_chamfers`, `hex_slab(chamfer_top, chamfer_bottom)`, `ColumnForm::Faceted`,
`CeilingForm::Coffered`, `trim_height` 24. The register is right; the tiles are
a corridor with a nice dado.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_straight_reveal` | three `band` courses per face at decreasing inset (112 → 104 → 96), each with a recessed channel above it | ~22 |
| `hall_gate_monument` | a `hall_straight` whose doored faces gain two `prism` piers and a lintel *inboard* of the face plane — the seam is untouched, the doorway reads as a gate | ~16 |
| `room_forerunner_dais` | three `hex_slab` courses at shrinking radius rising to a flat top; the thing an axis arrives at | ~10 |
| `hall_junction_6way_rotunda` | all six faces doored, `pylon(Faceted)` at each corner, coffered lid | ~26 |

**The one that matters:** `hall_gate_monument`. Ceremony is entirely about how
you pass through a threshold, and it is buildable today without touching the
contract.

---

## 2. Lumen — register 3, Overlit Grid

### What the geometry is

1. **The ceiling floats.** It stops short of the walls all round, and the slot
   is where the light is. This is the whole thing, and `hall_straight_soffit`
   already proves it works.
2. **No fixture is ever visible.**
3. **No scale cue and no shadow.** Nothing in the room tells you how big it is
   or which way you came in.

### What we have

`hall_straight_soffit` (SOFFIT_UNDERSIDE 56, SOFFIT_HALF_RUN 60), and nothing
else in the family. One soffit straight in a district of hard-cornered turns
reads as an accident.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_turn_60_soffit`, `hall_turn_120_soffit` | the soffit constants applied to `hall_turn` — the family completed | ~18 each |
| `hall_junction_3way_soffit` | same, at a crossing | ~20 |
| `hall_straight_cove` | the mirror: undercut the floor slab so a slot runs at skirting height and the floor appears to float | ~16 |
| `room_lumen_void` | soffit on all six faces, nothing else at all — the "there is nothing here" cell a Lumen district needs many of | ~14 |

**The one that matters:** the two soffit turns. A district is a family, not a
tile, and the user already confirmed this family reads.

---

## 3. Backrooms — register 10, Liminal Grid

### What the geometry is

1. **The ceiling is just above your head**, and it is a grid of identical
   panels with a light in every third one.
2. **Openings have no frame.** The wall simply stops. There are no doors.
3. **Rooms have no purpose.** Columns stand on a strict grid for no structural
   reason. Nothing is anywhere for a reason.

### What we have

An `expanse` footprint and a generated Liminal kit that is a floor, not a
district. The composition proved the *field* reads; the cell does not.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_expanse_dropped` | a full-hex lid at 80 units (see §0.1) with `ceiling_fixture` on a strict pitch — the Backrooms cell | ~14 |
| `hall_straight_threshold` | a straight whose aperture has no frame, chamfer or reveal: the wall stops | ~12 |
| `room_pillar_field` | nine `pylon(Square)` on a rigid grid, no other feature, no explanation | ~16 |

**The one that matters:** `hall_expanse_dropped`, and it is blocked on the §0.1
decision. Everything else in this district is a rounding error next to ceiling
height.

---

## 4. Silo — register 7, Wellshaft

### What the geometry is

1. **The stair is *in* the shaft.** Not beside it. The helix wraps the void and
   you see it spiralling away below you. Our silo composition puts the ramp in
   the neighbouring cell, which is a different building.
2. **Landings cantilever into the void**, with railings, at every level.
3. **You see many storeys of other people at once** — the sightline down the
   middle is the point of the whole place.
4. **Industrial fabric**: grating underfoot, exposed structure.

### What we have

`hall_gallery_wellshaft` (walkway ring, open middle, parapet) and
`hall_ramp_gallery`. The void exists; nothing descends through it.

### Tiles to author

| tile | recipe | hulls | risk |
| --- | --- | --- | --- |
| `hall_gallery_cantilever` | a gallery with one wedge extended into the void as a platform, parapet round its lip | ~30 | low |
| `hall_gallery_grating` | the ring floor as twelve narrow `prism` slats instead of six wedges | ~34 | low, tight on budget |
| `hall_gallery_stair` | the gallery walkway carrying a stair segment that descends around the ring, plus `stair_node` entities and a vertical port, so a stack of six is one continuous helix round one void | ~34 | **high** — this is a traversal-graph change, not a decoration |

**The one that matters:** `hall_gallery_stair`, and it is the hardest thing in
this document. It is also the difference between "a shaft" and "the silo".
Budget it as its own wave and expect bot-traversal fallout, the way
[the descent defect](bug_backlog.md) went.

---

## 5. BLAME! — register 6, Megastructure

### What the geometry is

1. **Scale with no reference.** Achieved by *removing* human-sized detail, not
   by adding big things. A Nihei panel has no doorknobs.
2. **Structure that passes through** — a conduit enters one edge and leaves
   another with no origin and no terminus. It is not for anything.
3. **Accretion and ruin.** Floors end in a broken edge over a drop. Stairs
   arrive nowhere.
4. **Deep black recesses** the eye cannot resolve.

### What we have

`hall_gallery_megastructure` and the highest `trim_height` (48) of any register.
The composition already reads, because staggering the shafts was free.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_gallery_broken` | a gallery missing two of its six wedges, the walkway ending in a raw edge over the void — makes "storeys simply absent" read as ruin rather than as a bug | ~22 |
| `hall_conduit` | two twelve-sided `regular_polygon` cylinders crossing the cell from face to face at head height, with just enough clearance to pass under | ~18 |
| `hall_straight_megabeam` | ceiling replaced by one beam 3 m deep down the length, the void above it unlit | ~14 |

**The one that matters:** `hall_gallery_broken`. It costs less than any other
tile here — it is a gallery with two prisms deleted — and it converts an
existing composition from "incomplete" to "ruined", which is the whole register.

---

## 6. Library of Babel — register 8, Infinite Gallery

Borges is unusually literal about geometry, which makes this the easiest
district to author faithfully.

### What the geometry is

1. Hexagonal galleries — **we get this for free**, and no other project does.
2. **Twenty shelves, five to a side, on four of the six walls.** The remaining
   two open onto hallways.
3. **A very low railing** round the air shaft. Low is the word that does the
   work: it is what makes the shaft frightening.
4. In each hallway, **a closet for sleeping upright**, and **a spiral stair
   going up and down out of sight**.

### What we have

`hall_gallery_infinite` with three doors. The shaft, the ring and the parapet.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_gallery_shelved` | five `band` courses on each of four faces, over the existing ring and parapet | ~32 |
| `hall_gallery_lowrail` | parapet dropped from 20 units to 8 | ~26 |
| `hall_closet_spiral` | the hallway cell: a niche one body wide on one side, a stair opening on the other | ~20 |

**The one that matters:** `hall_gallery_shelved`, at 32 of the 36-hull budget.
It is the most directly quotable tile in the corpus — the text specifies the
count — and it will be the tightest.

---

## 7. Feudal Japan, empty — register 9, Thinning

### What the geometry is

1. **Post and beam. No wall carries anything.** The structure is a grid of
   slender columns; what stands between them is a screen, and a screen may be
   absent.
2. **The engawa**: a raised timber veranda running outside the rooms under a
   deep overhanging eave. The building's edge is a *threshold zone*, not a line.
3. **The floor changes level** to divide space, instead of walls.
4. **The eave soffit is ribbed** — from below, the roof is a rhythm of rafters.
5. **Emptiness is the subject.** Everything above is a way of defining a room
   without enclosing it.

### What we have

The Thinning register — zero trim, fewest members, flat lid — which is the right
*instinct* and produces a bare corridor.

### Tiles to author

| tile | recipe | hulls |
| --- | --- | --- |
| `hall_engawa` | perimeter deck one step (16 units) above a lower middle, slim `pylon` at the deck edge, deep soffit over the deck only | ~24 |
| `room_shoin_bay` | eight slim columns on a strict grid, four faces with no wall at all, `band` rafters across the lid | ~22 |
| `hall_step_platform` | two `hex_slab` floors at 0 and 16 units with one long step between them — level change as the only divider | ~14 |

**The one that matters:** `room_shoin_bay`. It is the exact inverse of
`room_pillar_field` in §3 — the same emptiness, one with structure that explains
it and one without — and having both makes each read as a choice.

---

## 8. Build order

Batched so each wave is one catalogue hash move rather than fifteen.

| wave | contents | why here |
| --- | --- | --- |
| **1** | `hall_gate_monument`, the two soffit turns, `hall_gallery_broken`, `hall_step_platform` | Five tiles, no new capability, one hash move. Each is the cheapest tile in its district and four of the five are edits of tiles that already exist. Proves the approach before anything expensive. |
| **1.5** | §0.2, the authored emissive surface | Unblocks the reveal, the cove and the fluorescent run. Do it after Wave 1 so there is something to light. |
| **2** | `hall_straight_reveal`, `hall_straight_cove`, `hall_gallery_shelved`, `hall_gallery_lowrail`, `hall_engawa`, `room_shoin_bay` | The district-defining tiles that need §0.2 or are simply larger. |
| **2.5** | §0.1 decision, then `hall_expanse_dropped`, `hall_straight_threshold`, `room_pillar_field` | The Backrooms wave, gated on the ceiling-height call. |
| **3** | `hall_gallery_stair`, `hall_gallery_cantilever`, `hall_gallery_grating`, `hall_conduit` | The traversal wave. `hall_gallery_stair` will move the bot graph; keep it away from everything else so the fallout is attributable. |
| **4** | `room_forerunner_dais`, `hall_junction_6way_rotunda`, `hall_closet_spiral`, `hall_straight_megabeam`, `room_lumen_void` | Filling out. Each district gets its second and third tile so the solver has something to choose between. |

After each wave: re-pin `committed_arc_s_catalog_identity_is_pinned`, re-render
the seven compositions from `docs/compositions/`, and look at them in
`section: "half"`. The compositions are the regression test for this work — if a
new tile does not change how its district's composition reads, it was not worth
the hash move.

## 9. What constrains all of it

- **36 hulls per cell.** `hall_gallery_shelved` at 32 and `hall_gallery_grating`
  at 34 are the tiles that will hit it. Raising it again needs the phase-110
  argument rerun, not a bigger number.
- **Everything must be inboard of the face plane.** Chamfers, reveals, piers,
  shelves, rafters — all of them stop short of the seam, or the tile stops being
  compatible with the corpus. This is why `hall_gate_monument` works: the gate is
  a frame *behind* the aperture, not a change to it.
- **The aperture is 72 wide by 64 tall and is shared.** Monumental gates, crawl
  spaces and Borges' hallways all pass through the same hole.
- **Every wave is a LAN lockout.** The simulation fold moves on any catalogue
  change, so peers on the old build fail the handshake. Batching is not tidiness,
  it is the whole reason to batch.
- **Registers are dialect, not identity.** The compositions established that.
  Nothing in this plan should be solved by adding a register.
