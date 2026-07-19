# Phase 99 — Caged Anchor Lantern & Guardian

**Status:** complete; automated behavior verified, visual playtest pending.

The generic personal headlamp is gone. One procedural caged lantern is now the
personal practical light and the durable anchor tool. Players collect room
caches with `E`, deploy only onto a looked-at open named threshold with `F`, and
recover their own deployed lantern with `R`. Inventory is uncapped; purposeful
anchor/recovery/Guardian rooms guarantee caches.

The physical Guardian is authoritative simulation state. Observation and
anchored geometry freeze it; otherwise it pressures the leading player.
Capture spends a recovery route when available and otherwise returns the player
to the farthest valid room, then returns the Guardian home for a readable new
pressure cycle. As a competitive hazard it stands down once only one active
runner remains, so it cannot camp the last survivor or interfere with
single-runner traversal fixtures. Guardian occupancy and deployed lantern
thresholds pin mutation.

The game projects procedural lantern and Guardian geometry, style-owned
materials, objective-proximity lantern brightness, Guardian flicker, HUD state,
semantic cues, and audio. Snapshots/replays include lantern and Guardian state.

Focused tests cover cache guarantees, collect/deploy/recover rules, observation,
anchor freezing, spawn, capture recovery, reset, and deterministic replay.
