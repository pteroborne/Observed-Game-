# Phase 57 — Sprite Metadata Pipeline & the OGA Lab

**Objective:** turn the raw OpenGameArt intake into a metadata-driven sprite
pipeline — frame metadata, deterministic derivation, atlas support — proven in a
grown `oga_25d_lab` before anything touches the played game. Read
[README.md](README.md) and [../../sprite_roadmap.md](../../sprite_roadmap.md)
(the design reference: metadata spec, slot plan, presentation rules) first.

## Read first

- `labs/sprite3d_placeholder_lab/` — the proven `bevy_sprite3d` integration:
  loaded-image gating, fallback meshes, yaw-facing billboards, reset safety.
  This lab grows into (or is superseded by) `oga_25d_lab`.
- `game/src/view/sprites.rs` + `crates/observed_assets/src/lib.rs` — how the game
  consumes today's single-frame sprite slots; the pipeline must slot under this
  convention, not replace it.
- `assets/oga_25d/raw/` + `assets/SOURCES.md` — the intake: LAB sprites/textures
  (zips), Nmn items/decorations/guard (paletted PNGs), knekko guard sheet &
  decorations, XCVG keycards, crosshair/cursor packs.
- `sprite_roadmap.md` "Frame metadata spec" — rectangles, pivot,
  pixels-per-metre, directional count, semantic clips, material role.

## Design rulings (already decided)

- **Metadata is checked in and machine-readable** (RON or JSON — pick one and
  use it for every sheet) in `assets/oga_25d/derived/`, one file per derived
  sheet. Combat frames in source sheets are remapped to the semantic clips
  (`idle/walk/operate/alert/disrupted/exit/pickup`) or dropped — never exposed.
- **Derivation is a deterministic, rerunnable script** (Rust bin or xtask-style;
  no ad-hoc by-hand crops) from `raw/` → `derived/`: same inputs, byte-identical
  outputs. Derived PNGs are checked in so the game never depends on running it.
- **Atlas over loose frames** where the grid is regular (Bevy
  `TextureAtlasLayout`); standalone derived PNGs only for irregular sheets.
- **Lab-first, game-untouched:** this phase does not modify the played game's
  visuals. The lab is the proof surface; game integration starts in Phase 58.
- Lab reset must despawn all projected entities/resources without leaks
  (standing lab rule).

## Files you may edit

`labs/oga_25d_lab/**` (new; may start as a copy/rename of
`sprite3d_placeholder_lab` — if renaming, update `Cargo.toml` workspace members
and `Catalogue.md`), a derive-script bin (workspace member or lab sub-bin),
`assets/oga_25d/derived/**` (generated + metadata), `assets/SOURCES.md`
(derived-artifact notes), `Catalogue.md`, `Cargo.toml` (workspace list only).
Do NOT edit `game/src/**` or `crates/observed_assets` this phase.

## Implementation

1. **Metadata format + loader.** Define the spec from `sprite_roadmap.md` as
   plain structs with a serde loader; unit tests validate every checked-in
   metadata file (frames in bounds, pivots inside frames, clips reference
   existing frames, directional counts consistent).
2. **Derive script.** Unpack/crop the initial set — items, keycards, one guard
   sheet, a decoration sheet, LAB sampler — into `derived/`, emitting metadata
   skeletons; hand-tune metadata (pivots, ppm, clip assignments) and check in.
3. **Lab scenes.** `oga_25d_lab` shows: (a) actors with directional facing and
   idle/walk/alert/disrupted clips, camera-orbitable; (b) object/keystone/
   equipment stand-ins at game-plausible scale; (c) animated decorations;
   (d) LAB texture samples on simple shells; (e) a debug overlay of current
   sheet/frame/direction/clip; (f) billboard vs directional toggle; (g) reset.
4. **Capture.** `OBSERVED2_CAPTURE` evidence: one shot of the actor/object/
   decoration line-up, one with the metadata debug overlay visible.

## Success criteria

- Every metadata file passes the in-bounds/consistency tests; the derive script
  reruns byte-identically (test or documented check).
- The lab runs, animates directional clips, falls back procedurally when a
  derived file is missing, and resets without leaked entities/resources.
- Evidence PNGs land under `docs/evidence/` (`oga_25d_lab*.png`).
- `Catalogue.md` documents the lab; `cargo test -p oga_25d_lab` (or the renamed
  crate) passes; full verification recipe green.
