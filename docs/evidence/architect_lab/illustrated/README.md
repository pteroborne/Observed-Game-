# Illustrated tiles and rotation proof — 2026-09-11

The five illustrated studies are integrated into the playable WASM interface.
These captures show the shipped renderer, not a composited concept mockup.
Open **Tiles** to inspect and rotate the artwork; the hunt pauses during inspection.

Preview through the host's existing Tailscale connection:
`http://100.71.157.43:8774`. The preview server must remain running on the host.
HTTP access and the browser gameplay flow were checked through that address from the host; physical phone performance still requires
Will's hands-on playtest.

## Rotation fix

The old browser placed increasing axial rows upward but drew their diagonal ports
downward. Its corresponding Bevy board also disagreed with its face vectors.
The browser now derives directions from Rust's `HexFace` deltas and uses positive
screen Y for increasing rows. The native diagnostic uses negative world Y.

`RogueGame.render_contract()` supplies face order, opposite faces, neighbor deltas,
and all card masks. The hand, preview, and placed tile use the same painted
orientation for identical masks, including rotationally symmetric shapes.
Hand artwork updates when the player rotates. Exact wall/aperture geometry sits
over the raster illustration and always reads the simulation's actual ports.

## Checks completed

- Formatting and workspace Clippy, warnings denied.
- Full workspace tests: **2,061 passed, 0 failed, 37 existing ignored**, no warnings.
- Web-feature Clippy, warnings denied; renderer-free web tests: **20 passed**.
- Combined desktop/web focused rotation suite: **29 passed**.
- Native Bevy lab launched, rendered, saved evidence, and exited successfully.
- Browser: all **6 directions**, **30 shape/rotation combinations**, **64 horizontal
  port masks**, **18 legal WASM preview→placement cases**, and doors placed on
  **all 6 target edges**. SVG edge transforms are checked against actual neighbors.
- Touch UI: six rotations restore the original mask; hand, preview, and committed
  artwork agree. All five images decode successfully. Gallery works at phone,
  narrow phone, desktop, and landscape sizes without horizontal overflow.
- Full mobile gameplay: tap → preview → play → capture → victory → restart;
  real touch pan/pinch without accidental selection; both two-floor scenarios.

## Captures and sources

- `tile-gallery.png`: the actual tile viewer, cropped by the browser to its dialog.
- `tile-set.png`: desktop viewer over the paused board.
- `phone-tiles.png`: the same viewer on a 390px touch viewport.
- `phone-preview.png`: rotated repair preview in the playable interface.
- [Gameplay captures](../mobile/README.md) include all responsive sizes and victory.
- [Artwork sources, prompts, and runtime contract](../../../../labs/architect_lab/web/art/README.md).

The study includes straight, bend, junction, door, and prison art. Other WFC port
patterns use painted flagstone material with exact procedural edges; this is not
claimed to be a complete authored tile catalog or an isometric 3D renderer.
