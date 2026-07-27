# Phase 107 — District Composition Profiles

**Status:** `[x]` — landed 2026-07-27. Closes bug backlog #17.

Districts now differ in **what gets built in them**, not only in how they are
lit. Palette work alone could never deliver that; this is the substance.

## The trap, and what it actually was

The arc opened with a standing warning: composition tendencies were compiled off
because enabling them broke `bot_soak_has_no_stalls`, and the note on the flag
said softening the constants only moved the failure around — correctly reading
the root cause as navigation rather than weighting, without identifying it.

Reproduced first, as the plan required. With tendencies on, all four bots piled
on one cell, `(7,7,L3)`, and none escaped. The route there reads:

```
(6,6,L3) Shaft     (6,7,L3) Junction
(7,7,L3) Room  up=ShaftOpen
(7,7,L4) Room  down=ShaftOpen
```

The route climbs **inside a two-level room**. Nothing can do that. The bot's
stair handling is gated on `HexArchetype::Shaft` and its waypoints are tuned to
the generic switchback tower's own local geometry, and a room cell is projected
through `blueprint_cell_archetype`, which returns `sanctuary` for every role and
every cell (backlog #15) — so there are no treads inside it either.

Measured across the 28 routable soak layouts:

| | layouts routing through a room-internal climb |
|---|---|
| tendencies off | **0** of 28 |
| tendencies on | **1** of 28 — exactly the stalling seed |

So it was never the weighting. It was a latent trap that any change to
composition had a chance of exposing, and the old lottery simply never rolled
into it. `topology::is_connection_open` no longer treats a room-to-room vertical
port as a connection: a route must not promise a climb the facility cannot
deliver. The soak passes with tendencies on.

## The design changed once, for a good reason

The first profiles boosted shafts in the vertical districts — Wellshaft 2.6x,
Megastructure 1.5x. The soak stalled again, on a different seed and a different
cell: a bot exiting laterally from inside a switchback tower near spawn.

That is the "softening moves the failure around" pattern, and tuning would have
been the wrong response twice. The right reading is that **boosting shafts was
working against the arc's own goal**. The facility is already roughly half shafts
(backlog #13); adding more puts more of the fragile generic switchback on the
routes bots follow, and makes the very problem Phase 109 exists to fix worse.

So no district boosts shafts above baseline. Verticality is expressed by
suppressing shafts *less* than the neighbours do — Wellshaft holds them at 1.0
while an open district cuts them to 0.3 — and by **ramps**, which are genuinely
traversable, where a district wants a built ascent. The relative reading is
identical, the absolute count falls everywhere, and the soak passes.

A test now pins that: no register may return a shaft multiplier above 1.0.

## Measured

Seed `0xa11ce3d000000008`, production `28 x 20 x 10`, 5 433 placed cells.

**Facility-wide, as a side effect of the profiles:**

| archetype | before | after |
|---|---|---|
| shaft | **47 %** | **31 %** |
| corner | 18 % | 25 % |
| junction | 16 % | 21 % |
| ramp | 14 % | 17 % |
| straight | 4 % | 5 % |

**Per district — the identities the arc promised:**

| district | shaft | junction | corner | ramp | reads as |
|---|---|---|---|---|---|
| Liminal Grid | **23 %** | **36 %** | 19 % | 13 % | vast and open |
| Overlit Grid | 24 % | **11 %** | **38 %** | 18 % | winding |
| Wellshaft | **42 %** | 21 % | 15 % | 19 % | vertical |
| Megastructure | 34 % | 19 % | 22 % | **21 %** | a built ascent |

Liminal Grid runs 3.3x the junctions of Overlit Grid; Overlit Grid runs 2.5x the
corners of Wellshaft; Wellshaft runs nearly twice the shafts of Liminal Grid.

Infinite Gallery is the honest outlier: its profile asks for straights and few
shafts, and it lands at 42 % shaft. It is the smallest district (286 cells) and
sits where the forced spawn-to-exit route needs height. Weights are tendencies;
the constraint solver has the last word, and it should.

## Two rules this phase changed

**The register is now a structural input.** `context.rs` used to state that the
weighting derived "purely from the cell's position in the grid — never from the
architecture register", to keep atmosphere and structure separable. That
separation was the reason districts were only lighting. `DistrictPalette` remains
atmosphere-only — style still never decides structure — but the register, which
is a semantic label the solver owns, now biases what gets built. Both doc
comments are updated rather than left contradicting the code.

**Variety is scored per district.** `archetype_variety_score` was a global
Shannon entropy over every cell, which rewards a uniformly mixed facility —
precisely the layout this phase exists to stop producing. Scored globally, a
facility with strongly characterised districts looks *worse* than mush, so
`generate_best` would have been pushed to pick the mush. It now measures inside
each district and averages: still asks whether any one neighbourhood is
monotonous, while leaving districts free to differ.

## Evidence

`docs/evidence/arc_o/phase_107/` — the district view for each pinned seed, and
the census manifest. The composition table above is reproducible from the lab's
own status line.

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_107"; cargo dev-run -p iso_observer_lab
```

## Hand-off to Phase 108

- The `Expanse` archetype lands into a system that already knows how to want
  more of something per district. Liminal Grid's profile is where it belongs,
  and `district_multiplier` gains one arm per register rather than a new
  mechanism.
- Shafts are down to 31 % but they are still the largest single archetype, and
  the generic switchback is still the only one. Phase 109 is where that closes;
  this phase deliberately did not paper over it by weighting alone.
- **The bot cannot exit a switchback tower laterally in every geometry.** That
  surfaced twice here and was routed around rather than fixed. It belongs to bot
  navigation, not composition, and it is now recorded in the backlog.
