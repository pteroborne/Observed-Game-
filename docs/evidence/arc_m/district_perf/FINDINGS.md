# Per-district FPS investigation (2026-07-25)

Hardware: NVIDIA TITAN X (Pascal), i7-9700, 1440×900, Vulkan, dev profile (optimised).
All comparative numbers come from runs with `OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1`.

## The measurement had to be built first

Two things made the existing harness unable to answer the question:

1. **Vsync flattens everything.** The shipping window is `AutoVsync`. Every register the
   GPU can finish inside the refresh interval reports the same ~16 640 µs median. The first
   per-register table looked like this — six registers, all exactly 60 fps, no signal:

   | register | median | fps |
   |---|---|---|
   | wellshaft | 16 652 | 60 |
   | infinite_gallery | 16 643 | 60 |
   | liminal_grid | 16 635 | 60 |
   | facet_monument | 16 562 | 60 |

   `OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED` releases the cap for a capture. `vsync_uncapped`
   is recorded in the report, because medians from a capped run are not comparable.

2. **600 ticks is not a route.** The default capture is ~10 s of simulation, in which the
   bot may never leave the register it spawned in — the first run bucketed 531 of 531
   frames into `facet_monument`. `OBSERVED2_CAPTURE_HEX_WFC_TICKS` extends the run.

## What the drop actually is

It is **severe and location-specific, not district-wide**. Seed `0xC0FFEE`, uncapped:

| register | resident pieces | frame | sim | render | fps | n |
|---|---|---|---|---|---|---|
| megastructure | 1 359 | 112.5 ms | 13.0 | **99.5** | **9** | 43 |
| facet_monument | 852 | 39.1 ms | 10.8 | **28.3** | 26 | 142 |
| overlit_grid | 1 694 | 20.1 ms | 7.2 | 13.0 | 50 | 2 092 |
| infinite_gallery | 1 987 | 13.8 ms | 5.2 | 8.6 | 72 | 3 154 |
| monolith | 1 605 | 11.4 ms | 2.4 | 9.0 | 88 | 3 195 |
| thinning | 1 947 | 10.9 ms | 2.0 | 8.9 | 92 | 2 003 |
| institutional | 1 987 | 10.8 ms | 2.3 | 8.4 | 93 | 1 724 |

The same register is fine elsewhere: in seed `0xA11CE`, `megastructure` runs at 17 ms /
58 fps. So the register is a correlate, not the cause — something about *where* those
cells sit in that particular facility is.

**It is not draw calls.** Megastructure carries the *fewest* resident hull meshes (1 359)
of any register in that run and costs 12× the render time of `institutional`, which carries
1 987. Cost per resident piece ranges from 4.25 µs to 73.2 µs — a 17× spread. This is a
per-pixel problem, not a geometry-count problem.

**Simulation scales with it too** (2.0 ms → 13.0 ms across the same rows), so whatever
drives it is not purely GPU-side.

## Ruled out by A/B probe

| hypothesis | probe | result |
|---|---|---|
| practical point-light shadows (4 lights × 6 cubemap faces) | `PRACTICAL_SHADOW_BUDGET` 4 → 0 | megastructure 99 → 81 ms. Minor. |
| clustered-forward light iteration | `PRACTICAL_RANGE` 14 → 6 m | megastructure 94.8 → 91.3 ms. Negligible. |
| streaming churn / visibility pop-in | churn vs steady frame split | 143 churn frames in 12 393; megastructure's slow frames are *steady* frames. Not it. |
| key spotlight range | megastructure `key_range` 75 → 40 | 99.5 → 67.1 ms render. **Real, ~32 ms.** |
| key spotlight shadow pass | megastructure `key_shadows_enabled` → false | 99.5 → 63.0 ms render. **Real, ~36 ms.** |

The key light is a genuine and large contributor — and among shadow-casting registers the
render cost rank-orders almost perfectly by `key_range` (75 → 99 ms, 64 → 28 ms, 50 → 9 ms,
45 → 8.4 ms, 30 → 8.6 ms). But removing it entirely still leaves **~54 ms of the ~90 ms
excess unexplained**, so it is not the whole story.

## ROOT CAUSE: a fixed-timestep catch-up spiral

The remaining excess is not rendering at all. Three further measurements settled it.

**1. GPU pass timings (`OBSERVED2_CAPTURE_HEX_WFC_GPU`).** Megastructure's frame is 95 ms
but its render passes total **6.0 ms** — while the *fast* registers spend 9–10 ms on the
GPU. The GPU is doing less work in the slow register. Every earlier probe failed because
they were all tuning that 6 ms.

**2. Main-schedule bracket.** Of a 119 ms frame, 106 ms is the main app schedule and only
13 ms is the render sub-app.

**3. Fixed steps per rendered frame.** The whole thing:

| register | frame | 1 sim step | steps/frame | sim total | fps |
|---|---|---|---|---|---|
| megastructure | 116.1 ms | 13.1 ms | **7** | **92.0 ms** | 9 |
| facet_monument | 45.1 ms | 10.7 ms | **3** | 32.2 ms | 22 |
| overlit_grid | 20.7 ms | 7.3 ms | 1 | 7.3 ms | 48 |
| infinite_gallery | 14.7 ms | 5.3 ms | 1 | 5.3 ms | 68 |
| institutional | 12.0 ms | 2.4 ms | 1 | 2.4 ms | 84 |
| thinning | 11.0 ms | 2.0 ms | 1 | 2.0 ms | 91 |

The chain:

1. The fixed timestep is 60 Hz — **16.67 ms** per step (`game/src/screens.rs:164`).
2. In these regions one simulation step costs **13.1 ms**, 79% of the entire frame budget
   before a single triangle is drawn. In `thinning` the same step costs 2.0 ms.
3. Add ~16 ms of `Update` and rendering and the frame overruns the timestep.
4. `RunFixedMainLoop` drains its accumulator by running catch-up steps — 7 of them.
5. Each catch-up step costs another 13.1 ms, which lengthens the frame, which demands more
   steps. It only stops at Bevy's `Time::virt` **`DEFAULT_MAX_DELTA` of 250 ms** — which is
   exactly the `250000` µs maximum frame time sitting in every report in this directory,
   including the pre-existing `phase_96` and `phase_101` ones from before this
   investigation.

So the district was never the cause. Simulation cost varies ~6.5× by region, and where it
approaches the timestep budget the catch-up loop multiplies it by up to 7.

## What the expensive step was: redundant pathfinding

Timing the phases of `HexWfcMatch::step` put **`guardian.step` at 97.2%** of an expensive
step — 6 327 µs of 6 508 µs. Everything else (player movement, mutation, observation,
map knowledge) was rounding error.

Inside it, `player_sees_guardian` ran this **first**, for every player, every tick:

```rust
let visible_route = world.route_between(player.cell, guardian.cell)
    .is_some_and(|route| route.len() <= 2);
if !visible_route { return false; }
// ... only then the O(1) distance and facing checks
```

`route_between` is a full A* over the facility graph with no early cutoff, so when the
Guardian is far away or unreachable it exhausts the entire connected component — thousands
of cells of `BTreeMap` work — to answer "is the Guardian within two cells?". The O(1)
proximity test that would have rejected the same case in nanoseconds ran *after* it.

Two fixes, both semantics-preserving:

1. **`player_sees_guardian`: reorder the guards.** Every conjunct is pure, so the boolean
   result is identical; only the evaluation order changes. Proximity and facing now
   pre-filter, and the graph search runs only when the Guardian is genuinely close.
2. **`bot_command`: stop routing twice.** `bot_behaviour` routed to the objective to pick
   Seek vs Explore, then `bot_command` recomputed the identical route to follow it. A new
   private `bot_behaviour_and_route` returns both, so the A* runs once per bot per tick.

### Result (seed `0xC0FFEE`, uncapped)

| register | sim before | after both | fps before | after | steps before | after |
|---|---|---|---|---|---|---|
| megastructure | 12 330 µs | **2 459 µs** | 13 | **42** | 4 | **1** |
| facet_monument | 10 019 µs | **2 190 µs** | 24 | **51** | 2 | **1** |
| overlit_grid | 6 896 µs | 2 062 µs | 48 | 62 | 1 | 1 |
| infinite_gallery | 4 990 µs | 1 887 µs | 64 | 70 | 1 | 1 |
| institutional | 2 414 µs | 1 543 µs | 87 | 92 | 1 | 1 |
| thinning | 1 956 µs | 1 366 µs | 102 | 104 | 1 | 1 |

**Every register is back to one fixed step per frame — the spiral is gone.** The
region-to-region spread in simulation cost fell from 6.5× to 1.8×, and every step now sits
far under the 16.67 ms budget. Whole-run p95 improved 33 594 → 21 261 µs. All 152
`observed_match` tests and the full workspace sweep pass unchanged, as they must for a pair
of changes that alter only evaluation order.

## Round two: bounding the searches that saturate

The same pattern turned up three more times, all in code presentation calls **every frame**.
Each of these computes a value that stops changing past a known route cost, yet ran an
unbounded A* that expanded the whole component to find that out:

| caller | saturates at | was |
|---|---|---|
| `HexGuardianState::pressure_for` | cost 12 000 → 0.0, same as no route | unbounded |
| `player_sees_guardian` | one edge, ≤ `MAX_CONNECTION_COST` | unbounded |
| `HexWfcMatch::lantern_proximity` | the spawn→exit baseline → 0.0 | unbounded |

New `HexWfcWorld::route_within_cost` abandons the search once no candidate within the bound
remains. The pruning is **exact, not approximate**: `heuristic` scales `travel_distance` by
`COST_DOOR_LATERAL`, the cheapest edge tier, so it never overestimates; with an admissible
heuristic `f = g + h` is a lower bound on any completion, and discarding `f > max_cost` can
never drop a route that would have come in under. `bounded_routing_agrees_with_unbounded_
inside_the_bound` asserts this across a 12-seed corpus, including pairs the bound genuinely
excludes.

**And the relayout spike.** `spawn_cells` called `derive_trim` on the *whole* facility —
summarizing all ~109 k projected pieces into a `BTreeMap` and sweeping every cell — then
threw away everything but the handful of changed cells. `derive_trim_for` summarizes only
the changed cells plus their lateral neighbours (all a seam classification needs) and emits
for the changed cells alone. Ownership rules are untouched, so it is exactly the subset of
the full derivation, asserted by `scoped_trim_matches_the_owned_subset_of_the_full_
derivation`. Rebuild median roughly halved, 16 651 → 8 455 µs.

### Final state (uncapped, both seeds)

| | seed `0xC0FFEE` | default seed |
|---|---|---|
| frame median | 10 498 µs | 10 742 µs |
| frame p95 | 12 917 µs | 12 627 µs |
| worst register | megastructure, **80 fps** | shadow_screen, **83 fps** |
| fixed steps/frame | 1 everywhere | 1 everywhere |

The cell that opened this investigation at **9 fps now runs at 80**. `lantern::sync_dynamic`
went 11.4 ms → 2.4 ms. Every register on both seeds now sits between 80 and 104 fps with a
single fixed step per frame.

## Round three: the last redundant route, and what `after_hex_update` was

**The lantern baseline.** `lantern_proximity` normalises against the spawn→exit route cost.
That term is identical for every lantern and changes only on relayout, yet it was an A*
recomputed on every call, every frame. It is now a `spawn_to_exit_cost` field refreshed at
the single point the facility changes (`commit_mutation`) — safe for the digest, because
`HexMatchSnapshot` allowlists its fields rather than reflecting over the struct. A cache can
only fail by going stale, so `cached_spawn_to_exit_cost_survives_a_committed_relayout` drives
a match through a real commit and asserts the field equals a fresh computation every tick.
`lantern::sync_dynamic`: 2.4 ms → **1.16 ms**.

**`after_hex_update` was Bevy's visibility pass.** Splitting `PostUpdate` around
`TransformSystems::Propagate` and the `VisibilitySystems` sets attributes the whole 4.6 ms:

| span | cost |
|---|---|
| `post_update:visibility` (propagate + check + mark-hidden) | **3.9 ms** |
| `post_update:transform_propagate` | 0.97 ms |
| `after_hex_update` (true remainder) | 0.08 ms |

Note the marks are named for the work they bracket, not where they sit — a mark opens the
span that runs *after* it. The first labelling had them off by one and was corrected.

## Still open

**The Phase 101 gate still fails**, at p95 ≈ 30 450 µs — and notably it did not move across
any of the three fix rounds (30 488 → 30 368 → 30 454). So the p95 was never the simulation
or the rebuild.

It is vsync beating. That gate runs capped, where the budget is one 16.67 ms refresh
interval and a miss costs a whole extra one. Uncapped the same build now sits at 10.7 ms
median / 12.6 ms p95 — comfortable, but only ~75 % of budget, so the occasional spike tips
over and lands at 33 ms. Closing the gate means finding headroom in the ~8 ms of
`Update` + `PostUpdate` that remains, not in what has been fixed here.

Also still open:

- **`post_update:visibility`, 3.9 ms — now the single largest cost in the frame, and an
  entity-count problem rather than an algorithmic one.** Bevy propagates and checks
  visibility across the whole hierarchy every frame, and the facility spawns **109 155 mesh
  entities** up front. Hiding a cell stops it *rendering* but not its children being walked.
  No further micro-optimisation reaches this; it needs fewer entities. Two candidates, both
  real refactors of `view/shell.rs` worth deciding deliberately rather than assuming:
  - **Merge each cell's hull meshes into one mesh** — ~20× fewer entities (23 hulls per
    cell on average). Constrains per-piece materials to be uniform within a cell, so it
    interacts with the role/register tinting.
  - **Despawn out-of-stream cells instead of hiding them** — keeps entity count
    proportional to the streaming window rather than the facility, but moves cost to the
    streaming boundary, where it would need amortising to avoid recreating the kind of
    spike `derive_trim_for` just removed.
- **`leading_player`** still routes once per player per tick inside `guardian.step`, and
  `recovery_destination` once per blueprint. Same redundant-pathfinding pattern, no longer
  dominant.
- `key_range` / `key_shadows_enabled` remain `observed_style` values governed by
  `agents.md:46`. The earlier "~35 ms" attributed to them was measured inside the spiral and
  should be disregarded; with GPU passes totalling 6 ms they were never that expensive.

## Fixed along the way

- **Degenerate key cones.** `ShadowScreen` and `Institutional` set `key_shadows_enabled`
  with real intensity and range, but inherited `key_inner_angle`/`key_outer_angle` of
  `0.0` from the keyless `Hollow` district. Bevy builds the shadow projection as
  `perspective_infinite_reverse_rh(outer_angle * 2.0, ..)`, so a zero outer angle puts
  `1/tan(0)` = inf into the matrix — and a zero-width cone emits no light at all. Both
  registers were paying for a shadow map that lit nothing. The palette invariant test now
  asserts `0 < inner <= outer < PI/2` whenever the key casts.
- **Per-frame visibility churn.** `sync_streamed_cells` assigned `*visibility`
  unconditionally; `Mut` deref marks `Changed<Visibility>`, which is exactly the filter
  Bevy's `visibility_propagate_system` runs on, so the whole ~5 600-cell hierarchy
  re-propagated to ~109 000 children every frame to reach a state it was already in.

## Evidence index

Every run below is under `docs/evidence/arc_m/`. All are uncapped unless noted;
`vsync_uncapped` is recorded in each `timings.json` because capped medians are pinned to
the refresh interval and are not comparable between registers.

**Note the `timings.json` sitting beside this file is the first exploratory run**, kept
only for the vsync illustration above. The authoritative before/after pair is
`route_*` → `round3_*`.

**Only the measurements are committed.** Five of these runs also produced startup
screenshot sets (`startup_frame_*.png`, `first_visible_frame.png`) totalling 77 MB. They
were deliberately left out of the repository: every argument in this document is made from
the timing tables, and the frames are near-identical to one another because the camera and
facility are fixed and only the internal timings differ. Re-generate them with
`OBSERVED2_CAPTURE_HEX_WFC_PHASE96` if a visual check is ever needed.

| run | what it establishes |
|---|---|
| `district_perf` | first per-register table; every register at 60 fps (the vsync trap) |
| `district_perf_{A11CE,B0B,C0FFEE,D00D}` | multi-seed sweep; same register 17 ms in one facility, 112 ms in another |
| `pieces_C0FFEE` | resident hull-mesh counts — kills the draw-call hypothesis |
| `probe_nopractshadow_*`, `probe_range6_C0FFEE` | rejected: practical shadows, light range/clustering |
| `probe_keyrange40_C0FFEE`, `probe_keyshadowoff_C0FFEE` | key light is a contributor, not the cause |
| `stream_probe_*` | rejected: streaming churn (slow frames are steady frames) |
| `gpu_C0FFEE` | GPU passes total 6 ms in the *slow* register vs 9–10 ms in fast ones |
| `cpu_C0FFEE`, `split_C0FFEE` | 106 of 119 ms is the main schedule, not the render app |
| `sys_C0FFEE`, `steps_C0FFEE` | per-system spans; 7 fixed steps per frame — the spiral |
| `route_C0FFEE` | **baseline before the fixes** |
| `fixed_C0FFEE` | after the guardian guard reorder |
| `fixed2_C0FFEE` | after the bot route de-duplication |
| `postupdate_default` | `PostUpdate` split (labels off by one; superseded by `round3_*`) |
| `round3_default`, `round3_C0FFEE` | **final state**, corrected labels |
| `phase_101_after_fix`, `phase_101_after_all_fixes` | gate unmoved across rounds — it is vsync beating |
