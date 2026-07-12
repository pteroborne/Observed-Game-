# Rapier Portal Composition Lab

This lab combines the teleport-place mechanic with authored convex modules and a
small deterministic WFC composition. It recreates two existing hallway types:

- **Gantry** — raised ramp/decks above a safe ground bypass.
- **Colonnade** — a wide pseudo-room with repeated pillars and a clear central lane.

The two modules are authored as separate TrenchBroom brush entities. WFC composes
them into a two-module hallway in either order. The hallway exists in its own local
coordinate space; the source room renders a transformed duplicate through its open
threshold, and crossing teleports the capsule into the real hallway Place.

## Lock contract

- Looking through the threshold observes and freezes the current composition.
- Looking away releases that temporary lock.
- An anchor freezes the composition regardless of player gaze.
- The frame light reports **anchor state only**. Player observation never changes it.
- BLAME can recompose only while neither lock applies.

## Controls

- `WASD`: move
- Arrow keys: look
- `Shift`: sprint
- `Space`: jump
- `B`: request a BLAME recomposition
- `E`: toggle the anchor
- `R`: reset
- `F1`: toggle debug overlays

Run with `cargo run -p rapier_portal_lab`.
