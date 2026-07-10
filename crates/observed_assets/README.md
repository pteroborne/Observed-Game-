# observed_assets

This production crate contains the runtime asset-slot manifests for Observed 2. It enables loading external 3D models and sound effects (models, sounds, textures) without hard-coding string file paths directly inside gameplay systems.

## Module Structure

- **[`lib.rs`](src/lib.rs):** Defines `AssetKind` enum (representing different models, sounds, and textures) and contains the asset loader registries and tests.

## Audio Slots

- General match cues live in named sound slots such as `FOOTSTEP`, `REROUTE`,
  `ESCAPE`, `DOOR`, `KLAXON`, `COLLAPSE_STING`, `UI_CLICK`, `UI_HOVER`, `JUMP`,
  `LAND`, `TOOL_INTERACT`, `KEYSTONE`, `EXIT_UNLOCK`, and `GUARDIAN_DREAD`.
- Per-district ambience is exposed as `DISTRICT_AMBIENCE` in district order
  (archive, reactor, atrium, foundry, hollow, spillway), so game presentation code
  does not hard-code file paths.
- Hallway-flavour ambience beds live in `AMBIENCE_CORRIDOR` (generic halls) and
  `AMBIENCE_GANTRY` (the two-level jump hall); hallways take one of these instead
  of a district bed.

## Sprite Slots

- Dev 2.5D placeholders live in named texture slots under `assets/sprites/`:
  `RUNNER_STAND`, `RUNNER_WALK1`, `RUNNER_WALK2`, `RIVAL_STAND`,
  `RIVAL_WALK1`, `RIVAL_WALK2`, `GUARDIAN_STAND`, and `CONTROL_DEVICE`.
- These slots are presentation-only; missing images fall back to procedural game
  meshes rather than changing simulation state.

## Dependencies
- Standard Rust library
- `bevy` (prelude, asset types) when Bevy feature is enabled

## Audit Notes
- **Bloat:** None. File is concise and focused.
- **Overlap:** Re-declares some asset configurations, but serves as the clean boundaries for CC0 assets.
