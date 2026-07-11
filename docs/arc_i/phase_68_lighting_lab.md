# Phase 68 — `lighting_lab`: Nine Liminal Dioramas

**Objective:** prove the arc's entire lighting vocabulary in one new lab before
any game code changes: nine static dioramas, one per user-chosen reference, each
isolating one liminal element, each screenshotted and viewed. Prototype the
luminance-corridor check against these captures. Read [README.md](README.md)
first.

## Read first

- `docs/light_and_line_arc_plan.md` — the reference table (the nine scenes, their
  liminal elements, and their registers) and the four axes they span.
- An existing lab for the crate skeleton and reset discipline (e.g.
  `labs/style_lab` or `labs/outline_legibility_lab`); `Catalogue.md` for how labs
  are described and the reset rule (reset must remove all entities/resources).
- `crates/observed_style` — palettes, `drained`, signal treatments; the lab reads
  style, it does not fork it.
- `game/src/screens/place/lighting.rs` — the current game rig (unshadowed point
  fill + flicker fixtures) the lab is designed to replace the thinking behind.
- Arc H capture gotchas in [README.md](README.md) rule 2 (Vulkan, `OBSERVED2_CAPTURE`).

## Design rulings (already decided)

- **Static dioramas, freestanding geometry** (user ruling 2026-07-11). No
  gameplay, no reuse of the game's place spawners; findings transfer as
  parameters. Keys 1–9 switch scenes; each key fully tears down the previous
  scene (lab reset rule).
- **The nine scenes** (from the plan's table): 1 Japanese slat corridor (low warm
  shadow-casting spot, thin vertical occluders; also a paper-screen emissive
  variant behind the slats), 2 Brutalist mass (monolithic grey forms, one hard
  shaft through a single big aperture, deep reveals, no fill), 3 Backrooms
  (low ceiling, emissive panel grid, cranked ambient, zero shadows, yellow
  shift), 4 Severance corridor (long narrow white symmetric, continuous cool
  emissive strip, one sparing accent color), 5 Halo hall (tall obtuse-angle
  polygon volume, near-zero cool ambient, emissive cove seam-lines, one wide
  high volumetric shaft), 6 BLAME! void (huge dark volume, heavy distance fog,
  one faint distant light, silhouetted struts, camera low), 7 Silo shaft
  (vertical well, warm caged practicals descending at intervals, tight ranges so
  each pool is an island), 8 Library of Babel (hexagonal gallery, warm center
  lamp, doorways on alternating faces showing identical lit hexagons beyond —
  fake the repetition with mirrored geometry; the game's preview system is NOT
  imported), 9 Rudon's Plane (one very long corridor whose fixture spacing, trim
  density, and palette saturation decay with distance into fog).
- **Shadow casters are `SpotLight`s** wherever possible (one shadow-map face vs
  a point light's six). Scene 1 is where shadow-map resolution/bias is tuned
  until thin-slat blades are stable; record the settings.
- **The volumetrics × bloom matrix lives in scene 5** (and optionally 1):
  `VolumetricFog`/`FogVolume` swept off→heavy, crossed with bloom on/off — the
  known HDR/bloom gotcha area gets explicit captures of all four corners.
- **The signal kit is staged in every scene:** keystone object, anchor torch,
  exit-style door, one rival sprite — placed visibly. A register that hides a
  signal is a finding, not a style.
- **Luminance-corridor check, prototyped here:** a pure function over a capture
  (percentile-luminance floor AND ceiling — not-too-dark, not-blown-out) with
  unit tests against these nine captures plus the archived near-black
  `docs/evidence/phase_62_long_hallway.png` (must FAIL the floor) and an
  all-white fixture (must FAIL the ceiling). Lives somewhere Phase 69 can
  promote into the game's visual audit without rework.
- **Frame-time notes per scene** on this machine (Vulkan), recorded in the
  as-landed note — volumetric scenes especially.

## Files you may edit

`labs/lighting_lab/**` (new crate), workspace `Cargo.toml` (member entry),
`Catalogue.md` (lab description). Nothing under `game/` or `crates/` changes in
this phase.

## Success criteria

- `cargo run -p lighting_lab` boots to scene 1; keys 1–9 switch scenes with no
  entity/resource leaks (reset test or leak check in the lab).
- `OBSERVED2_CAPTURE=docs/evidence/lighting_lab/scene_<n>.png`-style capture
  path produces one PNG per scene; all nine **viewed** and individually
  described in the as-landed note (what the light does, whether every signal
  reads, what failed).
- The volumetrics × bloom matrix captures exist and are viewed; the interaction
  verdict is written down.
- The luminance-corridor check exists with the pinned fixtures above passing/
  failing as specified.
- Frame-time table for the nine scenes in the as-landed note.
- Full verification recipe green.
