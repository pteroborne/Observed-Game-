# Gantry Lab

Phase 40 feasibility lab for a two-level jump-map hallway.

Run:

```powershell
cargo run -p gantry_lab
```

The lab renders the pure `observed_traversal::gantry` course: raised platform decks are
the fast route, a missed commitment drops the runner into a visible lower landing, and
the safe bypass stays on the lower floor. The deterministic bot can run all three
routes:

- `1` clean jump: fast upper-platform route to the intended exit.
- `2` fall recover: intentional miss, lower-floor landing, side-exit recovery.
- `3` safe bypass: lower-floor route with no jump risk.
- `R` reset the current route.

Success criteria are unit-tested in `observed_traversal`: lower-floor landings are
navigable, route timing is fast/medium/slow, and repeated runs are deterministic.
