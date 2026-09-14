# Illustrated tactical tile study

Five original raster illustrations generated with the **built-in imagegen tool**:
`straight`, `bend`, `junction`, `door`, and `prison`. Art direction: 1980s British
fantasy wargame, Gary Chalk reference, bold umber ink, chunky limestone, parchment
and muted ochre washes, sparse olive moss. The original prompts and generation
paths are preserved in [prompts.json](prompts.json).

Original PNGs are in `source/`; the web bundle serves only the 512×512 WebP
copies, encoded with ImageMagick at quality 82. All five browser assets together
are 496,612 bytes. The originals remain unchanged, including the generated copies
outside the repository. No external image service is used at runtime.

## Topology contract

The drawing is a tactical abstraction over the actual `HexWfcWorld`, not a second
simulation or a rendered 3D tile. `RogueGame.render_contract()` provides Rust's
face deltas, opposite faces, and all five shapes' six rotated masks. `hex-layout.js`
projects those deltas into screen space; `tile-art.js` cuts exact apertures and
walls from each cell's mask. Authored paint cannot add a traversable edge.

Positive screen Y runs downward. Clockwise order is E, SE, SW, W, NW, NE. The
browser uses `x = sqrt(3) * radius * (q + r/2)`, `y = 1.5 * radius * r`.
Bevy's world Y runs upward, so its board uses the opposite Y sign.

Straight, bend, and junction masks select the matching illustrated shape. Other
WFC signatures use an enlarged crop of the central flagstones plus the same exact
edge geometry. These are deliberate material fallbacks, not extra authored tile
families. Prison edges still follow its actual room; a door remains separate
threshold equipment. Closed gates cross the threshold; open gates leave a gap.

Equivalent symmetric masks use one canonical painted orientation in the hand,
preview, and placed tile, so committing a card cannot unexpectedly flip the art.
The tile-study gallery allows all six explicit artwork rotations for inspection.
Its prison is a four-port example; real prison masks are not hardcoded to it.

## Verification

`node scripts/verify-architect-rotations.cjs` checks all six face directions against
real neighbors, all 30 shape/orientation combinations, SVG transforms and exact
apertures for all 64 horizontal signatures, and 18 legal WASM preview→placement
cases, plus door placement onto all six target edges. It also checks the actual touch UI's six turns and visual orientation after
placement, gallery scrolling on narrow/landscape screens, and successful decoding
of every asset. Gameplay and gesture checks remain in
`scripts/verify-architect-web.cjs`.
