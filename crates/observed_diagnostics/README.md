# `observed_diagnostics`

The **`observed_diagnostics`** production crate provides pure, engine-independent schemas and rule checks for automated visual audits.

It converts rendered game snapshots into structured findings that AI agents and test suites can inspect to catch legibility and visual regressions.

---

## Diagnostic Checks

- **Geometry Footprints**: AABB overlap verification (`check_geometry`) to detect colliding room/corridor bounding boxes.
- **Threshold Integrity**: Ensures visible thresholds have rendered frames, door leaves, point lights, and control-colored status indicators (`check_thresholds`).
- **Lighting & Emissive Luminance**: Verifies light intensities and material emission levels meet Legibility Contract minimum thresholds (`check_lights`, `check_materials`).
- **Tactical Map (Tac-Map)**: Validates rendered room, route, keystone, rival, and player marker element counts match simulation state (`check_tac_map`).
- **Monitors & Displays**: Detects black-screen or hidden monitor regressions (`check_monitors`).

---

## Testing

Run unit tests:
```bash
cargo test -p observed_diagnostics
```
