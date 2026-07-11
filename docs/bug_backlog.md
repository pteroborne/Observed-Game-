# Bug Backlog

Known defects from playtesting, recorded 2026-07-09. Every remaining open entry
is scheduled into an Arc H phase (see per-entry pointers). New findings land here
first, then get scheduled. Keep entries self-contained enough to hand to an agent
cold.

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

## Minor / hygiene

**Scheduled: Arc H Phase 61 (as-landed notes).**

- Phase 54/55 docs never got "As landed" notes (56 has one; 56 also has a
  "Review fixes" section).

## Fixed

- ~~Unknown `OBSERVED2_BOTS` tokens panic and the all-on digest is unpinned~~ —
  fixed 2026-07-11 in the Phase-66 implementation. Unknown tokens warn and use
  the all-on default; the default director corpus digest is pinned at
  `0x539C35C626B9B30F`.

- ~~Observation rooms don't show camera views~~ — fixed 2026-07-11 by Arc H
  Phase 65. Both rooms now show live 3×3 schematic feeds from simulation data;
  anchor/guardian signals remain cyan/red, panel geometry is flush, and the
  feeds never mutate `MapKnowledge`. See
  [phase_65_observation_rooms.md](arc_h/phase_65_observation_rooms.md).

- ~~District ambience wash / location beds never playing~~ — fixed 2026-07-09
  (see `docs/arc_f/phase_56_audio_content_spatial.md` "Review fixes": bed sinks
  excluded from the director's blanket write; corridor/gantry beds wired).
