# Hex Wellshaft Prototype Lab

**Wellshaft Lab** evaluates 5-level vertical silo hubs (six threshold bridges radiating from landings around a central hexagonal pillar, connected by real stair treads).

---

## Core Features & Functionality

- **Multi-Level Vertical Silo Hub**: 5-level hexagonal wellshaft connected by stair treads matching the production AABB step-up limits.
- **Radiating Threshold Bridges**: 6 threshold bridges radiating outward from landings on each level.
- **Diagnostic Capture Vantages**: Stages top-down descent, ascent, and plan views during evidence captures.

---

## Controls

- `WASD`: Walk
- `Mouse`: Look around
- `Shift`: Sprint
- `Space`: Jump
- `R`: Reset spawn pose

---

## Run & Evidence Capture

```powershell
cargo run -p wellshaft_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/wellshaft_lab"
cargo run -p wellshaft_lab
```
