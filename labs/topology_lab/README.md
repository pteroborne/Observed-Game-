# Topology Lab

**Topology Lab** is the foundational 2D room/corridor graph topology laboratory for Observed 2.

---

## Core Features & Functionality

- **ASCII Map Parsing**: Parses 2D ASCII layout grids (`logic.rs`) into explicit `RoomNode`s, `HallwayNode`s, and threshold endpoints (`ThresholdEndpoint`).
- **Graph Decoherence & Connectivity**: Shuffles unpinned links using a deterministic `SimpleRng` and validates connectivity across all room nodes.
- **Resettable App State**: State-scoped entities teardown cleanly on `R` reset.

---

## Controls

- `Space`: Force one graph decoherence pulse
- `R`: Reset topology layout

---

## Run

```powershell
cargo run -p topology_lab
```
