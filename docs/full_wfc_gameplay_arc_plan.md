# Arc K — Full-WFC Gameplay Lock

**Status:** implementation complete; hands-on user playtest pending.  
**Baseline:** `experiment/full-wfc-facility`.  
**Goal:** make the continuous full-WFC facility the canonical, feature-complete local
match and leave networking transport as the next arc.

## Locked rulings

- Full-WFC is one continuous physical lattice. The isolated portal-place match becomes
  a hidden regression fixture.
- Normal matches contain every gameplay-required room role. The eight room templates
  and ten traversal archetypes are weighted catalogs; a dedicated fixture proves all
  of them in one generated facility.
- Corridors may have two to four exits. Puzzles remain in rooms; corridor branches are
  traversal/risk choices.
- Default play is four teams of two: one human plus an AI teammate against three bot
  teams. All eight actors consume the same deterministic input boundary.
- A team escapes only after collecting two physical single-pickup keystones, completing
  the two-operator station, and getting both members through the exit. Eight keystones
  exist so every team can finish, and a team carrying two cannot hoard more.
- Landmarks, occupancies, and deployed equipment pin their module. Generic unseen
  facility fabric may refactor.
- A pad pins only its cell geometry. An anchor freezes exact nearby threshold
  attachments. Only an anchor changes frame indicator lights.
- Guardian catches send the caught player to the eligible room with the greatest live
  weighted-A* cost to the exit.
- The tac-map is a survivor sketch: traversed history persists, glimpses are hollow,
  post-mutation routes become uncertain until revisited, and anchor truth remains
  knowledge-scoped.
- Every gameplay event has both an audio cue and a semantic visual treatment.

## Landing sequence

1. **Phase 78 — Decomposition and ratchets `[done]`.** Split production files over 1,000 lines,
   keep new/touched files under 600 lines, remove new complexity suppressions, and
   preserve the experimental digest while splitting the solver and renderer.
2. **Phase 79 — Stable catalog `[done]`.** Project collapsed cells into stable room/corridor
   instances, guarantee role quotas, support two-to-four-ended corridors, retain IDs
   through safe relayouts, and make relayout solving incremental and cancellable.
3. **Phase 80 — Continuous production traversal `[done]`.** Generate render, Rapier, nav, and
   interaction geometry from one snapshot; run every player through the deterministic
   Rapier KCC; stream presentation chunks without despawning logical state.
4. **Phase 81 — Complete local match `[done]`.** Add the four-by-two roster, team objectives,
   physical keystone pool, dual station, standings, collapse, countdown, bots, results,
   deterministic input frames, snapshots, and replay digest.
5. **Phase 82 — Tools and mechanisms `[done]`.** Promote anchors and pads into stable,
   team-keyed simulation equipment and port every role-owned interaction.
6. **Phase 83 — Guardian and mutation feedback `[done]`.** Move Guardian decisions into the
   pure match, add farthest-room catches and shared vision pressure, and replace fixed
   pulses with pressure-driven warned relayout windows.
7. **Phase 84 — Tac-map and feedback coverage `[done]`.** Ship the multi-level survivor sketch,
   split the audio director, and enforce audio+visual coverage for every gameplay event.
8. **Phase 85 — Promotion and release gate `[playtest]`.** Make full-WFC the Play flow, hide the
   legacy match, version replay input, prove headless/interactive parity, refresh
   evidence, and produce the multiplayer handoff.

## Hard acceptance gates

- Stable domain IDs only; no Bevy `Entity` is durable state.
- Simulation never imports presentation. Hardware adapters emit `PlayerIntent` plus
  abstract action buttons.
- Every relayout preserves all occupied-player and remaining-objective routes and
  changes no observed, occupied, landmark, anchored, or equipment-pinned geometry.
- Every room template and traversal archetype has render/collision agreement and
  two-way traversal coverage, including multi-exit and vertical cases.
- Same seed plus input frames yields the same canonical digest headlessly and in Bevy.
- The 100-seed × 50-relayout corpus, full workspace tests, Clippy, formatting,
  lifecycle leak checks, visual audit, evidence review, and user playtest all pass.

## Implementation record — 2026-07-14

- Full-WFC is the `Play` and `Rematch` flow. The isolated-place match is reachable
  only through legacy regression code.
- The facility projects stable `RoomId`/`CorridorId` catalogs, guarantees the
  multiplayer role quotas, supports two-to-four-ended halls, and includes a complete
  fixture covering all eight room templates and ten traversal archetypes.
- Relayout work advances one deterministic attempt per fixed tick during its warning
  window. Exhausted topological solves fall back to one unseen architecture mutation
  with identical openings; latest observation, occupancy, landmark, anchor, pad, and
  route checks still gate the commit.
- One geometry snapshot owns render pieces and Rapier colliders. Presentation streams
  nearby cells by visibility without despawning logical state.
- The authoritative match is four teams of two with eight physical keystones, shared
  team inventory caps, dual-operation, anchors, paired team pads, Guardian control and
  weighted-farthest catch setbacks, bots, countdown/results, versioned input/snapshot
  digests, and a versioned eight-actor tactical replay.
- The survivor sketch records traversed/glimpsed/stale/anchored knowledge per team and
  level. All event variants have an exhaustive semantic visual and audio cue; Guardian
  pressure drives Geiger cadence, light drain, flicker, fog distance, and mutation
  breathing/cut feedback.
- Automated gates passed: focused facility/match/game tests, strict Clippy, the full
  workspace including all labs and doctests, the 100-seed × 50-pulse mutation corpus,
  and the 36,000-tick autonomous match soak.
- Visual evidence: [gameplay](evidence/full_wfc/full_wfc_gameplay_arc.png) and
  [survivor tac-map](evidence/full_wfc/full_wfc_tacmap.png).
- Remaining gate: hands-on user playtest and tuning. Online/LAN transport remains the
  next arc by design.

## Out of scope

LAN/online transport, discovery, prediction, reconciliation, dedicated-server
deployment, matchmaking, and network protocol migration. This arc must leave those as
adapters around the stable input-frame/snapshot boundary rather than gameplay rewrites.
