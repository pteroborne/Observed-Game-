# Initial Library and Lumen compositions — 2026-10-08

The next [quality-programme](../../quality/aaa_bar.md) slice selects two distinct
connected three-cell hall compositions in each target district: a lower canopied
gallery near the floor arrival and a taller court near the floor departure. The
court starts at a junction with at least three exits; its anchor is at least six
lateral cells from the gallery. Selection uses existing solved connections and
does not change rooms, door topology, floor count or solver draws.

Library uses shelving, piers and a suspended canopy; Lumen uses a quiet canopy,
tall piers and peripheral platform ledges. Canopies have a centre hanger that
remains when neighbouring geometry changes. These first references retain their
authored enclosure against unbuilt flanks. A card replacement returns to the
ordinary kit, including its normal outdoor treatment.

## Native inspection

Four unretouched 1440 × 900 game-window captures on seed 1:

- [Library gallery](native/vista_01_library_gallery.png)
- [Library court](native/vista_02_library_court.png)
- [Lumen gallery](native/vista_03_lumen_gallery.png)
- [Lumen court](native/vista_04_lumen_court.png)

These are supported body-height viewpoints in the real 24 × 17 × 8 physical
facility, with ordinary Desktop presentation and district lighting. The Facility
race adapter holds the environment for inspection; Architect Ascent shares this
geometry. The camera is staged, not walked. Adjacent JSON files record seed,
camera, tick, input compatibility, content identity and the actual selected module.
The initial exposed-site attempt was rejected during review: ordinary open-edge
trimming was stripping the intended enclosure. The corrected gallery/court kits
retain their authored shell. Neighbouring ordinary halls can still show exposed
undersides through their doors; that wider construction gap remains open. Only
the corrected views are retained.

## Simulation and mutability

`observed_authoring::initial_composition` selects physical modules once after the
WFC solve. The facility stores each selected runtime variant; physical projection
honours it at cell revision zero. No pin, fixed structure, observation exemption,
extra card, protected route or team discovery is added. Unrelated local changes
do not reroll the composition globally.

A card rebuilding an initial module retires that choice, even when its door layout
stays the same: this is a real physical-kit change, so it is accepted rather than
classified as a no-op. The preview retires the same choice. Afterward the existing
no-op refusal remains. Active choices participate in snapshot digests. Rules,
physical revisions, incremental colliders and fresh projection agree after the play.

Forty new source maps provide ten canonical flat-hall masks in each of two kits
and two districts, expanded into all six rotations. They have ordinary door
interfaces and supported sources. They are reserved for explicit initial selection,
so adding them does not change the ordinary card/relayout module lottery.
The source archive contains **478** maps; **382** remain active and 96 retired.

Composition compatibility advances **6 → 7**, with the hashed
`initial_hall_compositions` control enabled. The input wire remains version **13**.
Catalog: `2bd68f6ac106342c842c5c4cb67c55385a559b2ab6c37fb9fd2f4715971d0d81`.
Profile: `42603bf9f2b4c546aff2b45875a2b95cb19b3b20504a1a0902d223c1fac30847`.
Simulation: `15efcbcef864cfa37fb9ffa23238c526dc8ca7f482b61e2141295332e2094d89`.

## Reproduce

```bash
cargo run -p observed_authoring --bin tilec -- gen-tiles
cargo run -p observed_authoring --bin tilec -- build
cargo run -p observed_authoring --bin tilec -- profile-halls on
cargo run -p observed_authoring --bin spatial_audit > /tmp/initial-halls.csv
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_COMPOSITION_REFERENCE=1 \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/initial-hall-views \
cargo dev-run -p observed_game
```

`profile-halls on|off` explicitly upgrades a version-6 profile to version 7 while
retaining its other controls; it writes the profile and digest together. Initial
hall selection applies only to production-sized boards (at least 24 × 17 × 8).
Compact regression/lab boards retain their ordinary initial modules.

## Acceptance still open

The 24 fixed seeds have four complete compositions and twelve selected hall cells
each. This is structural coverage, not proof that a human recognises the places or
chooses satisfying routes. The remaining district references, all-floor landmarks,
20–30 minute human matches, graphical frame budgets and physical Deck/LAN acceptance
remain open. Final gate and affected simulation measurements are recorded below.

The [refreshed survey](survey.csv) solves all 24 seeds, with four compositions and
twelve hall choices each; the slowest solve plus selection is 1,486 ms on the
development build. The [7,200-tick simulation instrument](simulation-timing.txt)
measures median **264.302 µs**, p95 **349.531 µs**, maximum **4.200 ms**, five plays,
zero catches, outcome Running. It excludes command generation and rendering and
does not establish a graphical frame budget or prison-transition acceptance.

Final verification: `cargo fmt --all`, warning-free `cargo dev-clippy`, and
`cargo dev-test` pass: **2,860 passed, zero failed, 44 ignored**, across 297 targets.
The full-match soak, peer/replay determinism and new mutation/projection checks
pass. The compact climb baseline remains tick **9,786**, digest **`f1a39fdfa7834108`**.
[Machine-readable checks](checks.json) and [evidence manifest](manifest.json) retain
the results. The complete extended instrumentation suite was not run.
