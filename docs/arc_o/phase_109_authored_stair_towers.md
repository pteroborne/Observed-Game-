# Phase 109 — Stair Towers & Vertical Rebalance

**Status:** `[x]` — landed 2026-07-27. Closes bug backlog #13 and #19.

Vertical circulation was one shape everywhere, and it could not become anything
else, because the thing that walked it had the shape memorised.

## Why this was blocked, and what unblocked it

There were zero authored `stair_tower` modules, so every `Shaft` cell in every
district resolved through the `generic` fallback to one procedural switchback.
That is backlog #13. The obvious fix — author more towers — was not available,
and the reason is worth stating precisely because it is the whole phase:

The objective bot climbed by **hardcoded numbers**. `stair_command` followed the
switchback by rise thresholds and named tread points. `stair_lateral_command`
crossed its floor by sixty lines of per-face local coordinates plus a
rectangle-crossing test against the guarded opening. Every one of those constants
was true of exactly one tower. A second tower shape would have been geometry no
bot could walk — and the coupling was tight enough that even *fixing* the
existing tower broke the steering, which is why a known half-metre defect sat
behind an ignored test for an entire arc.

So a tile now ships **two lines beside its brushes**:

- `StairSpine` — the climb, from the foot of the first flight to the deck above.
- `DeckPath` — the walkable floor, routed around the stairwell.

Both are derived from the same constants as the geometry, so they cannot drift
from it. The bot follows whichever the cell it stands in declares and knows
nothing else about towers. Everything after this was possible only because of it.

## Four things that had to be measured, not tuned

Each of these stalled the soak, and each has a general reason behind it:

**Nearest segment, not nearest node.** A target chosen by proximity to a waypoint
flips as you walk away from it — the ~31,000-tick spin the old code's comment
recorded. Distance to a *segment* falls and rises exactly once along a leg, so
the choice advances with the body. The authoring gate rejects any spine whose
stretches pass within a body's width of each other, so that choice is never
ambiguous.

**Height counts triple.** Measured plainly, a body standing on a switchback's
deck is *nearer* the flight passing overhead — 2.8 m through the ceiling against
3.8 m along the floor — and gets steered into the underside of its own staircase.
A follower on walkable surfaces cannot treat a metre up as a metre sideways.

**Arrival is explicit.** A spine's last node lies flat on the deck it arrives at,
so "am I still climbing?" cannot be answered by height. Without an explicit end,
a body standing exactly on the exit node was handed the point it already
occupied, forever.

**The climb starts at the flight, not out on the deck.** The spine originally
included the run across the floor to reach the foot. Two descriptions of the same
floor is one too many: a body crossing the deck came within capture range of the
spine's flat lead-in, was taken to be on the climb, and was steered down it —
through a floor pier. Getting to a climb is the floor's business.

A fifth is worth recording as an anti-lesson. Softening the step where the flight
meets the turn landing, by extending the landing back to meet it, made things
*worse*: the slab's underside then occupied the headroom a climbing body needs.
It was caught only because it broke the soak with the tower unmirrored, which is
how it was told apart from the mirroring work landing at the same time.

## The lip is gone

With the steering decoupled, the switchback lands flush on the deck above instead
of half a metre proud, and `the_switchback_stair_lands_flush_on_the_deck_above`
is no longer `#[ignore]`d. The `on_incoming_flight` box in `finish_stair_command`
went with it — it existed only to compensate for that overshoot, and any "still
climbing" test that stays true on the deck fights the lateral steering until the
body orbits the disagreement.

## Two shapes, and the invariant that took

The vertical districts now build **handed** towers: Wellshaft and Megastructure
turn the other way from everywhere else, so the stairwell opening faces
north-east instead of north-west. The quantized hexagon is symmetric about
`x = 0`, so the mirror is exact — same geometry, same guarantees, and the climb
and floor path reflect with the brushes so nothing can drift.

Mirroring surfaced a latent bug worth remembering for Phase 110: the *interior
hints* passed to the brush builders were not being reflected with the corners.
`sloped_deck_brush` derives the height it probes at from the mean of the corner
heights, which is only the true mid-height at the flight's plan midpoint. An
unreflected hint lands outside the reflected wedge, `oriented_plane` reads the
normal backwards, and the flight comes out a quarter-metre low along its whole
run. Any rotated or mirrored geometry can hit this.

It also surfaced a real invariant, now pinned by a test:

> **A shaft column must use one tower shape all the way up.**

A tower's opening is the hole the flight below arrives through. Two shapes in one
column leave the lower flight topping out under the upper cell's solid deck — the
surfaces union, so nothing *looks* broken, and a body simply climbs into the
underside of the floor above and stops. Districts drift between levels, so a
column crossing a district boundary is routine, and choosing per cell made this
the common case. Stair towers are now selected from the register at the **column
base**, which is the one thing every cell in a column agrees on.

## Measured

Seed `0xa11ce3d000000008`, production `28 x 20 x 10`, 5 473 cells.

| archetype | P104 | P107 | P108 | **P109** |
|---|---|---|---|---|
| shaft | **47 %** | 31 % | 26 % | **17.7 %** |
| corner | 18 % | 25 % | 20 % | 23.3 % |
| expanse | — | — | 20 % | 21.5 % |
| junction | 16 % | 21 % | 14 % | 16.4 % |
| ramp (both halves) | 14 % | 17 % | 14 % | 15.3 % |
| straight | 4 % | 5 % | 5 % | 5.6 % |

The rebalance halved the base shaft weights. That is a bigger change than it
sounds: the shaft family is 190 alphabet entries (a doorless through-shaft plus
every one- and two-door mask against three vertical combinations) against a
handful for a straight, so equal per-entry weight was never equal weight. That
arithmetic is how the facility became 47 % stairs. Verticality now comes from the
district profiles, which raise it where it is the identity, instead of from a
baseline that raised it everywhere.

**Backlog #13's headline: 47 % → 17.7 % across the arc.**

Per district, vertical circulation (shaft + ramp):

| district | shaft | ramp | vertical | reads as |
|---|---|---|---|---|
| Megastructure | 25 % | 20 % | **45 %** | a built ascent |
| Wellshaft | 19 % | 25 % | **44 %** | vertical |
| Liminal Grid | 18 % | 9 % | 27 % | open — 29 % expanse |
| Overlit Grid | 15 % | 10 % | 25 % | winding — **45 % corner** |

Overlit Grid runs 3.5x the corners of Megastructure; Liminal Grid runs over twice
the expanses of Overlit Grid; the two vertical districts run nearly twice the
vertical share of the two horizontal ones.

## What this phase did not do

**The two shapes are a handed pair, not two designs.** The plan asked for a
Wellshaft tower, a Megastructure tower and a neutral one — three skeletons. There
are two, and the vertical districts share the second. A genuinely new skeleton
(a three-flight spiral, a dogleg) is authoring work with a visual iteration loop,
and it belongs with Phase 110's per-district kits rather than being rushed here.
What this phase delivered is the part that made any of it possible: the contract,
the decoupling, and a proven exact reflection to build the next one against.

**The `stair_tower` coverage exemption stays.** Removing it would require an
exact tower for every register, and there are three. It is no longer hiding what
it was hiding — the monoculture is measured, halved, and broken — but it is still
an exemption, and it comes down in Phase 110 with the authored kits, alongside
the `expanse` one (backlog #20).

## Evidence

`docs/evidence/arc_o/phase_109/`.

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_109"; cargo dev-run -p iso_observer_lab
```

The physical gate is `every_generated_stair_tower_is_physically_climbable`: it
drives a Rapier capsule up each tower's own declared climb and asserts it arrives
at the deck above. Geometry that validates and a spine that parses still prove
nothing about whether a body fits between the treads and the soffit, and finding
out from the bot soak instead costs an afternoon of bisecting a stalled match.
Any new tower shape must pass it.

## Hand-off to Phase 110

- **Reflect interior hints.** See above; it is silent and it is a quarter of a
  metre.
- **The column invariant binds any per-district kit**, not just handed ones. A
  district-exclusive tower is subject to the same rule: a column takes its shape
  from its base cell's register.
- Two coverage exemptions are outstanding, `stair_tower` and `expanse`, and both
  are Phase 110's to remove.
