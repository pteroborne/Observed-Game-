# Spectator overview

`O` opens it while spectating, `F` cycles the followed body, `R` rotates a
detent. `OBSERVED2_SPECTATE_OVERVIEW=<detent>` opens it for capture, because a
capture run has no keyboard.

## What is shared with the studio, and what is not

`observed_style::iso` holds the **reading**: pitch, the six detents anchored at
45 degrees, orthographic framing, the `Layer` focus rule, and the `survives`
cutaway predicate. `composition_studio` delegates to it, so the two agree.

What is **not** shared is the **drawing**. The studio meshes authored hulls and
applies the cutaway to them. The overview draws massing prisms and never calls
`survives`. That is the whole of why the studio reads as a building and this
reads as a field of blocks - same camera, different renderer.

## Open: the cutaway culls almost everything

The overview draws only real authored geometry inside `tile_radius`, cut away
with `iso::survives`, and it comes out nearly empty. **The scattered practicals
are the tell**: those are lights belonging to cells that *are* resident, spread
across the frame, while almost none of their hulls draw. So residency is
delivering and the cutaway is eating the result.

Ruled out already:

- **Not lighting.** The overview now takes the studio's fill (ambient 260 vs
  play's ~80) and its bearing-derived directional key. Barely changed the
  image. `GlobalAmbientLight` is the right lever - no camera-level
  `AmbientLight` in this project - so the darkness is a symptom, not the cause.
- **Not the framing.** `OBSERVED2_SPECTATE_TRACE=1` confirms the camera stands
  outside the facility, the far plane clears it, and the orthographic scale
  fits the radius box.

**Prime suspect, untested: the cell-local conversion in `cutaway_measure`.** It
makes a hull's height range cell-local by subtracting
`hex_origin(piece.source_cell).y`. Every cell's floor slab should therefore land
at 0.0..0.5 and survive test 1 unconditionally - and floors are visibly *not*
surviving. If a piece's `source_cell` is a room anchor on a different level than
the piece itself, that subtraction is wrong by whole 8 m levels, which would
push floors above `HEAD_CLEARANCE` and cull them **as ceilings**.

Cheapest way to confirm: count `survives` outcomes by test - floor / ceiling /
near-wall / kept - over one frame. If floors are being culled as ceilings, that
is the bug, and it is one line.

## Attempted and reverted: follow the body at a tile radius

2026-08-03. The right idea, and it went blank; recorded so the next attempt
starts from the measurement rather than the plan.

The pass did three things at once - tagged every hull with its cutaway measure,
applied `iso::survives` to the resident authored geometry, and re-framed the
camera on a `radius`-sized box around the body instead of the whole facility.
The result was a black frame.

**Bisected: the cutaway was not the cause.** Disabling `sync_cutaway` and
recapturing was still blank, which puts it in `framing_around`. Untested
suspicion, worth checking first and cheap to check: the fog is derived from
`iso.far`, and `far` shrinks with the box, so a tighter radius pulls
`fog_start` in with it - the same coupling that caused the original blank, just
reached from the other direction. Print `iso.far`, the camera distance, and the
fog range before touching geometry again.

The lesson that keeps repeating here: change one thing, capture, look. Three
changes in one pass cost a bisect to undo.
