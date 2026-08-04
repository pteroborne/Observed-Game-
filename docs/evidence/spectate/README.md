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

## Where the code lives

- **`view/camera.rs`** - where you look *from*. The play eye pose, the
  spectator chase, the overview's isometric framing, the orthographic
  projection, and the fog scale that follows the framing.
- **`view/spectate.rs`** - what you *see*. The overview's state and keys, the
  cutaway over the resident geometry, the storey filter, the practicals, and
  the overview key light.
- **`view/lighting.rs`** - the district light rig. It used to own the camera as
  well, for no better reason than that both touch `GameCam`.

**`OverviewFrame` has one writer and three readers.** `sync_frame` computes the
framing once; the camera pose, the orthographic scale and the fog range read
it. They each used to derive it, which is exactly how they drifted: two were
moved to frame the body's tile while the third still framed the whole facility,
so the camera and the zoom disagreed and the view came out a thumbnail. The
whole-facility framing helpers are deleted - with one writer there is no third
call site, and with no function there is nothing to call by mistake.

## Partly solved: the floating stubs were partly the light fixtures

A practical spawns **two** entities: a visible diffuser mesh and the point light
beside it. Only the light carried `HexPractical`, and the mesh carried nothing -
no storey tag and no `Cutaway`. So diffusers drew on every storey at once
whatever the cutaway said, and a ceiling-mounted one whose ceiling had been cut
is a thin bright stub hanging in the air.

They are geometry, so they now get what geometry gets: the storey filter and the
cutaway, measured as a point because a diffuser is small next to the tests being
applied to it. That removed a good half of the stubs.

**A cluster at the far side remains, and is not fixtures.** Earlier work
(`5ffe59e`) established by two independent tests that those are low geometry on
the body's own storey - narrow wall segments and jambs left standing when the
cutaway removed what stood beside them. `survives` keeps far walls and drops
near ones, and a jamb whose neighbours went reads as a post in mid-air.

Two directions, neither obviously right: cull a hull whose neighbours in the
same cell were culled (needs adjacency the pieces do not carry), or widen
`INTERIOR_RADIUS` so narrow perimeter fittings count as interior and stay with
the floor.

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
