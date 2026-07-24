# Raw Rapier Controller Lab

**Rapier Controller Lab** tests raw 60 Hz kinematic capsule stepping (`step_character`) directly over Rapier 3D colliders (`observed_traversal::rapier_controller`).

---

## Core Features & Functionality

- **Raw Rapier Character Controller**: Runs Rapier 0.34 kinematic capsule physics with `enhanced-determinism` enabled.
- **Structural Colliders**: Static colliders generated from `StructuralCollider` specifications (axis-aligned walls, yawed diagonal walls, low autostep platforms).
- **Pure Intent Boundary**: Input translates into `PlayerIntent` before stepping character simulation.

---

## Controls

- `WASD`: Move
- `Q` / `E`: Turn camera yaw
- `Shift`: Sprint
- `Space`: Jump
- `R`: Reset physics stage

---

## Run

```powershell
cargo run -p rapier_controller_lab
```
