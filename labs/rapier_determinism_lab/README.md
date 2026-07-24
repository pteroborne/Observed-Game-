# Rapier3D Determinism Spike Lab

**Rapier Determinism Lab** evaluates `rapier3d 0.34` lockstep bit-identical determinism with the `enhanced-determinism` feature enabled.

---

## Core Features & Functionality

- **Dual-World Lockstep Monitor**: Steps two identical physics worlds (`world_a` rendered, `world_b` headless shadow) simultaneously and computes FNV-1a state hashes each tick.
- **Bit-Identical Verification**: Tests smooth sphere, capsule, and convex hull bodies over 600 fixed-step ticks to prove bit-for-bit trajectory matching.
- **Divergence Detection**: Live UI readout flags any state hash divergence (`MATCH` vs `DIVERGED`).

---

## Controls

- `R`: Reset physics worlds

---

## Run & Evidence Capture

```powershell
cargo run -p rapier_determinism_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/rapier_determinism_lab"
cargo run -p rapier_determinism_lab
```
