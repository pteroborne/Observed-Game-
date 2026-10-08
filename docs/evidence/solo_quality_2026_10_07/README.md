# Solo quality evidence — report 2026-10-07, implementation 2026-10-08

The [player report](../../playtests/2026-10-07-solo-quality.md) preserves the two
supplied images: [ceiling flicker](ceiling-flicker.png) and [Guardian/ramp](guardian-ramp.png).
The accepted programme is [the eight-floor quality bar](../../quality/aaa_bar.md).

## Supported Guardian fixtures

[On the ramp](guardian/ramp.png), [inside a room](guardian/room.png).
Both are unretouched 1440×900 native game-window captures on seed 1. The camera/
placement is staged in the Facility race adapter sharing the canonical major's
actual collision, sight and presentation; this is not a completed Ascent session.
The fixture now queries a full-size supported placement against current geometry,
selects a real standable viewer position with unobstructed sight, and fails unless
the local viewer actually sees a physically placed, observation-frozen Guardian.
[The ramp record](guardian/ramp.json) and [room record](guardian/room.json) retain
feet, status, local visibility, camera pose, tick and input version 13.

Reproduce with `OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1
OBSERVED2_CAPTURE_HEX_WFC_GUARDIAN=/tmp/observed-major-proof cargo dev-run -p observed_game`
(join the environment assignments on one shell command).

## Canonical Ascent walking evidence

[59.7-second walkthrough](walkthrough.mp4), [15 s](walk-15s.png),
[30 s](walk-30s.png), [45 s](walk-45s.png), [frame timestamps](walk-frames.csv).
This is the normal seed-1 Solo Architect Ascent bot route, viewed through an
Observer's eyes with ordinary streaming budgets. It covers the Backrooms, the
Library and the next climb. It is silent; 612 native PNG frames were encoded using
actual wall-clock timestamps to 1,792 H.264/yuv420p frames at 30 fps, 1440×900.
One queued screenshot was dropped at exit; all saved frames and the encoded movie
decode cleanly. Screenshot overhead makes this visual evidence, not a benchmark.
Source PNG sequences remain outside git.

The inspected samples retain coherent near floors, ceilings and mounted fixtures.
They do not establish that every seed or the player's exact unrecorded location
is flicker-free. The player recheck remains open.

## Screenshot-free benchmark

[Raw report](performance/timings.json), [desktop budget check](performance/desktop-budget.json).
TITAN X Pascal / i7-9700, Vulkan, 1440×900, uncapped, seed 1, full 24×17×8 Ascent,
7,200 simulation ticks in one match. No screenshot readback in this run.

| Warm measurement | Result |
| --- | ---: |
| Median | 10.281 ms |
| p95 / p99 | 18.984 / 23.231 ms |
| Maximum / largest mutation window | 48.527 ms |
| Cold view construction | 5.118 s |

**Desktop budget remains red.** p95 exceeds 16.67 ms and the mutation maximum
exceeds 33.33 ms. No prison catch occurred in this workload, so it says nothing
about repairing the previously recorded prison-transition hitch. This is current
performance evidence, not a matched before/after claim: physical major movement
changes event timing. Steam Deck and physical-machine LAN acceptance are unrun.

Reproduce without the video variable:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_SOLO_ROUTE=/tmp/observed-quality-performance \
OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED=1 OBSERVED2_CAPTURE_HEX_WFC_GPU=1 \
cargo dev-run -p observed_game
python3 tools/check_quality_report.py /tmp/observed-quality-performance/timings.json --preset desktop
```

Add `OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO=/tmp/observed-quality-walk` and
`OBSERVED2_CAPTURE_HEX_WFC_TICKS=3600` for a separate short visual run.

The relevant ignored production simulation instrument also ran: 7,200 ticks,
median 264.313 µs, p95 345.693 µs, maximum 3.916 ms, five plays and zero catches.
This measures the physical simulation step, excluding command generation and
rendering. The whole probe took 59.75 s; do not confuse its simulation-step timing
with the cost of a complete game frame.
