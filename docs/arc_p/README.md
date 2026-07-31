# Arc P — Choice Has Air

Arc P is the implementation response to the unfinished Phase 113 run-through:
the canonical hex facility was coherent on paper but still felt murky, samey,
and claustrophobic, with too little space or reason to make a conscious choice.
It changes the played `HexWfc` path only. The deprecated isolated-Place and square
full-WFC fixtures remain regression surfaces.

## Design contract

- **Rooms and open volumes are release beats.** They are brighter, carry longer
  sight lines, expose multiple thresholds, and hold readable mechanisms.
- **Corridors are commitment beats.** They keep the district's tighter fog and
  lower practical energy; traversal and mutation risk remain here.
- **Open means connected.** A percentage of `Expanse` cells is insufficient.
  Every active production level must contain a seven-cell, three-exit connected
  volume, and all walkable cells must remain within 24 route edges of a decision
  beat.
- **Choices recur.** Production uses repeated room quotas rather than one copy of
  each role. Keystone supply scales with teams and stays globally contested.
- **Mechanics are authored and typed.** Room modules declare stable sockets;
  simulation never infers a mechanism from mesh position or Bevy entity identity.
- **The objective is team-shaped.** Claim two keystones, synchronize both sides
  of one station, then regroup at the exit. A solo co-op roster may use either
  station side. Monitor surveys are optional; anchors are unchanged.
- **No colour-only language.** World mechanisms and discovered map rooms carry
  literal labels in addition to style-owned signal treatments.

## As-landed architecture

`observed_style::architecture_for_composition` is the single mapping from
district + room/hall/vertical semantics to atmosphere. Presentation uses it for
the first visible frame, live transitions, and practical energy. Settings persist
a clamped 50°–80° FOV, defaulting to 60°.

`HexRoomQuotas` is supplied only by production-scale match construction, so the
small solver fixtures retain their fast unique-role grammar. Production collapse
reinforces neighbouring expanses and calls `open_volume_failure` plus
`room_quota_failure` before accepting a result.

Strict authored rooms accept `tile_socket` entities with a stable ID, typed kind,
footprint cell, position, and yaw. `tilec` validates per-role socket cardinality,
rotates sockets with the room variant, and includes them in the simulation-content
hash. Geometry projects them into `HexRoomSocket` records.

`HexObjectiveState` owns globally available keystones and team-local monitor
surveys. `HexTeamObjectiveState` owns collection and synchronized-station
progress. Human, local bot, and server bot commands all cross the same
`HexPlayerCommand` boundary. Snapshots digest objective and revealed-role state;
the incompatible input/snapshot shape is version 5 and replay identity is
`hex_wfc_v3`.

## Phase hand-offs

- Phase 114 — Bright Decision Beats `[x]`
- Phase 115 — Open-Volume Contract `[x]`
- Phase 116 — Repeated Decision Rooms & Typed Sockets `[x]`
- Phase 117 — Deliberate Team Objective Loop `[x]`
- Phase 118 — Human Choice Gate `[ ]`

## Phase 118 checklist

- Run a normal production match at 60° FOV and inspect at least one corridor,
  authored room, vertical cell, and connected expanse.
- At an expanse, name at least three visible/traceable exits before choosing one.
- Confirm KEY, SYNC, SURVEY, ANCHOR, GUARD, RECOVER, and EXIT treatments remain
  readable through fog/bloom without relying on colour.
- Complete two keystones, a dual station, and the team exit with bots enabled;
  then repeat the station interaction with bots disabled or a second human.
- Open the survivor map before and after a monitor survey and verify only entered
  or locally surveyed room functions appear.
- Confirm a multi-hex room reads as one volume: sibling cells have fully open
  spans, named exits have threshold frames, and unnamed perimeter faces are
  solid walls.
- Launch `hex_wfc_lab` in its production corpus mode and inspect the whole-map
  atlas plus all 18 played room/hall concept vantages. Use this to distinguish
  a generation-distribution problem from a single-tile presentation problem.
- Keep Arc O Phase 113 open until its sixteen-seat real-UDP and broader human
  playtest gates are separately complete.
