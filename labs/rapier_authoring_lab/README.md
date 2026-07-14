# Rapier Authoring Lab

This lab answers the next production question after `trenchbroom_lab` and
`rapier_determinism_lab`: **can a TrenchBroom-authored facility retain stable
room/port/door semantics while its actual convex brush geometry drives a smooth
Rapier capsule controller?**

The `.map` importer is data-only. Imported entities project into `RoomId`, `PortId`,
model-owned door state, and convex point clouds. Raw `rapier3d 0.34` builds one static
convex collider per brush and runs the kinematic character controller at 60 Hz with
`enhanced-determinism`. Bevy renders those same Rapier hulls and never owns simulation
state.

## Controls

- `WASD`: move
- Arrow keys: look
- `Shift`: sprint
- `Space`: jump
- `O`: toggle the corridor-to-ramp door
- `R`: reset the whole imported course
- `F1`: toggle convex-hull and semantic debug overlays

## Success conditions

1. Every structural and door brush becomes a real Rapier convex hull, including the
   non-axis-aligned ramp.
2. The capsule is blocked by the closed model-owned door and passes after it opens.
3. The capsule climbs the imported ramp and reaches its raised platform without
   AABB stair approximations.
4. Two independent worlds replay the same intents and door mutation bit-for-bit.
5. Reset restores the authored door, player, collider, and entity counts.

Run with `cargo run -p rapier_authoring_lab`.

