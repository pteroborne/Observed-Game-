# Reactor and Sky initial compositions — 2026-10-08

This [quality-programme](../../quality/aaa_bar.md) slice extends the mutable initial
hall references through the final two districts. Reactor gets a lower service
gallery and a taller court with wall-backed equipment banks and heavy horizontal
courses. Sky gets a pier-supported perimeter portico and an open court, both with
guarded sealed sides and a central roof opening. Sky practicals mount to real door
frames rather than an absent ceiling. Shared district materials remain authoritative.

Each reference is a connected three-cell composition over the existing solved
hallway network: gallery near floor arrival, junction court near departure, with
separated anchors. All 24 fixed survey seeds receive fourteen compositions and
forty-two selected cells across floors 1–7. The Backrooms floor retains its
[earlier room references](../spatial_reference_2026_10_08/README.md).

Selection changes no rooms, doors, fixed structures, observation protection or
team discovery. A card rebuilding a selected cell retires only its initial choice,
even for a same-door physical replacement; previews and incremental/fresh geometry
agree. The new sources stay outside the ordinary card/relayout lottery. Observer
and full-size major controllers traverse all doorway pairs and rotations without
jumping. The distant mesh reuses the physical hulls and retains Sky's central opening.

## Native references

Four unretouched seed-1 game-window captures:

- [Reactor gallery](native/vista_01_reactor_gallery.png)
- [Reactor court](native/vista_02_reactor_court.png)
- [Sky portico](native/vista_03_sky_gallery.png)
- [Sky court](native/vista_04_sky_court.png)

These are staged, supported body-height viewpoints in the real 24 × 17 × 8
facility, with normal Desktop streaming. The Facility adapter holds the environment;
Architect Ascent uses the same geometry. Adjacent JSON retains pose, content identity
and the physical/presentation cell census. They establish neither a walked match
nor human landmark recognition. Optional finishes can still arrive after the base
shell; the previous repair keeps Zen walls opaque during that preparation.

The inspected Reactor views show lower enclosure versus a taller chamber, though
the shared panel texture still dominates and the service banks need richer functional
detail. Sky's open centre and peripheral shelter read more clearly, with a distinct
empty court and cast light on the deck. Standard full-height doorway frames remain
for compatibility with neighbouring modules. These references are still sparse;
they do not establish satisfying player choices or recognisable landmarks.

## Content and compatibility

Forty added source maps cover ten canonical masks in each of the four new forms,
expanded into all six rotations. The archive is **578 maps**, **482 active** and
96 retired. Profile compatibility advances **8 → 9**; input remains **13**.
Peers must use matching content.

Catalog: `bade05845c5bdf43447f0411fd226b63b6d9f133ddd583d9da7bc3dbd344344c`.
Profile: `5543702475eff2359af0e25f424b93dbff4854985d26c116af138f96cbfb0ba9`.
Simulation: `259e2a8f29fd1607dc84a0e0d02d23c256fdb4325d32a6a390fe8f3e8e5331b3`.

## Reproduce

```bash
cargo run -p observed_authoring --bin tilec -- gen-tiles
cargo run -p observed_authoring --bin tilec -- build
cargo run -p observed_authoring --bin tilec -- profile-halls on
cargo run -p observed_authoring --bin spatial_audit > /tmp/reactor-sky-survey.csv
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_COMPOSITION_REFERENCE=1 \
OBSERVED2_REFERENCE_POSES=reactor_gallery,reactor_court,sky_gallery,sky_court \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/reactor-sky-views \
cargo dev-run -p observed_game
```

`profile-halls on|off` explicitly migrates versions 6, 7 or 8 to 9 while retaining
other controls and writing the digest together. Production-sized boards receive
initial compositions; compact regression/lab boards retain ordinary modules.

## Acceptance remains open

This completes the first district-reference coverage through Sky, not the eight-floor
AAA milestone. Human recognition, readable choices/reveals, complete-match pacing,
more convincing construction, moving-camera ceiling/flicker checks and physical
Deck/LAN acceptance remain open. The [last fresh frame measurement](../visibility_reference_2026_10_08/README.md)
was red, with 19.045 ms warm p95 and 7.014 s cold view construction; those figures
precede this content revision and do not establish its performance acceptance.

The [refreshed survey](survey.csv) solves all 24 seeds, each with fourteen
compositions and forty-two selected hall cells. Its slowest solve plus selection
is **1,462 ms**. The affected [7,200-tick simulation instrument](simulation-timing.txt)
measures median **255.634 µs**, p95 **336.962 µs**, maximum **3.846 ms**, five plays,
zero catches, outcome Running. It excludes command generation and rendering and
does not establish catch/prison-transition or graphical frame acceptance.

Final engineering verification: `cargo fmt --all`, warning-free `cargo dev-clippy`,
and `cargo dev-test` pass: **2,864 passed, zero failed, 44 ignored**, across 297 targets.
Source reproducibility, all module doorway pairs, 24-seed placement, card/preview
retirement, incremental/fresh geometry, Sky roof openings, full-match soak and
replay/determinism checks pass. [Checks](checks.json) and the [evidence manifest](manifest.json)
retain the results. The complete extended instrumentation suite was not run.
