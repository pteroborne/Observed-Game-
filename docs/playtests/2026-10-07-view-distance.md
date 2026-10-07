# Wider views across floors — 2026-10-07

Detailed residency now enters at **180 m** and retires at **240 m**, up from
90 / 120 m. Physical three-dimensional distance replaces the old two-storey
admission cutoff, so nearby upper floors, galleries, and stacked rooms can stay
prepared. Observation range, collision, and Architect legality are unchanged.

## Pop-in fixes

- Portal traversal still prepares connected passages, and exposed geometry in a
  conservative viewing cone covers sight through windows, across open air, and
  between disconnected floors. Nearby preparation extends to 48 m in three
  dimensions. Cached windows invalidate on height and pitch as well as heading,
  location, doors, and geometry revisions.
- Drawing follows the body intent **and** the displayed camera, independently of
  portal reach. A padded 65-degree half-cone and the nearby shadow neighbourhood
  retain meshes during camera smoothing; prepared cells retire by distance rather
  than disappearing merely because a walkable portal window changed.
- Visible upper-floor passages outrank hidden current-floor candidates. Candidate
  priority keys are calculated once, duplicate sorting is removed, and readiness
  counting does not sort discarded lists.
- Structural work has priority over decorative work, with separate bounded queues
  of 64 requests each. A new cell publishes its complete structural shell without
  waiting for bookcases or other dressing, then receives the completed dressing.
  Existing projections stay until a complete replacement is ready. Pending dressing
  retries only after cache progress or a geometry change, avoiding repeated expensive
  staging work while a worker is busy.
- Exterior proxies are hidden for an entire detailed room footprint, including
  multi-floor footprints, rather than only the anchor cell. This prevents proxy
  surfaces from overlapping the rest of a resident room.
- GPU light clustering reserves 16,384 Z-list entries up front. Wider-view trials
  required buffer growth from 2,048 through 8,192 and warned of briefly corrupted
  lighting. The final measured run contains no cluster-resize warning.

Main-thread spawning remains capped at eight parents, now with a **2 ms** budget
between cell attempts. Entry still loads the safe neighbourhood coherently. Mesh
preparation remains presentation-only and its timing cannot affect simulation.

## Measured tradeoff

[Before](../evidence/view_distance/before-timings.json) and
[after](../evidence/view_distance/after-timings.json) use the same Solo Architect
Ascent seed 1, Observer-eye route, TITAN X Pascal/i7-9700, 1440 × 900, uncapped Vulkan
presentation and GPU profiling. Both record 7,200 simulation ticks across two
matches; all 19 mutation ticks and generation numbers match. Screenshot capture
is excluded from these performance runs.

| Measurement | 90 m | 180 m |
| --- | ---: | ---: |
| Warm frame median | 11.231 ms | 10.173 ms |
| Walking p95 / p99 | 15.799 / 17.580 ms | 17.857 / 21.601 ms |
| Peak resident cell parents | 174 | 1,094 |
| Largest mutation frame window | 41.884 ms | 48.197 ms |
| Unexplained warm frames above 50 ms | 0 | 0 |

The wider range costs roughly **2.1 ms at p95** on this hardware. Its p99 remains
below 25 ms, but **p95 misses the earlier 16.67 ms target**. This is a coverage/performance
tradeoff, not a claim of uninterrupted 60 fps. The remaining 71.483 ms warmed
outlier is the Guardian catch/prison transition on tick 6,717, not ordinary walking
or streaming. Cold startup and the restart/loading interval remain separate in the
raw reports.

The first unoptimised 180 m trial measured 19.786 ms p95. Streaming bookkeeping,
visibility outside the displayed camera, repeated dressing attempts, and lighting
buffer growth were addressed before selecting this final configuration.

## Verification

`cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test` pass:
**2,835 passed, zero failed, 45 ignored**. The extended ignored suite was not run.
The 87 focused view tests include cross-floor physical distance, exposed/disconnected
upper-floor prefetch, pitch invalidation, displayed-camera retention, priority for
visible upper floors, room-footprint proxy coverage, safe shell publication,
complete replacements, retry progress, cache reset, and preserved larger custom
cluster limits.

Reproduce the performance route:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_SOLO_ROUTE=/tmp/observed-view-distance-perf \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 \
OBSERVED2_CAPTURE_HEX_WFC_GPU=1 cargo dev-run -p observed_game
```

For a separate visual run, add
`OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO=/tmp/observed-view-distance-video`. Screenshot
readback changes timing, so that run must not be used for the table above.

## Visual comparison

The separate [112-second walkthrough](../evidence/view_distance/walkthrough.mp4)
covers the first match's mutation and floor-transition sequence. It is silent,
1440 × 900 H.264 at 30 fps, encoded from timestamped game-window captures with normal
streaming budgets. Capture overhead is not included in the benchmark table.

At the Library approach, compare [90 m](../evidence/view_distance/before-80.png)
with [180 m](../evidence/view_distance/after-80.png). At the next-floor opening,
compare [90 m](../evidence/view_distance/before-84.png) with
[180 m](../evidence/view_distance/after-84.png). Previously fragmented proxy surfaces
and grey gaps in these views are replaced by detailed distant interiors. Near
walls, floors, ceilings, and practicals remain coherent in these inspected samples.
The [35-second](../evidence/view_distance/corridor-35.png) and
[62-second](../evidence/view_distance/corridor-62.png) corridor samples retain the
long views through the intervening passages.

These samples and regressions demonstrate the improvement on the established
route; they do not prove every procedurally generated seed is free of pop-in.
