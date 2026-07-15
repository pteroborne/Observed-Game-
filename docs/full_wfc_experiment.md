# Continuous Full-WFC Experiment

Branch: `experiment/full-wfc-facility`

This branch keeps the existing teleport match under **Play** and adds a separate
**Experiment: Full WFC** main-menu entry. The experimental facility exists in one
continuous 8 x 5 x 3 world-space lattice. It does not create isolated Places or
teleport a player when a threshold is crossed.

## Implemented rules

- Every committed generation has a weighted-A* route from spawn and every occupied
  `PlayerId` cell to the exit.
- A five-second deterministic pulse may delete and recreate any unobserved room or
  hall. Presentation rebuilds only the changed cell entities.
- Occupancy pins its module. Looking at a room threshold pins the room, its exact
  non-branching hall chain, and the destination room. Other threshold faces on that
  destination room may still change if they are not observed.
- The first currently observed terminal chain claims the exit. All competing exit
  faces are sealed in both navigation and continuous movement. Releasing observation
  releases the claim.
- The carried candle is a real point light. Its intensity and range increase with a
  normalized scalar from the same weighted A* travel costs used by the route guard.
  The selected route is never shown in the played mode.
- Vertical connections are continuous climb shafts: Space climbs up and Ctrl climbs
  down while the player is in the shaft opening. No teleport transaction is involved.

## Evidence

The resettable lab shows all three levels and the otherwise invisible invariants:

![Full-WFC simulation lab](evidence/full_wfc/full_wfc_lab.png)

The assembled first-person mode uses the shared semantic style treatments. Cyan
frames are mutable thresholds, gold is the currently observed/frozen threshold, red
is a competing exit path sealed by the single-claim rule, and the green beacon is the
exit:

![Full-WFC played mode](evidence/full_wfc/full_wfc_game.png)

Reproduce the captures:

```powershell
$env:OBSERVED2_CAPTURE='docs/evidence/full_wfc/full_wfc_lab.png'
cargo run -p full_wfc_lab

$env:OBSERVED2_CAPTURE_FULL_WFC='docs/evidence/full_wfc/full_wfc_game.png'
cargo run -p observed_game --bin observed
```

## Verification

- Facility tests: 57 passed; the extended 100-seed x 50-pulse gate passed separately
  (5,000 constrained collapses in 292 seconds).
- Full-WFC lab tests: 2 passed.
- Assembled game tests: 302 passed, including legacy Spectate navigation and the new
  experiment launch.
- Clippy passes with warnings denied for `observed_facility`, `full_wfc_lab`, and
  `observed_game`.

## Deliberate first-slice limits

This experiment has one local runner, simple authored module primitives, and a
grid-aware continuous controller. It deliberately excludes rivals, guardian,
keystones, carried items, progression, collapse pressure, and networking. Those
systems remain in the legacy match until this fundamental movement/observation loop
proves fun. The controller is not yet promoted to the production Rapier KCC; that is
the next integration decision after playtesting the continuous topology.
