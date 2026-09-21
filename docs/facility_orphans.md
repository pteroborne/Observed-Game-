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

## Decided: the interruption is the mechanic

**2026-09-20, Will's call.** Cutting off whatever sits behind a gap until the Architect
repairs it is **intended**. It is the pressure loop the damage exists to create — *"a
missing tile interrupts the hunt, reconnect the route with a card"* — and not a defect to
be guarded against.

The attempt to guard against it is what settled the question. Rejecting any gap that
stranded a cell held the connectivity invariant and cost the scenario: Deep Stack lost
four of its five interruptions and an 855-beat match collapsed to six. A guard that
removes the feature it protects is answering the wrong question.

So there is nothing to fix in the generator, and nothing to fix in the damage step.
`observed_facility` ships 48 of 48 facilities fully connected; everything downstream of
that is the scenario doing its job.

### What the real defects were, and where they went

Every symptom traced back to **downstream code assuming a connectivity nothing promised**,
rather than to the damage itself:

- `EconomyState::new` placed fixtures on stranded cells — **fixed**, fixtures are now
  placed by reachability (backlog #44).
- Topology counted stranded cells as components — **fixed**, `meaningful_component_count`
  measures against the set reachable at match start.

Both are closed. The rule for anything new that reads the facility: **ask for the
reachable set, never the placement map.** A cell existing is not a cell anyone can stand in.

### What is guarded now

- `every_passable_cell_belongs_to_one_facility` (`observed_facility`) — the solver ships
  connected facilities, 4 shapes × 12 seeds. Cheap, and it keeps "the solver is fine" a
  fact rather than a memory.
- `every_scenario_still_interrupts_its_route` (`architect_lab`) — the interruption counts
  are pinned, so a future connectivity guard cannot quietly make the scenarios easier.
  That is the failure this investigation actually produced, and it is worth a test.

### Still open

**Pocket.** One mandated gap strands 8 of its 16 passable cells, and it resolves in three
to six beats. Three independent lines of evidence now say it is too small to carry the
rules it demonstrates. That is a scenario-design decision, tracked with the pacing work,
not a connectivity one.

## Method notes worth keeping

This diagnosis was wrong twice before it was right, and both corrections came from a
measurement rather than a re-reading.

1. **Blamed the solver.** The `Void` beside the orphan was punched by the lab a few lines
   after the solve. *Look at what the failing thing actually loads* — including what the
   caller does to it afterwards.
2. **Measured it wrong.** `disconnected_cells` flood-filled from the lowest-sorted cell,
   which in a damaged facility can itself be stranded; Full Ascent came back as 110 of 111
   cells disconnected from one orphan. An absurd number is a gift.
3. **Assumed a tree.** When a non-stranding gap could not be found, the natural conclusion
   was that the facility has no alternative routes. It has plenty — 53% to 85% of cells
   can be removed harmlessly. The binding constraint was that gaps must be route
   midpoints, and the route is the spine.
