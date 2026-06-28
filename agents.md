# AGENTS.md

## Project Overview

This repository contains an experimental PC game built with **Rust and Bevy**.

The long-term concept is a competitive traversal game set inside an out-of-control megastructure. Multiple teams navigate architecture whose connections can change when unobserved. Players cannot directly harm opponents, but can manipulate shared machinery, routes, equipment, and environmental systems.

This concept is not yet proven. Development must proceed through small, independently testable technical prototypes before implementing higher-level game systems.

## Project Structure Catalogue

Before selecting files to change, review [Catalogue.md](Catalogue.md) for the
current project structure. The catalogue is the authoritative overview of the
refactored workspace: promoted production crates, lab/debug projections, the
assembled `game`, docs, assets, evidence screenshots, and per-file ownership.
Use it for orientation before drilling into a specific crate, lab README, or
roadmap phase.

## North Star (added 2026-06-21)

The 2D foundation, the higher-level systems, the first-person / Hybrid-maze arcs,
and the assembled `game` are all built (see [ROADMAP.md](ROADMAP.md)). The
"Current Development Goal" section below is the *historical* 2D-foundation framing;
it is preserved for context but no longer describes the active goal. The two active
goals are:

### Goal 1 — Make a *fun* game

Fun here is a specific combination, not a vibe: **cooperative *and* competitive**
play expressed through three pillars —

* **Exploration** — the megastructure is genuinely unknown and changes when
  unobserved; discovering and re-reading it is the point. Do not pre-solve the
  player's path for them.
* **Puzzle solving** — readable, manipulable systems (observe-to-freeze, route
  cables, shared machinery) that teams reason about together.
* **Traversal** — movement itself is a challenge (climb, grapple, elevation,
  carry), not just walking a corridor.

The competitive frame is teams racing; the cooperative frame is coordinating
*within* a team (and against shared hazards) to out-traverse the others. Most of
this depth is already proven in the labs — the work is **integrating it into the
played game**, not inventing it.

#### Nodes and edges (rooms vs corridors)

Rooms (graph nodes) and corridors (graph edges) have **distinct, non-overlapping
jobs**, so play has a tension↔release rhythm instead of uniform mush:

* **Rooms = decide / observe / co-operate.** Where you choose which way to commit,
  observe (open doors to freeze a connection), operate a mechanism (seize, route
  cable, the two-operator hazard), and regroup. The comparatively safe "decision"
  beat.
* **Corridors = traverse / risk / mystery.** Where you move (elevation, the risky
  shortcut vs the safe bypass), face time-pressure danger (pressure gates, the
  encroaching collapse, a door slamming), and meet the unknown (closed doors,
  dead-ends). The committed, tense beat.

Keep them separated: **puzzles live in rooms, twitch-dangers live in corridors**
(the one hybrid that belongs in a room is the co-op coordination hazard). This is
both the most fun (a clear rhythm, a home for each pillar) and the most feasible (it
reuses the proven labs — room mechanisms from seize/equipment/hazard, corridor
danger from the pressure gate — rather than inventing systems).

**Doors are the threshold between the two, and the diegetic face of observe/
decohere:** a closed door hides the layout (mystery), opening one *observes and
freezes* that connection, and a door slamming shut means the connection went fluid
(it may reroute). A closed door is also a clean, deterministic occluder, which makes
"rewire only when unobserved" a *readable rule* rather than fragile occlusion math.
The door **mechanic** is proven in its own feasibility lab (`door_lab`) before it
folds into the game.

### Goal 2 — Develop effectively *with agents*

Lean into what an LLM agent is good at and away from what it is not.

* **Reusable, testable modules.** Keep the lab discipline: break each concept into
  a small pure module that is simple to code, understand, test, and reuse (the way
  `observed_core` and `player_input` are shared). This is already working; protect
  it.
* **Code-as-art over authored assets.** Authoring and curating textures, meshes,
  and asset packs is an agent weakness, and the drop-in-CC0 path produced an
  incoherent, illegible game. Visual identity is therefore **generated from code**:
  geometry from primitives, with **color / emission / light / fog as a deliberate
  visual language**. The chosen direction is **neon-noir** (dark facility, neon
  edges, fog and bloom, high contrast) — striking, fully procedural, and verifiable
  through the existing `OBSERVED2_CAPTURE` screenshot loop (which an agent can read
  and iterate against).
* **The Legibility Contract (a hard rule).** Atmosphere never hides information.
  Gameplay-critical signals — your path, threats, interactables, and other actors —
  must always punch through the neon-noir fog/bloom at a guaranteed brightness and
  contrast. Every on-screen state must have a documented meaning (a legend); no
  unlabeled coloured markers.

The visual language lives in **one shared, tested module** — the `style` module,
proven in `style_lab` — that maps *semantic state → visual treatment*. Presentation
code asks the module how to draw a thing; it never invents ad-hoc colours. This
turns "art" into a systems problem an agent can own.

## Current Development Goal

Build a reliable **2D technical foundation** for later gameplay experiments.

Focus first on:

* Application and menu states
* Player input and control abstraction
* Basic movement
* Running and jumping
* Climbing and traversal
* Player interactions
* Modular room definitions
* Room spawning and replacement
* Carryable and deployable objects
* Debugging and visualization tools

Do not begin implementing the full quantum maze, competitive match structure, facility director, networking, or procedural megastructure until the supporting systems have been demonstrated independently.

## Development Philosophy

### Build small test applications

Major systems should first be implemented as isolated labs or examples.

Each lab should:

* Test one primary technical question
* Launch independently
* Reset without restarting the application
* Include clear debug visualization
* Avoid dependencies on unfinished game systems
* Define observable success and failure conditions

Do not attempt to build the complete game as the first playable prototype.

### Prove systems before generalizing them

First make the smallest useful implementation work.

Only extract a generalized framework after:

1. The prototype functions correctly.
2. Its requirements are understood.
3. Its likely reuse has been demonstrated.
4. Its failure cases have been identified.

Avoid speculative abstractions.

### Prefer readable constraints over hidden complexity

Technical constraints may become game rules.

Examples:

* Use explicit climbing markers instead of detecting every climbable surface.
* Use grapple sockets instead of full rope physics.
* Use authored room ports instead of arbitrary procedural geometry.
* Use visible room-transition phases instead of instantaneous asset replacement.
* Use discrete graph connections instead of requiring continuous physical simulation.

A limitation is acceptable when it is consistent, readable, and capable of producing meaningful gameplay.

## Proposed Repository Structure

```text
/
├── AGENTS.md
├── Cargo.toml
├── assets/
├── crates/
│   ├── app_core/
│   ├── input/
│   ├── player/
│   ├── interaction/
│   ├── world/
│   ├── ui/
│   └── debug_tools/
├── labs/
│   ├── menu_lab/
│   ├── control_lab/
│   ├── movement_lab/
│   ├── climbing_lab/
│   ├── interaction_lab/
│   ├── room_lab/
│   ├── equipment_lab/
│   └── facility_sandbox/
└── tests/
```

Treat this structure as a direction, not an obligation. Do not create empty crates or modules before they are needed.

## Core Architectural Rules

### Separate input from player behavior

Gameplay systems must not read keyboard or controller input directly.

Input sources should produce an abstract intent:

```rust
pub struct PlayerIntent {
    pub movement: Vec2,
    pub look: Vec2,
    pub jump_pressed: bool,
    pub sprint_held: bool,
    pub interact_pressed: bool,
    pub climb_pressed: bool,
}
```

Character systems consume `PlayerIntent`.

This allows the same player systems to later support:

* Keyboard and mouse
* Controllers
* Bots
* Recorded inputs
* Replays
* Network clients

Do not assume the existence of only one player.

### Separate simulation from presentation

Logical state should not depend on sprites, cameras, UI entities, or rendered scenes.

Examples:

* A room may exist logically without being rendered.
* Equipment state must remain valid while its visuals are despawned.
* Player ownership must not be inferred from sprite appearance.
* Map and spectator views should read simulation state rather than reconstruct it from rendering entities.

### Use stable domain identifiers

Do not use Bevy `Entity` values as persistent game identities.

Prefer domain identifiers such as:

```rust
pub struct PlayerId(pub u16);
pub struct TeamId(pub u8);
pub struct RoomId(pub u32);
pub struct PortId(pub u32);
pub struct EquipmentId(pub u32);
```

Bevy entities may reference these IDs, but should not replace them.

### Use explicit ports and sockets

Rooms and equipment should connect through authored, typed connection points.

Potential socket types include:

* Door
* Passage
* Ladder
* Ledge
* Grapple
* Power
* Machinery
* Equipment
* Observation

Connections must be validated rather than inferred from approximate visual placement.

### Prefer data-driven room definitions

Room topology and gameplay metadata should be represented in data rather than embedded throughout spawning systems.

A room definition may eventually contain:

```rust
pub struct RoomDefinition {
    pub id: RoomDefinitionId,
    pub bounds: RoomBounds,
    pub ports: Vec<RoomPort>,
    pub surfaces: Vec<SurfaceDefinition>,
    pub traversal_points: Vec<TraversalPoint>,
    pub machinery_points: Vec<MachineryPoint>,
}
```

Begin with hand-authored templates.

Do not begin with arbitrary procedural mesh generation.

### Make multiplayer-shaped assumptions early

Networking is not an early milestone, but local systems must support multiple players.

Avoid:

* Global single-player resources
* Queries that assume exactly one player
* Hard-coded keyboard ownership
* Interactions that cannot resolve simultaneous users
* Equipment state stored exclusively on the current carrier
* Camera state mixed with player state

Before networking begins, core systems should already support four local player entities driven by human input, bots, or scripted commands.

## Technical Prototype Sequence

### 1. Project Foundation

Establish:

* Rust workspace
* Bevy application
* Logging
* Application states
* Asset organization
* Configuration loading
* Debug feature flags
* Fast prototype reset

### 2. Menu Lab

Test:

* Boot state
* Main menu
* Settings menu
* Controls screen
* Gameplay loading
* Pause menu
* Return to main menu
* Cleanup of gameplay entities and resources

Success means repeated transitions do not leak state or duplicate entities.

### 3. Control Lab

Test:

* Keyboard input
* Controller input
* Player-to-controller assignment
* Abstract `PlayerIntent`
* Input rebinding
* Loss of focus
* Input recording and playback
* Switching an entity between human and scripted control

### 4. Movement Lab

Test movement in a simple grey-box environment.

Implement incrementally:

* Walk
* Run
* Acceleration and deceleration
* Jump
* Coyote time
* Jump buffering
* Falling
* Ground detection
* Slopes
* Stairs
* Moving platforms
* Respawn after leaving bounds

Movement must be predictable and debuggable before it becomes elaborate.

### 5. Climbing Lab

Treat climbing as separate authored traversal modes.

Initial mechanics:

* Ladders
* Ledge grabbing
* Ledge hanging
* Pull-up
* Drop
* Sideways ledge movement
* Explicit climbable surfaces
* Socket-based grapple traversal

Do not implement universal surface climbing or simulated rope swinging during this phase.

### 6. Interaction Lab

Create a reusable interaction framework supporting:

* Activate
* Hold
* Pick up
* Drop
* Carry
* Operate
* Climb
* Shared interaction
* Exclusive interaction
* Interrupted interaction

Test with:

* Lever
* Door
* Carryable object
* Timed control
* Two-player control
* Equipment socket

Interaction prompts and state changes must be visible and unambiguous.

### 7. Room Lab

Create a small modular room vocabulary:

* Straight corridor
* Corner
* Junction
* Control room
* Machine chamber
* Freight room
* Shaft
* Platform room

Test:

* Loading a room definition
* Spawning and despawning
* Rotation
* Port alignment
* Connecting rooms
* Collision generation
* Debug rendering of bounds and ports
* Replacing one room with another
* Cleanup of all room-owned entities

### 8. Equipment Lab

Create a generic persistent item framework before specialized game equipment.

Test objects:

* Portable battery
* Structural jack
* Cable spool
* Deployable light
* Grapple device

Required operations:

* Spawn
* Pick up
* Carry
* Drop
* Hand to another player
* Deploy
* Connect to a socket
* Recover
* Lose power
* Persist after players leave
* React safely to room replacement

Equipment state must not exist only as a temporary player ability.

### 9. Local Team Simulation

Test four player entities in the same world.

Include:

* Independent intents
* Distinct ownership
* Shared interactions
* Item contention
* Narrow passages
* Multiple players climbing
* Multiple players using machinery
* Player separation and reunion

Bots or scripted input may control players not assigned to humans.

### 10. Facility Sandbox

Combine only the proven systems.

Initial objective:

> Move four players and one portable power source through a small modular facility.

Include:

* Main menu
* Four player entities
* Running and jumping
* One climbing mechanic
* Five or more connected rooms
* One carryable power source
* One powered door or lift
* One deployable structural tool
* One room replacement
* Basic schematic map
* Debug or spectator camera

Do not add competition or quantum graph behavior to this sandbox until the underlying systems are stable.

## Later Gameplay Systems

The following are intentionally deferred:

* Quantum room states
* Observation and decoherence
* Mutable graph constraints
* Persistent route infrastructure
* Team splitting and backtracking incentives
* Cooperative megastructure hazards
* Multiple competing teams
* Capacity-limited exits
* Eliminated teams joining the facility AI
* Facility-director controls
* Replay and spectator presentation
* Online networking
* Matchmaking
* Progression and cosmetics

When these systems are started, each should first receive its own isolated feasibility lab.

## Debugging Requirements

Invisible mechanics require visible debug representations.

Relevant labs should visualize:

* Player intent
* Velocity
* Ground contact
* Collision shapes
* Climb detection
* Interaction range
* Current interaction target
* Room bounds
* Room ownership
* Port types and alignment
* Active connections
* Moving-platform attachment
* Equipment ownership
* Equipment socket state
* Pending room transitions
* Entity counts before and after reset

Prefer a simple debug overlay over relying entirely on console output.

## Testing Expectations

For each completed system:

* Add focused unit tests for pure logic.
* Add integration tests where Bevy scheduling or entity lifecycle matters.
* Run `cargo fmt`.
* Run `cargo clippy` and address relevant warnings.
* Run `cargo test`.
* Verify the affected lab manually.
* Confirm that resetting or exiting the lab removes its entities and resources.

Do not claim completion solely because the project compiles.

When behavior is difficult to automate, document the manual test procedure.

## Dependency Policy

* Prefer the Rust standard library and Bevy’s built-in systems.
* Add third-party dependencies only when they remove substantial technical risk.
* Do not add a physics, networking, UI, serialization, or input plugin casually.
* Explain the benefit and maintenance cost before adding a major dependency.
* Treat the versions committed in `Cargo.toml` and `Cargo.lock` as authoritative.
* Do not upgrade Bevy or other major dependencies unless requested or required by the current task.

## Coding Conventions

* Use standard Rust formatting.
* Prefer clear names over terse abstractions.
* Keep systems focused on one responsibility.
* Use components for entity-local state.
* Use resources for genuinely world-level state.
* Use events or observers for meaningful state transitions.
* Avoid oversized systems that perform input, simulation, rendering, and audio together.
* Avoid premature plugin fragmentation.
* Document invariants and non-obvious safety assumptions.
* Do not suppress warnings without explaining why.
* Do not leave placeholder systems presented as completed features.

## Working Method for Codex

Before changing code:

1. Review [Catalogue.md](Catalogue.md) for the current workspace structure.
2. Read the relevant crate and lab.
3. Identify the smallest system needed for the task.
4. Check whether an existing abstraction already owns the behavior.
5. State any assumption that materially affects architecture.

While changing code:

1. Keep the change limited to the requested milestone.
2. Preserve separation between input, simulation, and presentation.
3. Add debug visibility for new invisible state.
4. Add or update tests.
5. Avoid unrelated refactoring.

After changing code:

1. Format the project.
2. Run relevant tests.
3. Run Clippy where practical.
4. Launch or validate the affected lab.
5. If `Catalogue.md` was updated for the change, commit and push the verified work.
6. Report:

   * What changed
   * What was tested
   * Known limitations
   * The next smallest technical step

## Non-Goals

Unless explicitly requested, do not:

* Build the complete game loop.
* Add online networking.
* Add matchmaking.
* Implement full rope physics.
* Implement universal climbing.
* Generate arbitrary procedural geometry.
* Build complex enemy AI.
* Add combat.
* Add progression systems.
* Add polished art pipelines.
* Refactor the entire workspace while implementing one lab.
* Create abstractions for hypothetical future requirements.

## Current Priority

Begin with these independently runnable labs:

1. `menu_lab`
2. `control_lab`
3. `movement_lab`
4. `interaction_lab`
5. `room_lab`

The first integration target is `facility_sandbox`.

Higher-level game mechanics should wait until a player can reliably launch the application, move, jump, climb, interact with objects, traverse modular rooms, and return to the menu without state or entity lifecycle problems.
