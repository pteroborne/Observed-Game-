# The facility is dropped on the floor by the lab, not by the solver

Findings note. Supersedes an earlier version of this file that blamed the WFC; that
conclusion was wrong and the correction is recorded below.

## What is measured

Two questions, answered separately:

**Does the solver ship connected facilities?** Yes. `every_passable_cell_belongs_to_one_facility`
in `observed_facility` solves the four `architect_lab` scenario shapes across twelve seeds
each — 48 facilities — and every one is a single connected component. Zero orphans.

**Then where do the orphans come from?** From `ArchitectLab::generate_with_team_size`,
which punches holes in the facility *after* it solves:

```rust
tile.space = HexSpace::Void;
tile.doors = 0;
tile.up = PortClass::Sealed;
tile.down = PortClass::Sealed;
```

One gap in Pocket, two in Quick Climb, three in Full Ascent, five in Deep Stack, taken
from midpoints along the entrance-to-exit route. They are deliberate — *"A missing tile
interrupts the hunt. Reconnect the route with a card."* Scenario damage, not a bug.

Measured on both sides of that step (`who_actually_orphans_the_cells`):

| Mode | As solved | Scenario gaps | Stranded after |
|---|---|---|---|
| Pocket | 0 | 1 | **8** |
| Quick Climb | 0 | 2 | 5 |
| Full Ascent | 0 | 3 | 3 |
| Deep Stack | 0 | 4 | 20 |

**One gap in Pocket strands eight cells — half the facility.**

## The actual defect

Interrupting the route is intended. Stranding a third of the building is not, and nothing
measures the difference. The damage step chooses its gaps by three tests — not
retraction-protected, laterally flanked on the route, at least three cells from another
gap — and **none of them asks what the cell was holding up**. A route midpoint that is
also a cut vertex takes a limb of the facility with it.

So the cells that looked like generator orphans are the far side of a severed route.

## Two corrections worth keeping

**The first diagnosis was wrong, and wrong in a familiar way.** It blamed the solver for
shipping a one-door Room whose door faced a `Void`, and cited `constraints.rs` describing
that exact hazard. The description is real and the room is real — but the `Void` next door
was punched by the lab a few lines after the solve, not placed by the collapse. *Look at
what the failing thing actually loads* is the rule this project keeps relearning, and
"what it loads" includes everything the caller does to it afterwards.

**The first measurement was also wrong.** `disconnected_cells` originally flood-filled from
the lowest-sorted passable cell and reported everything it missed. In a damaged facility
that start cell can itself be stranded, which inverts the answer — Full Ascent came back
as 110 of 111 cells "disconnected" from a single orphan. The absurdity of the number is
what exposed it. It now ranks components by size and calls the largest one the facility.

## What this has already broken

Every symptom traced to this is downstream code assuming connectivity nothing guaranteed:

- **Power was unrecoverable.** `EconomyState::new` picked each floor's generator as the
  lowest-sorted non-Void placement, which landed on stranded cells. Backlog #44. Fixed by
  placing fixtures by reachability.
- **The recharge economy leaked** — one of Deep Stack's five stations, same cause.
- **Topology metrics were noise.** Raw component count never dropped below four, so any
  Sever threshold fired on beat one.

## What to do

The solver needs no change, and the assertion that proves it belongs in the gate anyway —
it is cheap, and it is what makes "the solver is fine" a fact rather than a memory.

The lab's damage step is where the work is. Options, in the order I would try them:

1. **Choose gaps that interrupt without stranding.** Before committing a gap, check what
   the removal disconnects; reject a candidate that strands cells beyond the route itself.
   Keeps the scenario's intent exactly and costs a flood fill per candidate at setup.
2. **Bound it.** Allow stranding up to some fraction of the facility and fail the scenario
   above that. Cheaper, weaker, and leaves the Pocket case needing a separate answer.
3. **Accept it as a property** and make every downstream consumer ask for the reachable
   set rather than the placement map. This is what the power fix already did locally, and
   doing it everywhere is a larger change than fixing the cause.

Pocket deserves its own decision regardless. Eight of sixteen passable cells stranded by a
single mandated gap, in a scenario that also resolves in three to six beats, is a facility
too small to carry the rules it is being asked to demonstrate.
