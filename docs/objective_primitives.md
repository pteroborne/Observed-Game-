# Objectives are made of three questions

Requirements note for the variable Rogue-objective system. Pairs with the objectives
catalogue (`RAI_objectives.xlsx`) and the design note `RAI-DESIGN.md`.

## Why primitives first

The catalogue holds fourteen candidate objectives. Building fourteen features is fourteen
chances to discover, after the fact, that the behaviour never happens — which has already
happened nine times in this project: the shove softlock, the unreachable requisition,
power that was never toggled, Pocket's three-beat walkover, and the survivable fall three
separate times. Each fix was correct. Each left the mechanic unreachable.

Reading the catalogue with that in mind, the fourteen collapse into three questions the
simulation has to answer cheaply, every tick, deterministically:

| # | Primitive | Question | Objectives it serves |
|---|-----------|----------|----------------------|
| **P1** | **Shape match** | Does the facility contain configuration X? | O1 composition, O5 district purity, O7 spine, O9 gallery, O14 monument |
| **P2** | **Topology property** | Does the traversal graph satisfy property P? | O2 connect N of a role, O6 close a circuit, O8 sever into N components, O10 depth frontier, O12 enclose |
| **P3** | **State hold** | Has condition C been true for N consecutive ticks? | O3 equilibrium, O4 sustain the horde, O11 darkness, O13 quarantine |

Build the three and the other eleven objectives become **data**: a predicate, a threshold,
a tell. Then dialling an objective is a config change and a lab run, not a feature.

The ordering is by cost. P3 needs no new machinery at all — a predicate and a counter. P2
needs a traversal graph maintained incrementally, or it re-derives per tick and lands us
back in backlog #10 and #12, where per-tick work became the largest frame cost. P1 needs
a matcher over placements and is the only one that needs authoring support, because a
shape the corpus cannot express is a shape the RAI cannot want. **P1 is therefore gated on
the level-design work (#30)** — a corpus of interchangeable tiles cannot say anything, so
it cannot have a purpose to infer.

## P3 — state hold

> Has predicate C held for N consecutive ticks?

**Reports progress, not a boolean.** Current streak, longest streak, total ticks true,
and the tick at which it completed. Without progress, an objective that never fires is
indistinguishable from one that nearly fired every match — and telling those two apart is
the entire reason this note exists.

Constraints:

- **Deterministic.** Seed plus ordered commands reproduce it exactly. No ambient state, no
  iteration-order dependence; the streak participates in determinism hashing.
- **O(1) amortised per tick.** Evaluating a predicate and bumping a counter is fine.
  Re-deriving a set every tick is not. A predicate that cannot be answered in constant
  time is a P2 predicate wearing a P3 costume, and should be declared expensive rather
  than hidden.
- **Resets cleanly** through all three paths: `desktop.rs`, `view.rs::reset_for_mode`,
  `web.rs`. Falls and requisition both have regression tests for exactly this; follow them.
- **A predicate going false resets the streak to zero.** Consecutive means consecutive.

## O11 Darkness, and why the catalogue's wording is unbuildable

The catalogue reads: *"the RAI wins if zero cells are observed for N consecutive ticks."*

That is unreachable by construction, and the code says so plainly. `refresh_observation`
(`sim.rs:858`) inserts, for every **Active** Observer, the cell they stand on — and only
then, conditionally, the cell they face:

```rust
for observer in self.observers.values().filter(|o| o.state == ObserverState::Active) {
    self.observed.insert(observer.cell);
    if self.economy.is_powered(observer.cell.level)
        && let Some(next) = self.step_through(observer.cell, observer.facing) {
        self.observed.insert(next);
    }
}
```

So `observed.is_empty()` holds **exactly when no Observer is Active** — every one jailed or
corrupted — which is the existing Rogue victory, checked in the same tick, a few lines
later. The naive Darkness cannot hold for even one tick before the match has already ended
by another route. It would have gated green, shipped, and never fired: defect number ten,
in a work item written to avoid defect number ten.

### The definition that does work

> **Darkness holds when at least one Observer is Active and none of them has a lit
> sightline.**

Nobody is looking *outward*. Observers still occupy cells; they simply illuminate nothing
beyond themselves, because their floor has lost power or they are facing a wall.

This keeps what made the objective worth building:

- It **inverts the Observer's core verb.** Observation currently costs nothing and protects
  everything. Under Darkness, looking outward is the only thing preventing a loss.
- It is **legible in the architecture** without being stated — the facility gets quieter and
  dimmer, which is the tell.
- It is **free to compute.** The sightline extension is already being decided inside the
  existing loop; counting it is a counter increment, not a new pass.
- It gives the dormant power system a reason to exist. Power has never once been toggled in
  a recorded match; an objective that pays the Rogue for cutting it is the first pressure
  to do so.

And it stays honest about the degenerate case: **zero Active Observers is not darkness, it
is a Rogue victory already won.** The predicate must require at least one Active Observer,
or it becomes a second name for the condition that is about to fire anyway.

### What we do not yet know

Whether it is **reachable under bot play** is an open empirical question, and the point of
the first slice. It requires every Active Observer simultaneously facing a sealed port or
standing on an unpowered floor. Bot Observers walk toward the exit, so they mostly face
open space; the streak distribution will tell us whether N can be set anywhere useful, or
whether Darkness needs the Rogue to actively cut power — which would make it the first
objective that demands a behaviour the Rogue bot does not yet have.

A run that never fires is a result. Retuning N until a number appears is not.
