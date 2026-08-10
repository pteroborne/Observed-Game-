# T-3 — the cutaway says what it is cutting

Backlog [#36](../../../bug_backlog.md). The spectator cutaway was the only
feature the 2026-08-09 Deck-to-Deck playtest praised unprompted, so this packet
is investment rather than repair. What it lacked was clarity on three points:
what is being cut, where the viewer is, and which bodies are which.

## The captures

Same seed, same eight frames, same build. `before/` is that build with
`view/spectate.rs` reverted to `c7abb6c` and `view/cutaway_marks.rs` removed —
so the only difference between the two directories is the marks themselves.

| | |
|---|---|
| `before/hex_wfc_000..007.png` | detent 0, no marks |
| `after/hex_wfc_000..007.png` | detent 0, with marks |
| `after_detent_3/hex_wfc_000..007.png` | detent 3, to show the hoop turning |

```powershell
$env:OBSERVED2_SPECTATE_OVERVIEW = "0"
$env:OBSERVED2_CAPTURE_HEX_WFC_STYLE = "docs/evidence/arc_t/cutaway/after"
cargo dev-run -p observed_game
```

`hex_wfc_007.png` is the frame to compare: by then the spectator bot has walked
clear of its spawn, so the bodies have separated and the picture is not a pile.

## What the before frame actually shows

A grey plan hanging in black. It is genuinely handsome, and:

- Nothing says a cut is happening. A viewer with no prior knowledge reads a
  building that simply has no roof and no near walls.
- Nothing says which storey. The facility is ten levels; the picture is one.
- **The followed body is not drawn at all.** `entities::setup` skips
  `local_player` deliberately, because in play you are inside its head. Pulled
  back to forty metres, the camera follows something invisible.
- The other bodies are 0.25 m capsules at 46% alpha, sized for arm's length in a
  corridor. In `before/hex_wfc_007.png` one is a pale sliver two pixels wide
  inside the lit room, and nothing about it says whose it is.

## What the marks add, reading by reading

**What is being cut** — an amber hoop at the height the ceiling came off
(`SchematicRole::Selected`, "the cell currently under inspection"), and a green
hexagon on the storey floor marking how far the view reaches
(`SchematicRole::Grid`, "the annotation lines that carry no state of their
own"). The hoop is **three sides of six**, dropped by the same near-arc test
`iso::survives` drops walls with — so the hoop is open exactly where the
building is open, and the gap always faces the camera. Compare `after/` with
`after_detent_3/`: the ring turns with the detent because it is asking the same
question the walls are.

The green hexagon answers a question the before frame could not: whether the
plan fades into black because the building ends, or because the view does.

**Where the viewer is** — the followed body now exists: a cyan disc at its feet
and a pin (`MarkerRole::You`) tall enough to cross the cut plane, so it is
visible from outside whatever room it is standing in. Beside the ring, a
compressed storey ladder lights the rung for the floor on show. The rungs are
**not** at true heights — ten storeys at 8 m is an 80 m spine in a 42 m frame,
which would be honest and off the side of the picture — so the ladder reads as
"third of ten" by position in the stack rather than by altitude.

**Which bodies are which** — every body inside the framed radius gets the same
disc-pin-nose vocabulary in `MarkerRole::{You, Teammate, Rival}`. The followed
body's pin is longer and its disc wider, so "you" wins against three others
without relying on hue alone. Bodies keep their own height, so a rival one floor
down reads as one floor down. Bodies outside the frame are hidden — facility-wide
markers were a documented red herring in this view's history.

### The colour language, and one deliberate crossing

Every colour is an `observed_style` treatment used verbatim — no scalar, no
tint. Two registers are in play and they are kept apart on purpose:

| mark | role | reads as |
|---|---|---|
| cut-plane hoop | `SchematicRole::Selected` | amber |
| framed extent, ladder post, unlit rungs | `SchematicRole::Grid` | green |
| followed body, lit rung | `MarkerRole::You` | white-cyan |
| teammate | `MarkerRole::Teammate` | blue |
| rival | `MarkerRole::Rival` | orange |

The first pass drew the hoop in `ObservationPanelRole::Footprint`, which is a
fair reading of "schematic outline" and is within a few percent of
`MarkerRole::You`. Diluting the body hues is the one thing this packet could not
afford, so the section rig moved to the schematic register instead.

The lit ladder rung is the single crossing between the two, and it crosses
because of what it says: not "this storey is under inspection" but "the body you
are following is on this one" — the same fact the pin is reporting from inside
the plan. The capture settles the risk: a white rung at the bottom of a green
stack is not going to be mistaken for a body in a room.

## Cost

Two meshes, five materials, and about thirty entities: up to twelve ring bars, a
ladder of at most eleven pieces, and three small parts per framed body. No
lights. Nothing scales with the ~109k hulls a production facility spawns, which
is the budget the Deck actually has.

The static half is rebuilt only when the followed cell, detent, tile radius,
level count or followed body changes — at most once per tile crossing, which is
also the rate at which the camera itself steps. Only the body transforms are
written per frame.

## Not done here

The cutaway stays in the spectator view. Promoting it into the first-person
match is T-7/T-8, and that packet explicitly wants this clarity work finished
first.
