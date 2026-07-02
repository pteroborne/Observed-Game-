# observed_observation

This production crate models the grid-based observation and decoherence graph. It tracks client visibility coverage across logical rooms and determines which connections rewire when unobserved.

## Core Rules & Logic
- **Room Spacing:** Sets bounds definitions and distances for the room grid (`ROWS`, `COLS`, `ROOM_HALF`, `ROOM_SPACING`).
- **Observation:** Rooms occupied by players freeze their four doorways, pinning the connection path.
- **Decoherence:** Unpinned doorways undergo dynamic, deterministic re-matching upon query, creating the shifting layout mechanic.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Defines `ObservationWorld`, `Side`, `DoorId`, `Door`, and contains the grid alignment calculations.

## Audit Notes
- **Bloat:** None.
- **Overlap:**
  - `lib.rs:L80` contains a duplicate private implementation of the `SplitMix` PRNG.
  - `Side` duplicates the N/E/S/W direction coordinates defined as `observed_facility::Cardinal`.
