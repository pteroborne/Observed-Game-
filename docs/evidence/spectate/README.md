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
