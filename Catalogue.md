# Project Catalogue

This catalogue provides an architectural map of the **Observed 2** workspace. It outlines the modular structure of the project, details the responsibilities of promoted production crates, groups the feasibility labs, maps the assembled game package, documents duplicate code patterns, and identifies bloated source files targeted for future modularization.

---

## Workspace Structure Overview

The project is structured into three main areas to enforce clean boundaries between pure simulation logic, isolated feasibility testing, and game assembly:

```text
/
├── Cargo.toml                # Workspace configuration listing resolver 3 members
├── agents.md                 # Project north-star goals, core architectural rules, and coding conventions
├── CLAUDE.md                 # Streamlined runbook: commands, verification, and evidence pipeline
├── ROADMAP.md                # Narrative timeline tracking completed milestones and recent phase details
├── crates/                   # Promoted production crates containing pure, deterministic simulation logic
├── labs/                     # Independently runnable feasibility prototypes and visual test showcases
├── game/                     # The assembled first-person 3D player-facing game package
├── assets/                   # Procedural fallback markers & drop-in models/sounds (observed_assets-governed)
└── docs/                     # Visual evidence (screenshots, GIFs), design plans, and audit evaluations
```

---

## Production Crates (`crates/`)

These crates represent the stable core of the game's simulation layer. They contain **no rendering or engine presentation code** and must remain fully deterministic, portable, and unit-testable. Each crate is fully documented in its local `README.md`:

1. **`player_input`** — [README](crates/player_input/README.md)
   - *Purpose:* Defines abstract `PlayerIntent` and `PlayerId` boundaries. Decouples physical input hardware (keyboard/mouse, controllers, network inputs, replay tapes, or bots) from character behavior.
2. **`observed_core`** — [README](crates/observed_core/README.md)
   - *Purpose:* Domain identifiers and basic structural helpers (`RoomId`, `PortId`, `EquipmentId`, `TeamId`, `Side`, `ThresholdSlotId`).
3. **`observed_doors`** — [README](crates/observed_doors/README.md)
   - *Purpose:* Pure logic for doors acting as observation gates. Closed doors hide and free connections; open/observed doors freeze connectivity.
4. **`observed_facility`** — [README](crates/observed_facility/README.md)
   - *Purpose:* Facility topology rules: authored room definition templates, transform alignments, port connectivity, and overlapping geometry validation.
5. **`observed_interaction`** — [README](crates/observed_interaction/README.md)
   - *Purpose:* Persistent equipment systems (batteries, structural jacks, light spools) and a deterministic tick-based interaction engine resolving holding, activating, quorum, interruptions, and item contention.
6. **`observed_match`** — [README](crates/observed_match/README.md)
   - *Purpose:* The competitive match brain, containing:
     - `competition`: team standings, race metrics, capacity-limited exit gates.
     - `director`: AI director pressure models, collapse scaling, and catch-up mechanisms.
     - `elimination`: multi-round elimination-series state, first-escape countdowns, adversary escalation, and team-keyed tool ownership.
     - `teamplay`: seeded two-member bot teamplay, co-op room beats, tool usage, guardian setbacks, and round outcomes for spectator-driven series play.
     - `maze`: seeded spatial labyrinth generator translating graphs into walkable corridor geometry.
     - `hybrid`: deterministic orchestration of the first-person hybrid round-stepping and matching.
7. **`observed_net`** — [README](crates/observed_net/README.md)
   - *Purpose:* Hostile transport repair, wire protocol checksum verification, and deterministic lockstep simulation state serialization.
8. **`observed_observation`** — [README](crates/observed_observation/README.md)
   - *Purpose:* The underlying graph database tracking visibility state. Pinned observed rooms are frozen, while unobserved paths decohere and rewire.
9. **`observed_progression`** — [README](crates/observed_progression/README.md)
   - *Purpose:* Cosmetic profile unlocks, matchmaking queue status, lobby formation, reconnect logic, and session lifecycles.
10. **`observed_style`** — [README](crates/observed_style/README.md)
    - *Purpose:* The semantic visual design system (neon-noir district palettes, emissive intensities, signaling tiers, Outline overlay rules, and accessibility legend mappings).
11. **`observed_traversal`** — [README](crates/observed_traversal/README.md)
    - *Purpose:* Fixed-timestep physics simulation containing AABB collisions, climb/ladder states, slope mechanics, and stairs.
12. **`observed_assets`** — [README](crates/observed_assets/README.md)
    - *Purpose:* Local directory asset-slot index maps, avoiding hardcoded string paths in simulation presentation.
13. **`observed_diagnostics`** — [README](crates/observed_diagnostics/README.md)
    - *Purpose:* Pure visual-audit schemas and checks for converting rendered-game state into agent-readable validation evidence.

---

## Code Duplications & Design Overlaps

The following duplication patterns have been identified in the codebase. These should be unified or centralize in the future to keep the code DRY (Don't Repeat Yourself):

### 1. `SplitMix` Pseudo-Random Number Generator (PRNG)
- **Status:** Centralized.
- **Details:** The identical `SplitMix(u64)` structure lives in `observed_core::prng` and is imported as a shared utility throughout the production crates. The game layer had regressed with two re-duplicated stream copies (`game/src/maze.rs`, `game/src/guardian.rs`); Refactor Arc G1 converted both onto the shared `SplitMix`, with seeded determinism tests pinning the streams bit-identical before and after.
- **Not duplicates (do not "unify"):** `game/src/hallway.rs`, `game/src/teleport/geom.rs`, and `observed_style::district_for` each contain a keyed, one-shot splitmix64 **hash finalizer** (seed/key mixed once, no stateful `next_u64` advance) used to derive a stable value from a room/edge key. These have different mixing and state-advance semantics from the streaming `SplitMix` generator by design; rewriting them onto `SplitMix::next_u64()` would shift their deterministic outputs (hallway layouts, room shapes, district palettes).

### 2. Cardinal Directions representation
- **Status:** Consolidated.
- **Details:** Consolidated the overlapping compass direction representations (`Side` and `Cardinal` enums) into a single `direction` module in `observed_core`.

### 3. `PacketError` Enum
- **Status:** Consolidated.
- **Details:** Consolidated the duplicate enums from `crates/observed_net/src/netmatch.rs` and `crates/observed_net/src/protocol.rs` into a unified root `PacketError` in `crates/observed_net/src/lib.rs`.

---

## Feasibility Labs (`labs/`)

The 54 prototype labs in `labs/` are independent Bevy applications designed to isolate and test specific technical questions. They follow a strict sandbox model, allowing full reset (`R` key) without restarting. They are grouped here by testing domain:

### Foundation & Controls
- `menu_lab` & `control_lab`: Boot states, pause systems, rebind overlays, intent playback, controller assignment.
- `session_lab`: Lobby states, team assignment, remote peer simulator.

### Traversal & Physics
- `movement_lab` & `climbing_lab`: Walk, run, jump, coyote buffers, ladders, ledge-grabbing, socket-based grapple ropes.
- `fps_controller_lab`, `fps_elevation_lab` & `gantry_lab`: 3D transition, dynamic AABB collision, elevation changes, and two-level jump-map hallway timing.
- `wellshaft_lab`: Production-controller prototype for a multi-threshold vertical silo, proving a hexagonal center pillar, six radial landing bridges, visible/collidable spiral stair treads, reset stability, and staged top/bottom/plan evidence.
- `rapier_determinism_lab`: Isolated feasibility spike (the "only if the custom controller proves insufficient" escape hatch) answering whether **rapier3d 0.34** can step convex + smooth (ball/capsule/convex-hull) colliders in fixed-dt lockstep with `enhanced-determinism` and stay bit-for-bit reproducible. Raw rapier (not `bevy_rapier`) owns the step loop; two identical worlds run side by side with a live hash-divergence monitor plus an offline bitwise-replay unit test. Verdict: bit-identical, no promotion into the game.
- `rapier_authoring_lab`: Combined TrenchBroom/Rapier vertical slice. A typed, editable Quake `.map` projects into stable room/port/door semantics and one static Rapier convex hull per brush; a fixed-step kinematic capsule traverses the exact sloped ramp, reacts to model-owned door collider mutation, resets cleanly, and replays scripted intents bit-for-bit. This remains lab-local pending playtest and production-boundary review.

### Observation & Procedural Geometry
- `observation_lab` & `door_lab`: 2D graph transitions, unobserved doorway rewiring, door leaf slam animations.
- `constraint_lab`: Route-spine constraint validation over changing graphs.
- `fps_observation_lab`, `fps_rewire_lab` & `fps_reroute_lab`: 3D continuous visibility verification, off-camera replacement, and passage previews.
- `wfc_proc_gen_lab`, `ldtk_schematic_lab`, `room_lab` & `topology_lab`: Wave Function Collapse layout generation, LDtk tileset parser, ASCII level editor, wall alignment checks. `wfc_proc_gen_lab` also archives the game's former hallway-interior WFC generator (`hallway_wfc.rs`, moved from `game/src/wfc_maze.rs` in Refactor Arc G1 — never called in the shipping game, whose interior mazes are the randomized-DFS + braid generator in `game/src/maze.rs`), kept compiling with a connectivity smoke test in case WFC returns for map generation.

### Match Rules & adversarial Systems
- `competition_lab` & `director_lab`: Round standings, exit gates, collapse pressure, catch-up zones.
- `guardian_ai_lab` & `hazard_lab`: "Weeping-angel" style guardian AI pathing, two-player machinery gates.
- `replay_lab` & `match_replay`: Recording lockstep inputs, tape playback overlays, replay seek UI.

### Networking
- `network_lab` & `net_match_lab`: Simulated packet drop/jitter repair, lockstep synchronizers, live multiplayer match sessions.

### Presentation & Asset Integration
- `oga_25d_lab`: Proof surface for the 2.5D OpenGameArt intake metadata pipeline, showcasing directional actors, gameplay objects, animated decorations, and LAB texture samples with a debug metadata overlay and billboard vs directional toggle.
- `lighting_lab`: Nine static procedural dioramas isolating liminal registers (directionality, brightness, scale, repetition, fog, bloom, shadow quality) for lighting design validation and relative-luminance corridor audits.

---

## Assembled Game (`game/`)

The `game` package builds the final playable binary. It acts as an integration layer, composing the production crates and proven lab systems. Refactor Arc G (2026-07-02, see [docs/refactor_game_arc_plan.md](docs/refactor_game_arc_plan.md)) replaced the old flat `screens::*` grab-bag with an explicit-imports layout — presentation (`view`) reads simulation (`sim`), never the reverse, and no module re-exports its submodules with a glob:

- **`game/src/main.rs` & `lib.rs`:** `main.rs` is a one-line binary entrypoint; `lib.rs` owns Bevy app/plugin composition, the `GameState` state machine (Splash → Main Menu → Loadout → Lobby → Match → Results), and the top-level camera/light setup. `arch_check.rs` is a `#[cfg(test)]`-only ratchet: source-scanning tests that fail the build if a glob re-export, a non-test `use super::*`, or a `sim/` → `view`/`screens` import creeps back in.
- **`game/src/sim/`:** Simulation-side Bevy resources — no rendering, UI, or asset types. `director.rs` holds `MatchDirector`, the single owner of the live networked match plus the elimination series (`tick`, `run_to_completion`, `outcome`, spectator pumping, and forcing/suppressing scripted rounds for evidence capture). `state.rs` holds the teleport/body/intent/spectator/lobby resources (`TeleportState`, `SpectatorBot`, `MatchIntent`, `ItemIntent`, `MatchPaused`, `LobbyRuntime`, etc.). `nav.rs` is the pure brain→`Nav` projection used for bot pathing.
- **`game/src/view/`:** Presentation building blocks that read `sim` but never write it. `theme.rs` holds the menu/HUD colour palette and UI bundle helpers. `assets.rs` is the drop-in asset slot registry plus `MatchAssets::load`. `components.rs` holds presentation markers and feedback-state resources (camera/sun tags, teleport animation state, etc.).
- **`game/src/layout.rs`:** Game-owned spatial constants for the teleport place model — `PLACE_TILE`, `HALL_WIDTH`, `WALL_HEIGHT` — now sourced independently of the abandoned `observed_match::maze` tile grid.
- **`game/src/screens/`:** The state machine and screens. `screens.rs` is the menu domain (button/action types) plus the two composition plugins (`ScreensPlugin` for menu-like screens, `MatchPlugin` for the first-person match). Submodules: `menu.rs`, `loadout.rs`, `lobby.rs`, `hud.rs`, `audio.rs`, `input.rs`, and `match_runtime/` (the match's own lifecycle: `session.rs` enumerates the match-resource set once for setup/teardown and the no-leak test, `crossing.rs` is the fixed-step teleport/crossing driver, `ambience.rs` handles atmosphere/decohere feedback, `spectator.rs` drives the spectator bot, `input.rs` is match input), and `place/` (the renderer: `factory.rs`/`shell.rs`/`monitors.rs`/`animate.rs`/`lighting.rs`/`mesh.rs`/`preview.rs`/`item_visuals.rs`, recomposed by a thin `mod.rs`).
- **`game/src/evidence/`:** Every opt-in `OBSERVED2_*` pipeline, consolidated under one tree. `capture/` holds the showcase/tour/bot-POV screenshot and GIF drivers. `audit.rs` + `snapshot.rs` + `tags.rs` are the visual audit (staged inspection scenarios, world → `observed_diagnostics` collectors, and the presentation-facing marker components the audit identifies visuals by). `driver.rs` holds helpers shared by every scripted driver. All of it is a no-op in normal play.
- **`game/src/teleport/`:** Discrete room/hallway footprint building (`geom.rs`), navigation/connection tracking (`nav.rs`), crossing/transition math and spawning (`transition.rs`), and doorway gap geometry. The hallway library includes the two-level Gantry jump-map hall (height-gated thresholds, walkable `DeckSeg` decks, understory rerouting) and the WFC-selected six-level Wellshaft (hex pillar, radial bridges, bidirectional spiral treads, two live graph thresholds, four sealed service bays).
- **`game/src/map_catalog.rs` & `map_validation.rs`:** Active map selection (`OBSERVED2_MAP`, defaulting to the procedurally generated `liminal_wfc_v1`; `dev`/`sector_relay_v1` selects the authored nine-room fixture) with validated `MapSpec` builder plumbing, an in-process per-`(map, seed)` build cache (generation is expensive; the test suite enters the Match ~150 times), plus pure semantic-map geometry audits.
- **`game/src/camera.rs` & `bot.rs`:** Shared viewport math (first-person, spectator, preview) and dynamic navmesh/grid-fallback bot automation for walkthrough screenshots and the `Spectate AI` body.
- **`game/src/navmesh.rs`, `guardian.rs`, `items.rs`, `keystones.rs`, `flow.rs`, `rivals.rs`, `tacmap.rs`, `maze.rs`, `hallway.rs`:** dynamic navmesh generation from the current place geometry; the "weeping-angel" guardian AI in-match; presentation-layer droppable items (anchor torch, etc.); the keystone-gated exit inventory check; the pure career/flow model tying match → progression; presentation-only rival avatars shown when sharing a room; the toggleable tac-map overlay; the per-hallway interior maze generator (randomized-DFS + braid); and the authored teleport-hallway pieces themselves.

---

## Completed Refactorings & Modularizations

To keep code easily consumable by AI agents and developers, large singleton files have been refactored into smaller, focused modules:

### 1. `game/src/teleport.rs`
- **Status:** Completed.
- **Details:** Refactored the 2,006-line singleton file into a folder module `game/src/teleport/` split by SOLID responsibility:
  - `mod.rs`: Place/Gap types, constants, and exports.
  - `geom.rs`: Room footprint polygon builders, S-bend chicane/maze/colonnade hallway geom generation, and analytic polygon containment.
  - `nav.rs`: Navigation and connection tracking.
  - `transition.rs`: Crossing detection math, spatial 2D/3D portal alignment transforms, spawning, and Bevy FpsArena construction.
  - `test.rs`: Unit and integration test suite.

### 2. `crates/observed_match/src/hybrid.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,156-line orchestrator file into a folder module `crates/observed_match/src/hybrid/`:
  - `mod.rs`: Entrypoints, re-exports, and base types.
  - `match_state.rs`: Player positions, active places, targets, and round ticking.
  - `round_step.rs`: Simulation ticking, action application, match outcomes, and objective/escaped/escaped_count calculations.
  - `replay.rs`: Record, play, replay tapes, frame ticks, and local client action simulation.
  - `test.rs`: Competitive round matching and replay tapes unit tests.

### 3. `labs/topology_lab/src/lib.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,024-line monolithic test runner into a folder module `labs/topology_lab/src/`:
  - `lib.rs`: Entrypoints and re-exports.
  - `model.rs`: Graph nodes, hallways, links, slot IDs, and PRNG.
  - `logic.rs`: Connectivity validation, ASCII parser, and decoherence links shuffler.
  - `app.rs`: Bevy lab Feasibility prototype camera, movement simulation, text UI rendering, and quantum shuffle keyboard events.
  - `test.rs`: Connectivity tests and decoherence shuffling tests.

### 4. `crates/observed_progression/src/session.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,022-line file into a folder module `crates/observed_progression/src/session/`:
  - `mod.rs`: Entrypoints, phase state machines, time counters, and re-exports.
  - `lobby.rs`: Account, client connection status, lobby formation, team assignments, and rematch state tracking.
  - `matchmaking.rs`: Matchmaker rating calculations, queue enqueuing/dequeuing, region pairing, and ticket matching.
  - `test.rs`: Matchmaker rating tests, matchmaking queue pairing, and lobby rematch flows.

### 5. Refactor Arc G — Game-Layer Architecture Cleanup (2026-07-02)
- **Status:** Completed. Full record: [docs/refactor_game_arc_plan.md](docs/refactor_game_arc_plan.md).
- **Details:** `game/`'s flat `screens::*` god-module (710-line `screens.rs`, 8 glob-reexported submodules, `use super::*` everywhere) was dissolved into the explicit-imports `sim`/`view`/`layout`/`screens`/`evidence` layout described above (`screens.rs` alone went 710 → 177 lines); a new `game/src/arch_check.rs` ratchets the no-glob-reexport / no-super-glob / sim-never-imports-view rules as tests. `MatchDirector` (`sim/director.rs`) replaced the four parallel, loosely-correlated match models (`MatchRuntime`'s live match, the wall-clock-timer-driven `SeriesRuntime`, the spectator `TeamplayMatch` pump, and a second headless `flow::play_match()` path) with one owner and one `outcome()` resolution rule, pinned by a headless-vs-interactive characterization test. The place renderer (`screens/place/mod.rs`, 1,288 lines) was split into `factory`/`monitors`/`animate`/`shell` with `mod.rs` down to 42 lines, and its `Box<dyn SpawningStrategy>` / `GatewayPolicy` pattern bloat was flattened into plain functions and a `ThresholdStyle` data struct. The match's resource lifecycle (13+ resources hand-removed in `cleanup_match_resources`) was enumerated exactly once (`match_runtime/session.rs`'s `for_each_match_resource!`), which surfaced and fixed four resources (`Guardian`, `ActionLog`, `TeleportAnimation`, `LastTeleportPad`) that had been leaking across match exits; a no-leak test now guards the full set. The two hand-rolled evidence pipelines (`capture/` and the old `diagnostics.rs`) were consolidated under `game/src/evidence/` behind shared `MatchDirector` scenario-staging methods. The dead `game/src/wfc_maze.rs` (zero callers, the game's only reason to depend on `ghx_proc_gen`) was archived to `labs/wfc_proc_gen_lab/src/hallway_wfc.rs` rather than deleted, in case WFC returns for map generation.
