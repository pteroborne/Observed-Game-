# Kinetic tool lab — evidence

![Shove preview](shove-preview.png)

Captured with:

```bash
OBSERVED2_CAPTURE=docs/evidence/kinetic_lab/shove-preview.png cargo run -p kinetic_lab
```

The capture pauses time and stands the Observer west of the ledge run with a
minor Guardian in the lane, so the frame shows a live preview rather than an
idle board.

## What the frame proves

The green line runs from the minor Guardian at `(4, 3)`, across the three amber
**ledge** cells at `(5..7, 3)`, to a double ring on the void rim at `(8, 3)`, and
the panel reads `lane: Void after 4 cells`.

That number is the point. A push carries `PUSH_IMPULSE = 3` cells, but ledges do
not consume impulse, so the target travels **four** and leaves the floor. The
rule "momentum carries across unrailed geometry" is legible in the frame before
the trigger is pulled, which is the lab's answer to whether a shove is fair — the
preview is the same pure `resolve_shove` the tick will run, not an estimate of
it.

The double ring is the second, non-colour channel on a lethal outcome; grey
would mean the target survives, amber that it lands on a tile already
retracting, red that structure blocks the shove.

## The rest of the board, left to right

| Mark | Cell | What it is |
| --- | --- | --- |
| Yellow square | `(1, 1)` | Generator — cut power here and sight collapses to your own cell |
| Grey hex | `(2, 2)` | Wall — a shove into it is `Blocked` and nothing moves |
| Green ringed square | `(2, 4)` | Recharge station, drawn live because the floor has power |
| Cyan square | `(3, 3)` | Observer, with the facing lane drawn east |
| Orange squares | `(4, 3)`, `(3, 4)` | Minor Guardians — neither freezes when looked at |
| Dark red hex | `(3, 5)` | Retracting tile, fading toward void as its countdown runs |
| Amber hexes | `(5..7, 3)` | The unrailed ledge run |
| Pink square | `(6, 5)` | Major Guardian, drawn awake because it is outside the lane |

## First person, same board

![First-person lane](fps-lane.png)

```bash
OBSERVED2_CAPTURE=docs/evidence/kinetic_lab/fps-lane.png \
  cargo run -p kinetic_lab --bin kinetic_fps
```

The same `KineticWorld`, the same `resolve_shove`, the same `lane: Void after 4
cells` — now standing on the floor rather than looking down at it. The capture
pauses the board first, because with capture working a Guardian one plate away
jails you in 24 ticks.

**Shape language.** Rank reads as the order of the solid. The orange **cube** is
a minor Guardian, the pink **tetrahedron** the major — a rarer solid for a rarer
thing. Both hold whole lattice cells and cross between them in a crisp snap that
finishes well inside their step interval, then wait. That clockwork read is not
decoration: Guardians move in quantised steps because the determinism contract
required it, and the presentation simply stopped apologising for it.

**The green beam** marks where a push would send the target. It is a beam and
not a floor decal because the lethal destination here is void about seventy
metres out, where a flat marker is both invisible and hidden behind the very
Guardian being aimed at. The amber plates between are the unrailed ledge run,
and the beam standing past their far end is the ledge rule stated in world
space: momentum outlives the three cells a push pays for.

Plate geometry is rectangular rather than hexagonal, and that is deliberate. The
lattice tiles exactly with 14x12 plates offset 7 per row, so they meet with no
gap and no overlap and a void cell leaves an exact hole. The hex lattice remains
the connectivity and targeting structure; an authored tile's *geometry* was
never required to be a hex prism, and rendering what you collide with is what
the Legibility Contract actually asks for.

## Caveat

Neither view answers whether a shove is *satisfying* in the sense a playtest
means — that needs a person at the keyboard, and it is the open human gate. What
these frames establish is that the rules are legible, reproducible, and identical
across two independent presentations of the same simulation.
