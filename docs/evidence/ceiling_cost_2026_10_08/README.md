# Ceiling overlap and hall-proxy cost — 2026-10-08

This [quality-programme](../../quality/aaa_bar.md) pass removes duplicate underside
surfaces and reduces far-hall draw batches without changing the physical facility.

## Construction repairs

A flat exterior underside was drawn at the same height as an authored floor
underside. Projected halls now use their actual floor underside; they do not add
that second plate. Complex-module fallback plates disappear while their complete
footprint is covered by shown detail. Deep stepped hanging structures remain
separate and still follow overview visibility.

Floors and ceilings also own same-facing coplanar areas shared with wall/trim
meshes. The existing polygon clipping now subtracts just those covered regions
across material groups. Exposed faces, different heights, opposite normals and
roof openings remain. The same ownership data travels through structural mesh
preparation, ordinary cell projection, Zen finishes and far proxies. Cache identity
includes the ownership hulls, so a changed floor cannot reuse a stale wall mesh.
These are render meshes; collision and simulation retain the original hulls.

Identical local geometry can share a recipe across different cell origins.
Coordinates no longer fragment that identity, and signed zero has one canonical
numeric representation. Other hull coordinates remain exact. Far hall surfaces
are batched into their two existing finishes (roof and facade), preserving the
physical silhouette while limiting each proxy to two material draws. Active
practicals, near dressing and shadow policy retain their existing paths.

## Native comparison

- [Before, full geometry](before/vista_01_monument_terrace_court.png)
- [After, full geometry](after-full/vista_01_monument_terrace_court.png)
- [After, normal streaming](after-normal/vista_01_monument_terrace_court.png)

Unretouched 1440 × 900 seed-1 game-window captures at the same supported body-height
pose. The full-world pair disables residency filtering; the normal sample uses
Desktop streaming. The dark overlapping underside patches are reduced in the
inspected view. The terrace opening and exposed structural undersides remain.
Shared textures and lighting still make some areas dark; these static captures do
not prove every moving-camera flicker or every ceiling is repaired.

Their JSON reports verify identical seed, content, feet, yaw, pitch and complete
physical cell census. The Facility adapter holds the environment for inspection;
Architect Ascent shares this geometry. Cameras are staged, not walked. Input stays
**13**, composition profile **9**, simulation identity
`259e2a8f29fd1607dc84a0e0d02d23c256fdb4325d32a6a390fe8f3e8e5331b3`.

## Reproduce

Use the [previous isolated capture configuration](../visibility_reference_2026_10_08/config/)
with the current assets. Run the two modes separately; builds share one cache.

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_CONFIG_DIR=/tmp/observed-reference-config OBSERVED2_SEED=1 \
OBSERVED2_COMPOSITION_REFERENCE=1 OBSERVED2_REFERENCE_POSES=monument_terrace_court \
OBSERVED2_REFERENCE_RESIDENCY=full \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/ceiling-full \
cargo dev-run -p observed_game
```

Repeat with `OBSERVED2_REFERENCE_RESIDENCY=normal` and a different output directory.
For screenshot-free timing, omit all reference/camera variables:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_CONFIG_DIR=/tmp/observed-reference-config OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_SOLO_ROUTE=/tmp/ceiling-performance \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 \
cargo dev-run -p observed_game
python3 tools/check_quality_report.py /tmp/ceiling-performance/timings.json --preset desktop
```

The performance run uses Ascent's solo workload, normal Desktop presentation and
no screenshot readback or GPU timestamp queries. Before and after use the same
content/profile, board, preset, seed and development machine. Full-world reference
frame rates are not shipping performance evidence. Complete human matches, moving
camera checks, Deck and physical LAN acceptance remain separate.

## Matched timing results

[Before](performance/before.json) and [after](performance/after.json) are complete
7,200-tick Ascent workload runs (the baseline finished at tick 7,201). The development
machine, seed, 24 × 17 × 8 grid, Desktop preset and content/profile match. No
screenshot readback or GPU timestamp queries are included. These single-run
comparisons are measurements, not a statistical performance guarantee.

| Measurement | Before | After |
| --- | ---: | ---: |
| Warm median | 11.353 ms | 10.443 ms |
| Warm p95 | 18.368 ms | 17.763 ms |
| Warm p99 | 22.182 ms | 22.104 ms |
| Warm maximum | 46.706 ms | 56.542 ms |
| Cold view construction | 7.056 s | 7.617 s |
| Mesh misses over the run | 23,238 | 13,277 |

[Desktop acceptance remains red](performance/after-budget.json): p95 exceeds
16.667 ms and maximum exceeds 33.333 ms. Median/p95 and mesh misses improved in
these samples, while cold construction and the measured maximum worsened. This
pass repairs render correctness and reduces proxy draws; it does **not** close
frame pacing or cold-start acceptance. Further preparation/scheduling work remains.
A first ownership-only candidate had 9.323 s cold construction and was replaced
by the batched version; its figures are not presented as final acceptance.

The fully loaded before/after construction views share the same physical census.
The batched normal view retains the opening and structural surfaces with simpler
far shading. Artwork/moving-camera checks, catches/prison transitions, Deck and
physical LAN acceptance remain open. No simulation source or content was changed.

Final engineering gate: `cargo fmt --all`, warning-free `cargo dev-clippy`, and
`cargo dev-test` pass: **2,871 passed, zero failed, 44 ignored**, across 297
targets. The new ownership, cross-cell/signed-zero cache, changed-slab cache,
underside coverage/overview and two-draw proxy regressions pass. Full-match soak,
peer/replay determinism, supported fixtures and traversal checks also pass.
[Checks](checks.json) and the [manifest](manifest.json) retain actual results.
The complete extended instrumentation suite was not run; simulation was unchanged.
