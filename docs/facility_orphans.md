# Why the facility drops cells on the floor

Findings note. Answers Part A of the connectivity task; the fix is still open.

## The measurement

Every generated facility is one large connected body plus a scatter of cells with **zero
exits** — reachable from nowhere, reaching nowhere:

| Mode | Grid | Cells with zero exits |
|---|---|---|
| Pocket | 6×5×1 | 8 — `(0,0,0)`, `(2,1,0)`, `(2,2,0)`, `(2,3,0)`, `(3,1,0)`, `(3,2,0)`, `(3,3,0)`, `(3,4,0)` |
| Quick Climb | 8×6×2 | 5 |
| Full Ascent | 10×8×2 | 3 |
| Deep Stack | 8×6×5 | 5 |

Half of Pocket's passable cells are orphaned.

## The mechanism

`(0, 0, level)` is orphaned in **every mode**, always identically:

```
origin: space Room, doors 0b000001, up Sealed, down Sealed
  East:      mine open=true,  neighbour (1,0,0) -> Void open=false
  SouthEast: mine open=false, neighbour (0,1,0) -> Void/Hall open=false
  SouthWest / West / NorthWest / NorthEast: off-grid
```

A one-door Room whose single door opens **East onto a Void cell**. The tile is internally
valid. Its door is a promise the neighbour cannot keep.

The origin is not special — it is the *most exposed* case. At the grid corner four of six
lateral faces are off-grid, so the cell has only two chances to connect, and it drew a
one-door tile pointing at ground the collapse filled with Void. The mid-grid orphans are
the same failure with more faces to get unlucky on.

## The code already predicts this, and assumes it cannot happen here

`constraints.rs`, on `corridor_skeleton`'s `every_port` parameter:

> A spanning tree spends one port per room pair. Measured, that leaves 26 of a facility's
> 72 exterior doors facing ground the skeleton never claimed. **With the collapse filling
> that ground it does not matter — whatever lands outside the door will open toward it or
> the propagation will make it.** Under a carve it matters completely: the cell outside
> becomes `Void`, `Void` seals every face, and the door on the other side is a promise
> nothing can keep.

The bolded assumption is what these measurements contradict. `every_port` is wired to
`carve_unrouted`, so on the non-carve path the skeleton routes only the spanning tree and
the remaining doors are left to the collapse — which does **not** reliably open back. It
fills with Void, and the carve's failure mode arrives on the path that was believed
immune. The difference is that the carve *empties the domain* and fails loudly, while here
it silently ships an orphan.

Note the asymmetry that lets it through. `exits()` treats a lateral connection as open
only when **both** sides carry a door bit, but nothing during collapse rejects a door bit
facing a Void. `ports_compatible` would refuse it — `Door` never matches `Sealed` — but it
governs the vertical faces; laterals are decided by the door mask alone.

## What this has already broken

- **Power is unrecoverable.** `EconomyState::new` picks each floor's generator as the
  lowest-sorted non-Void placement, and `(0,0,level)` sorts first. So every mode's level-0
  generator sat on an orphan, and the floor everybody starts on could never be relit
  (backlog #44).
- **The recharge economy leaks** — one of Deep Stack's five stations, same cause.
- **Topology metrics are noise.** Raw component count never drops below four because each
  orphan counts as a component; any Sever threshold fired on beat one (backlog, #46 note).

Each has been worked around where it surfaced. None of them fixed this.

## Open question for the fix

Two shapes, and the choice is a design decision rather than a mechanical one:

1. **Forbid it at the source.** An open lateral port facing a Void cell becomes a
   contradiction the solver must avoid, the way `ports_compatible` already treats the
   vertical faces. Truest, and most likely to cost solve failures and retries.
2. **Seal it after the fact.** A deterministic post-pass closes any door with nothing
   behind it, turning an orphan into an ordinary sealed cell. Cheap and safe, but it
   silently rewrites authored tiles, and a room that becomes a sealed box is still not a
   room anyone can visit.

Neither makes the orphan *reachable* — that would need the collapse to place something
connectable there instead, which is option 1 with the retries paid.
