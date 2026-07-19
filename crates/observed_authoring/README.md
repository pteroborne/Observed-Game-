# observed_authoring

Pure TrenchBroom/Quake-map intake for the continuous hex facility. It converts
convex brushes to deterministic hulls, validates typed hex footprints and
ports, and compiles canonical `.map` sources into a content-hashed catalogue.
Strict cell rotations/register scopes and exact-contract whole-room modules are
expanded directly from that catalogue into the production WFC runtime; the
manifest is only the compatibility seam for unmigrated v1 maps.

New sources use the version-2 `tile_meta` / `tile_cell` / `tile_port` contract
documented in [the authoring guide](../../docs/authoring/trenchbroom_tiles.md).
The Arc-L generator under `tile_source` remains regression-fixture code only;
it is not permitted to overwrite hand-authored assets.
