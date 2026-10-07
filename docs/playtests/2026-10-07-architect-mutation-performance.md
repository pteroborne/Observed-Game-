# Architect mutation performance — 2026-10-07

Architect plays still pass the existing legality checks and commit their complete
physical change on the authoritative fixed tick. Preparation is optional: instant,
remote, and bot plays do not require a local preview. The rendering worker receives
only immutable local hull recipes; its completion time cannot affect simulation,
observation, movement, or which simultaneous command succeeds.

## Matched measurements

The [before](../evidence/architect_mutation/before-timings.json) and
[after](../evidence/architect_mutation/after-timings.json) recordings use Solo Architect
Ascent seed 1, Observer-eye mode, 1440 × 900, uncapped Vulkan presentation, GPU
profiling, and ordinary streaming budgets on the TITAN X Pascal/i7-9700. Each records
7,200 simulation ticks across two matches. All 19 mutation ticks and generations
match. The [per-mutation CSV](../evidence/architect_mutation/mutations.csv) retains
individual fixed-step measurements and the final frame windows.

| Measurement | Before | After |
| --- | ---: | ---: |
| Warm walking p95 / p99 | 15.566 / 17.593 ms | 15.799 / 17.580 ms |
| Largest mutation-associated frame | 66.271 ms | 41.884 ms |
| Mutation fixed-step median / maximum | 26.437 / 42.753 ms | 17.301 / 21.160 ms |
| Presentation catalogue update median / maximum | 4.146 / 4.458 ms | 0.139 / 0.179 ms |
| Warm mutation frames above 50 ms | 6 | 0 |
| Unexplained warm streaming frames above 50 ms | 0 | 0 |

The largest final warmed frame is **64.312 ms**, at the Guardian catch/prison
transition on tick 6,717. That remaining transition cost is separate from this
mutation pass. Mutations can still exceed a 16.67 ms frame; this does not claim
uninterrupted 60 fps. Walking percentiles meet the 16.67 ms p95 / 25 ms p99 targets.

Cold view construction is reported separately: before 5.231 / 5.252 seconds,
after 5.282 / 5.227 seconds. The final roughly 8.75-second restart interval and
initial streaming frames are excluded from the warmed cohort, which begins at
300 ticks in each match. Raw frames and event-context hitches remain in both reports.

The final report is diagnostic schema **5**. Its mutation frame measurement takes
the maximum of the submission and following wall-clock intervals, covering fixed
work and subsequent rendering. The old recorder kept only the following interval,
which understated mutation frames; the baseline maximum above therefore comes from
its raw event-associated hitch records, not that old per-commit field. Simulation
input compatibility remains **11**: these optimisations preserve rules and collision
content, so no deterministic baseline or authored-content hash was changed.

## What changed

- Scene-owned, bounded caches reuse exact local convex shapes and warm authored
  hulls before play. Collider IDs, transforms, friction, collision semantics, and
  atomic rejection remain per-instance. Cold shapes retain the normal bake path.
- Sparse cell-to-piece indices support changed-owner projection reads and trim
  halos. Traversal guide compatibility maps update affected keys. Presentation
  retains stable piece IDs instead of packed-vector offsets and patches only
  affected cells and whole-room owners, including neighbours whose exposure changed.
- Owned replay history copies its pointer/index buffers and patches changed owners.
  Previous frames remain immutable. Final replay recording has a 0.974 ms median
  and 1.130 ms maximum on mutation ticks; earlier phase probes measured roughly
  9–10 ms scanning the whole structure.
- The Rogue's nearest-Guardian score uses one bounded multi-source search instead
  of overlapping searches per released Guardian. Distances and decision scoring
  remain equivalent, including closed doors, duplicate origins, and search limits.
- One presentation worker prepares demanded structural and decorative mesh recipes
  before optional catalogue warming. Requests deduplicate and cap at 128; the merged
  mesh cache caps at 4,096 recipes. Mesh identity includes the exact hull multiset,
  so rewrites cannot reuse stale geometry under an unchanged tile name.
- Resident parents remain drawn until a complete replacement is ready. Cold staging
  parents, including their lights and trim, are discarded when a decorative mesh is
  still pending. Publication still follows the normal eight-parent / 3 ms budget;
  entry construction retains its safe-neighbourhood contract. Reset drops pending
  tasks and caches with the visual resource.

Early trials exposed two roughly 100 ms Library streaming hitches. Moving optional
catalogue warming alone did not fix them: generated bookcase recipes were still
built during demanded cell spawning. The final path includes those decorative
recipes, and the final measurement has neither hitch. Intermediate trials are not
presented as acceptance evidence.

## Verification

`cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test` pass on the final source:
**2,828 passed, zero failed, 45 ignored**. Focused regressions cover cached/cold
controller equality, stale-preview refusal after observation changes, simultaneous
seats, nearest-hunter equivalence, packed-vector removals/restoration, immutable
replay history, exact mesh identity, bounded requests/cache reset, and complete
parent replacement without leaking staging meshes or children.

The separate 7,200-tick production Ascent probe passed: median **237.389 µs**,
p95 **333.693 µs**, maximum **4.834 ms**. The extended ignored suite was not run.

Reproduce the performance route without screenshot readback:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_SOLO_ROUTE=/tmp/observed-mutation-perf \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 \
OBSERVED2_CAPTURE_HEX_WFC_GPU=1 cargo dev-run -p observed_game
```

The separate [walking capture](../evidence/architect_mutation/walkthrough.mp4)
uses the same route and normal budgets. It covers the first match's mutation sequence
(111.97 seconds), stopping before the unrecorded restart/loading gap. It is silent
and uses recorded frame timestamps. Screenshot overhead is excluded from the performance
recordings above.

Matched views at [80 seconds before](../evidence/architect_mutation/before-80.png)
and [after](../evidence/architect_mutation/after-80.png), and at
[84 seconds before](../evidence/architect_mutation/before-84.png) and
[after](../evidence/architect_mutation/after-84.png), retain the pre-existing fragmented
far exterior silhouettes. This pass does not repair that exterior geometry. Nearby
corridor shells remain present in the inspected turn and mutation sequences.

To capture the walkthrough, add
`OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO=/tmp/observed-mutation-video` to a separate run.
