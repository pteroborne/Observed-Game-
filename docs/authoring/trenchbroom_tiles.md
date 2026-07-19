# TrenchBroom tile authoring

The Quake `.map` files are the canonical structural sources. Generated RON is
never where geometry is edited, and the retired `bake_tiles` command now fails
instead of overwriting a TrenchBroom save.

## Install and open

1. Run `powershell -ExecutionPolicy Bypass -File tools/trenchbroom/install_config.ps1`.
2. Start TrenchBroom, select **Observed 2**, and choose `assets/tiles` as its game path.
3. Create a strict source with
   `cargo run -p observed_authoring --bin tilec -- new cell my_district/my_tile assets/tiles/authored/my_tile.map`.
4. Open that map. The FGD exposes `tile_meta`, `tile_cell`, and `tile_port` in
   the entity browser. TrenchBroom writes a point entity's `origin`; do not
   type boundary coordinates by hand.

`tilec new room ...` creates a whole-room starting point. Add one `tile_cell`
per relative axial footprint cell and set `room_role` before adding geometry.
The source origin is cell `(0, 0, 0)`. Editor Z is game Y; 16 editor units are
one metre and one facility level is 128 editor units.

## Commands

- `tilec validate <map>` parses one source and runs its geometry contract.
- `tilec audit [root]` validates all non-ignored maps, reports legacy versus
  strict coverage, hull sharing, runtime compatibility entries, and content hash.
- `tilec build [root] [catalog] [manifest]` produces a deterministic compiled
  hull catalogue and the legacy runtime manifest from those same maps.
- `tilec new <cell|room> <id> <path>` refuses to overwrite an existing file.

Run these commands through `cargo run -p observed_authoring --bin tilec -- ...`
from the workspace root.

## Enforced contract

All sources get convex-brush, declared-footprint, typed-port, unique-key/ID,
and deterministic-catalog validation. Version-2 sources additionally enforce:

- a connected cell/level footprint;
- ports owned by footprint cells and placed at the exact external face centre;
- no ports on internal whole-room seams;
- continuous centre-and-six-inset floor coverage unless a cell declares `floor = open`;
- at least 2.2 metres of sampled capsule headroom;
- a continuous ramp hull with no steeper than a 0.65 rise/run ratio;
- no more than 32 convex hulls for a cell kit or 128 for a whole-room module;
- stable IDs, weights, rotation policy, register scope, and content hashes.

`register_scope = all` stores structure once and exposes all six compiled
rotations when `rotation_policy = sixfold`. Use an explicit register list only
when geometry itself changes; colour, emission, lights, and fog remain owned by
`observed_style`.

## Incremental adoption

The 1,332 Arc-L sources remain version-1 compatibility maps so the played game
does not change in a big-bang migration. Their `.map` geometry is still
canonical and may be edited safely. New version-2 modules bypass the legacy
manifest: the game verifies `compiled_catalog.ron`, expands declared rotations
and register scopes, and adds strict cells directly to WFC selection. Matching
whole-room modules are selected by role, exact rotated footprint, every external
port, and named thresholds; one whole-room instance takes precedence over all
per-cell fallbacks in its footprint. Authored `weight` participates in seeded
selection. No Rust edit is required after `tilec build`.

The compiler hash is carried in match snapshots/replays and checked against
`compiled_catalog.sha256` at game startup. A changed simulation catalogue is
therefore an explicit compatibility change rather than silent geometry drift.
`assets/tiles/.tileignore` contains only the five duplicated teaching/template
maps.

The floor gate samples each declared cell at its centre and six inset corners;
headroom is checked at the capsule centreline. This catches centre-only slabs,
full-room holes, and the fake-ramp class of defects, but is not an arbitrary
navigation-mesh proof. The tile lab and controller walk corpus remain the final
traversal gates.
