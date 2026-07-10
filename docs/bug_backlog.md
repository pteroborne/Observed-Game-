# Bug Backlog

Known defects from playtesting, recorded 2026-07-09. As of 2026-07-10 every open
entry is scheduled into an Arc H phase (see per-entry pointers). New findings land
here first, then get scheduled. Keep entries self-contained enough to hand to an
agent cold.

## Open

### 1. Control rebind captures its own activation key
**Scheduled: Arc H Phase 63** ([arc_h/phase_63_control_rebind.md](arc_h/phase_63_control_rebind.md)).
"Press Enter to remap" then immediately binds the control to Enter — the capture
starts listening while the activation press is still live. **Ruling: replace the
custom capture flow with the proven `control_lab` rebind overlay machinery instead
of maintaining our own.** Whatever the implementation, the capture must ignore the
key event that activated it (start on key-release, or swallow the first press).
Look at: `game/src/screens/match_runtime/pause_settings.rs` (rebind capture),
`game/src/screens/settings.rs`, `labs/control_lab` (the proven overlays).

### 2. Stretched textures and bad "ceiling tile" geometry
**Scheduled: Arc H Phase 62** ([arc_h/phase_62_style_reconciliation.md](arc_h/phase_62_style_reconciliation.md)) — widened by the 2026-07-10 Arc F review: the district palette is visually absent under the imported albedos, so this is a style-layer reconciliation, not just a UV fix.
Wall/floor/ceiling albedo textures stretch oddly (UVs likely not scaled to world
units per face, so long walls smear the texture). Separately, "ceiling tiles" were
added at some point as actual geometry and don't visually work — evaluate removing
them or replacing with a flat textured ceiling. Look at: the place renderer's shell
build (`game/src/screens/place/` — wall/floor/ceiling mesh + UV generation),
`game/src/view/assets.rs` texture material setup.

### 3. Hallway thresholds overlap or land in corners
**Scheduled: Arc H Phase 64** ([arc_h/phase_64_threshold_integrity.md](arc_h/phase_64_threshold_integrity.md)).
Some doorway thresholds in hallway interiors render overlapped with walls or
smashed into random corners. This should be impossible by construction (door
columns are chosen by the maze/WFC generators and projected into `DoorGap`s), so
something in the projection or interior-wall placement disagrees with the gap
positions. Reproduce across seeds, then look at: `game/src/teleport/geom.rs`
(hallway interior projection + gap placement), `game/src/wfc_interior.rs` /
`observed_facility::wfc` door-column choice, and the DFS maze door columns.
The map-audit evidence pipeline (`OBSERVED2_CAPTURE_MAP_AUDIT`) should be able to
catch this class of defect once the geometry check knows thresholds must not
intersect interior walls.

### 4. Observation rooms don't show camera views
**Scheduled: Arc H Phase 65** ([arc_h/phase_65_observation_rooms.md](arc_h/phase_65_observation_rooms.md)) — ruling: schematic feeds from sim data, not render-to-texture; panels never write into MapKnowledge.
The tether/guardian observation rooms are supposed to show a "camera view" of each
room in the map (the Arc D "living monitors" promise); currently each panel shows a
room number, an object jutting through the panel, and a blue screen. Look at:
`game/src/screens/place/monitors.rs` (panel build + what feeds the screens),
Phase 31/45 monitor work in ROADMAP for the intended behavior. Decide whether the
panels should be real render-to-texture views or a legible schematic "feed"
(fog-of-war rules apply either way — a camera room revealing the whole map must
stay consistent with `MapKnowledge`).

## Minor / hygiene

**Scheduled: Arc H Phases 61 (as-landed notes) and 66 (env-var panic, digest test).**

- `BotPopulations::from_str` panics on an unknown `OBSERVED2_BOTS` token; prefer a
  logged warning + default (a typo'd env var should not kill the boot).
- Phase 54's "all-on default digest pinned unchanged" characterization test was
  never literally added; coverage is indirect via the seed-corpus and career-loop
  tests. Add the digest pin (`game/src/sim/director.rs`,
  `apply_population_config`).
- Phase 54/55 docs never got "As landed" notes (56 has one; 56 also has a
  "Review fixes" section).

## Fixed

- ~~District ambience wash / location beds never playing~~ — fixed 2026-07-09
  (see `docs/arc_f/phase_56_audio_content_spatial.md` "Review fixes": bed sinks
  excluded from the director's blanket write; corridor/gantry beds wired).
