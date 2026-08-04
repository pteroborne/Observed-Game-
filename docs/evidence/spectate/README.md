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

## Known artifact: fixtures left hanging by the cutaway

Thin stubs float above the plan. They are not from the storey above - the level
filter assigns each hull to one storey by its midpoint, and they survive that -
they are fixtures on *this* storey whose ceilings the cutaway removed, leaving
them attached to nothing.

That is inherent to a cutaway rather than a bug in the filter: `survives` drops
a hull whose lowest point clears head height, so a lintel or a light housing
hung just under a ceiling goes with it, while one hung a little lower stays and
is left in mid-air. The studio has the same rule and would show the same thing
given the same fixtures.

Two directions if it becomes worth fixing: cull anything whose *support* was
culled (needs a parent relationship the pieces do not currently carry), or
lower `HEAD_CLEARANCE` for the game's fixtures specifically. Neither is
obviously right, and the plan reads well enough with them.

## Solved: position and scale must frame the same box

The overview came out a thumbnail of geometry in an empty frame, and the cause
was not the cutaway, the lighting, or residency - all three were working.

`sync_camera` framed with `framing_around` (a radius box on the body) while
`sync_projection` still framed with `framing_fitted` (the whole facility). So
the camera followed the body and the *zoom* stayed set for all 340 m of
building. Position and scale have to answer about the same box or they disagree
about what is being looked at.

Attribution is what found it, after two wrong guesses. `trace_cutaway` counts
`survives` outcomes by test, and reported floor=176 kept, ceiling=168 cut,
interior=147 kept, near=183 cut, far=171 kept - **58 percent surviving**, with
`min_y=0.00 max_y=16.00`. That refuted "the cutaway is culling everything" and
"the cell-local conversion is wrong by whole levels" in one line, and left the
framing as the only thing it could be.

The scattered dots in earlier captures were facility-wide markers, not cells
without geometry - a red herring that cost one of the wrong guesses.

Result: `overview_tiles_cutaway.png`.

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
