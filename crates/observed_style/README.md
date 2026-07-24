# `observed_style`

The **`observed_style`** production crate defines the semantic design tokens, HSL/RGB palette registries, and contrast rules for Observed 2's neon-noir aesthetic.

It guarantees that gameplay-critical signals (paths, threats, interactables, player avatars, door identity reads) always punch through atmospheric fog and bloom at guaranteed contrast thresholds (**Legibility Contract**).

---

## Semantic Token Categories

- **`SurfaceRole`**: Structural surface treatments (`Plain`, `Spine`, `SafeBypass`, `GantryDeck`, `WellshaftStone`, `GantryEdge`, `Understory`, `Rubble`).
- **`MarkerRole`**: Gameplay beacon markers (`NextRoom`, `Exit`, `Control`, `Collapse`, `You`, `Teammate`, `Rival`, `Director`).
- **`ThresholdFrameState`**: Always-open threshold frame indicator lights (`Mutable`, `Anchored`, `Sealed`).
- **`DoorIdentityRole`**: Pre-commit doorframe semantic reads (`KeystoneVault`, `PowerCache`, `Reactor`, `Control`, `Survey`, `Sensor`, `FalseExit`, `Decoy`, `DeadEnd`).
- **`OutlineRole`**: Mesh-outline legibility treatments (`OpenDoor`, `ClosedDoor`, `Interactable`, `Hazard`, `Rival`, `ObjectiveBeacon`, `Pickup`, `LocalPlayer`).
- **`District` & Palettes**: Six district atmosphere registers (`Archive`, `Reactor`, `Atrium`, `Foundry`, `Hollow`, `Spillway`).
- **`team(index)` & `team_legend()`**: Legibility-contract team colors mapped by team index.

---

## Testing

Run unit tests and color contrast verification:
```bash
cargo test -p observed_style
```
