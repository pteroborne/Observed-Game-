# Zen and Monument initial compositions — 2026-10-08

This [quality-programme](../../quality/aaa_bar.md) slice extends the
[Library/Lumen system](../initial_halls_2026_10_08/README.md) through Zen, Monument
interiors and Monument terraces. Each floor receives a connected three-cell
lower gallery near arrival and a distinct taller junction court near departure.
The solved rooms and door topology remain unchanged.

Zen galleries use separate wall-backed screen rails; their courts use low garden
ledges. Monument interiors use heavier peripheral ledges and wall courses.
The terrace floor has separate source modules: sealed sides become guarded
parapets, gallery canopies rest on piers, and courts have no authored roof cap.
Their court lights mount to an existing doorway rather than an absent ceiling.
Neighbouring ordinary halls and floors can still enclose a terrace's view; this
pass supplies the physical kit, not a globally cleared outdoor skyline.

## Native inspection

Ten unretouched seed-1 game-window views retain Library/Lumen coverage and add:

- [Zen gallery](native/vista_05_zen_gallery.png) and [court](native/vista_06_zen_court.png)
- [Monument interior gallery](native/vista_07_monument_gallery.png) and [court](native/vista_08_monument_court.png)
- [Monument terrace gallery](native/vista_09_monument_terrace_gallery.png) and [court](native/vista_10_monument_terrace_court.png)

The cameras are staged at supported body height in the production 24 × 17 × 8
facility. The Facility adapter holds the environment for inspection; Architect
Ascent shares the physical geometry. These captures establish neither a walked
match nor human recognition of the landmarks. Adjacent JSON records the actual
module, seed, pose, tick and compatibility identity. Exposed undersides in
neighbouring ordinary halls remain a wider construction issue. Repeated shared
surface patterns still dominate these views; the new perimeter forms do not yet
establish strong focal points or human landmark recognition.

## Simulation and content

All 24 fixed survey seeds receive ten compositions and thirty selected hall
cells. Initial choices remain revision-scoped and mutable: a card rebuild retires
only its affected choice, including a same-door physical replacement. Previews
and incremental/fresh projection agree. No protected cells, topology pins or
automatic team discovery are added. All doorway pairings in all six rotations
are checked with both Observer and full-size major controllers, without jumping.

Sixty added maps cover both forms in Zen, Monument interiors and terraces.
They stay outside the ordinary card/relayout lottery. The source archive now has
**538** maps: **442 active**, 96 retired. Profile compatibility advances **7 → 8**;
input compatibility remains **13**. Peers must use matching content.

Catalog: `d1602b954c408239ab8dd4944cc2fcbf08007d9f460d8c1c2c5eac76f4b1fb09`.
Profile: `8e419298dca0e8132e69b43aeaabbc9c67ab847a3ecdcc344228c4be3d60322f`.
Simulation: `9922a1750e5c27dd861c3e77035c096a492fa6dd4ef16f5325f71ef16e149adb`.

## Reproduce

```bash
cargo run -p observed_authoring --bin tilec -- gen-tiles
cargo run -p observed_authoring --bin tilec -- build
cargo run -p observed_authoring --bin tilec -- profile-halls on
cargo run -p observed_authoring --bin spatial_audit > /tmp/district-halls.csv
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_COMPOSITION_REFERENCE=1 \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/district-hall-views \
cargo dev-run -p observed_game
```

`profile-halls on|off` explicitly migrates version 6 or 7 to version 8, preserving
other controls and writing the profile digest together. Selection applies only
to production-sized boards; compact labs keep their ordinary modules.

## Acceptance still open

Reactor and Sky are the next spatial slice. Human landmark recognition, complete
20–30 minute matches, graphical frame budgets, Deck and physical LAN acceptance
remain open. These references do not close the eight-floor milestone.

The [refreshed survey](survey.csv) solves all 24 seeds, each with ten compositions
and thirty initial choices; the slowest solve plus selection is **1,451 ms**.
The [7,200-tick simulation probe](simulation-timing.txt) measures median
**256.916 µs**, p95 **336.362 µs**, maximum **4.079 ms**, five plays, zero catches,
outcome Running. It excludes command generation and rendering and does not
establish catch/prison-transition acceptance or a graphical frame budget.

Final verification: `cargo fmt --all`, warning-free `cargo dev-clippy`, and
`cargo dev-test` pass: **2,860 passed, zero failed, 44 ignored**, across 297 targets.
Full-match soak, peer/replay determinism and mutation/projection checks pass.
[Machine-readable checks](checks.json) and the [evidence manifest](manifest.json)
retain the actual results. The complete extended instrumentation suite was not run.
