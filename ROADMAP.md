# Observed Game Roadmap

This document outlines the current active development goals, completed milestones, and upcoming phases for the game.

## Current Goals (North Star)
1. **Make a fun game:** Establish tension↔release rhythms between decision-making Rooms (cooperative, puzzle-solving) and hazard-filled Corridors (traversal, risk).
2. **Develop effectively with agents:** Focus on reusable modules, code-as-art neon-noir procedural aesthetics, and clear evidence-gathering pipelines.

---

## Active & Upcoming Phases

**Arc C — Contention & Depth** (plan and design rules: [docs/contention_arc_plan.md](docs/contention_arc_plan.md)). The design arc that turns the proven observe-to-freeze foundation into a game worth racing: observation becomes a shared, contested resource between teams, and the board gains the identity, verticality, and visible pressure to make that contest legible. Lab-first throughout; solvability ("no team is ever left without a path") is the arc-wide invariant.

### Phase 42 — The Race Reads as a Race `[x]`
Rivals become presences: team-colored anchor torches and frame lights on edges they froze, sound bleeding through occupied thresholds, recognizable bot-team personalities (the freezer, the sprinter, the saboteur), and tac-map attribution of rival-frozen edges ("Team 3 was here"). Human multiplayer remains the post-arc horizon — the lockstep spine is ready; the race has to be worth racing first.

---

**Arc D — Liminal Scale & Living Fixtures** (plan and design rules: [docs/liminal_arc_plan.md](docs/liminal_arc_plan.md)). The scaling arc that expands the facility from a proof-of-concept nine-room dev map into a liminal, humanoid-scale labyrinth (24–32 rooms, procedurally generated) and repairs two incomplete shipped features (observation monitors, spectator piloting). Generation is proven lab-first; the spine (start → keystones → exit) is a first-class protected output with corpus validators. All arc-C invariants (determinism, solvability under collapse/anchors) are re-proven on generated maps. Human multiplayer remains the post-arc horizon — the lockstep spine is proven; the race has to be worth racing first.

### Phase 43 — Living Fixtures `[ ]`
Fix shipped features (no map changes): role-driven monitor rooms render real previews via a shared room-preview helper, monitor sightings feed the RivalSightings ledger as read-only, guardian console lands on an interactive RoomRole::GuardianControl object, gantry entry becomes deck-level (no mount stairs), spectator bot visibly pilots gantry jumps with fall recovery, and EXIT_ROOM consumers migrate to CompetitiveFacility::exit_room().

### Phase 44 — Map-Agnostic Plumbing `[ ]`
Add selection layer and builder contract: `game::map_catalog::active_map_spec(seed)` returns the active MapSpec; `OBSERVED2_MAP` env var selects by name (default sector_relay_v1). Pure refactor; lands green.

### Phase 45 — WFC Topology In The Lab `[ ]`
Procedural generation proof: `observed_facility::wfc` (feature-gated) implements Wave Function Collapse topology generation. Extended `wfc_proc_gen_lab` emits validated MapSpecs (24–32 rooms, role coverage including 6+ Monitor rooms, protected spine). Corpus tests validate generation determinism, spine coverage, role distribution, bounded retry, and MapSpec validation across 50+ seeds.

### Phase 46 — The Liminal Flip `[ ]`
Default switch + comfort scale pass: WFC maps become default (OBSERVED2_MAP=dev reverts to sector_relay_v1); room/hall dimensions scale by role for liminal breathing; district assignment and palette variance across the bigger map; per-seed memoization for fast test iteration; characterization + solvability corpus gates re-run on generated maps and evidence refreshed.

### Phase 47 — WFC Corridor Interiors `[ ]`
DFS-maze hallways: archived `hallway_wfc.rs` ported onto WallSeg geometry; role-driven interiors (Decision corridors get mazes, Pressure corridors simpler); DFS fallback on WFC timeout; representative pinned seeds for manual review; bot routing and solvability tests pass with interior mazes.

---

## Recent Milestones (Completed)

### Phase 41 — Pressure Made Flesh `[x]`
Made the director's pressure readable in the played game:
- **Collapse in geometry:** collapse-sealed room slots now stay in the room nav as non-traversable `GapKind::Collapsed` thresholds, so claimed doorways render as rubble instead of silently disappearing.
- **Style-owned rubble:** the match asset registry builds a `SurfaceRole::Rubble` material, collapsed thresholds spawn `Collapsed rubble` leaves, and the visual audit status distinguishes `collapsed_rubble`.
- **Drained and klaxon lighting:** place palettes now derive from live collapse state; dying/collapsed places use `observed_style::drained`, and the elimination first-escape countdown drives a facility-wide `observed_style::klaxon` light state.
- **Presentation refresh:** the place render signature tracks sealed slots, collapse state, and countdown state, so pressure changes rebuild the current place without waiting for another teleport.
- **Verification:** added Phase 41 game regressions for drained/klaxon palette state and rubble threshold rendering. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Phase 40b — The Gantry Made Real `[x]`
Replaced the flat in-game placeholder with the genuine two-level jump-map hall:
- **Height-aware thresholds:** `DoorGap` carries `floor_y` and crossings are feet-height gated, so the deck-level exit is reachable only from the deck a ground body walks beneath.
- **Walkable decks:** `PlaceGeom` carries `DeckSeg` decks that `place_arena` extrudes as standable solids; the gantry projects the pure course's six platforms plus a five-step mount stair under the controller's 0.45 step limit.
- **Four real thresholds:** ground entry, deck-level upper exit to the destination, ground safe-bypass to the destination, and the understory side exit back the way you came — falling now genuinely reroutes the crossing.
- **Readable commitment:** decks render with the `GantryDeck` surface, emissive `GantryEdge` rim strips mark every jump line, and an `Understory` landing marker shows where a fall puts you before you jump; legend-backed per the Legibility Contract.
- **Verification:** 116 game tests and workspace clippy clean; the capture helper now honors a gap's true target so evidence drivers cannot misroute the side exit.

### Phase 40 — The Gantry (Jump-Map Halls) `[x]`
Finished the Phase 40 vertical corridor proof:
- **Pure traversal model:** `observed_traversal::gantry` owns the authored two-level course, upper platform thresholds, lower understory exits, deterministic route runner, and timing windows.
- **Readable style roles:** `observed_style` now includes semantic treatments for gantry decks, lit jump edges, and visible understory landings, keeping platform commitment cues legend-backed.
- **Playable lab:** new `gantry_lab` renders the jump-map hall and lets agents review clean-jump, fall-recover, and safe-bypass runs (`1`/`2`/`3`, `R` reset).
- **Game template hook:** `HallwayFlavor::Gantry` is in the assembled game's hallway library with a walkable lower-bypass projection that respects the current two-endpoint teleport contract.
- **Verification:** focused tests are green for `observed_traversal`, `observed_style`, `gantry_lab`, `style_lab`, and the game hallway/bot gantry paths.

### Phase 39 — Doors With Identity `[x]`
Finished the Phase 39 `discovery_lab` read-layer proof that typed rooms can be read before commitment:
- **Style-owned door reads:** `observed_style::DoorIdentityRole` now owns legend-backed glyphs, colors, emissions, and ambience labels for typed-room doorframes, and `style_lab` renders the new semantic row.
- **Threshold reads vs truth:** `discovery_lab` separates team-map knowledge (tile fill) from current doorframe reads (frame/glyph/bleed). Decoys advertise an exit-like `E` signal until direct visitation resolves the truth as `!`.
- **Type-true payoffs:** Sensor visits tag adjacent rooms as Sensor-fed team-local map knowledge; Decoys never count as keystones and track resolved false-exit lies.
- **Reader bot proof:** the seeded corpus asserts a scripted reader bot escapes every run and beats the random-door bot by at least `READER_BOT_TARGET_VISIT_MARGIN` mean visits over `READER_BOT_SEEDS`.
- **Verification:** focused tests are green for `observed_style`, `style_lab`, and `discovery_lab`.

### Phase 38 — Contested Observation `[x]`
Finished the Phase 38 `contention_lab` proof that observation is objective and shared while knowledge remains team-local:
- **Shared pins and private knowledge:** `ContentionWorld` keeps team-attributed presence and anchors as one shared freeze predicate, while each team records its own stale/fresh doorway ledger for fog-of-war over truth.
- **Route-denial evidence:** the headless seed-corpus race now has a deterministic Denier policy that evaluates route candidates with a one-decoherence probe, then spends its bounded anchor budget on the room that best improves the acting team's predicted path position.
- **Room rhythm in the race model:** each tick separates the room decision beat (observe/place anchors), decoherence, and traversal, so anchors affect the next graph truth before teams commit to a corridor step.
- **Verification:** Phase criteria pass over the 200-seed corpus: denial changes 159/200 outcomes (79.5%), improves team 0 mean placement from 3.270 to 2.995, produces byte-identical repeated digests, preserves solvability after every decoherence, and has no all-Denier stalemate (all 200 seeds finish, worst pinned fraction 0.778).

### Phase 37 — Game-Layer Architecture Cleanup (Refactor Arc G) `[x]`
Dissolved the assembled game's accumulated architecture debt in six verified stages (plan and as-landed record: `docs/refactor_game_arc_plan.md`):
- **Dead code & dedup:** archived the unused hallway-WFC generator into `wfc_proc_gen_lab` (dropping `ghx_proc_gen` from the game), converted the game's copy-pasted splitmix64 PRNGs to `observed_core::SplitMix`, and moved the game's spatial constants into a game-owned `layout` module.
- **Screens hub dissolved:** `screens.rs` shrank from 710 to 177 lines; shared state moved to `sim/` (simulation resources), `view/` (theme, drop-in asset registry, presentation components), with explicit imports everywhere and `arch_check` ratchet tests banning glob re-exports, non-test `use super::*`, and sim→presentation imports.
- **One match brain:** the new `MatchDirector` owns the live networked match and the elimination series behind a single tick/outcome API; headless career matches run the same director as the on-screen match, pinned by a characterization test.
- **Renderer flattened & session lifecycle:** the place renderer's strategy/policy indirection became plain functions and data; `place/mod.rs` went from 1,288 to 42 lines; every Match resource is enumerated once and a no-leak test guards `OnExit(Match)` (fixing four resources that previously leaked across matches).
- **Evidence consolidated:** both screenshot pipelines (captures and the visual audit) now live under `game/src/evidence/` behind one `configure()`, staging the brain through shared director helpers; the `OBSERVED2_*` env-var surface is unchanged.
- **Verification:** every stage landed green — 672 workspace tests, workspace clippy clean.

### Phase 36 - Map Iteration & Render-Bounds Validation `[x]`
Added a validation loop for the semantic Sector Relay map before expanding map content:
- **Pure semantic map audit:** `game::map_validation` builds teleport-place nav snapshots from `MapSpec`, then checks room bounds, doorway gaps, polygon wall splits, interior wall bounds, and representative semantic room coverage.
- **Many-iteration traversal check:** Sector Relay rooms and hallways are audited across multiple match seeds and decoherence versions; failures report map name, seed, version, place, room role, connections, bounds, gap count, and spawn.
- **Bot routing hardening:** local bot routing keeps the existing navmesh fast path and now falls back to a conservative grid route when generated labyrinth obstacles defeat the navmesh.
- **Visual evidence hook:** `OBSERVED2_CAPTURE_MAP_AUDIT=<dir>` captures representative semantic rooms for quick review without changing normal play.

### Phase 35 — Bot-Spectated Procedural Co-op Race `[x]`
Integrated the original co-op/team race goals into the AI spectator path:
- **Procedural co-op seed plan:** seeded role assignment now chooses keystone rooms, a two-operator gate, an anchor beat, a team-keyed teleport relay, a guardian pressure room, and a control room over the protected nine-room route.
- **Pure two-member teamplay brain:** `observed_match::teamplay` simulates two members per team, co-op station completion, shared keystones, anchor torch use, team-keyed teleport pads, guardian setbacks, decoherence events, and deterministic round outcomes.
- **Elimination-series bridge:** bot-spectated co-op outcomes now feed the existing series rules, so escapes, eliminations, adversary growth, and final standings come from the teamplay run instead of an unrelated autoplay timer.
- **Spectator integration:** `Spectate AI` owns the procedural co-op state, advances it while the first-person bot drives the visible body, and reports seed/plan/team metrics in the HUD and TAC-MAP.
- **Focused verification:** added pure tests for seed solvability, two-operator gating, tool use, guardian setbacks, determinism, and game integration assertions for the spectator menu path.

### Phase 34 — Spectate AI Menu Mode `[x]`
Integrated the autoplay slice with the assembled first-person match:
- **Main-menu spectator option:** added `Spectate AI`, launching directly into the 3D Match on the fixed demo seed.
- **Bot-controlled first-person body:** reused the capture bot's navmesh/threshold routing so the spectator follows a real body through the same collision, doorway, teleport, keystone, and guardian systems.
- **Manual input handoff boundary:** normal Play still uses player input; Spectate AI suppresses manual movement and keeps the cursor unlocked while the bot drives.
- **HUD feedback:** the Match HUD clearly reports when the body is AI-controlled.

### Phase 33 — AI Elimination-Series Slice `[x]`
Added the next match layer implied by the finished roadmap reflection:
- **Elimination series:** active teams race repeated keystone-route rounds until one team remains.
- **First escape countdown:** the first team through the exit starts a deterministic countdown; teams that fail to escape before survivor slots fill or the countdown expires are eliminated.
- **Adversary escalation:** eliminated teams join the facility adversary for later rounds, raising pressure in the autoplay model.
- **Fully AI playtest path:** every team can run automatically for capture/evidence, while the assembled game keeps the first-person match as a manual takeover surface.
- **Team-keyed tools:** pads and anchor state now carry team identity so future team inventories can share the same rule boundary.

### Phase 32 — ASCII Map Editor & Topology Validation `[x]`
Add structured editing and geometry validation capabilities to prepare the workspace for custom facility topologies:
- **ASCII Map Editor:** Design a simple, human-readable text representation of rooms, hallways, and portals, along with a parser that constructs the in-memory graph.
- **Topology Validators:** Add automated validation rules to ensure generated or loaded levels have no overlaps, no wall segments shorter than `MIN_WALL_LENGTH`, and all room ports align correctly.

### Phase 31 — Specialized Room Types `[x]`
Implemented specialized room types to diversify gameplay:
- **Master Room (Room 4):** 8-sided regular polygon geometry with direct one-way exits to all other rooms.
- **Tether Camera Room (Room 5):** 3x3 holographic display panels glowing cyan when the corresponding room has a player anchor torch active.
- **Guardian Observation Room (Room 6):** 3x3 warning panels flashing red when the guardian enters the corresponding room.
- **Guardian Control Room (Room 3):** Central interactive console that allows players to reassign the guardian to hunt rival teams.
- **Interior Collisions:** refactored analytical containment to check `geom.interior` so players physically collide with the Room 3 console and other interior obstacles.

### Phase 30 — Fix Bot Pathfinding (Lab & First-Person) `[x]`
Replaced ad-hoc grid pathfinding with a stable, dynamic navmesh pathfinder in both the simulation lab and the main game. Prevents the spectating bot from getting stuck on pillars.
