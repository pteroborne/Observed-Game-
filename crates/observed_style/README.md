# observed_style

This crate defines the semantic design tokens for the game's neon-noir aesthetic. It guarantees that gameplay information is always legible regardless of lighting or atmospheric conditions.

## Style Tokens & Roles
- **Semantic State Mapping:** Connects game states (Control, Danger, Alert, Neutral) to specific emissive colors, marker layers, and line overlays.
- **District Palettes:** Defines structural lighting tones for facility sectors.
- **Legibility Rules:** Enforces minimum contrast constraints and floor lighting values so players, hazards, and pathways are never hidden by atmospheric bloom or fog.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Defines semantic roles (`MarkerRole`, `OutlineRole`, `SurfaceRole`), color palettes, brightness checks, and visual legends.

## Audit Notes
- **Bloat:** `lib.rs` (792 lines) is relatively large but holds the complete visual language logic, keeping presentation code strictly separated.
- **Overlap:** None.
