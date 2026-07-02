# observed_doors

This production crate implements the **doors-as-observation-gate** mechanic. It governs how opening and closing player-operated doors modifies the level connection topology.

## Core Rules & Logic
- **Closed Doors:** Hide the connection, making the doorway free to decohere and rewire to a different room.
- **Open Doors:** Observations pin both doorways, freezing the connection so it cannot rewire.
- **Protected Spine:** Connections designated as part of the spine remain pinned constantly to guarantee the exit remains accessible.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Orchestrates the `DoorWorld` state, manages the open/closed status vectors, tracks when doorways change destinations, and executes re-matching algorithms.

## Audit Notes
- **Bloat:** None.
- **Overlap:** None. Deterministic rewiring uses `observed_core::SplitMix`.
