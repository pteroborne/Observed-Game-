# Backrooms arrival and decision reference — 2026-10-08

The next engineering slice of the [premium quality programme](../../quality/aaa_bar.md)
improves two existing initial room footprints. The arrival has nested departure
jambs and a body-scale empty check-in counter. The decision room presents a long,
exposed exit and a screened side threshold that reveals on approach; its waiting
ledge and paired service piers give the two branches different return silhouettes.
Shallow practical housings attach to the actual ceiling.
Their exposed faces and departure downstands stay off the ceiling-cap planes,
avoiding overlapping faces with different render materials.

The ordinary Backrooms fluorescent field now occupies one in twelve ceiling bays,
instead of one in three: 600 × 1200 mm troffers spaced 2.4 × 3.6 m. Acoustic panels
dominate the ceiling again. Shared style owns the rhythm, illumination policy
and critical-signal treatment.

## Matched native views

Both sets are unretouched 1440 × 900 game-window captures, seed 1, 24 × 17 × 8,
with identical body-height camera poses and ordinary Desktop presentation.
This is the Facility race adapter with the environment held for inspection; the
authored geometry is also the geometry used by Architect Ascent. Cameras are
staged, not walked. These stills do not establish complete-session quality.

| View | Before | After |
| --- | --- | --- |
| Departure from spawn | [Before](before/vista_01_arrival_departure.png) | [After](after/vista_01_arrival_departure.png) |
| Decision-room arrival | [Before](before/vista_02_decision_arrival.png) | [After](after/vista_02_decision_arrival.png) |
| Approach reveals the side exit | [Before](before/vista_03_decision_reveal.png) | [After](after/vista_03_decision_reveal.png) |
| Return from the side bay | [Before](before/vista_04_decision_return.png) | [After](after/vista_04_decision_return.png) |

All four after views have adjacent JSON records with camera, tick and content
identity. Before catalog: `6e0aa7c414b47085fc0b2e320a6476fd497407fce8a5512518ad16f7abbbde39`;
after catalog: `b1d7b134e50ef3ccc5003ec67101546d8fbd23144f5840a780dd689316357323`.
The after simulation hash is
`675d5f9653e648e9cabc3b65a6f4dcdf09aa7393a67698075006476bb5f8cf03`.
Input version remains 13 and composition profile remains 6. No placement/port,
room-count, knowledge or permanent-protection policy changed in this slice.

The inspected after views retain readable paths, ledges, screens and equipment.
They are a first construction reference, with broad plain surfaces and distant
openings still visible as art-quality gaps. Human landmark recognition, connected
district references and two distinct compositions on every floor remain open.

## Initial-layout survey

[Raw CSV](survey.csv): seed 1 plus 23 fixed mixed seeds, using the committed
composition profile and the same **9–10 room** policy as the current production
match constructor. All 24 solved; maximum three attempts, slowest 1,382 ms on the
development build. Time is solve-only, excluding physics/view/loading.

| Floor / district | Seeds with no anchored gameplay room | Seeds with at least two |
| --- | ---: | ---: |
| 0 / Backrooms | 0 | 13 |
| 1 / Library | 5 | 2 |
| 2 / Lumen | 8 | 0 |
| 3 / Zen | 0 | 5 |
| 4 / Monument interiors | 7 | 5 |
| 5 / Monument terraces | 5 | 7 |
| 6 / Reactor | 0 | 0 |
| 7 / Sky | 0 | 21 |

These count room **anchors**, not every cell of a multilevel room, generators,
ordinary corridor compositions or Architect-played wonders. They do not count
memorable landmarks or prove cadence. Eleven seeds put the Decision room on the
ground floor; thirteen put it in the Library. The arrival reference appears in
every production seed; the detailed decision reference applies on the ground floor.

The constructor still activates repeated objectives only at the older 28 × 20 × 10
threshold, as previously documented in the Ascent integration notes. Raising that
quota alone would add many fixed room footprints. The next initial-layout pass
should instead guarantee **mutable hall compositions** around arrivals/choices,
then verify reachability, exposure, return recognition and card rewrites. Preserve
the existing mandatory room/mechanism rules while doing so.

## Reproduce and verify

The forge source is `crates/observed_authoring/src/forge/backrooms/reference.rs`.
It dresses the generated low editions after height conversion, retaining proper
body-scale dimensions. It adds no new module IDs or archetypes.

```bash
cargo run -p observed_authoring --bin tilec -- gen-tiles
cargo run -p observed_authoring --bin tilec -- build
cargo run -p observed_authoring --bin spatial_audit > /tmp/spatial-survey.csv
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_SPATIAL_REFERENCE=1 \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/observed-spatial-reference \
cargo dev-run -p observed_game
```

Focused regressions traverse every ordered decision-doorway pair without jumping,
check a full-size major through the centre/departure lanes, retain exact footprints
and supported lights, and prove the screen/reveal against both source and actual
production projection. `cargo fmt --all` and warning-free `cargo dev-clippy`
pass. The final `cargo dev-test` passes **2,851 tests, zero failures, 44 ignored**,
across 297 targets, including the full-match soak. [Machine-readable checks](checks.json)
retain the result. The complete extended instrumentation suite was not run.

The affected [production simulation instrument](simulation-timing.txt) ran
7,200 ticks: median **263.590 µs**, p95 **345.752 µs**, maximum **3.922 ms**,
five card plays, zero catches, outcome Running. This times simulation stepping,
excluding command generation and rendering. It supplies no new desktop/Deck
frame-rate or prison-transition acceptance.

The plumb charge fixture now verifies full minor-body clearance before placement;
a clear ray alone could place its capsule through a new pier. Production correctly
refuses to rotate an overlapping body. The actual charge/event assertions are retained.
The fixed climb seed completes on tick **9,786**, digest **`f1a39fdfa7834108`**;
two independent runs agree before re-pinning the previous 9,788-tick baseline.
Repeated forge regeneration reports **zero changed sources**, and a second catalog
build retains the same digest. `gen-tiles` now normalizes both comparison strings,
so the QuakeMap writer's CRLF editions do not needlessly retrigger the editor watcher.
