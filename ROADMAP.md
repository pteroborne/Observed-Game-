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
