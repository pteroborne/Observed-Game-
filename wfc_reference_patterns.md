# WFC Reference Patterns Worth Borrowing

Concise design notes from Monoceros, Tessera, Townscaper, Unreal's WFC tooling, and model-synthesis research, evaluated against Observed's authored 3D hex-prism facility.

## Highest-value ideas

1. **Transition grammar after structural WFC**
   - Inspect shared faces, hex corners, vertical seams, room/hall boundaries, register changes, and solid/void edges.
   - Add non-authoritative trim such as railings, braces, buttresses, pipes, lintels, edge lights, and hanging structure.
   - Rebuild only the changed-cell halo after mutation.
   - Goal: hide tile seams and make the facility read as one megastructure.

2. **Contextual composition fields**
   - Multiply authored tile weights by deterministic context: level, distance from a landmark axis, district, route role, repetition, and mutation age.
   - Use tendencies rather than hard rules: towers near a central axis, gantries near void, heavy structures low, atria high, and Wellshafts clustered vertically.

3. **Generate several legal candidates, then score them**
   - Let WFC answer “is it valid?” and a deterministic evaluator answer “is it interesting?”
   - Score route redundancy, elevation change, vistas, whole-room usage, repetition, junction rhythm, landmark spacing, traversal time, and difference from the previous layout.

4. **Derived seam profiles as validation**
   - Keep explicit semantic ports authoritative.
   - Also derive collision and visual boundary signatures from authored geometry to detect mismatched cross-sections, elevations, trim lanes, and rotation seams.
   - Allow explicit adapter profiles where different-looking interfaces are intentionally compatible.

5. **Authoring distribution reports**
   - Extend `tilec audit` or the tile lab with placement frequency, unused tiles and rotations, overrepresented families, low-compatibility sockets, contradiction hotspots, whole-room fallback reasons, and collider cost.
   - Produce deterministic sample galleries so content authors can see whether new modules actually appear.

## Strong gameplay-facing ideas

6. **Expose solver uncertainty diegetically**
   - Convert mutation-pocket entropy and topology-change magnitude into structural noise, dust, threshold instability, lantern behavior, or vague TAC-map volatility.
   - Never reveal the solved future layout.

7. **Driven WFC for eliminated-team control**
   - Let eliminated teams modify bounded influence fields rather than place exact rooms.
   - Example actions: encourage verticality, dead ends, instability, Guardian approaches, decay, or reduced recovery-room likelihood.
   - WFC still guarantees routes, observation locks, anchors, and legal geometry.

8. **Global composition constraints outside the local solver**
   - Useful examples: minimum shaft spacing, landmark separation, vertical connectors per floor, junction-density limits, room-family repetition limits, and one dramatic vertical formation per district.
   - Prefer site planning or candidate scoring unless the condition is required for match validity.

## Ideas to avoid for now

- **Heterogeneous cell sizes:** they would weaken axial coordinates, atomic mutation, and stable collider identity. Whole-room modules already provide scale variation safely.
- **Unbounded runtime backtracking:** poor fit for deterministic real-time mutation. Keep bounded retries and local pockets.
- **Fully inferred adjacency:** geometry analysis should validate or suggest metadata, not silently determine gameplay topology.
- **A second authoritative decoration WFC:** transition dressing should remain derived, local, and non-colliding unless a concrete gameplay need proves otherwise.

## Suggested priority

1. Transition grammar
2. Contextual composition fields
3. Multi-candidate scoring
4. Seam-profile validation
5. Authoring distribution reports
6. Diegetic uncertainty
7. Eliminated-team influence fields

## Reference projects

- [Monoceros](https://www.monoceros.tools/) — modular 3D assembly, connectors, rotations, and multi-cell modules.
- [Tessera](https://www.boristhebrave.com/docs/tessera/6/) — big tiles, global constraints, backtracking, and authoring/debug tools.
- [Townscaper design talk](https://www.gamedeveloper.com/game-platforms/how-townscaper-works-a-story-four-games-in-the-making) — local transition rules and player-driven procedural architecture.
- [Unreal Wave Function Collapse plugin](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Plugins/WaveFunctionCollapse) — weighted, oriented 3D tile models.
- [Paul Merrell's Model Synthesis](https://paulmerrell.org/model-synthesis/) — larger-scale geometric and architectural constraint synthesis.
