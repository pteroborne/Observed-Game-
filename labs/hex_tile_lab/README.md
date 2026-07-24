# Hex Tile & Room Lab (Expanded Architectural & Lighting Showcase)

**Hex Tile Lab** is the interactive showcase for individual hex tiles, room compositions, district lighting schemes, cross-section cutaways, and camera inspection modes.

---

## Core Features & Capabilities

- **Ncurses-Style Menu Overlay (`F2`)**: Full keyboard-driven TUI menu overlay for filter selection, composition browsing, and lighting toggle options.
- **9 District Lighting Schemes (`1`–`9`)**: Previews standard neon-noir district lighting palettes from `observed_style`.
- **View Modes (`M` / `Tab`)**: Toggle between First-Person Walk, Turntable Orbit, and Free Look fly modes.
- **Cross-Section Cutaway (`X`)**: Toggles roof and upper-wall cutaways for multi-level ramp silos and room interiors.
- **Dev Mode Wireframe (`D`)**: Unlit high-contrast wireframe mode for inspecting tile convex hulls and port signatures.
- **Direct Authored Jumps**: Jump to the grounded sanctuary, two-level ramp, or supported silo helix.

---

## Run & Verification

```powershell
cargo run -p hex_tile_lab
```
