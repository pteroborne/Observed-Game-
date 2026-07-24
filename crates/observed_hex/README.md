# `observed_hex`

The **`observed_hex`** production crate serves as the single pure source of truth for the hexagonal prism grid vocabulary, axial coordinate math, and quantized metric geometry in Observed 2.

It ensures the hex WFC solver (`observed_facility::hex_wfc`), presentation projector (`observed_match::hex_wfc`), and TrenchBroom tile compiler (`observed_authoring`) share identical lattice metrics and face conventions.

---

## Submodules

- **[`coords`](src/coords.rs)**: Axial coordinates (`HexCoord(pub q: i32, pub r: i32, pub level: i32)`), grid size definitions (`HexGridSize`), and distance calculations (`travel_distance`, `lateral_distance`).
- **[`faces`](src/faces.rs)**: Eight-face prism adjacency model (`HexFace`: North, NorthEast, SouthEast, South, SouthWest, NorthWest, Up, Down).
- **[`metrics`](src/metrics.rs)**: Quantized physical metrics (7m flat-to-flat, 8.08m corner-to-corner, 5m level height) ensuring TrenchBroom integer map units snap cleanly.
- **[`ports`](src/ports.rs)**: Port class signatures (`PortClass`, `PortSignature`, `ports_compatible`) defining threshold connection compatibility across hex faces.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_hex
```
