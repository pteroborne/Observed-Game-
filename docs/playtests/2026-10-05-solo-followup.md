# Solo playthrough follow-up — 2026-10-05

The playthrough reported green, exposed Backrooms; inaudible movement and tool sounds;
a major Guardian moving while watched; unreliable kinetic hits; and a clearer replay
map than the live tactical map. The Library of Babel was the visual reference to keep.

## Changes

- Backrooms practical and key lights use warm white rather than green fluorescent
  light, with warm fog. Corridor walls remain enclosed even beside unbuilt cells.
  Room windows occupy at most one deterministic face per cell and use a 1.2 × 0.6 m
  transom, with its sill 1.65 m above the floor. The other districts keep their windows
  and open edges.
- Grounded Observer travel produces footsteps every 1.8 m; idle, airborne movement,
  switching the followed player and teleporting produce none. Event and kinetic-shot
  audio consume every simulation tick, including ticks between rendered frames. Released
  majors sound like a giant stone sliding while moving; minors make hollow box
  impacts on each completed flip. Frozen and idle majors fall silent. The followed Observer's catch
  stays listener-relative across the immediate prison teleport.
- Major Guardian sight uses the 56 m physical sight range, yaw, pitch and unobstructed
  body samples, including the crown above cover. Window glass and invisible rail guards stop bodies but permit sight;
  closed doors remain opaque. It no longer requires a one-edge walking route. Both original and
  released major models stop an unfinished visual glide as soon as they freeze.
- Input buttons remain pending until a simulation tick consumes them. Kinetic aim
  covers the visible Roller's cage rather than only its narrower traversal capsule.
  A confirmed local shot also names its push, pull or reorientation in the HUD.
  The kinetic tool still pushes/pulls; architecture and falls destroy minors.
- The live map shares the replay's actual authored mesh projection, district surfaces,
  floor slicing and removal of camera-facing walls. It shows one browsable floor,
  including upper geometry owned by a lower-floor room anchor.
  Team discovery still applies: whole-room hulls require a fresh, known footprint;
  stale and incomplete rooms retain flat memory markers. Player, exit, anchor,
  room-function and orientation landmarks remain explicit.
- Recharge bots retain a collider-checked detour during their final approach inside a cell,
  where the enclosed Backrooms can place a partition between body and station.
- Input compatibility advances to version 9 for the changed simulation behavior.

The Library materials and authored modules were the reference for this pass. The
Backrooms retain their own yellow wallpaper, carpet and dropped ceilings.

## Verification

Focused regression coverage checks long-distance observation versus blocked sight,
unfinished Guardian glides, cage-flank targeting, pending input across render frames,
grounded footstep cadence, enclosed Backrooms corridors and small transoms. Map
coverage compares its projected hull count with the shared replay cutaway and checks
that stale cells do not expose current hulls.

The shared cache contained an asset-root fallback compiled in a deleted checkout.
Verification uses `OBSERVED2_ASSET_ROOT="$PWD/assets"` to select this checkout's
committed assets explicitly; no cache was cleared. Saved game audio volumes were
100%, and the default system output was unmuted. The player confirmed that menu
clicks were audible while gameplay was silent. Speaker-level audio still needs a
human listening pass.

Final code gate: `cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test` passed;
2,800 tests passed, with the existing 45 ignored tests left out of the ordinary gate.
The relevant ignored `production_ascent_tick_times` instrument ran separately over
7,200 production ticks: median 0.248 ms, p95 0.370 ms, maximum 4.963 ms, five card
plays and no catches in this seeded run. These are measurements, not outcome targets.

The Guardian audio generator validated finite samples, peak headroom and loop seams.
The four changed OGGs also decoded without clipping. This pass uses original in-repo
synthesis; its source and regenerated metrics remain under `tools/` and
`assets/sounds/guardian/`, with provenance in `assets/SOURCES.md`.

## Captured evidence

- [Backrooms after the colour and enclosure pass](../evidence/solo_followup/backrooms.png)
- [Library reference](../evidence/solo_followup/library.png)
- [Guardian motion audition: stone drag, then box flips](../evidence/solo_followup/guardian-motion.mp3)
- [Live cutaway map with team discovery](../evidence/solo_followup/map.png)

The map capture stays in the completed match until its image is saved. For a quick
renderer check, `OBSERVED2_CAPTURE_HEX_WFC_MAP_FRAME=600` overrides the normal
7,200-frame exploration deadline. Native captures completed without playback
warnings; the game opened its audio stream through PipeWire.
