# Phase 96 — Measurement & First-Frame Lighting

**Status:** complete and focused-verified on the production-size hex facility.

## Goal

Close Arc L playtest finding #8 and establish the measurement surface for finding
#9 without redesigning mutation. The first visible hex-facility frame must already
use the current cell's exact `observed_style` palette, and one opt-in production-size
run must separate solve, authored projection, collider-scene construction, view,
fixed-step, and whole-frame wall-clock costs.

## Scope

- `game/src/hex_wfc/view/`: initial camera, ambient, clear/fog, and bounded key
  state; startup and mutation-view rebuild measurement.
- `game/src/hex_wfc/lantern.rs`: candle-scale carried practical and subdued
  first-person cage exposure.
- `game/src/hex_wfc/perf.rs`: evidence-only timing probe and report writer.
- `game/src/hex_wfc/mod.rs`: module registration plus begin/end fixed-step hooks.

This phase does **not** change WFC topology, relayout scheduling, traversal physics,
tile authoring, lantern/Guardian behavior, the survivor map, replay state, or
authoritative commit timing.

## As implemented

The startup wash was a small initialization bug. The hex key spotlight began as
Bevy's default white light at zero intensity and eased toward the current semantic
register. Its first ramp-up was therefore visibly desaturated. State entry now:

1. resolves the local cell's `observed_style::architecture` palette;
2. moves the shared camera to the authoritative eye pose;
3. installs exact HDR, bloom, distance fog, ambient, and palette-owned clear color;
4. disables the outgoing menu sun;
5. spawns the bounded key at its final semantic color, intensity, range, angle,
   and world pose;
6. only then enqueues facility shell geometry.

Later register changes still ease at the existing blend rate. The key uses the
palette's exact semantic color/cone/shadow rule with an indoor hex-cell lumen
scale. The caged lantern is the only player-following light. Its point-light
budget is 35–250 lumens and its carried cage uses a subdued style-derived
treatment while the objective core remains signal-tier.

The final visual pass also fixed two apparent lighting problems at their actual
sources: room shells no longer assign the bright route-floor treatment to every
convex wall face, and rival runners are compact translucent capsules rather than
opaque emissive cuboids. The held lantern sits low in the first-person frame so
the cage reads without masking a threshold.

## Performance evidence hook

From PowerShell:

```powershell
$env:OBSERVED2_CAPTURE_HEX_WFC_PHASE96='docs/evidence/arc_m/phase_96'
$env:OBSERVED2_SEED='0xF011FAC11177'
cargo run -p observed_game --bin observed
```

The hook is absent-resource gated and inert in normal play. It autonomously opens a
production `28 x 20 x 10` spectated match, captures
geometric startup frames 2/4/8/16 plus `first_visible_frame.png` at frame 32,
and writes `timings.json` after tick 600. The startup sequence makes the first
render-ready facility frame inspectable without assuming how many extraction frames
the screenshot backend needs. The report
contains:

- independent solve, geometry-projection, and Rapier-scene construction times from
  a discarded reproduction of the live seed/config;
- startup and mutation-triggered view rebuild times;
- fixed-step and Bevy frame-delta minimum, median, p95, maximum, and mean;
- each observed mutation commit's tick, generation, and fixed-step time.

All measurements use `std::time::Instant` or Bevy's frame delta and remain outside
simulation. They never influence a tick, budget, commit decision, digest, or replay.
The Phase 96 numbers are diagnostic baselines, not the Arc M performance closure
claim; Phases 100–101 own bounded mutation and the ten-commit 16.7/33.3 ms gate.

## Focused gates

- Each register's initial key treatment is derived from the corresponding
  `observed_style` fields with one documented absolute hex-cell lumen scale.
- The startup camera equals the authoritative player eye pose.
- Timing summaries use nearest-rank percentiles, preserve spikes, and represent an
  empty sample set explicitly.
- Evidence paths remain under the requested directory.
- Existing hex determinism and soak gates remain unchanged; wall-clock resources are
  presentation/evidence-only.

## Evidence

The deterministic `0xF011FAC11177` final audit completed through tick 600 on a
`28 x 20 x 10` facility. Frames 2 and 4 are screenshot-backend extraction
lead-in. Frame 8 is the first render-ready image: the dark facility, open
threshold, and recognizable caged lantern are present with no menu-light or
white-shell transient. Frame 32 retains the treatment after the spectated bot
advances; nearby rivals remain recognizable without masking the architecture.

- `docs/evidence/arc_m/phase_96_final/startup_frame_002.png` — expected pre-render black
  extraction.
- `docs/evidence/arc_m/phase_96_final/startup_frame_004.png` — expected extraction lead-in.
- `docs/evidence/arc_m/phase_96_final/startup_frame_008.png` — first render-ready facility
  frame; startup-lighting acceptance evidence.
- `docs/evidence/arc_m/phase_96_final/startup_frame_016.png`
- `docs/evidence/arc_m/phase_96_final/first_visible_frame.png` — frame 32 retained as the
  stable startup reference.
- `docs/evidence/arc_m/phase_96_final/timings.json`

The final audit measured 1,341.305 ms for solve, 876.060 ms for projection,
1,632.922 ms for collider-scene construction, and 371.530 ms for startup view
construction. The generation-1 fixed commit at tick 542 took 12.532 ms; its view
delta took 3.436 ms and its attributed frame took 28.520 ms. The historical
`docs/evidence/arc_m/phase_96/timings.json` is retained separately: it records
the original 235.694 ms global-mutation spike that motivated Phases 100–101.

Focused verification completed:

- `cargo check -p observed_game --lib`
- `cargo clippy -p observed_game --lib`
- `cargo test -p observed_game hex_wfc::view::lighting::tests`
- `cargo test -p observed_game hex_wfc::perf::tests`
- the instrumented production run above, including manual inspection of every
  startup capture
