# FPS Visibility Lab

Phase 21 promotes the room-granular sight from `fps_observation_lab` into a
continuous first-person visibility field. The camera observes fixed sub-room cells
only when they are inside its horizontal frustum, within range, and unobstructed by
the authored wall segments. A doorway connection freezes only when one of its
endpoints is directly visible; deterministic decoherence re-pairs only connections
whose endpoints are unseen.

The result gives "freeze what you see" a precise partial-room meaning. Looking
through a doorway may reveal a narrow slice of the next room without freezing every
connection owned by that room.

## Functionality evidence

![FPS Visibility showcase](../../docs/evidence/fps_visibility_lab.png)

Green floor cells are visible; dark cells are outside the frustum or occluded.
Gold doorway markers and graph links are frozen by direct sight. Cyan links are
unseen and have been deterministically re-paired across several decoherences. The
room coverage percentages in the monitor make partial observation explicit.

## What it demonstrates

- Continuous visibility from camera position and yaw, sampled on a stable 5 by 5
  field in every room.
- Frustum, range, and wall-segment occlusion tests in pure floor-plane geometry.
- Partial-room observation: visible coverage is tracked per room rather than as a
  room-sized boolean.
- Precise freezing: seeing a doorway endpoint freezes its whole graph connection;
  merely seeing another cell in that room does not.
- Deterministic camera stepping and visibility: the same `PlayerIntent` tape
  reproduces the same poses and observed sets.
- Deterministic decoherence that changes only fully unseen doorway connections.

## Controls

- `A` / `D` or left / right arrows: look
- `W` / `S`: move forward / backward
- `Q` / `E`: strafe
- `Space`: decohere now
- `P`: toggle auto-decoherence
- `R`: reset
- `F1`: toggle debug visualization

The camera is intentionally constrained to the centre room. Phase 21 isolates the
visibility question; Phase 20 already owns general first-person movement and
collision.

## Debug visualization

- Green/dark sub-room cells for visible/unseen coverage
- White frustum-edge rays
- Grey wall occluders and orange doorway apertures
- Gold visible/frozen doorway endpoints and connections
- Cyan unseen/free connections
- Monitor with pose, cell/door counts, per-room coverage, decoherence count,
  lifecycle health, and `[PASS]` / `[FAIL]`

## Success conditions

1. Small camera turns and movements update the visible cell field continuously.
2. Walls block cells outside a doorway aperture; a neighbouring room can be only
   partly visible.
3. The same input sequence reproduces identical camera poses and visible sets.
4. Every connection with a visible endpoint keeps its partner through decoherence.
5. At least one unseen connection rewires, while the matching remains valid.
6. Reset restores the authored pose and topology without leaking camera or UI
   entities.

## Manual verification

1. Run `cargo run -p fps_visibility_lab`.
2. Slowly turn with `A` / `D`. Green cells should enter and leave along the white
   frustum edges rather than toggling an entire room at once.
3. Move sideways with `Q` / `E` and watch the visible slice through a doorway
   change as the wall occludes different cells.
4. Leave auto-decoherence on, or press `Space`. Gold links must hold while cyan
   links change.
5. Press `R` repeatedly and confirm the monitor stays `[PASS]`.

## Regenerating the evidence screenshot

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/fps_visibility_lab.png"
cargo run -p fps_visibility_lab
```
