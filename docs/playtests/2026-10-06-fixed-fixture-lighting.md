# Fixed fixture lighting — 2026-10-06

Removed the legacy district spotlight from the canonical hex facility. Its position
and colour followed the viewed Observer's current cell, producing a bright pool
that travelled through the building. There is no replacement automatic player light.
Fixed authored practicals, their bounded nearby shadow budget, sky, ambient fill,
fog and the carried lantern remain. The cutaway overview retains its own studio
light, active only in that view. Legacy tile/kinetic lab inspection lights remain
separate from production first-person lighting.

## Gameplay evidence

[First-person gameplay: Backrooms into the Library](../evidence/fixed_fixture_lighting/gameplay.mp4)

The 60-second clip follows an autonomous Observer through the production Ascent
match: Backrooms corridors, the ascent into the Library, its bookshelves and ramp,
and the approach to the next floor. Paths and changes in height remain visible;
light pools sit at authored fixtures rather than travelling with the Observer.
The held lantern is still visible. The Backrooms have warm fluorescent light and
retain darker recesses; the Library reads as a brighter room composition.

- [Backrooms corridor](../evidence/fixed_fixture_lighting/backrooms.png)
- [Library ramp](../evidence/fixed_fixture_lighting/library.png)

Capture command:

```bash
OBSERVED2_ASSET_ROOT="$PWD/assets" \
OBSERVED2_CONFIG_DIR=/tmp/observed-solo-capture-config \
OBSERVED2_CAPTURE_HEX_WFC_EYES=/tmp/observed-fixed-light-eyes \
cargo dev-run -p observed_game
```

This capture advances game time by 1/15 second per saved frame. Encoding the first
900 frames at 15 fps, then duplicating frames to 30 fps, gives normal simulation
speed rather than the capture mode's usual 2x playback:

```bash
ffmpeg -framerate 15 \
-i /tmp/observed-fixed-light-eyes/frames/frame_%05d.png \
-t 60 -vf fps=30 -c:v libx264 -pix_fmt yuv420p -crf 20 \
-movflags +faststart docs/evidence/fixed_fixture_lighting/gameplay.mp4
```

The recording is visual evidence without an audio track. Native frames and encoded
keyframes were inspected; the MP4 is H.264, 1440 × 900, yuv420p, 30 fps. Source frames
remain outside git. The full capture exited successfully; two final queued
screenshots were dropped during shutdown, beyond the excerpt used here.

## Power-cut evidence

[Generator cut, restored, recharge and dead station](../evidence/fixed_fixture_lighting/power.mp4)

A second production Ascent capture walks to the generator and operates it through
normal input, then recharges at a station and stages the floor unpowered again.
The floor darkens, while its generator, nearby path, tool and dead station stay
readable without the removed district key. The run completed all eight capture
beats in 495 frames. Its 16.5-second video is recorded at 30 fps, matching the
capture's 1/30-second game-time step.

[Unpowered generator and path](../evidence/fixed_fixture_lighting/unpowered.png)

Reproduce with `OBSERVED2_CAPTURE_HEX_WFC_POWER=/tmp/observed-fixed-light-power`
in place of the eyes capture variable above, then encode its frames at 30 fps.
Both recordings use existing production capture paths; the power scene stages
fixture approaches and initial charge for a repeatable demonstration.

## Verification

Nine focused lighting checks passed, covering nearest-fixture shadow selection and
fixed wonder lights remaining in place across cell changes and generator toggles.
The obsolete test for the removed spotlight's initial values was deleted.

Final gate passed: `cargo fmt --all`, `cargo dev-clippy`, and `cargo dev-test`.
The workspace completed 2,799 tests with no failures or warnings; the existing
45 ignored tests remain outside the ordinary gate. `git diff --check` and all
report links passed verification, and both MP4s decoded without errors.
