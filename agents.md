# AGENTS.md

## Project Overview

This repository contains an experimental PC game built with **Rust and Bevy**.

The long-term concept is a competitive traversal game set inside an out-of-control megastructure. Multiple teams navigate architecture whose connections can change when unobserved. Players cannot directly harm opponents, but can manipulate shared machinery, routes, equipment, and environmental systems.

## Project Structure Catalogue

Before selecting files to change, review [Catalogue.md](Catalogue.md) for the current project structure. Use it for orientation before drilling into a specific crate, lab README, or roadmap phase.

## North Star

The 2D foundation, the higher-level systems, the first-person / Hybrid-maze arcs, and the assembled `game` are all built (see [ROADMAP.md](ROADMAP.md)). The two active goals are:

### Goal 1 — Make a *fun* game

Fun here is a specific combination, not a vibe: **cooperative *and* competitive** play expressed through three pillars —

* **Exploration** — the megastructure is genuinely unknown and changes when unobserved; discovering and re-reading it is the point. Do not pre-solve the player's path for them.
* **Puzzle solving** — readable, manipulable systems (observe-to-freeze, route cables, shared machinery) that teams reason about together.
* **Traversal** — movement itself is a challenge (climb, grapple, elevation, carry), not just walking a corridor.

The competitive frame is teams racing; the cooperative frame is coordinating *within* a team (and against shared hazards) to out-traverse the others. Most of this depth is already proven in the labs — the work is **integrating it into the played game**, not inventing it.

#### Nodes and edges (rooms vs corridors)

Rooms (graph nodes) and corridors (graph edges) have **distinct, non-overlapping jobs**, so play has a tension↔release rhythm instead of uniform mush:

* **Rooms = decide / observe / co-operate.** Where you choose which threshold to commit to, hold a connection through player observation or anchor it durably, operate a mechanism (seize, route cable, the two-operator hazard), and regroup. The comparatively safe "decision" beat.
* **Corridors = traverse / risk / mystery.** Where you move (elevation, the risky shortcut vs the safe bypass), face time-pressure danger (pressure gates, the encroaching collapse, a route refactoring), and meet the unknown (changing openings, dead-ends). The committed, tense beat. Full-WFC corridors may expose two to four exits; those branches are traversal/risk choices, never room-style machinery puzzles.

Keep them separated: **puzzles live in rooms, twitch-dangers live in corridors** (the one hybrid that belongs in a room is the co-op coordination hazard).

**Always-open threshold frames are the diegetic face of observe/decohere.** The canonical game is the continuous full-WFC facility: rooms and halls occupy one stable world-space lattice and crossing a threshold is physical, not a portal teleport. A player's observation of a threshold temporarily freezes its visible connection and geometry, but does **not** change the frame's indicator light. A placed anchor freezes the connection durably, and the frame light reports that anchor lock. Only geometry that is neither player-observed, occupied, landmark-pinned, equipment-pinned, nor anchored may refactor. Teleportation is reserved for explicit gameplay actions such as team pads and Guardian setbacks. The former isolated-Place/preview match is **deprecated, sunsetted, and archived**; it remains only as a regression testing fixture for unit/integration tests and must not be referenced for new features or production systems.

### Goal 2 — Develop effectively *with agents*

Lean into what an LLM agent is good at and away from what it is not.

* **Reusable, testable modules.** Keep the lab discipline: break each concept into a small pure module that is simple to code, understand, test, and reuse (the way `observed_core` and `player_input` are shared). Protect it.
* **Code-as-art over authored assets.** Visual identity is **generated from code**: geometry from primitives, with **color / emission / light / fog as a deliberate visual language**. The chosen direction is **neon-noir** (dark facility, neon edges, fog and bloom, high contrast) — striking, fully procedural, and verifiable through the existing `OBSERVED2_CAPTURE` screenshot loop.
* **The Legibility Contract (a hard rule).** Atmosphere never hides information. Gameplay-critical signals — your path, threats, interactables, and other actors — must always punch through the neon-noir fog/bloom at a guaranteed brightness and contrast. Every on-screen state must have a documented meaning (a legend); no unlabeled coloured markers.

The visual language lives in **one shared, tested module** — the `style` module, proven in `style_lab` — that maps *semantic state → visual treatment*. Presentation code asks the module how to draw a thing; it never invents ad-hoc colours.

* **Evidence capturing with FFmpeg.** To make visual evidence in walkthroughs more interactive and easier to review, agents can use `ffmpeg` to compile sequential screenshot folders (e.g. `docs/evidence/bot_pov/bot_pov_*.png`) into a single high-quality loopable animated GIF (e.g. `docs/evidence/bot_pov/bot_pov.gif`) using a palette filter, and embed it in markdown walkthrough files.

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

### Compile-Time and Linking Speedups

To optimize build and link times during active development (especially with multiple parallel worktrees):
* **Disable dependency debug info**: Add `debug = false` to `[profile.dev.package."*"]` in `Cargo.toml`.
* **Share target directory**: Point worktrees to a central target directory using `CARGO_TARGET_DIR` or `.cargo/config.toml`:
  ```toml
  [build]
  target-dir = "O:/Observed 2/target"
  ```
* **Fast Linker (LLD)**: Configure `.cargo/config.toml` to use the Rust toolchain's LLD on Windows:
  ```toml
  [target.x86_64-pc-windows-msvc]
  linker = "rust-lld.exe"
  ```
* **Dynamic Linking**: Use the `.cargo/config.toml` development aliases (`cargo dev-run`, `cargo dev-test`, and `cargo dev-clippy`) to enable Bevy's `dynamic_linking` feature without enabling it in release builds.

## Core Architectural Rules

### Separate input from player behavior

Gameplay systems must not read keyboard or controller input directly.

Input sources should produce an abstract intent `PlayerIntent` which character systems consume. This allows the same player systems to later support controllers, bots, recorded inputs, replays, and network clients. Do not assume the existence of only one player.

### Separate simulation from presentation

Logical state should not depend on sprites, cameras, UI entities, or rendered scenes.
* A room may exist logically without being rendered.
* Equipment state must remain valid while its visuals are despawned.
* Player ownership must not be inferred from sprite appearance.
* Map and spectator views should read simulation state rather than reconstruct it from rendering entities.

### Keep the game's module flow one-way and explicit

Inside `game/`, presentation reads simulation, never the reverse: `view/` and the
screen systems may import `sim/`; `sim/` must never import `view/` or `screens/`.
State imports explicitly from its owning module — no glob re-exports
(`pub use x::*`) between modules and no `use super::*` outside `#[cfg(test)]`
modules, so every file states what it actually depends on. These rules are enforced
by the `arch_check` ratchet tests in `game/src/arch_check.rs`; if one fails, fix the
dependency direction rather than the test.

### Use stable domain identifiers

Do not use Bevy `Entity` values as persistent game identities.
Prefer domain identifiers such as `PlayerId(pub u16)`, `TeamId(pub u8)`, `RoomId(pub u32)`, `PortId(pub u32)`, `EquipmentId(pub u32)`. Bevy entities may reference these IDs, but should not replace them.

### Use explicit ports and sockets

Rooms and equipment should connect through authored, typed connection points.
Connections must be validated rather than inferred from approximate visual placement.

### Prefer data-driven room definitions

Room topology and gameplay metadata should be represented in data rather than embedded throughout spawning systems. Begin with hand-authored templates. Do not begin with arbitrary procedural mesh generation.

### Make multiplayer-shaped assumptions early

Networking is not an early milestone, but local systems must support multiple players. Avoid global single-player resources, queries assuming one player, hard-coded keyboard ownership, or camera state mixed with player state.

## Debugging Requirements

Invisible mechanics require visible debug representations.
Relevant labs should visualize:
* Player intent, velocity, ground contact, collision shapes
* Climb detection, interaction range, current interaction target
* Room bounds, room ownership, port types and alignment, active connections
* Moving-platform attachment, equipment ownership, equipment socket state
* Pending room transitions, entity counts before and after reset

Prefer a simple debug overlay over relying entirely on console output.

## Testing Expectations

For each completed system:
* Add focused unit tests for pure logic.
* Add integration tests where Bevy scheduling or entity lifecycle matters.
* Run `cargo fmt`, `cargo clippy`, and `cargo test`.
* Verify the affected lab manually, confirming that resetting or exiting the lab removes its entities and resources.

## Dependency Policy

* Prefer the Rust standard library and Bevy’s built-in systems.
* Add third-party dependencies only when they remove substantial technical risk.
* Explain the benefit and maintenance cost before adding a major dependency.
* Treat the versions committed in `Cargo.toml` and `Cargo.lock` as authoritative.

## Coding Conventions

* Use standard Rust formatting.
* Prefer clear names over terse abstractions.
* Keep systems focused on one responsibility.
* Use components for entity-local state, resources for world-level state, and events or observers for transitions.
* Avoid oversized systems that perform input, simulation, rendering, and audio together.
* Document invariants and non-obvious safety assumptions.
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
2. Run relevant tests and Clippy.
3. Launch or validate the affected lab.
4. If `Catalogue.md` was updated for the change, commit and push the verified work.
5. Report changes, testing, limitations, and the next step.

## Non-Goals

Unless explicitly requested, do not:
* Build the complete game loop, online networking, matchmaking, progression systems, or complex enemy AI.
* Implement full rope physics or universal climbing.
* Generate arbitrary procedural geometry.
* Refactor the entire workspace while implementing one lab.
* Create abstractions for hypothetical future requirements.
