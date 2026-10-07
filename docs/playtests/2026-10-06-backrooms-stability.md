# Backrooms stability, atmosphere, and targeting — 2026-10-06

Follow-up: the [Architect mutation performance pass](2026-10-07-architect-mutation-performance.md)
reduces the matched worst mutation frame from 66.3 to 41.9 ms. The prison transition
remains a separate outlier.

This follows the [quality pass](2026-10-06-backrooms-quality.md) and addresses
the seven Solo playthrough observations: hitches, pop-in, surface shimmer,
roof landings, quiet Guardians, missing atmosphere, and weak targeting feedback.

## Changes

- Detailed residency reaches 90 m and retires at 120 m. Conservative typed-port
  windows respect closed doors, keep room footprints together, and prefetch
  around corners. The occupied room takes priority. Ordinary mesh preparation
  stops after a 3 ms budget between cells, retaining the eight-cell maximum.
  Collision and observation remain complete regardless of rendering residency.
- Distant exterior proxies remain visible when detailed parents are hidden and
  use the lower Backrooms enclosure height. Baffle, cut, and recess editions have
  three times the selection weight of compatible unobstructed editions.
- Permanent decorative shadow exclusions survive storey changes. Redundant
  upper caps and duplicate ceiling construction are removed. First-person TAA
  stabilizes subpixel detail; camera cuts and geometry changes reset history.
- Projection carries explicit enclosure-ceiling semantics. Descending Observers
  pass through those roofs, retain their horizontal position and momentum, and
  land inside lower floors. Raised galleries retain their storey membership;
  unsupported edge recovery chooses the built floor below. Movement filtering is per body and stable collider
  ID; structural rays, platforms, prison movement, and Guardian movement retain
  their collision rules. Compatibility is input version **11**, with regenerated
  catalogue and content hashes.
- Guardian loops, transitions, impacts, and catch cues gain approximately 3 dB.
  The physical soundscape adds ventilation, powered ballast buzz, localized
  machinery/water, and occasional creaks. It uses existing volume preferences
  and bounds environmental audio to two crossfading beds and four emitters.
- The outlined 18 px reticle has a clear centre, different readiness shapes,
  and action/range text. Visible kinetic candidates can report excessive range;
  fixture and door readiness follows authoritative action queries. Walls prevent
  revealing hidden targets. Power, charge, and cooldown refusals remain distinct.

The audio assets are original reproducible synthesis from
`tools/generate_backrooms_audio.py`, recorded in [the source ledger](../../assets/SOURCES.md).

## Native measurements

The [final timing report](../evidence/backrooms_stability/timings.json) uses
Observer-eye mode, Solo Architect Ascent, seed 1, 1440 × 900, Vulkan, uncapped
presentation and GPU profiling on the TITAN X Pascal/i7-9700. It records 7,200
simulation ticks: **120 seconds of play across two matches**. The harness restarts
the same seed through normal state cleanup when the first match ends early.

| Measurement | Result |
| --- | ---: |
| Warm frame median | 10.764 ms |
| Warm frame p95 / p99 | 15.433 / 17.754 ms |
| Backrooms frame p95 / p99 | 15.334 / 18.249 ms |
| Total recorded frame samples | 10,804 |
| Warm samples | 9,885 |
| Startup view builds | 5.279 / 5.217 seconds |
| Largest warm outlier | 62.512 ms |

The percentile targets pass. Remaining warm outliers above 50 ms all coincide
with architecture mutations or the Guardian catch/prison transition, with events,
streaming work, and mesh-cache counters retained in the report. These spikes are
still perceptible; this pass does not claim uninterrupted 60 fps. The raw maximum
includes the roughly 9.4-second restart/loading interval, which is excluded from
the warmed cohort. Mesh counters describe each cache instance and reset with a
new match.

The [earlier baseline](../evidence/backrooms_stability/before-timings.json) used
the old chase-camera driver and capped frame deltas. It is retained for provenance,
not presented as an equivalent first-person comparison. The previous unbudgeted
90 m trial produced a roughly 70 ms ordinary streaming hitch; the budgeted run's
remaining large outliers have mutation or teleport context.

Reproduce the performance workload with normal streaming budgets:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_SOLO_ROUTE=/tmp/observed-stability-perf \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 \
OBSERVED2_CAPTURE_HEX_WFC_GPU=1 cargo dev-run -p observed_game
```

For a separate walking capture, add
`OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO=/tmp/observed-stability-walk`. Its PNG frames
and CSV timestamps preserve the ordinary game clock and actual pauses. Screenshot
readback adds overhead, so that recording is not the performance measurement.

## Verification and evidence

The [two-minute walkthrough](../evidence/backrooms_stability/walkthrough.mp4)
uses Observer-eye mode, normal streaming, recorded frame timestamps, and actual
game audio. Real pauses and the end-of-match transition are retained. The
sound recording used an isolated application sink, not a microphone or other
applications. The delivered mix peaks at **-5.2 dBFS**. Both MP4s decode cleanly.

The [power and targeting demonstration](../evidence/backrooms_stability/power-targeting.mp4)
shows excessive range, interaction readiness, restoration, recharging, and the
unpowered station. Inspect [restored panels and interaction readiness](../evidence/backrooms_stability/power-4-generator-restored-1280x800.png)
and [the unpowered station](../evidence/backrooms_stability/power-8-station-dead-1280x800.png).

Native falls end [inside the Backrooms](../evidence/backrooms_stability/low-240.png)
and [inside the Library](../evidence/backrooms_stability/tall-240.png). The
[low](../evidence/backrooms_stability/low.json) and
[tall](../evidence/backrooms_stability/tall.json) records retain before/after
coordinates and cell identity. Both retain their exact horizontal coordinates.
To reproduce the staged production-controller proof:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_BACKROOMS_REFERENCE=1 \
OBSERVED2_CAPTURE_ROOF_FALLS=/tmp/observed-stability-falls \
cargo dev-run -p observed_game
```

Regression coverage includes one-way low/tall roofs, upward collision, preserved
platforms and structural rays, storey membership of raised galleries, portal
prefetch and door invalidation, permanent shadow exclusions, real beveled-cap
fluorescent construction, occluded targeting, distinct reticle shapes, bounded
ambience voices/crossfades, and muting audio before its sink has loaded.

`cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test` passed on the final
source: **2,815 passed, zero failed, 45 ignored**. The separate 7,200-tick production
Ascent probe also passed: median 237.907 µs, p95 350.040 µs, maximum 5.151 ms.
The complete extended ignored suite was not run.
