# `observed_authoring`

The **`observed_authoring`** production crate provides pure TrenchBroom / Quake `.map` file parsing and tile compilation pipelines for the continuous hex facility.

It converts convex brushes to deterministic collision hulls, validates typed hex footprints and ports against `observed_hex` metrics, and compiles `.map` sources into a content-hashed authoring catalog.

---

## Submodules

- **[`brush`](src/brush.rs):** TrenchBroom `.map` brush parser, plane intersection math, vertex extraction, and convex hull generation.
- **[`tile`](src/tile.rs):** Tile prototype structures (`TilePrototype`), quantized footprint validation, and port binding math.
- **[`source`](src/source.rs):** Version-2 `tile_meta`, `tile_cell`, and `tile_port` metadata schemas and module validation rules.
- **[`catalog`](src/catalog.rs):** Runtime authoring catalog (`RuntimeAuthoringCatalog`), module templates, catalog auditing (`audit_district_variations`), and SHA-256 fingerprinting.
- **[`manifest`](src/manifest.rs):** Compatibility manifest (`Manifest`, `TileKey`) mapping legacy tile formats.
- **[`tile_source`](src/tile_source.rs):** Regression-fixture tile generator (used exclusively for automated unit and determinism tests).

---

## Coordinate Conventions

- **TrenchBroom Space**: Z-up integer map units (`UNITS_PER_METER = 16.0`). Integer scaling guarantees hex corners land on exact grid coordinates (e.g., 7m = 112 units, 8m = 128 units).
- **World Space**: Y-up floating point meters (`world = (x / 16.0, z / 16.0, -y / 16.0)`).
- Local tile origin is centered at cell midpoint; level 0 floor rests at `y = 0.0`.

## Authored practical lights

Strict maps may place `tile_light` point entities at visible ceiling or wall
fixtures. The compiler validates each origin against the declared footprint,
rotates it with the module, and exposes the semantic `practical` source to the
lab and game. Sources never author RGB values: `observed_style` keeps ownership
of district colour while presentation owns the bounded energy/shadow budget.

---

## Testing & Verification

Run unit tests and tile validation checks:
```bash
cargo test -p observed_authoring
```
