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

## Caveat

This is a top-down schematic with discrete steps. It demonstrates that the rules
are legible and reproducible; it does **not** answer whether a shove is
*satisfying* at the moment of contact, which needs the first-person controller
and is the next increment.
