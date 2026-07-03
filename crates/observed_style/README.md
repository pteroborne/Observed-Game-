# observed_style

This crate defines the semantic design tokens for the game's neon-noir aesthetic. It guarantees that gameplay information is always legible regardless of lighting or atmospheric conditions.

## Style Tokens & Roles
- **Semantic State Mapping:** Connects game states (Control, Danger, Alert, Neutral) to specific emissive colors, marker layers, and line overlays.
- **Door Identity Reads:** Defines legend-backed doorframe glyph treatments for typed-room reads, including Sensor map feeds and false-exit Decoy signals.
- **Gantry Traversal Reads:** Defines semantic surface treatments for raised gantry decks, lit jump edges, and visible understory landings.
- **Rubble Surfaces:** Collapse-sealed thresholds rendered as dark ash rubble fill with dying-ember emissive, signal-tier to stay legible.
- **District Palettes:** Defines structural lighting tones for facility sectors, including drained collapse-state transitions.
- **Klaxon State:** Facility-wide escape countdown lighting (red alarm tier, signal-tier for legibility).
- **Legibility Rules:** Enforces minimum contrast constraints and floor lighting values so players, hazards, and pathways are never hidden by atmospheric bloom or fog.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Defines semantic roles (`MarkerRole`, `DoorIdentityRole`, `OutlineRole`, `SurfaceRole`), color palettes, brightness checks, and visual legends.

## Audit Notes
- **Bloat:** `lib.rs` (792 lines) is relatively large but holds the complete visual language logic, keeping presentation code strictly separated.
- **Overlap:** None.
