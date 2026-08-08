# TR-11 — Retire the compatibility steering

**Status:** DONE, `324dc34`..`5e892c4` on `refactor/traversal-contract-integration`.
See §9 for what the execution found — the packet is complete except for one
symbol on §4's deletion list, which is kept for a measured reason.

**Hat:** Contract (this packet deliberately moves two pinned values).
**Owner:** one agent. **Serial** — it changes emitted intents.

**Base SHA:** `6ca9cfb` on `refactor/traversal-contract-integration`.
Tree is clean and green: `cargo dev-test` 1832 passed, clippy clean.

---

## 1. What this packet is for

Refactor Arc S set out to remove a **shotgun surgery** smell: one concept —
physical module traversability — fragmented across authoring, catalog selection,
projection, match AI, movement sync, tools and tests.

Most of it is already gone and live in production. What remains is the last
fragment: **the bot still infers a module's shape from its archetype.** Two
places do it —

- `ramp_walk_dir`, which reads `RampUp`/`RampHead` to pick a walking direction;
- `vertical_command` / `finish_stair_command`, which decide a cell is climbable
  from what it is called.

Both dissolve the same way, and the mechanism to dissolve them is already built,
tested and merged. This packet switches production onto it and deletes what it
replaces.

**This is a deletion, not a construction.** If you find yourself adding a
subsystem, stop and re-read.

## 2. Do this first — the whole switch is one line

`leg::ships_a_graph` gates the graph runtime on `guide.graph.is_some()`, which
nothing in the corpus sets. But `ResolvedModuleGraph::resolve` **already**
compiles the compatibility spine and deck into the same graph when that field is
empty. So widening the gate puts every annotated module on graph legs without
touching projection at all:

```rust
// crates/observed_match/src/hex_wfc/model/bot/leg.rs
pub(super) fn ships_a_graph(game: &HexWfcMatch, cell: HexCoord) -> bool {
    game.geometry.guides.contains_key(&cell)
}
```

A guide is only recorded when a climb or a deck exists, so the compile inside
`resolve` cannot fail for anything this admits.

**Do not** populate `ProjectedTraversalGuide.graph` in `geometry.rs` to achieve
this. It is unnecessary, it is a different packet's file, and it would move the
projection digest for no reason.

## 3. What happens when you do — measured, not predicted

Run `cargo test -p observed_match hex_wfc`. Expect **exactly eight failures**,
in three groups. If you see a different set, stop and work out why before
continuing.

### Group A — the boundary (2 tests). Update these deliberately.

| test | file |
|---|---|
| `perimeter_tower_local_intent_and_body_trace_is_pinned` | `hex_wfc/model/tests.rs:635` |
| `headless_gate_bot_walks_ramps_and_stairs_deterministically` | `hex_wfc/model/tests.rs:591` |

The tower climb **still completes** — measured at tick **1075** against the
pinned **1066**. Nine ticks, 0.15 s, body arrives at the top. The graph follower
selects targets slightly differently; nothing is broken.

This is the intent-trace boundary the plan predicts, and it is the *only*
sanctioned pin movement in this packet. Record before/after for the completion
tick, the trace digest, the body bit-pattern and the snapshot digest in your
handoff. Do not touch any other pin: the catalog hash, the composition profile,
the simulation hash and both spectator **selection** digests must all still pass
unedited, because selection is not steering.

### Group B — the cursor path going dead (6 tests). Re-point these **by hand**.

All in `crates/observed_match/src/hex_wfc/model/bot/driver.rs`:

```
same_tick_and_logical_level_perturbation_retain_the_committed_direction   :530
relayout_invalidation_is_scoped_to_the_leased_module                      :549
controller_recovery_emits_once_and_revokes_the_stale_lease                :570
displacement_escape_and_target_changes_revoke_only_the_affected_cursor    :611
disappearing_projected_guide_clears_a_stale_cursor                        :649
recorded_driver_frames_replay_without_driver_state                        :943
```

They assert on `driver.cursor(id)`. Once legs take the route first, no cursor is
ever created, so they fail by observing a mechanism that is no longer used.

**The ratchets themselves are intact** — verify this rather than assume it:
`legs` is cleared, removed and retained beside `cursors` at every invalidation
site in `invalidate_from_match`, `clear_player` and `reset`. These tests protect
ratchet invariants 5, 6 and 7 (a lease survives height rounding; relayout
invalidation is scoped; teleport/setback/escape/recovery clear the cursor).
Those properties must still be asserted after you are done — against the leg.

> **Trap, hit once already.** Bulk-rewriting `driver.cursor(id).is_none()` to a
> mechanism-agnostic accessor **silently inverted** one test's meaning. Move
> these six one at a time and read each assertion's intent before changing it.

### Group C — the one test that must keep naming `cursor`

`a_graph_shaped_module_is_crossed_without_any_bot_branch` (`driver.rs:749`)
asserts `driver.cursor(id).is_none()` and means it: its claim is that the graph
path is used **instead of** the compatibility one. Naming the mechanism is the
entire point. Leave it naming `cursor`.

## 4. Then delete what legs replace

Every production caller is in one function, `driver.rs::cached_bot_command`
(lines ~238, 323–342). Remove:

| symbol | where |
|---|---|
| `HexBotDriver::cursors`, `cursor()`, `follow_cursor` | `model/bot/driver.rs` |
| `TraversalCursor`, `TraversalLease`, `ModuleInstanceId` | `model/bot/driver.rs` |
| `HexWfcMatch::traversal_lease` | `model/bot.rs:179` |
| `vertical_command` | `model/bot.rs:244` |
| `finish_stair_command` | `model/bot.rs:383` |
| `ramp_walk_dir` | `model/bot.rs:562` |

Keep `lateral_waypoint` and `steer_toward`: an ordinary flat hop between cells is
not a graph leg, and its 1.0/sprint intent is correct and unrelated. See §6.

`stair_lateral_command` (`model/bot.rs:501`) already had its archetype gate
removed (`fc7546d`) and
may survive or go with the rest — decide by whether legs reach every case it
serves, and say which in the handoff.

**Exit criterion:** `grep -rn "HexArchetype::" crates/observed_match/src/hex_wfc/model/`
returns only `leg.rs`'s named legacy adapters. No archetype decides how a body
moves.

## 5. Gates

```
cargo test -p observed_traversal
cargo test -p observed_match hex_wfc
cargo test -p observed_authoring
cargo test -p observed_game survey_spectator_routes_across_seeds -- --ignored --nocapture
cargo fmt --all
cargo dev-clippy
cargo dev-test
git diff --check
```

The 24-seed survey **asserts** and must report zero stalls. `dev-test` was 1832
passed / 0 failed at the base SHA.

Regenerate nothing: no `.map`, catalog, manifest, FGD, `.sha256` or certificate.
`tilec gen-tiles` / `build` must not run. The catalog hash `74ead7a6…`, profile
`99e682b1…` and simulation hash `9937ed51…` must be unchanged — this packet
changes steering, not content.

## 6. Do not get drawn into these

Each is real, understood, and **out of scope**. They are recorded in
`docs/traversal_contract_refactor_plan.md`.

- **Cell-to-cell movement becoming a graph leg.** It would put a `Walk` edge
  where 1.0/sprint is used today and slow production 4.35×. Module-local
  traversal is 0.35/no-sprint; a lateral hop is 1.0/sprint; both are correct for
  what they do. `a_compiled_climb_graph_emits_what_the_spine_it_came_from_emits`
  asserts the module-local tuning by value so this cannot arrive quietly.
- **Migrating the corpus to authoring v3.** Blocked on the vertical-interface
  model, below. Nothing in this packet needs it.
- **The vertical interface's clearance model.** A `ClearanceVolume` is a box
  straddling the level plane that must stay empty — which models a handoff
  through *open air*. A stair is not that: a body crosses by standing on a
  surface that is itself at the seam. Needs `InterfaceProfile` to distinguish
  "through an opening" from "up a surface". Separate packet.
- **Regenerating anything.** Publisher-only.

## 7. Environment notes

- All sibling worktrees share `O:/Observed 2/target`. If you see
  `rust-lld: permission denied`, or an `unresolved import` for a symbol that
  plainly exists and compiles standalone, that is a **stale artifact**, not a
  code fault: `cargo clean -p <crate>`. It has cost a real diagnosis already.
- PowerShell: `cargo ... | Select-Object -First N` reports exit 255 because
  `-First` kills the upstream pipeline. Not a test failure.
- Commit a compiling leaf early and at every milestone. Agents have lost hours
  of uncommitted work to session limits on this arc twice.

## 8. Report back

```
Packet: TR-11
Base SHA:
Commit SHA:
Changed files:
Invariant moved / behavior intentionally changed:
Commands and results:
Catalog hash before/after:
Selection digest before/after:
Intent/body trace before/after:      <- expected to move; give both values
Completion tick before/after:        <- expected 1066 -> 1075; confirm or explain
Generated artifacts: none | exact list
Known follow-ups/conflicts:
git status --short:
git diff --check:
```

Be honest about anything incomplete. A truthful partial result is worth more
than a green board here: this packet moves a pinned trace on purpose, and the
integrator has to be able to trust which movements were intended.

---

## 9. What execution found

```
Packet:       TR-11
Base SHA:     6ca9cfb  (executed from 14b2d28, which adds only this document)
Commits:      5e892c4  the switch, the deletions, the six re-pointed tests
              324dc34  the ramp finding, and both Group A pins
Generated artifacts: none. No .map, catalog, manifest, FGD, .sha256 or
              certificate was written; `tilec` was not run.
git status --short: clean.   git diff --check: clean.
```

### §2 and §3 were exactly right

Widening the gate produced **exactly the predicted eight failures**, in the
three named groups, with the tower completing at tick **1075** against the
pinned 1066 — the number §3 predicted, to the tick. The six Group B tests were
moved one at a time. Their ratchet invariants are unchanged; "ascending", which
used to be `TraversalDirection::Forward`, is now stated in the terms a leg is
expressed in — the leased exit is the module's `Up` port, asserted once in the
shared fixture.

Group C lost its one assertion. `driver.cursor(id).is_none()` cannot survive
§4, which deletes `cursor()`: once there is no compatibility lease to take, the
claim "the graph path is used instead of it" has nothing left to name. The
test's remaining substance — the crossing works, the intent is the production
follower's verbatim, a `Climb` edge runs across a flat hall — is untouched, and
a comment records what the assertion used to say.

### §4 assumed legs reach every case. Two do not.

The deletion list was written without running it, and it over-reaches by two
entries. Both were found by measurement, not by reading.

**`ramp_walk_dir` must stay, and it is not a tuning problem.** A procedural
`hall_ramp` projects **neither spine nor deck**, so no guide is recorded for its
cell and there is no contract data for a leg to execute. `legacy_cell_adapter`
cannot stand in either: a ramp's vertical ports are not lateral doorways, and
the gate corpus's ramps open on **one** face, so `ramp_faces` has no second
doorway node to raise a climb edge toward. Deleting the function does not move
the inference into data — it removes the only thing that walks a body up a ramp.
Measured: the headless gate stopped completing at all, and all four soak bots
stalled at `(2,0,2)`, `hall_ramp` variant 0, `RampUp`, one door, `up:
RampOpen`. It survives as `unannotated_ramp_command` — a heading, no state, no
lease — carrying its own reason and its retirement condition.

**Authoring a spine on the ramp kit is what retires it.** Nothing else does, and
that is the single next step for this smell.

**`stair_lateral_command` stays too** — §4 left the decision to measurement, and
the answer is that legs do **not** reach every case it serves. Deleting it moved
the headless gate's completion tick off its pin. (First measured as "never
fires" via an `eprintln` probe; that reading was wrong, because `cargo test`
captures output from *passing* tests and only prints it for failing ones. The
gate caught the mistake. Do not probe a passing test without `--nocapture`.)

So the exit criterion holds in leg execution but not absolutely:
`grep -rn "HexArchetype::" crates/observed_match/src/hex_wfc/model/` returns
`leg.rs`'s two named legacy adapters **and** `bot.rs`'s
`unannotated_ramp_command`. That third hit is content debt with a name, not
steering that escaped the refactor.

### Everything else on the list was genuinely dead

`HexBotDriver::{cursors, cursor, follow_cursor}`, `TraversalCursor`,
`TraversalLease`, `ModuleInstanceId` (and both re-exports), `traversal_lease`,
the climb half of `vertical_command`, `climb_command` and `finish_stair_command`
are gone. Their removal moved **no** measurement: the gate and the tower report
the same values with them deleted as with them present. `bot.rs` no longer
imports `observed_traversal` at all — every follow now goes through `leg.rs`.

Net: **-493 / +142** lines of code.

### Pins

| | before | after |
|---|---|---|
| tower completion tick | 1066 | **1075** |
| tower traced ticks | 963 | **973** |
| tower intent/body trace | `0xe7358686ec1823c9` | **`0x5adc2eb981ea1880`** |
| tower body position | (13.855, 9.411, 3.464) | **(14.015, 9.410, 3.490)** |
| gate completion tick | 5596 | **5511** |
| gate snapshot digest | `0x44209934 6f7eb43e` | **`0x457d40ff581e0bd1`** |

The tower climb still completes; the body ends 0.17 m from where it did, at the
same height, on the same tread. The gate re-times *downward*. Both are the
sanctioned intent-trace boundary and no other pin moved — `compiled_catalog`
`74ead7a6…`, `composition_profile` `99e682b1…` and the simulation hash
`9937ed51…` are unchanged and their tests passed unedited, because selection is
not steering.

### Gates

```
cargo test -p observed_traversal                        72 passed / 0 failed
cargo test -p observed_match hex_wfc                     77 passed / 0 failed
cargo test -p observed_authoring                        178 passed / 0 failed
survey_spectator_routes_across_seeds --ignored           0 of 24 seeds stalled
cargo fmt --all                                          clean
cargo dev-clippy                                         clean
cargo dev-test                                         1832 passed / 0 failed
git diff --check                                         clean
```

`dev-test` is 1832, matching the base SHA exactly.
