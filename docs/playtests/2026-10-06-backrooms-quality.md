# Backrooms quality pass — 2026-10-06

The Backrooms now use a coherent low-ceiling kit, original PBR finishes and a
regular fluorescent ceiling field. The target is overlit liminal realism at
60 fps, with the existing actors, equipment and gameplay signals retained.

## Construction and compatibility

- `LowDoor` gives Backrooms thresholds three metres of clearance above the
  floor. It uses the previously unused lateral signature lane; existing valid
  signatures retain their encoding. Normal-height districts retain `Door`.
- Low editions cover ordinary cells, whole rooms, the stair foot and the
  Cistern entrance. The Cistern's bath, water, galleries and aqueduct design
  are retained. The stair interior rises behind its low entrance.
- Ceiling brushes carry explicit material tags through compilation and
  projection. Low ceilings no longer inherit floor materials, and cutaway
  views remove them without treating ordinary decks as roofs.
- Card plays, physical geometry, neighbour previews, door panels and reset
  use the same clearance rules. Input compatibility is version 10, and the
  regenerated catalogue/hash sidecar binds the new geometry to LAN/replay
  compatibility.

## Finishes and lighting

The [material set](../../assets/textures/backrooms/README.md) provides neutral
2K albedo, normal and ORM maps. Shared style owns colour and emission. Carpet
uses a separate half-metre normal repeat; wallpaper has shallow seams rather
than the previous additional groove overlay. Acoustic panels use a 600 ×
1200 mm module.

Skirting, T-bars and illuminated panels are fitted to the actual authored
hulls and merged by finish. The ceiling grid is phased in world space across
cells. Every third complete panel emits light; existing bounded fixed
practicals provide illumination. Decorative meshes do not add shadow draws,
and the merged panel field replaces individual diffuser entities.

Ambient fill is more neutral, floor finishes are matte, and local contact
shading remains. Power cuts switch the ceiling field off through the normal
fixture projection. There is no automatic player-following light.

## Playable reference

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_BACKROOMS_REFERENCE=1 cargo dev-run -p observed_game
```

This uses production loading, collision, input and rendering. `1`–`6` select
the straight, both turns, junction, field and room views; normal controls walk
the connected facility. `R` exits the match and loads the same seed again.
The reference holds rewrites and threats so it can be inspected freely.

For a reproducible reset capture, also set
`OBSERVED2_BACKROOMS_REFERENCE_CAPTURE=/tmp/observed-backrooms-reset`.
The harness exercises reset, saves before/after images and writes entity
counts. The measured counts were **7,049 before and 7,049 after**.

For the six production portraits:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_BACKROOMS_PORTRAITS=1 \
OBSERVED2_CAPTURE_HEX_WFC_SURFACES=/tmp/observed-backrooms-portraits \
cargo dev-run -p observed_game
```

## Evidence and verification

The [baseline](../evidence/backrooms_quality/before.png) and
[matched final view](../evidence/backrooms_quality/after.png) use the same
deterministic surface-capture pose. Additional final views cover
[straight](../evidence/backrooms_quality/straight.png),
[60-degree turn](../evidence/backrooms_quality/turn_60.png),
[120-degree turn](../evidence/backrooms_quality/turn_120.png),
[junction](../evidence/backrooms_quality/junction.png),
[field](../evidence/backrooms_quality/field.png) and
[room](../evidence/backrooms_quality/room.png).

The [60-second walkthrough](../evidence/backrooms_quality/walkthrough.mp4)
runs at normal simulation speed and continues upstairs into the Library.
The [power demonstration](../evidence/backrooms_quality/power.mp4) shows
the generator, power cut, restoration, recharging and dead station through
production controls. Compare the ceiling field
[without power](../evidence/backrooms_quality/power-off.png) and
[with restored power](../evidence/backrooms_quality/power-restored.png).
The [Cistern entrance](../evidence/backrooms_quality/cistern-entry.png)
and [water view](../evidence/backrooms_quality/cistern-water.png) show
the retained interior after a card is committed through the Architect desk.

Reset evidence includes [before](../evidence/backrooms_quality/before-reset.png),
[after](../evidence/backrooms_quality/after-reset.png) and
[entity counts](../evidence/backrooms_quality/reset.json).

`cargo fmt --all`, `cargo dev-clippy` and `cargo dev-test` passed. The full
test gate reports **2,806 passed, zero failed and 45 ignored**. Coverage includes
typed ports, real controller traversal in both directions, ceiling clearance,
door collider replacement, card compatibility, whole-room projection, cutaway
selection, authored hull limits, seeded replay baselines and entity budgets.
The separate `production_ascent_tick_times` instrumentation completed 7,200
ticks with median 238 µs, p95 343 µs and maximum 4.99 ms. The complete extended
ignored suite was not run.

The [native timing report](../evidence/backrooms_quality/timings.json) was
captured at 1440 × 900 on a TITAN X Pascal and i7-9700, with Vulkan, uncapped
presentation and GPU profiling. Seed: `263960012067191`.

| Measurement | Result |
| --- | ---: |
| Overall frame median / p95 | 10.684 / 14.193 ms |
| Backrooms frame median / p95 | 10.937 / 14.064 ms |
| Required / observed mutations | 10 / 10 |
| Slowest measured mutation frame | 15.539 ms |
| Visible geometry update at mutation | 4.178–4.446 ms |
| Frame samples | 31,405 |

The percentile and mutation checks pass the 16.67 ms budget. The report also
contains frame outliers capped at 250 ms and a roughly 5.2-second startup view
build; this is not a claim that every frame meets 60 fps. The opt-in benchmark
uses eight observers and continues another walking lap in the same world when
the first race finishes before ten mutations. This affects only the measurement
harness. To reproduce it:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_CAPTURE_HEX_WFC_PHASE101=/tmp/observed-backrooms-perf \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 \
OBSERVED2_CAPTURE_HEX_WFC_GPU=1 cargo dev-run -p observed_game
```

## Seam-column removal

The automatic decorative seam buttresses have now been removed on every floor,
including initial loads and scoped geometry rebuilds. An eight-floor regression
checks that occupied connections receive no decorative columns. Formatting,
Clippy and the full workspace gate passed again: 2,806 tests, zero failures.

Compare the [earlier station passage](../evidence/backrooms_quality/station-recharging.png)
with the [same production view after removal](../evidence/backrooms_quality/without-seam-columns.png).
The earlier portraits, reset counts and timing report above precede this removal.
