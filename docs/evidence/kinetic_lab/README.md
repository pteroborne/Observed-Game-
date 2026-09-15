# Kinetic tool lab — evidence

![Shove preview](shove-preview.png)

Captured with:

```bash
OBSERVED2_CAPTURE=docs/evidence/kinetic_lab/shove-preview.png cargo run -p kinetic_lab
```

The capture pauses time and stands the Observer west of the ledge run with a
minor Guardian in the lane, so the frame shows a live preview rather than an
idle board.

## What the frame proves

The green line runs from the minor Guardian at `(4, 3)`, across the three amber
**ledge** cells at `(5..7, 3)`, to a double ring on the void rim at `(8, 3)`, and
the panel reads `lane: Void after 4 cells`.

That number is the point. A push carries `PUSH_IMPULSE = 3` cells, but ledges do
not consume impulse, so the target travels **four** and leaves the floor. The
rule "momentum carries across unrailed geometry" is legible in the frame before
the trigger is pulled, which is the lab's answer to whether a shove is fair — the
preview is the same pure `resolve_shove` the tick will run, not an estimate of
it.

The double ring is the second, non-colour channel on a lethal outcome; grey
would mean the target survives, amber that it lands on a tile already
retracting, red that structure blocks the shove.

## The rest of the board, left to right

| Mark | Cell | What it is |
| --- | --- | --- |
| Yellow square | `(1, 1)` | Generator — cut power here and sight collapses to your own cell |
| Grey hex | `(2, 2)` | Wall — a shove into it is `Blocked` and nothing moves |
| Green ringed square | `(2, 4)` | Recharge station, drawn live because the floor has power |
| Cyan square | `(3, 3)` | Observer, with the facing lane drawn east |
| Orange squares | `(4, 3)`, `(3, 4)` | Minor Guardians — neither freezes when looked at |
| Dark red hex | `(3, 5)` | Retracting tile, fading toward void as its countdown runs |
| Amber hexes | `(5..7, 3)` | The unrailed ledge run |
| Pink square | `(6, 5)` | Major Guardian, drawn awake because it is outside the lane |

## First person, same board

![First-person lane](fps-lane.png)

```bash
OBSERVED2_CAPTURE=docs/evidence/kinetic_lab/fps-lane.png \
  cargo run -p kinetic_lab --bin kinetic_fps
```

The same `KineticWorld`, the same `resolve_shove`, the same `lane: Void after 4
cells` — now standing on the floor rather than looking down at it. The capture
pauses the board first, because with capture working a Guardian one plate away
jails you in 24 ticks.

**Shape language.** Rank reads as the order of the solid. The orange **cube** is
a minor Guardian, the pink **tetrahedron** the major — a rarer solid for a rarer
thing. Both hold whole lattice cells and cross between them in a crisp snap that
finishes well inside their step interval, then wait. That clockwork read is not
decoration: Guardians move in quantised steps because the determinism contract
required it, and the presentation simply stopped apologising for it.

**The green beam** marks where a push would send the target. It is a beam and
not a floor decal because the lethal destination here is void about seventy
metres out, where a flat marker is both invisible and hidden behind the very
Guardian being aimed at. The amber plates between are the unrailed ledge run,
and the beam standing past their far end is the ledge rule stated in world
space: momentum outlives the three cells a push pays for.

Plate geometry is rectangular rather than hexagonal, and that is deliberate. The
lattice tiles exactly with 14x12 plates offset 7 per row, so they meet with no
gap and no overlap and a void cell leaves an exact hole. The hex lattice remains
the connectivity and targeting structure; an authored tile's *geometry* was
never required to be a hex prism, and rendering what you collide with is what
the Legibility Contract actually asks for.

## The shove in motion

[`kinetic_shove.mp4`](kinetic_shove.mp4) — 7 seconds, 1440x900, 60 fps.

```bash
OBSERVED2_CAPTURE_SEQUENCE=docs/evidence/kinetic_lab/frames \
  cargo run -p kinetic_lab --bin kinetic_fps
ffmpeg -y -framerate 60 -i docs/evidence/kinetic_lab/frames/frame_%04d.png \
  -c:v libx264 -pix_fmt yuv420p -crf 20 -movflags +faststart \
  docs/evidence/kinetic_lab/kinetic_shove.mp4
```

The frame directory is gitignored; the mp4 is the tracked artefact, exactly as
for `district_tour.mp4`.

The run holds on the lane for a second and a half with the preview beam already
up, pushes at tick 90, follows the target down as it crosses the ledge run and
goes over the rim, then walks. Two things only a recording can show: the
clockwork snap-and-hold of a Guardian between plates, and a shove actually
*leaving* — the target flies, tumbles, and drops, rather than blinking out.

**The recording is a real run, not an animation.** Starting positions and
heading are staged, then every tick after that is driven by `PlayerIntent` and
`ToolRequest` through the same `Embodiment::step` a player's hands use. Because
the model is deterministic, the tape reproduces frame for frame. Capture parks
`FixedUpdate` and advances exactly one tick per rendered frame, so saving a PNG
per frame — far slower than the simulation — cannot let catch-up skip state; the
file is a true 60 Hz record rather than sampled moments.

## The full tour

[`kinetic_tour.mp4`](kinetic_tour.mp4) — 73 seconds, 1440x900, 60 fps, captioned.

```bash
OBSERVED2_CAPTURE_SEQUENCE=docs/evidence/kinetic_lab/frames \
  cargo run -p kinetic_lab --bin kinetic_fps
ffmpeg -y -framerate 60 -i docs/evidence/kinetic_lab/frames/frame_%04d.png \
  -c:v libx264 -pix_fmt yuv420p -crf 21 -movflags +faststart \
  docs/evidence/kinetic_lab/kinetic_tour.mp4
```

It demonstrates, in order: the lane drawn with and without a target; the
crosshair's three states; a lethal push carrying past the impulse into void; a
refused push and a refused generator, each announced; a pull; a push onto solid
floor that only staggers; recharging at a live station; cutting and restoring
power; a jump; walking off the rim into void and respawning; being jailed; and a
reset.

**It is driven, not animated.** A closed-loop director reads authoritative state
and emits `PlayerIntent` and `ToolRequest` through the same `Embodiment::step` a
player's hands use. Starting positions and a handful of mid-run Guardian
placements are staged, and every one of those is captioned "Staged:" on screen
as it happens.

### The same tour with `--no-jail`

[`kinetic_tour_nojail.mp4`](kinetic_tour_nojail.mp4) — 74 seconds.

```bash
OBSERVED2_CAPTURE_SEQUENCE=docs/evidence/kinetic_lab/frames \
  cargo run -p kinetic_lab --bin kinetic_fps -- --no-jail
```

Identical script, except the finale. The HUD carries `rules off: jail`
throughout, so the two recordings can never be confused for one another.

The ending is the interesting part. The same event happens — a Guardian walks
onto the Observer's plate — and the run simply continues: no overlay, no capture,
`free` in the HUD, and the crosshair sitting green on the cube now standing on
top of you. That is the flag's whole argument in one shot: with jail off a
Guardian stops being a fail state and becomes a thing in your way.

The script chooses its own finale from the rules rather than being captioned by
hand, because a caption reading "ends the run" over a run that does not end is
exactly the class of claim this lab has already been caught making.
`the_finale_caption_matches_the_rules` asserts that the jailing cut never
mentions the flag and the lenient cut never claims a jail.

### The headless counterpart

`demo::run_headless` plays the identical script with no window and no frame
timing, and `the_scripted_demo_demonstrates_every_claim_it_makes` asserts that
each captioned claim actually occurred, in order, and that the Observer is not
jailed before the finale.

That test exists because it was needed. Four cuts of this script were silently
broken — jailed at tick 299, then 982, then 1685, and one that stopped a single
stride short of the rim so the last third of the tour never ran. Each was found
in milliseconds by the headless log rather than by watching frames, and each was
invisible in a still. The recurring cause is worth keeping in mind when writing
any scripted scenario here: **Guardians pursue continuously**, so "parked out of
the way" has to be measured in ticks, not in distance. The board is nine plates
wide and a minor crosses one every 150 ticks, which means every corner is within
reach of a seventy-second recording.

## The siege, and what it exposed

[`kinetic_siege.mp4`](kinetic_siege.mp4) — 72 seconds, a one-minute siege in the
solved facility.

```bash
OBSERVED2_CAPTURE_SEQUENCE=docs/evidence/kinetic_lab/frames \
  cargo run -p kinetic_lab --bin kinetic_fps -- --siege --minutes=1
```

The run ends **OVERRUN on wave 5**, with the line that matters underneath it:
*0 Guardians into the void.* A full minute of siege and the tool killed nothing.
That is not a broken recording. It is the finding.

**The kinetic tool is close to inert in a corridor facility.** It was designed
and tuned on the authored board, which is an open plain with a void rim — there,
a push carries three plates and usually ends in nothing. In a building, a shove
travels along a lattice face and stops at the first wall, which in a corridor is
right there. Measured on the pinned facility: most plates have exactly two open
faces, and the overwhelming majority of shoves resolve `Blocked` after a plate or
less. The tool staggers things against walls instead of removing them.
`the_tool_is_weak_in_a_corridor_facility` pins that ratio so it cannot drift
without somebody noticing.

Two fixes were needed before the siege was even a game, and both were invisible
on the open board:

- **Pursuit was greedy.** Guardians took whichever neighbouring plate reduced
  the distance most, which is correct on a plain and strands them in the first
  corridor that runs the wrong way before it runs the right way. They milled
  about near their spawns and an Observer who never moved *won* — which looks
  exactly like a working siege until you watch one. Pursuit is breadth-first now.
- **The recording bot walked into walls**, for the same reason, and one whole cut
  was a single grey rectangle. It holds its post now and lets them come, which is
  what a siege is anyway.

What to take from the video: the facility reads, the waves arrive and grow, the
clock creates real pressure, and the tool does not answer it. That is a design
question — more open geometry, a different verb, or accepting that the tool's job
is crowd control rather than kills — and it is better to have it now than after
the thing is promoted anywhere near production.

## Caveat

Neither view answers whether a shove is *satisfying* in the sense a playtest
means — that needs a person at the keyboard, and it is the open human gate. What
these frames establish is that the rules are legible, reproducible, and identical
across two independent presentations of the same simulation.
