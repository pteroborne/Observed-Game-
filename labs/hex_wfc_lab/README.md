# Hex WFC Lab

**Hex WFC Lab** showcases the Phase 90 animated Wave Function Collapse (WFC) solver on a 3D hexagonal lattice.

---

## Core Features & Functionality

- **Step-by-Step Solve Replay**: Animates WFC constraint propagation (blueprint cell selection, forced cells, domain pruning, and cell collapse) step by step.
- **Multi-Level Hex Lattice**: Solves across 4 vertical levels (`cols: 12, rows: 9, levels: 4`), connecting rooms, lateral halls, ramps (`+R`/`-R`), and well shafts (`+S`/`-S`).
- **3D Interactive Mode & Plan Mode**: Switch between 2D plan view and 3D camera navigation.
- **Relayout Demo**: Interactive demonstration of unobserved WFC relayouts during play.

---

## Controls

- `Space`: Toggle step-by-step solve playback / pause
- `N`: Single-step forward
- `+` / `-`: Increase / decrease solve animation speed
- `PgUp` / `PgDn`: Slice vertical lattice level
- `1`–`9`: Select preset seed
- `R`: Solve next random seed

---

## Run & Evidence Capture

```powershell
cargo run -p hex_wfc_lab
```
