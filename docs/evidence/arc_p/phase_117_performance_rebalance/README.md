# Arc P performance regression correction

Captured 2026-07-28 from the canonical production `28×20×10` game with:

```powershell
$env:OBSERVED2_CAPTURE_HEX_WFC_PHASE96='docs/evidence/arc_p/phase_117_performance_rebalance'
$env:OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED='1'
$env:OBSERVED2_CAPTURE_HEX_WFC_TICKS='360'
cargo dev-run -p observed_game
```

The 580 uncapped frame samples in [timings.json](timings.json) measure an
8.547 ms median and 10.403 ms p95. One 250 ms capture/startup outlier remains in
the maximum sample and is preserved in the report rather than trimmed. The 360
fixed-step samples measure a 0.100 ms median and 0.182 ms p95. In the first Arc
P pass, the *median* itself hit the profiler's 250 ms frame ceiling alongside a
32.756 ms median fixed step.

The corrected path performs no whole-facility graph search during an unchanged
fixed tick. Objective candidates and Guardian leadership use deterministic
lattice distance; a bot retains its exact movement route until its target,
logical path, or facility generation changes. Lantern progress and pressure
signals retain their exact bounded routes but recompute only when the relevant
player/Guardian cell, inventory, or facility generation changes.

The harness itself deliberately performs one discarded `route_between_cells`
probe per rendered frame for attribution, so these figures include diagnostic
work absent from ordinary play.
