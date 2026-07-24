# `observed_content`

The **`observed_content`** production crate provides pure, deny-unknown-fields schemas and SHA-256 fingerprinting for immutable game content manifests.

It enforces strict separation between:
- **`SimulationContentHash`**: Hashes traversal physics parameters, module definitions, convex collision hulls, port alignments, and navigation graphs.
- **`PresentationContentHash`**: Hashes visual dressing, district lighting scales, and optional asset paths.

---

## Core Types

- **`ContentManifest`**: Top-level manifest struct containing schema version, traversal profiles, district definitions, module lists, and asset declarations.
- **`ArchitectureRegister`**: Code-owned enum (`ShadowScreen`, `Monolith`, `OverlitGrid`, `Institutional`, `FacetMonument`, `Megastructure`, `Wellshaft`, `InfiniteGallery`, `Thinning`) providing stable `u8` IDs for procedural architecture registers.
- **`TraversalProfile`**: Standardized physical traversal parameters (`walk_speed`, `run_speed`, `gravity`, `radius`, `half_height`, `step_height`).
- **`BakedModule`**: Serialized convex hull geometry (`ConvexHull`) produced by TrenchBroom compilation.
- **`PlaceLayoutSnapshot`**: Authored module placement snapshots consumed by physics, rendering, and spectator AI.

---

## Verification

To run unit tests and validation checks:
```bash
cargo test -p observed_content
```
