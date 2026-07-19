# Phase 97 — First-Class TrenchBroom Pipeline

**Status:** complete; automated round-trip verified.

Quake `.map` files are canonical, human-owned sources. The retired generator
refuses to overwrite them. The repository now owns a TrenchBroom game config,
FGD, installation script, and a deterministic `tilec` CLI with `new`,
`validate`, `audit`, and `build` commands.

`tilec build` emits a SHA-256 content-hashed hull catalogue, its network hash
sidecar, and the v1 compatibility manifest. The game verifies the catalogue and
sidecar together. Strict v2 cells enter WFC directly with compiled rotations,
register scope, and authored weight; no Rust catalogue edit is required.

The committed audit covers 1,332 valid canonical sources and 1,299 deduplicated
hull sets. The current corpus intentionally remains v1-compatible while new or
migrated sources use the strict contract.

Verification:

- `cargo test -p observed_authoring --lib` — 23 passed.
- `cargo clippy -p observed_authoring --all-targets -- -D warnings`.
- `cargo test -p hex_tile_lab --lib`.
- Game-side cached catalogue load and SHA-256 agreement tests.

The editor configuration is ready to install, but TrenchBroom GUI interaction
remains part of the final hands-on gate rather than an automated claim.
