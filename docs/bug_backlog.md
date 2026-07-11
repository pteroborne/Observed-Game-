# Bug Backlog

Known defects from playtesting, recorded 2026-07-09 (the original four were
fixed in Arc H and hand-audited in the 2026-07-11 ship-gate playtest). Open
entries are post-ship findings, unscheduled. New findings land here first, then
get scheduled. Keep entries self-contained enough to hand to an agent cold.

## Open

### 5. Bot-POV walkthrough stalls in the observation room
**Found 2026-07-11 during the Phase 66 ship-gate evidence refresh.** The
`OBSERVED2_CAPTURE_BOT` walkthrough (all bots on, seed 1) reaches Room 11 — the
tether observation room — around shot 55 of 120 and never moves again; the last
half of the GIF is a frozen frame. The committed 2026-07-10 GIF shows the same
tail-freeze trait (frames 40–48 identical), so this is pre-existing, not an Arc H
regression. Look at: `game/src/evidence/capture/bot_pov.rs` (route driving) and
whatever nav target the bot chases after keystones are force-collected — it may
be waiting on a door/decoherence that never happens while the capture holds
`runtime.done = false`.

### 6. World-space evidence captures render nearly black
**Found 2026-07-11 during the Phase 66 ship-gate evidence refresh.** The
`phase_62_*` capture set (match, long hallway, hallway doorway, drained room) and
the visual audit's geometry/lighting scenarios render as near-black voids — the
committed Phase 62 evidence has looked like this since it landed, so Phase 62's
own success criteria ("two districts unmistakable in a capture", "no smeared
textures on long walls") have never actually been demonstrated by its captures.
Rooms lit by emissives (observation monitors, torch glow) read fine, and the
style-presence material check passes, so this is either (a) capture vantages
pointing at unlit space, (b) the screenshot path losing HDR/bloom that the live
render has, or (c) the world genuinely being too dark to read — the human
playtest adjudicates which. Look at: `game/src/evidence/capture/scenarios.rs`
(camera staging), `game/src/screens/match_runtime/ambience.rs` (palette
luminance), Bevy screenshot tonemapping.

### 7. Audio mix balance: effects too loud, ambience too quiet
**Found 2026-07-11 during the Phase 66 user playtest listen-through.** The
relative mix is off: one-shot effects (cues, UI, stings) overpower the ambient
beds, and the district/location beds sit too quiet to register as atmosphere.
Rebalance the default gain staging between effect cues and beds — this is a mix
change, not a regeneration; the palette itself is fine. Look at:
`game/src/screens/audio.rs` / the `AudioDirector` gain constants
(bed sink levels vs. cue playback volumes), and confirm the settings sliders
(master/SFX/music) still scale sensibly around the new defaults.

## Minor / hygiene

**Scheduled: Arc H Phase 61 (as-landed notes).**

- Phase 54/55 docs never got "As landed" notes (56 has one; 56 also has a
  "Review fixes" section).

## Fixed

- ~~Control rebind captures its own activation key~~ — fixed by Arc H Phase 63
  (`control_lab` overlay machinery adopted; capture arms on the activation key's
  release, conflicts surface visibly). Overlay evidence captured and viewed
  2026-07-11 (`docs/evidence/phase_63_rebind_overlay.png`); hand-audited in the
  ship-gate playtest.

- ~~Stretched textures and bad "ceiling tile" geometry~~ — fixed by Arc H
  Phase 62 (palette-over-albedo, world-unit UVs, triangulated ceiling geometry
  removed, style-presence audit check added). Hand-audited in the ship-gate
  playtest 2026-07-11. Note: the phase's own captures render nearly black
  (see open #6), so the in-game look is the verified artifact, not the PNGs.

- ~~Hallway thresholds overlap or land in corners~~ — fixed by Arc H Phase 64
  (audit-first reproduction, generator/projection agreement fixed at the source,
  permanent map-validation gate). Threshold renders captured and viewed
  2026-07-11 (`docs/evidence/phase_64_threshold_integrity/`); hand-audited in
  the ship-gate playtest.

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
