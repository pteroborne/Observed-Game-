# AGENTS.md - Historical Archive

This document contains historical design context, initial goals, and development phases for **Observed 2** that have been successfully completed.

---

## Historical Current Development Goal (2D Foundation Phase)

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

---

## Initial Proposed Repository Structure

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

---

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

---

## Historical Later Gameplay Systems

The following were initially deferred and then implemented/proven during subsequent development phases:
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

---

## Historical Priorities

Begin with these independently runnable labs:
1. `menu_lab`
2. `control_lab`
3. `movement_lab`
4. `interaction_lab`
5. `room_lab`

The first integration target is `facility_sandbox`.

Higher-level game mechanics should wait until a player can reliably launch the application, move, jump, climb, interact with objects, traverse modular rooms, and return to the menu without state or entity lifecycle problems.
