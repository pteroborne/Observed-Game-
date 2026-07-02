# player_input

This production crate establishes the input abstraction layer for Observed 2. It translates raw hardware scans or controller updates into semantic intents.

## Input Intent Models
- **`PlayerId(pub u16)`:** Stable identifier mapping a participant to their input device slot.
- **`PlayerIntent`:** An abstract container capturing movement vectors, camera lookup scales, and action flags (jump, sprint, interact, climb, recover).

## Module Structure
- **[`lib.rs`](src/lib.rs):** Defines `PlayerId`, `PlayerIntent`, and default control behaviors.

## Audit Notes
- **Bloat:** None.
- **Overlap:** None.
