# Recorded world playback

The canonical Hex facility and Architect Ascent replay now show the recorded world,
replacing the numbered-room trace. The replacement lives on `codex/end-to-end-ux`.

## Player experience

- The facility uses its actual authored hulls. Rewrites and retractions appear at
  their recorded moments, and scrubbing backward restores the earlier building.
- Observers move at their recorded physical positions. Their shared eye forms show
  their heading; the major and minor Guardian forms show recorded positions and
  hunting/frozen states. A corrupted Observer loses their physical body.
- Following a jailed Observer switches to that team's recorded prison maze. Its
  way out carries a green exit beacon.
- Follow, Team, Floor and Eyes views let the viewer inspect a player, a team's
  positions, a whole floor, or the recorded Observer vantage. Floor and rotation
  controls work independently of time.
- The timeline supports pointer scrubbing. The same time operations are available
  through focused buttons for keyboard/controller navigation: individual moments,
  ten-second jumps, and previous/next events. Playback speeds are 0.25x, 0.5x, 1x,
  2x and 4x, measured against recorded ticks rather than the number of samples.
- Card events identify the faction, card type/district, rotation and target. Card
  targets receive a visible event beacon. Instability tiles, recharge stations and
  deployed doors have legend-backed signals. Capture, prison release, rescue,
  corruption, Guardian releases/losses and the final victory remain in the timeline.
- Back to results preserves rewards and rematch setup. Continuing returns to the
  local main menu or the LAN lobby.

## Recording and ownership

`game/src/sim/replay/scene.rs` retains owned visual data, not a physics world.
Positions are normally sampled every six physical ticks (10 Hz), with additional
samples for events, geometry revisions and the final match state. Playback
interpolates continuous positions, but snaps across different physical spaces or
large teleports. The geometry at the selected sample stays discrete.

Each facility revision has an immutable structural snapshot. Unchanged revisions
share their snapshot, and unchanged collider pieces share their data across
revisions by stable collider ID and exact piece equality. Prison snapshots retain
team and maze seed identity. Records retain stable PlayerIds independently of the
viewer focus, including spectator focus changes and four-seat teams.

The replay camera, render target, meshes and actors belong to Replay. Presentation
reads only the tape and its own controls; it cannot step the live match, award
progression, or change the recorded data. Exiting removes its scene and render
resources. Local steps and accepted authoritative LAN frames use the same recorder.
LAN resync now rebuilds Ascent recording context as well as the physical tape.

Ascent's accepted-play event descriptions now retain the card label and faction,
including Rogue directive/sensor/surge plays. This changes diagnostic event text;
command legality, simulation outcomes and network versions stay the same.

The old schematic remains solely as compatibility presentation for tapes from the
deprecated isolated-place regression fixture. Canonical launches record world
playback from tick zero.

## Validation and evidence

See [the evidence set](../evidence/ux/replay_1280x800/README.md) for native 1280x800
captures, widget/text bounds, a playback video and source provenance. These use
real rule and physics ticks with a staged legal Rogue card and staged physical
catch/corruption transitions. The rules resolve the final Rogue victory. They are
presentation evidence, not a completed human game.

Regression coverage checks movement, card descriptions/targets, changing geometry,
shared immutable pieces, prison history, corruption, Guardian populations,
backward/forward sampling, interpolation boundaries, irregular event timing,
scrubbing, camera changes, scene cleanup/reentry and tape immutability. The existing
real-UDP multi-client and late-joiner tests now exercise the recorder, including
named card events applied through authoritative LAN frames. The multi-client
fixture now pins its facility seed: an unseeded run could end before its scheduled
late join and fail before reaching the replay assertions.

The required fmt and dev-clippy gates pass without warnings. `dev-test` passes
2,705 tests with 45 existing ignored tests; the game accounts for 574 passes and
six of those ignored tests. Results are recorded beside the evidence.
The known ignored asserting `hex_full_match_soak` remains an outstanding extended
suite issue; this work does not claim `dev-test-all` is green.

## Completion boundary

This replaces playback for the last match held in memory. Saved replay files, a
replay library, recorded audio/chat, live streaming and exact reconstruction of
first-person HUD/tool animations are outside this slice. Geometry and physical
positions are recorded, rather than replaying the simulation commands anew.

Human keyboard/pointer/controller acceptance and two graphical LAN game processes
remain open. The real-UDP tests establish the automated recording path, not those
human acceptance checks. Cosmetics previews and loading wording follow afterward.

## Reproduce the native captures

Use the shared build cache serially, as described in the repository runbook:

```bash
CARGO_TARGET_DIR=/srv/build-cache/cargo cargo build -p observed_game --bin observed --features bevy/dynamic_linking
```

Launch `/srv/build-cache/cargo/debug/observed` from a temporary working directory
with a separate temporary config directory. Set `OBSERVED2_ASSET_ROOT` to this
worktree's `assets`, `OBSERVED2_CONFIG_DIR` to that temporary config directory,
`OBSERVED2_CAPTURE_FRONTEND` to the output directory, and
`OBSERVED2_CAPTURE_FRONTEND_REPLAY=1`. On Linux the dynamic build also needs
`/srv/build-cache/cargo/debug/deps` and the Rust toolchain's `lib/rustlib/x86_64-unknown-linux-gnu/lib`
on `LD_LIBRARY_PATH`.

Add `OBSERVED2_CAPTURE_REPLAY_VIDEO=1` for the motion sequence. The driver saves
`frames/playback_000.png` onward at 70 evenly spaced recorded timestamps, waiting
for the scene to settle and each GPU capture to complete. This keeps the encoded
sequence independent of render/readback latency. The evidence video omits the
first render-target warmup frame. Encode:

```bash
ffmpeg -y -framerate 10 -start_number 1 -i frames/playback_%03d.png -c:v libx264 -pix_fmt yuv420p -crf 20 -movflags +faststart playback.mp4
```
