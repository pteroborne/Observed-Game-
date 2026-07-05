# Observed Game Roadmap

This document outlines the current active development goals, completed milestones, and upcoming phases for the game.

## Current Goals (North Star)
1. **Make a fun game:** Establish tension↔release rhythms between decision-making Rooms (cooperative, puzzle-solving) and hazard-filled Corridors (traversal, risk).
2. **Develop effectively with agents:** Focus on reusable modules, code-as-art neon-noir procedural aesthetics, and clear evidence-gathering pipelines.

---

## Active & Upcoming Phases

*Arc C — Contention & Depth ([docs/contention_arc_plan.md](docs/contention_arc_plan.md)) and Arc D — Liminal Scale & Living Fixtures ([docs/liminal_arc_plan.md](docs/liminal_arc_plan.md)) are both complete (Phases 38–47; see Recent Milestones). No active phase is scheduled.*

**Post-arc horizon.** The next mechanical step is **`LocalAction::PlaceAnchor`** — bringing first-person anchor placement into the shared lockstep race (a deliberate wire-protocol/replay-format change, held out of Arc C on purpose). Beyond that: **human multiplayer** over the proven lockstep spine — the transport is ready; the arcs made the race worth racing. Smaller queued follow-ups: a third hall endpoint so the gantry's understory exit reaches a genuinely different neighbour; the decoherence counter-tool (Phase 38's criterion (d) never triggered it).

---

## Recent Milestones (Completed)

### Phase 47 — WFC Corridor Interiors `[x]`
Closed Arc D by reviving the archived hallway-interior WFC generator into the game:
- **Generator home:** the hallway-interior WFC logic archived in Arc G1 now lives in `observed_facility::wfc` behind the `wfc` feature (`generate_interior_walls`/`InteriorSeg`), so `ghx_proc_gen` stays out of the game crate; `game::wfc_interior` is the pure `InteriorSeg → WallSeg` adapter that picks the same door columns the DFS maze would.
- **Role-driven, deterministic selection:** a grid hallway on a `CorridorRole::Mystery` edge (resolved via `MapSpec::corridor_role_between`, frozen into `FrozenDest.corridor_role` so preview and crossing agree) renders a WFC labyrinth; every other role and the specless dev map keep the DFS+braid maze; a WFC non-convergence falls back to DFS as a pure function of the seed.
- **Proven real, not silent fallback:** a pinned-seed test shows WFC converges with zero retries on every real hallway grid size (4×4/5×6/6×7/7×5/4×8); selection, fallback determinism, and bot routing through a WFC interior are all covered. The lab archive shrank to a re-export of the live code.
- **Verification:** 800 workspace tests, 35 `observed_facility --features wfc` tests, clippy clean with the feature on and off.

### Phase 46 — The Liminal Flip `[x]`
Completed Arc D's default-map flip and liminal comfort pass:
- **Generated maps by default:** `game::map_catalog` now defaults to `liminal_wfc_v1`; `OBSERVED2_MAP=dev` / `sector_relay_v1` keeps the old authored fixture available for regression testing.
- **Fast generated-map iteration:** map specs are memoized per `(map, seed)`, and the game-side corpus proves generated maps produce complete placed matches across seeded `MatchDirector` runs.
- **Liminal scale dials:** `game::layout` owns room and hall scale multipliers; room footprints scale by `RoomRole`, while authored non-grid/non-Gantry hallway templates stretch/widen without changing edge variation determinism.
- **Validation and evidence:** semantic map geometry audits now use a liminal renderer-frame bound, and the generated-map audit evidence in `docs/evidence/map_audit/` was refreshed after the scale pass.
- **Verification:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p observed_game`, `cargo test --workspace`, and `OBSERVED2_CAPTURE_MAP_AUDIT=docs/evidence/map_audit cargo run -p observed_game` pass.

### Phase 44 — Map-Agnostic Plumbing `[x]`
Added the map selection layer needed before generated maps enter the game:
- **Validated catalog:** `game::map_catalog` owns active map selection, normalizes `OBSERVED2_MAP`, defaults to `sector_relay_v1`, and validates every registered `MapSpec` builder before returning it.
- **Startup plumbing:** interactive match setup, headless `play_match()`, keystone defaults, debug room coercion, and map-audit capture now read the active map spec rather than directly constructing Sector Relay.
- **Role-aware geometry:** the teleport `Nav` snapshot now carries the active map room role so monitor-room footprint shaping comes from the selected `MapSpec`, not a hard-coded sector lookup.
- **Verification:** added catalog tests for default selection, aliases, and unknown-name rejection; `cargo test -p observed_game` passes.

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
