# Arc D — Liminal Scale & Living Fixtures

Planned 2026-07-03, immediately after Phase 42 (Arc C) established the race as contested
and readable. This arc is a **scaling arc**: it expands the facility from a proof-of-concept
nine-room dev map into a liminal, humanoid-scale labyrinth (24–32 rooms, procedurally
generated) and repairs the two shipped features that the gantry-and-scale work exposed
(observation monitors, spectator piloting). Based on an external plan (Codex) reviewed
and re-staged 2026-07-03; the review's key corrections are baked in below.

## The thesis

Arc C made the race contested and legible; the board itself is still a nine-room dev map.
Arc D takes the facility to liminal scale and repairs two incomplete shipped features.
The facility is a working procedural maze where every playstyle survives — solvability
under collapse and anchors is the immovable foundation. The race is worth racing only
when the board is large enough to hide and reveal, to reroute and pressurize.

## Design principles for the arc

1. **Fix shipped bugs before changing the world.** Monitors render static placeholders;
   spectators cannot piloting bots through gantry jumps. Both are mid-feature gaps
   upstream of scale work. Phase 43 closes them in the existing map, de-risking the
   procedural pivot.
2. **Lab-first for WFC topology per agents.md.** Generated maps do not ship before a
   lab proves the generation, dumps representative output, and the game confirms the
   corpus footprint is green.
3. **The spine is load-bearing.** Team progress, collapse, absorption, and solvability
   all key off the objective sequence (start room → keystones → exit). Generated maps
   must emit a protected start-to-exit spine as a first-class output with corpus
   validators ensuring coverage, role variety, determinism, and bounded retry failures.
4. **Liminal is dimensions, not just count.** The comfort-pass precedent (Phase 41b)
   proves that scale includes a room/hall dimension pass. Rooms breathe at the right
   scale; hallways are legible at a glance. Phase 46 includes that pass alongside the
   map flip.
5. **Determinism and arc-C invariants re-proven on generated maps.** Headless matches
   must equal interactive play; solvability must hold under all collapse/anchor
   combinations; characterization tests pin seeds and ensure replay byte-identity across
   PRNG and simulation layers.
6. **ghx_proc_gen enters behind a `wfc` feature with authored-map fallback.** Per the
   R11 dependency bar: problem proven (discovery_lab + room-role corpus), smallest
   adapter (WFC, seeded, validated), fallback intact, lab proves it, guard test gates
   the default flip. No tech debt carries forward.

---

## Threshold addendum (user design ruling, 2026-07-03)

Because thresholds teleport, the gantry's mount stairs are vestigial. A room's threshold
leads **directly onto a deck-level threshold** — you arrive on the platforms at height.
The entry landing is a slab, not a climb. Taking the safe ground lane is a chosen,
visible drop. This lands in D1 alongside the gantry deck-level entry.

---

## Stages

### D1 — Living Fixtures (fix shipped features, no map changes)

**Core work:**
- **Role-driven monitor rooms** now render live room previews. A `RoomRole::ObservationMonitor`
  displays nine panels (a 3×3 grid of adjacent rooms' geometry). Each panel uses the
  same room-preview rendering technique the threshold previews use — shared helper, shared
  material. Sightings from monitors feed the Phase 42 `RivalSightings` ledger as
  `Seen::Monitor` (never remotely freeze — monitors are read-only).
- **Guardian console from RoomRole::GuardianControl.** The existing guardian-interaction
  surface lands on an interactive console object matching the discovery-lab proof
  (interior collision + scene geometry).
- **Gantry deck-level entry.** Mount stairs removed; thresholds at the origin room
  project directly to deck-level thresholds on the gantry. The ground entry lands the
  body on the safe-bypass slab; the deck entry arrives at platform height. Per the
  threshold addendum.
- **Gantry-piloting spectator.** The bot driving the spectator body now visibly attempts
  gantry jumps with fall recovery. Falls land in the understory; the spectator advances
  to the recovered room and continues.
- **EXIT_ROOM migration.** Every `observed_match::mutable::EXIT_ROOM` consumer (mainly
  the director's first-escape check) moves to
  `CompetitiveFacility::exit_room()`, making the exit room role-driven and future-proof.

**Lab:** None — Phase 43 lands entirely in the existing sector_relay_v1 map.
**Verification:** game tests pass; monitors render live previews; spectator successfully
navigates gantries; no leaks when exiting a match.

---

### D2 — Map-Agnostic Plumbing (selection layer, no generation yet)

**Core work:**
- **Plumbing layer:** `game::map_catalog::active_map_spec(seed)` returns the active
  `MapSpec` for a given seed. During development, `OBSERVED2_MAP` env var selects
  the map by name; default is `sector_relay_v1` (pure refactor, lands green).
- **MapSpec builder contract:** ensures every map provides validated `MapSpec` output
  with required fields: room list, role assignment, hallway list, topology graph.

**Lab:** None — Phase 44 is a refactor.
**Verification:** `cargo test --all` passes; `OBSERVED2_MAP=sector_relay_v1 cargo run -p observed_game` behaves identically to before.

---

### D3 — WFC Topology In The Lab (procedural generation proof)

**Core work:**
- **WFC procedural generation.** `observed_facility::wfc` implements Wave Function Collapse
  topology generation. Started from archived `wfc_proc_gen_lab` code and ported to the
  current `observed_facility` crate. Feature-gated behind `wfc`.
- **Extended wfc_proc_gen_lab:** the lab now uses the generator, emits `MapSpec` output,
  visualizes rooms + hallways, and allows interactive seed tweaking. Success criteria:
  generation determinism, 24–32 dense rooms per seed, room-role coverage (incl. at least
  6 `ObservationMonitor` rooms to page all rooms at 9-panel density), spine emission
  (start → keystones → exit as a protected path), and `MapSpec::validate` passing.
- **Corpus tests:** a seeded suite validates (a) generation determinism (same seed =
  same geometry), (b) spine coverage and distinctness (every seed emits start, exit,
  and keystone rooms in a connected path), (c) role distribution (target counts met
  across the test set), (d) bounded retry on generation failure (WFC timeout and
  fallback to partial retry are logged, not errored), and (e) `MapSpec::validate`
  passes every output.

**Lab:** `wfc_proc_gen_lab` — extended to prove generation at scale. Success = corpus
passing, visualization legible, seed tweaking responsive.
**Verification:** lab renders deterministic, 24–32-room maps with visible spine; corpus
tests pin role coverage and generation stability over 50+ seeds.

---

### D4 — The Liminal Flip (default switch + comfort scale pass)

**Core work:**
- **WFC map becomes the default.** The game flips to procedurally generated maps by
  default. `OBSERVED2_MAP=dev` selects the old sector_relay_v1 for regression testing.
- **Room/hall scale pass.** Room dimensions scale by `RoomRole` (Reactor rooms taller,
  Decision rooms wider for sightlines, Keystone rooms prominent). Hallway lengths and
  widths scale by `CorridorRole` (main thoroughfares wider, side routes narrower).
  This is the liminal comfort pass — the facility breathes correctly at 24–32 rooms.
- **District assignment.** Rooms and hallways are assigned to flavor districts (neon,
  rust, bio) across the bigger map. Palette variance per district makes the space
  visually legible.
- **Per-seed generation memoization.** The test suite memoizes generated MapSpecs by
  seed so repeated runs stay fast (no re-generation per test).
- **Characterization + solvability/collapse corpus gates.** The existing game-layer
  determinism and solvability tests (characterization, collapse, bot-series) re-run
  on generated maps and pass with the same rigor as sector_relay_v1. All evidence
  captures (GIF, screenshots, audit renders) are refreshed.

**Lab:** None — this phase lands in the game.
**Verification:** `cargo test --workspace` passes on generated maps; `cargo run -p observed_game`
defaults to procedural, `OBSERVED2_MAP=dev` returns to the old map; evidence suite
regenerated; characterization and collapse tests pin determini­sm and solvability.

---

### D5 — WFC Corridor Interiors (DFS maze hallways)

**Core work:**
- **Archived hallway_wfc.rs ported.** The WFC maze interior builder (archived at the
  end of Arc G for exactly this moment) is ported onto current `WallSeg` geometry.
  Corridors now generate interior mazes (walls and passages within a hallway segment)
  based on `CorridorRole`.
- **Role-driven interiors.** `CorridorRole::Decision` corridors generate DFS mazes
  (multiple branching passages, sightline puzzles); `CorridorRole::Pressure` corridors
  generate simpler paths (faster traversal, fewer sightlines to exploit). Other roles
  remain straight-through.
- **DFS-maze fallback on generation failure.** If WFC maze generation times out, a
  simple DFS maze is generated instead. No hard failures; mazes always emit.
- **Representative pinned seeds.** The evidence suite includes 3–5 pinned seeds with
  representative corridor interior variety for manual review.

**Lab:** None — this phase lands in the game after D4.
**Verification:** corridor interiors render correctly; bot navmesh/routing adapts to
maze passages; characterization + solvability tests pass with interior corridors.

**As landed (2026-07-04, commit `47a6034`):** the archived generator moved into
`observed_facility::wfc` behind the `wfc` feature (`generate_interior_walls`/
`InteriorSeg`), keeping `ghx_proc_gen` out of the game; `game::wfc_interior` is the
pure `InteriorSeg → WallSeg` adapter, and it picks the same door columns the DFS maze
would so a fallback is seamless. Selection landed simpler than the plan's sketch:
only `CorridorRole::Mystery` edges take the WFC interior — every other role (and the
specless dev map) keeps the DFS+braid maze — resolved via the new
`MapSpec::corridor_role_between` and **frozen into `FrozenDest.corridor_role`** so the
doorway preview and the real crossing can't diverge (the same observe-to-freeze
discipline the whole teleport model rests on). A WFC non-convergence falls back to DFS
as a pure function of the seed. The load-bearing proof: a pinned-seed test shows WFC
converges with **zero retries** on every real hallway grid size (4×4/5×6/6×7/7×5/4×8),
so these are genuine WFC interiors, not silent fallback. The lab archive shrank to a
re-export of the live code. 800 workspace tests, 35 `--features wfc` tests, clippy
clean both feature-ways.

---

## Arc retrospective (Phases 43–47 landed 2026-07-03/04)

Arc D took the facility from a nine-room dev map to a generated, liminal-scale
labyrinth — and did it without ever letting the arc-C invariants slip. **The one
thing that changed the game:** the default course is now `liminal_wfc_v1`, ~30 rooms
of WFC-generated topology with a protected objective spine, role-typed rooms and
corridors, and liminal-scaled dimensions. Everything the earlier arcs built —
contested observation, door identity, the gantry, collapse, rival legibility — now
plays out on generated ground.

**What held the line:** the spine was treated as a first-class generated output (not
an afterthought), so progress/collapse/absorption/solvability never broke at scale;
the headless==interactive characterization test and a new full-match seed corpus were
re-proven *on generated maps* before the default flipped; generation is memoized so
the ~150-Match test suite still runs in ~5 seconds; and the WFC dependency stayed
behind a feature flag with an authored fallback per the R11 bar. Two shipped features
that the scale/gantry work exposed were repaired first (D1): monitors now render real
room miniatures via the shared preview technique, and spectator bots visibly pilot the
gantry's jump line.

**Deferred, deliberately:** `LocalAction::PlaceAnchor` (first-person anchors into the
lockstep race — a wire-protocol change, the recorded next mechanical step); a third
hall endpoint so the gantry understory reaches a different neighbour; the decoherence
counter-tool (never triggered). **Horizon:** human multiplayer over the proven
lockstep spine.

---

## Horizon (explicitly after the arc)

- **Human multiplayer.** The lockstep spine is proven; humans slot in when Arcs C–D
  make the race large, legible, and worth racing. Not before.
- **Counter-observation tools** (decoherence charge, observation jamming) — only if
  Phase 38's lab shows denial dominance.
- **Second traversal verb** (grapple sockets) — after Gantry proves the corridor beat.

## Non-goals (unchanged from agents.md, restated for this arc)

No combat or direct harm, ever — contention stays informational and spatial. No new
art-asset dependencies (code-as-art holds). No changes to the deterministic brain/transport
contract — every new rule must survive the replay and lockstep tests unchanged. No
procedural mesh geometry beyond authored primitives and WFC spatial layout.
