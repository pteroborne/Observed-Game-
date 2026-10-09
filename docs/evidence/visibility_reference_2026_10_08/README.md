# Real gaps versus streaming — 2026-10-08

The same supported seed-1 cameras were captured with normal Desktop streaming and
with every physical geometry owner loaded. The comparison found **both real open
space and missing presentation**, correcting the earlier provisional reading of
the Zen screenshots as predominantly open architecture.

## Findings and repair

Zen was hiding its complete base walls while the worker prepared paper/cedar
finish meshes. A new cell could therefore show a partial wall treatment as though
there were no wall at all. It now retains the opaque base shell until every wall
finish is ready, then swaps the treatment as a whole. Decoration can still arrive
later. The cold-recipe regression deliberately caches one finish and defers another.

Distant initial compositions and ordinary flat halls now reuse their authoritative
projected hulls, through the existing merged-mesh cache. This preserves walls,
doorways, canopy supports and terrace roof openings instead of replacing them with
generic exterior bands. The proxy omits dressing, active fixtures and shadows;
its simpler shading can still differ from the detailed cell. Complex modules retain
the existing procedural skin. Card changes rebuild affected proxies from the new
snapshot; retirement of an initial kit follows the ordinary replacement geometry.
No physical geometry, observation, collision, room topology or compatibility changes.

## Native comparison

| View | Normal streaming | All geometry |
| --- | --- | --- |
| Zen court | [normal](normal/vista_01_zen_court.png) | [full](full/vista_01_zen_court.png) |
| Monument terrace court | [normal](normal/vista_02_monument_terrace_court.png) | [full](full/vista_02_monument_terrace_court.png) |

[Before the Zen repair](before/vista_01_zen_court.png), its nearby walls were absent.
That diagnostic shot already includes the first initial-kit exterior change, but
precedes the wall fallback and expanded flat-hall proxies. The final normal view
retains opaque walls and the hallway silhouette through the doorway. The full view
has finished lattice and ceiling slats, which the normal sample is still preparing.
The Monument terrace's large blue opening remains with all geometry loaded; it is
not solely a distance-culling hole. Dark undersides/ceiling surface artifacts also
remain in the full view, so disabling streaming does not resolve that construction
issue. These static views do not prove every moving-camera pop or flicker is fixed.

All four final PNGs are unretouched 1440 × 900 native game-window captures. Their
JSON metadata verifies identical seed, content, feet, yaw and pitch for each pair.
Full mode has **2,272 / 2,272** geometry owners resident and shown, with zero shown
exterior proxies. Normal samples show 18 and 17 detailed owners respectively,
with 2,243 and 2,244 exterior cells. The facility has 2,263 Hall cells, 14 Room
cells, 916 Air cells and 71 Void cells; multi-cell modules explain the different
owner count. The per-cell census records physical pieces and initial variants.
“Shown” means requested entity visibility, not proof of camera-frustum visibility.

The Facility adapter holds the environment for inspection and uses the same
physical geometry as Architect Ascent. Cameras are staged, not walked. Simulation
identity remains `9922a1750e5c27dd861c3e77035c096a492fa6dd4ef16f5325f71ef16e149adb`;
input version is 13 and composition profile version is 8.

## Reproduce

Copy the two [capture configuration files](config/) to an isolated directory, then
run each mode separately. Builds share one cache and must not overlap.

```bash
mkdir -p /tmp/observed-reference-config
cp docs/evidence/visibility_reference_2026_10_08/config/*.json /tmp/observed-reference-config/
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_CONFIG_DIR=/tmp/observed-reference-config OBSERVED2_SEED=1 \
OBSERVED2_COMPOSITION_REFERENCE=1 \
OBSERVED2_REFERENCE_POSES=zen_court,monument_terrace_court \
OBSERVED2_REFERENCE_RESIDENCY=normal \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/observed-normal \
cargo dev-run -p observed_game
```

Repeat with `OBSERVED2_REFERENCE_RESIDENCY=full` and a different capture directory.
Full mode bypasses presentation residency limits only for explicit construction
references; it is not a shipping quality preset and does not alter simulation.
Its frame rate is not a production performance measurement.

## Fresh performance measurement

[Raw frame report](performance/timings.json) and [Desktop budget check](performance/desktop-budget.json):
seed 1, 24 × 17 × 8 Ascent, 7,200 ticks in one run, 1440 × 900, uncapped,
without screenshot readback or GPU timestamp queries. On the same development
machine used for the earlier quality record, warm-frame p95 is **19.045 ms**, p99 **23.737 ms**,
and warm maximum **49.375 ms**. Cold view construction is **7.014 s**; the
largest early frame is **365.866 ms**, outside the reported warm-frame budget sample. Desktop acceptance
remains **red** (16.667 ms p95 / 33.333 ms maximum). This is a new measurement, not
a matched claim of performance improvement over the earlier content/profile.
Proxy construction/preparation still needs optimisation before the AAA frame gate.

An earlier [GPU-instrumented attempt](performance/gpu-instrumented-stall.txt) stalled
following a swap-chain timeout and produced no progress or timing report. It was
stopped after six minutes. The retry without timestamp queries completed; its
per-pass GPU timing is unavailable. No successful GPU-budget/device acceptance is
inferred from this recovery. Steam Deck and physical LAN checks remain unrun.

Final engineering verification: `cargo fmt --all`, warning-free `cargo dev-clippy`,
and `cargo dev-test` pass: **2,863 passed, zero failed, 44 ignored**, across
297 targets. The cold-wall, doorway/terrace silhouette and capture-mode tests pass,
as do full-match soak and replay/determinism checks. [Checks](checks.json) and the
[evidence manifest](manifest.json) record the results. The complete extended
instrumentation suite was not run; no simulation source changed in this pass.
