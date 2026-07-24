# 3D Map Observer Lab

**Map Observer Lab** is an interactive free-fly and top-down camera inspection tool for the full WFC generated 3D facility.

---

## Core Features & Functionality

- **Camera Inspection Modes (`Tab` / `F` / `M` / `T`)**: Free-Fly flight, Top-Down Whole Map overview, and Top-Down focused room camera modes.
- **Instant Teleportation (`1`–`6`)**: Snap directly to key facility points (`Start`, `Wellshaft`, `Gantry`, `Climb`, `Keystone`, `Exit`).
- **Wireframe Edge Glow (`G`)**: Toggle neon gizmo edge outlines for walls (cyan), features (magenta), and floors (green).
- **Screenshot Capture (`C` / `Enter`)**: Save high-resolution inspection screenshots directly to `docs/evidence/map_observer/`.

---

## Controls

- `WASD`: Free fly / Pan camera
- `Right-Click Drag`: Look around in Free-Fly mode
- `Shift`: Sprint / fast flight
- `1`–`6`: Teleport to room/module targets
- `Tab`: Toggle Free-Fly / Top-Down mode
- `G`: Toggle wireframe gizmo edge glow
- `C` / `Enter`: Take screenshot

---

## Run

```powershell
cargo run -p map_observer_lab
```
