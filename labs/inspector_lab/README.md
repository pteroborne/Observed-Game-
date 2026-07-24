# Inspector Lab

**Inspector Lab** evaluates `bevy-inspector-egui 0.36` for live ECS entity and resource inspection behind an optional default-off feature flag (`dev_tools`).

---

## Core Features & Functionality

- **Live ECS World Inspection**: Optional integration of `WorldInspectorPlugin` to inspect entity components and resource states live in-game.
- **Built-in Bevy Diagnostics**: Uses Bevy's built-in `FrameTimeDiagnosticsPlugin` for frame time monitoring without third-party dependencies.
- **Additive Seam**: Standard lab builds run zero third-party UI dependencies; passing `--features dev_tools` enables egui live inspection.

---

## Run & Verification

Standard run (no dev_tools):
```powershell
cargo run -p inspector_lab
```

Run with live inspector enabled:
```powershell
cargo run -p inspector_lab --features dev_tools
```
