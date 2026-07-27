# Arc O — Legible Districts

Arc O makes the hex facility a set of *places*. The facility is hardened (Arc M) and
networked (Arc N), but it reads as undifferentiated, and four structural findings from the
2026-07-26 planning survey explain why. Its implementation contract is the approved
ten-phase plan recorded in `ROADMAP.md`:

1. Arc M closeout and the isometric observer lab;
2. the full-screen isometric tac map;
3. spatial districts;
4. per-district composition profiles;
5. the `Expanse` archetype;
6. authored stair towers and vertical rebalance;
7. district-exclusive tilesets;
8. rooms bound to districts;
9. co-op mode and the sixteen-seat roster;
10. the hands-on arc gate.

## The findings this arc exists to close

Recorded as bug backlog #13–#17
([../bug_backlog.md](../bug_backlog.md)); each was found by reading the solver,
the catalog, and the projector rather than by playing.

- **Districts are not spatial.** `register_for`
  (`crates/observed_facility/src/hex_wfc/relayout.rs:670`) draws an architecture register
  per hex. Nine of the ten registers are white noise; only `LiminalGrid` has a contiguous
  zone. There is nothing to head toward. (#14)
- **Vertical circulation is a monoculture.** Zero authored `stair_tower` modules exist, so
  every `Shaft` cell in every district resolves to one procedural switchback through the
  `"generic"` fallback. The weight table makes Shaft ~39 % and ramps ~36 % of the live
  connective alphabet, leaving ~24 % flat corridor. (#13)
- **Rooms have no geometric identity.** `blueprint_cell_archetype` discards both of its
  parameters and returns `"sanctuary"` unconditionally. (#15)
- **There is no vocabulary for open space.** `HexArchetype` is a closed eight-variant enum
  matched exhaustively in roughly fourteen places; `../tile_authoring.md` documents this as
  the reason novel archetypes never reach the game. Vast halls cannot be *asked for*. (#16
  covers the related dead content; the enum itself is addressed in Phase 108.)

## Scope rulings

- **Lab first, then the game.** The isometric observer is built before any composition
  change so every later phase is falsifiable on sight. The same renderer becomes the
  in-game map — one implementation, not two.
- **The map never sees ground truth.** In the lab it renders the world directly; in the
  game it reads `HexPlayerMapKnowledge` only. Fog-of-war and the no-rival-leakage property
  are not negotiable, and the full-screen view does not become a HUD.
- **The ten architecture registers are the vocabulary.** No new registers. The user-facing
  names map onto existing variants: liminal → `LiminalGrid`, lumen → `OverlitGrid`, silo →
  `Wellshaft`, megastructure → `Megastructure`.
- **Districts differ in what is built, not only in how it is lit.** Palette work alone
  cannot deliver this; the composition profiles are the substance.
- **Everything district-derived stays seed-stable and generation-independent.** Districts,
  profiles, and room binding all feed the LAN frame digest and the simulation-content hash.
- **The roster opens to sixteen; the wire format is the real work.** The simulation already
  tolerates a single team and a larger roster. The fixed four-command frame, the datagram
  budget, the 2v2 guards, and the two-team lobby are what actually block it.
- Out of scope: internet matchmaking, NAT traversal, relays, authentication, PvP combat,
  arbitrary procedural mesh generation, and any change to the deprecated `GameState::Match`
  or demoted `GameState::FullWfc` paths.

## Closure gates

- Districts are contiguous neighbourhoods, and a player can tell which one they are in from
  geometry alone before reading the palette.
- Liminal Grid produces continuous open volumes; Overlit Grid winds; Wellshaft and
  Megastructure are visibly vertical — shown in before/after isometric captures at the five
  pinned seeds.
- Vertical circulation varies by district, and the `stair_tower` exemption is gone from the
  generic-fallback coverage assertion.
- A district-exclusive tile is provably unreachable from a foreign register.
- Room roles have distinct geometry and appear only in their bound districts, degrading
  gracefully when a district is small or absent on a seed.
- The full-screen isometric map is usable at production facility size and leaks no rival or
  undiscovered information.
- A sixteen-player co-op match completes over real UDP with bots disabled, including a
  packet-loss test at the worst-case frame size.
- `bot_soak_has_no_stalls` passes with composition profiles enabled.

## Known traps

- **The bot soak.** Composition tendencies were compiled off (`8d6e10d`) because they broke
  `bot_soak_has_no_stalls`. Reproduce that failure before Phase 107 re-approaches it.
- **The datagram cliff is not hypothetical.** A sixteen-seat frame bundle at the current
  frame window is roughly 1 808 bytes against a 1 200-byte limit — `encode` returns
  `Oversized`. Chunking is required, not optional.
- **Know which WFC the `wfc` feature gates.** It gates `full_wfc` (the demoted square
  lattice) and its `ghx_proc_gen` dependency, *not* `hex_wfc` — the hex solver is pure
  data/math and always compiles (`crates/observed_facility/Cargo.toml`). So Arc O's solver
  work is covered by a plain `cargo dev-test`, but anything that also touches `full_wfc`
  needs `--features wfc` or it is silently unbuilt.
- **`tilec audit-seams` is a hardcoded narrative report.** Its pass does not check your
  tile. Trust `validate`, the CAD render, and your own captures.
- **Global variety scoring fights specialization.** `score_layout`'s `archetype_variety` is
  a Shannon entropy over all archetype kinds and will silently penalize exactly what
  Phase 107 produces.
- **`architecture_surface(register, Floor)` is register-blind.** Every base register falls
  through to `surface(SurfaceRole::Plain)`; only `LiminalGrid` has its own structural
  family. The per-neighbourhood colour lives in `architecture(register).accent`. And note
  the ten registers collapse onto **seven** accent families, because
  `district_for_architecture` maps them onto six districts plus the LiminalGrid override —
  so colour alone can never separate all ten. Phase 110 has to decide whether
  district-exclusive *tilesets* are enough, or whether the style mapping widens too.
- **The map vocabulary lives in `observed_style`, not in either renderer.**
  `hex_sketch` owns every slab height and footprint width, and `hex_link` owns the
  connection treatment; `iso_observer_lab` and `game/src/hex_wfc/view/map` each
  only map their archetypes onto its roles. Adding an archetype (Phase 108's
  `Expanse`) means adding one role there and an arm in each mapping — not a new
  table.
- **The 3D orthographic far plane defaults to 1000 m**, which a production facility's
  diagonal exceeds on its own. Any whole-facility camera must set it explicitly or it
  silently clips most of the map away.

Phase-specific as-landed notes and evidence links are added here as each slice is verified.

## Phase hand-offs

- [Phase 104 — Arc M Closeout & Isometric Observer Lab](phase_104_iso_observer.md) `[x]`
- [Phase 105 — Full-Screen Isometric Tac Map](phase_105_isometric_tac_map.md) `[x]`
- Phase 106 — Spatial Districts
- Phase 107 — District Composition Profiles
- Phase 108 — The `Expanse` Archetype
- Phase 109 — Authored Stair Towers & Vertical Rebalance
- Phase 110 — District-Exclusive Tilesets
- Phase 111 — Rooms Belong to Districts
- Phase 112 — Co-op Mode & the Sixteen-Seat Roster
- Phase 113 — Arc Gate
