# Ramp movement and held-item motion — 2026-10-07

Grounded movement now compensates for the speed lost when Rapier projects horizontal input onto an incline. A short ray in the existing filtered movement query reads the supporting face. Uphill compensation stays horizontal so it cannot bypass Rapier's wall-climbing guard; downhill input follows the support plane. Humans and bots use the same controller. Walk/run settings, jump velocity, slope limits, and authoritative geometry remain unchanged.

The shared lantern, kinetic tool, and pad sway now makes one rounded vertical rise per 3.2 m of grounded surface travel. Previously `abs(cos(phase * 2))` made four rises per 1.6 m. At 4.6 m/s this reduces vertical cadence from 11.50 to 1.44 Hz. Exponential pose smoothing softens simulation-tick updates, airborne movement lets the hands settle, and relocations clear the accumulated motion. Reduced hand motion still disables it.

## Measurements

The actual production capsule settled for 90 ticks, accelerated for 60, then travelled for another 60 ticks on identical rotated cuboid fixtures. Speeds are metres along the surface per second, not requested velocity. [All 24 before/after cases](../evidence/ramp_hand_motion/measurements.json) include uphill, downhill, across, diagonal, sprint, and bot-strength input.

| Uphill angle | Before | After | Walk target |
| --- | ---: | ---: | ---: |
| 0° | 4.600 | 4.600 | 4.600 |
| 20° | 4.323 | 4.606 | 4.600 |
| 35° | 3.765 | 4.597 | 4.600 |
| 48° | 2.886 | 4.452 | 4.600 |

At 48°, close to the existing 50° climb limit, conservative contact handling still reduces speed slightly. The regression requires at least 90% of target speed there, tighter bounds on normal inclines, and uninterrupted downhill grounding. It also verifies that sprinting into a wall at the top of a ramp cannot climb it and that jumping remains available.

The deterministic headless climb route finishes on tick **9,788**, down from **9,902**. Two independently generated runs agree on completion and final digest `805826a3a1aea01f`. Input compatibility advances **11 → 12**; traversal profile hash schema advances **2 → 3** to identify the new controller semantics. Authored geometry and catalog hashes are unchanged.

## Verification

- Focused traversal suite: 84 passed.
- Shared equipment suite: 7 passed, including cadence at 30/60/120 FPS, slope distance, settling during falls, teleport reset, reduced hand motion, and held-item clearance.
- `cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test`: clean. The workspace suite reports **2,839 passed, zero failed, 45 ignored** across 295 targets. Multiplayer peer determinism, ceiling passage, bot climbs/no-stall soaks, and complete match reset all pass.
- The separate 7,200-tick production Ascent timing probe passes: simulation-step median **258.082 µs**, p95 **371.264 µs**, maximum **5.004 ms**. Construction costs **486.496 ms** separately. This diagnostic measures the simulation step, including movement queries; it excludes command generation and rendering. The complete extended ignored suite was not run.

## Native evidence

The [walking recording](../evidence/ramp_hand_motion/walkthrough.mp4) follows Solo Ascent seed 1 from the Backrooms through the first climb into the Library. Both held items remain visible; [the ramp approach at 15 seconds](../evidence/ramp_hand_motion/ramp.png) shows the same first-person view. Normal simulation and streaming budgets are used. The silent 1440 × 900 H.264 recording preserves wall-clock timestamps; screenshot readback adds cost, so it is visual evidence rather than a frame-time benchmark.

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" OBSERVED2_SEED=1 \
OBSERVED2_CAPTURE_HEX_WFC_TICKS=2700 \
OBSERVED2_CAPTURE_SOLO_ROUTE="$PWD/scratch/ramp-hand-motion/timings" \
OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO="$PWD/scratch/ramp-hand-motion/video" \
cargo run -p observed_game
```
