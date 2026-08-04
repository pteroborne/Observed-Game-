# Spectator overview

| key | does |
|---|---|
| `O` | toggle the overview (spectating only) |
| `F` | cycle which body is followed |
| `R` | rotate one detent - six, 60 degrees apart |
| `[` / `]` | narrow / widen the tile radius, 2..12, default 3 |

`OBSERVED2_SPECTATE_OVERVIEW=<detent>` opens it for capture, because a capture
run has no keyboard. `OBSERVED2_SPECTATE_TRACE=1` logs the framing and a
breakdown of what the cutaway kept and why.

The view snaps to the *centre of the body's tile* rather than tracking the body
itself: following a walking body slides the whole facility under a fixed camera,
which is unreadable at this scale. Snapped, the view holds still while the body
crosses a tile and steps once when the body does.

## What is shared with the studio, and what is not

`observed_style::iso` holds the **reading**: pitch, the six detents anchored at
45 degrees, orthographic framing, the `Layer` focus rule, and the `survives`
cutaway predicate. `composition_studio` delegates to it, so the two agree.

What is **not** shared is the **drawing**. The studio meshes authored hulls and
applies the cutaway to them. The overview draws massing prisms and never calls
`survives`. That is the whole of why the studio reads as a building and this
reads as a field of blocks - same camera, different renderer.

## Known artifact: thin verticals left standing by the cutaway

Thin vertical stubs stand above the plan at its far side. The natural reading is
that they are bleeding through from the floor above. **They are not**, and it is
worth recording how that was settled, because the natural reading is wrong.

Two independent tests, both against the floor-above hypothesis:

1. `trace_cutaway` reports `from_other_level=0` - of the 160 hulls drawn, none
   comes from a cell on another level. On its own this is not conclusive: a
   two-level tile's upper half carries its *base* cell's level, so the metric is
   blind to exactly the case being proposed.
2. Halving the band to the bottom 4 m of the storey does not remove them. If
   they were upper-half or ceiling geometry they would have gone. More of them
   became visible instead, as the walls in front were culled.

So they are low geometry on the body's own storey - narrow wall segments and
door jambs at the far side, left standing when the cutaway removed what stood
beside them. `survives` keeps far walls and drops near ones, and a jamb whose
neighbours went reads as a post in mid-air.

That is inherent to a cutaway rather than a bug in the level filter, and the
studio's rule would do the same given the same geometry. Two directions if it
becomes worth fixing: cull a hull whose neighbours in the same cell were culled
(needs adjacency the pieces do not carry), or widen `INTERIOR_RADIUS` so narrow
perimeter fittings count as interior and stay with the floor. Neither is
obviously right.

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
