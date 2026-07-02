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
- **Details:** The identical `SplitMix(u64)` structure has been moved to `observed_core::prng` and is imported as a shared utility throughout the workspace.

### 2. Cardinal Directions representation
- **Status:** Consolidated.
- **Details:** Consolidated the overlapping compass direction representations (`Side` and `Cardinal` enums) into a single `direction` module in `observed_core`.

### 3. `PacketError` Enum
- **Status:** Consolidated.
- **Details:** Consolidated the duplicate enums from `crates/observed_net/src/netmatch.rs` and `crates/observed_net/src/protocol.rs` into a unified root `PacketError` in `crates/observed_net/src/lib.rs`.

---

## Feasibility Labs (`labs/`)

The 50 prototype labs in `labs/` are independent Bevy applications designed to isolate and test specific technical questions. They follow a strict sandbox model, allowing full reset (`R` key) without restarting. They are grouped here by testing domain:

### Foundation & Controls
- `menu_lab` & `control_lab`: Boot states, pause systems, rebind overlays, intent playback, controller assignment.
- `session_lab`: Lobby states, team assignment, remote peer simulator.

### Traversal & Physics
- `movement_lab` & `climbing_lab`: Walk, run, jump, coyote buffers, ladders, ledge-grabbing, socket-based grapple ropes.
- `fps_controller_lab` & `fps_elevation_lab`: 3D transition, dynamic AABB collision, elevation changes.

### Observation & Procedural Geometry
- `observation_lab` & `door_lab`: 2D graph transitions, unobserved doorway rewiring, door leaf slam animations.
- `constraint_lab`: Route-spine constraint validation over changing graphs.
- `fps_observation_lab`, `fps_rewire_lab` & `fps_reroute_lab`: 3D continuous visibility verification, off-camera replacement, and passage previews.
- `wfc_proc_gen_lab`, `ldtk_schematic_lab`, `room_lab` & `topology_lab`: Wave Function Collapse layout generation, LDtk tileset parser, ASCII level editor, wall alignment checks.

### Match Rules & adversarial Systems
- `competition_lab` & `director_lab`: Round standings, exit gates, collapse pressure, catch-up zones.
- `guardian_ai_lab` & `hazard_lab`: "Weeping-angel" style guardian AI pathing, two-player machinery gates.
- `replay_lab` & `match_replay`: Recording lockstep inputs, tape playback overlays, replay seek UI.

### Networking
- `network_lab` & `net_match_lab`: Simulated packet drop/jitter repair, lockstep synchronizers, live multiplayer match sessions.

---

## Assembled Game (`game/`)

The `game` package builds the final playable binary. It acts as an integration layer, composing the production crates and proven lab systems:

- **`game/src/main.rs` & `lib.rs`:** App loop initialization, Bevy configuration, screen registry.
- **`game/src/screens/`:** Concrete menu UI, Lobby status, HUD, and first-person Match Runtime orchestrators.
- **`game/src/camera.rs` & `bot.rs`:** Shared viewport math (first-person, spectator, preview) and dynamic navmesh-based bot automation for walkthrough screenshots.
- **`game/src/teleport.rs`:** Discrete room/hallway footprint building, doorway gap geometry, and crossing triggers.

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
