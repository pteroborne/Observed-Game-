# Guardian AI Lab

Feasibility lab proving the **"weeping-angel" guardian objective** logic and determinism.

## Concept

1. **Unobserved movement**: The guardian moves room-by-room toward its target player using Dijkstra/BFS shortest path routing.
2. **Observed freeze**: The guardian is **frozen** and cannot move if observed by the player (either in the same room or visible through open thresholds matching the player's look direction).
3. **Anchor observation & banishment**: Dropping an anchor torch in a room freezes the guardian if it is inside that room. If the guardian remains under anchor light for 30 consecutive seconds, it is banished (teleported to a random room), preventing it from camping at key points.
4. **Banishment on touch**: If the guardian touches the player, the player is teleported to a random room.
5. **Determinism**: Seeded PRNG ensures reproducibility.

## Controls

- `WASD`: Move player room-by-room.
- `Arrow Keys`: Change look direction (North, East, South, West).
- `T`: Drop or pick up an anchor torch in the player's current room.
- `Space`: Perform manual decoherence (scrambling unobserved room connections).
- `P`: Pause / resume guardian auto-movement timer.
- `R`: Reset the lab to its initial layout and reset all positions.
- `F1`: Toggle debug visualizer overlay.

## Verification

### Automated Tests
Run:
```bash
cargo test -p guardian_ai_lab
```

### Manual Verification
Run:
```bash
cargo run -p guardian_ai_lab
```
- Verify the player faces the correct direction (arrow indicator).
- Highlighted green outline shows the player's direct line of sight (observable rooms).
- Verify the guardian freezes when its room is within the green outline.
- Drop an anchor torch using `T`. If the guardian walks into it, check that it freezes and its timer counts down from 30.0s in the diagnostics panel, then banishes it to another room.
