# Observed Game Roadmap

This document outlines the current active development goals, completed milestones, and upcoming phases for the game.

## Current Goals (North Star)
1. **Make a fun game:** Establish tension↔release rhythms between decision-making Rooms (cooperative, puzzle-solving) and hazard-filled Corridors (traversal, risk).
2. **Develop effectively with agents:** Focus on reusable modules, code-as-art neon-noir procedural aesthetics, and clear evidence-gathering pipelines.

---

## Active & Upcoming Phases

*(No upcoming active phases currently scheduled. All target phases completed!)*

---

## Recent Milestones (Completed)

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
