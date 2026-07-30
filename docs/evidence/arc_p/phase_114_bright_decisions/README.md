# Arc P production visual and regression check

Captured 2026-07-27 and refreshed 2026-07-28 after the exposure/performance
regression correction, from the canonical game with:

```powershell
$env:OBSERVED2_CAPTURE_HEX_WFC_STYLE='docs/evidence/arc_p/phase_114_bright_decisions'
cargo dev-run -p observed_game
```

The capture solved the production 28×20×10 facility with repeated room quotas
and the open-volume gate, then drove the authoritative objective bot through
ordinary `HexPlayerCommand` input. Eight frames were written and inspected.

## What the frames establish

- [hex_wfc_002.png](hex_wfc_002.png) exposes three lateral thresholds from one
  position while the wall masses retain dark blue-black separation.
- [hex_wfc_004.png](hex_wfc_004.png) shows the bounded warm practical pool: it
  carries the nearby floor and wall without lifting the doorway beyond it.
- [hex_wfc_007.png](hex_wfc_007.png) retains silhouettes across a larger room;
  the central column and opposing wall remain distinct instead of converging on
  one fog/exposure value.
- Across the sequence, the 60° lens shows lateral choices that the former fixed
  camera cropped while avoiding obvious wide-angle distortion.

## Performance regression and correction

The same production `28×20×10` seed was measured uncapped for 360 fixed ticks
with `OBSERVED2_CAPTURE_HEX_WFC_PHASE96`:

| Build | Frame median | Frame p95 | Fixed median |
|---|---:|---:|---:|
| first Arc P pass | 250.00 ms (profiler ceiling) | 250.00 ms | 32.76 ms |
| corrected | 8.55 ms | 10.40 ms | 0.10 ms |

The correction removed repeated whole-facility routing from objective selection
and Guardian ranking, retained bot routes until their target or topology changes,
and cached lantern topology signals until their relevant cells change. The final
median is faster than the 10.58 ms pre-Arc-P uncapped default evidence.
The complete corrected report is
[phase_117_performance_rebalance/timings.json](../phase_117_performance_rebalance/timings.json).

This is an agent-viewed presentation check, not the Phase 118 human choice gate.
It does not establish feel, navigation comfort, or whether the full
keystone→station→exit sequence produces satisfying decisions in hand.
