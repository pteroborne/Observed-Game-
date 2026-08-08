# TR-11 — Retire the compatibility steering

**Status:** ready to execute. **Hat:** Contract (this packet deliberately moves
two pinned values). **Owner:** one agent. **Serial** — it changes emitted intents.

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
