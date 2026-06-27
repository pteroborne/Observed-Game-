# door_lab — doors as the observation gate

A feasibility lab for the **door mechanic** agreed in the gameplay-depth arc (see the
North Star in [agents.md](../../agents.md) and [ROADMAP.md](../../ROADMAP.md)).

**Primary technical question:** *can a player-operated door be the gate for the
observe/decohere mechanic — open = observed/frozen, closed = free to rewire — such
that rewiring only happens behind closed doors, a protected spine keeps the exit
reachable, reopening a closed door can reveal a changed partner ("the path changed"),
and dead-end pockets never sever the exit — all deterministically?*

## What it proves

The pure model [`crates/observed_doors`](../../crates/observed_doors) (promoted out of
this lab in refactor R5) reuses `observed_observation`'s graph structure (rooms,
doorways, the authored lattice, geometry) but replaces the **pinning rule**: a door is
pinned (frozen) iff it is *open* or on the *spine*.
Unit tests establish:

- opening a door freezes its connection across decoherence;
- **rewiring happens only behind closed doors** (no open door ever changes);
- closed, non-spine doors do rewire;
- the protected spine keeps the exit reachable through any rewiring;
- reopening a door that rewired while closed registers the changed partner (the
  mystery / "path changed behind you" loop);
- closing an open door counts as a slam;
- traversal requires an open door (a closed door is a mystery until opened);
- dead-end pockets are detected and never sever the exit;
- decoherence is deterministic (seeded); reset restores the authored world.

This is the *logic*; the diegetic 3D doorway presentation (closed leaves that hide
the layout, a slam when a connection reroutes) folds into the game's re-skin once
this proves out.

## Run

```powershell
cargo run -p door_lab
```

A 2D schematic: rooms as cells, doors as the links between them.

- `1` `2` `3` `4` — open/close the N E S W door of your room
- Arrows — walk through that door (only if it is open)
- `D` — decohere now (rewire closed, non-spine doors)
- `R` — reset · `F1` — toggle the overlay

GOLD = protected spine (always frozen). GREEN = open/observed → frozen. BLUE =
closed → free to rewire. Grey ticks = sealed walls. Blue room = start, green = exit,
red = a dead-end pocket.

Capture the evidence screenshot:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/door_lab.png"; cargo run -p door_lab
```

## Success conditions

- The overlay shows `[PASS]` (one camera, one UI root, a valid matching, exit
  reachable).
- Opening a door turns its connection GREEN and it stops rewiring on `D`; closing it
  turns it BLUE and it can rewire.
- The GOLD spine never changes and the exit stays reachable no matter how many times
  you decohere.
- `R` restores the authored world; entity counts are stable across resets.

## Tests

`cargo test -p door_lab` — 11 pure-logic (the claims above) + 2 lifecycle (boots
with one camera + overlay; reset restores without leaking).
