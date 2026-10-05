# Last Promenade evidence

**Sky / Thinning, floor 8 (simulation level 7).** One real finite Architect card places three joined sectors with raised mineral bridges, open floor, open sky and sheltered threshold landings. See the [design and inspirational references](../../last_promenade_wonder_proposal.md).

## Real Architect placement and first-person inspection

The capture runs the production game on seed 1, a 24 × 17 × 8 facility. Evidence staging selects a discovery location with a cloned legality probe and positions the team there; it does not globally reveal the live Architect map. The actual placement uses the normal finite card and command/refusal path. The fixture pauses only after all three physical placements match the accepted command.

| Image | What it establishes |
| --- | --- |
| [Architect board](architect-board-1280x800.png) | The team's discovered part of the facility before play. |
| [Card and target](architect-play-1280x800.png) | Sky district, the finite card, orientation and complete bridge thumbnail. |
| [Building in](architect-building-in-1280x800.png) | The accepted composition enters the actual board presentation. |
| [Built](architect-built-1280x800.png) | The committed three-cell structure. |
| [Arrival](architect-promenade-arrival-1280x800.png) | The wide doorway landing and ramp toward the exposed loop. |
| [Open well](architect-promenade-open-well-1280x800.png) | Real gaps reveal the surviving Reactor roof below. |
| [Gathering landing](architect-promenade-gathering-landing-1280x800.png) | Powered portal lights, pale crossings and open sky. |
| [Unpowered](architect-promenade-unpowered-1280x800.png) | Guide panels switch off; fixed sources use their existing standby scale while sky light remains. |

All stills are unretouched 1280 × 800 screenshots. Static inspection poses are evidence camera placements, not gameplay movement.

## Production-controller tour

[Walkthrough MP4](promenade-walk.mp4) follows the entry ramp, the complete bridge loop in both directions and the return to the threshold. The body uses the live committed collision snapshot and the production Rapier character controller, stepping twice at 1/60 s per saved 30 fps frame. It does not teleport between route points.

The H.264 / yuv420p MP4 contains **434 consecutive 1280 × 800 frames at 30 fps**, lasting **14.47 seconds**. The controller completes in 436 capture frames; the last two screenshot requests do not flush before application exit. Frame numbering is contiguous and `ffprobe` verifies the format, dimensions, count and duration. The tour is collision/route evidence; the held-tool presentation can retain the inspection body's pose while this evidence controller moves.

## Authoritative geometry and CAD

[CAD image](promenade-plan.png), [SVG](promenade-plan.svg), [joined evidence map](promenade-plan.map).

The three production sectors contain **60 convex hulls and 9 light sources** total, within the 128-hull composition budget. Each strict version-2 cell has 20 hulls, open floor, two internal Span faces and one exterior Door face. The thresholds meet the 0.5 m floor datum; ramps rise two metres to the 2.5 m bridge loop. The assembled bounds are approximately 28 × 6.125 × 28 m.

[`assemble_plan.py`](assemble_plan.py) assembles production orientations 0, 2 and 4 at their exact lattice translations. The joined `.map` is a version-1 room snapshot for CAD only; it is outside the active catalog. [`finish_cad.py`](finish_cad.py) normalizes the SVG and renders the image with librsvg. Production validation uses the six strict sector sources, rather than this legacy visualization contract. The CAD command's optional JPEG helper lacks Pillow in this environment; the SVG and PNG are successfully produced and visually inspected.

## Verification

- Forge regeneration is byte-reproducible; all six headings validate and use explicit open-floor policy.
- The production controller traverses both loop directions and all entry ramps at every orientation on Sky.
- Each sector's nine deck points weld into six graph nodes and five walk edges, with real ramp/bridge heights. The production bot driver enters, completes both circuits, and returns to the threshold at all six headings in an isolated three-cell fixture; no neighboring floor can bypass the crossings.
- Stepping off the lip falls through actual absent geometry. A second fixture projects genuine Reactor tiles below, catches the same fall on their roof, and walks/jumps back through the threshold and ramp.
- The waiting bay's physical blade blocks a sightline; the gathering platform has no overhead roof.
- The card is finite, appended after existing IDs and Sky-specific. Wrong district and protected sibling refusal leave the whole physical composition untouched. The atomic integration test retains the facility generators.
- All six existing/new wonder-card integration cases pass. Preview checks cover the complete footprint at all headings and require every raised walking hull to survive individually.
- Fixed lights work before entry, stay in place across all three cells, disable the moving district key and follow floor power. Visible panels use cached powered/unpowered materials; cell deletion removes their children and repeated spawning reuses meshes.

The final workspace gate passes: `cargo fmt --all`, warning-free `cargo dev-clippy` (19.88 s), and `cargo dev-test` (**2,726 passed, 0 failed, 45 ignored**). The ignored hand-availability measurement is recorded below. Two existing physical-placement fixtures now stage real finite Tile/Stair cards instead of relying on a particular shuffled hand; their protected-structure and physical-climb assertions remain intact.

The affected ignored hand-availability sweep completed in **157.50 seconds**, sampling **799 beats** across four live fixtures:

| Fixture | Sampled beats | Beats with a playable card anywhere | Beats with a playable card on an Observer floor |
| --- | ---: | ---: | ---: |
| Pocket | 4 | 3 | 3 |
| Quick Climb | 400 | 400 | 396 |
| Full Ascent | 18 | 17 | 17 |
| Deep Stack | 377 | 376 | 376 |

This measures finite-hand availability; it does not assert competitive balance.

Ordinary Sky materials and the existing summit victory rules are unchanged. No new victory mechanic is implemented.


## Reproduce

Set the machine's shared `CARGO_TARGET_DIR` and avoid overlapping Cargo builds. Begin with an empty capture directory.

```bash
cargo run -p observed_authoring --bin tilec -- gen-tiles
cargo run -p observed_authoring --bin tilec -- build
OBSERVED2_CAPTURE_HEX_WFC_ARCHITECT=/tmp/promenade-capture \
OBSERVED2_PROMENADE_PORTRAITS=1 \
RUST_LOG=warn,observed_game=info cargo dev-run -p observed_game
ffmpeg -y -framerate 30 -i /tmp/promenade-capture/promenade-walk-%03d.png \
  -c:v libx264 -pix_fmt yuv420p -crf 20 -movflags +faststart \
  docs/evidence/last-promenade/promenade-walk.mp4
python3 docs/evidence/last-promenade/assemble_plan.py
cargo run -p observed_authoring --bin tilec -- render-cad \
  docs/evidence/last-promenade/promenade-plan.map /tmp/promenade-plan.svg
python3 docs/evidence/last-promenade/finish_cad.py /tmp/promenade-plan.svg
cargo fmt --all
cargo dev-clippy
cargo dev-test
cargo test -p observed_match --lib how_much_of_the_hand_is_playable -- --ignored --nocapture
```

The final catalog contains **284 active modules / 380 authored source maps**, including the same 96 retired compatibility sources.

- Catalog: `028080c8b31907c1cd0197dedc2e8f21d64b6862320cf78d6ee53a32e2538b87`
- Profile, unchanged: `7b57da365f6c4de7876cd76adfd985db582d6f1610999c29d89d116739b639d1`
- Folded simulation: `efa5e5c931115aeed123e5aa701a22c49d64970d80a3d2e1138d0d1666a15fc5`

These checks establish traversal and integration rather than multiplayer balance.
