# observed_assets

This production crate contains the runtime asset-slot manifests for Observed 2. It enables loading external 3D models and sound effects (models, sounds, textures) without hard-coding string file paths directly inside gameplay systems.

## Module Structure

- **[`lib.rs`](src/lib.rs):** Defines `AssetKind` enum (representing different models, sounds, and textures) and contains the asset loader registries and tests.

## Dependencies
- Standard Rust library
- `bevy` (prelude, asset types) when Bevy feature is enabled

## Audit Notes
- **Bloat:** None. File is concise and focused.
- **Overlap:** Re-declares some asset configurations, but serves as the clean boundaries for CC0 assets.
