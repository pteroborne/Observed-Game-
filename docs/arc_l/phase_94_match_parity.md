# Phase 94 — Match-Layer Parity: Bots, Routing, Thresholds

**Wave 5, serial** (spans the facility/match seams; no concurrent phase). You own
`crates/observed_match/src/hex_wfc/` and the facility `hex_wfc` topology costs.
Do not touch anything under `full_wfc`. Arc context:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Make the hex facility playable by agents: routing costs that understand ramps
and shafts, bots that physically traverse them, and threshold/door semantics on
hex ports — everything Phase 95's game shell will drive.

## Source material

- `crates/observed_match/src/full_wfc/model.rs` + `model/movement.rs` — the
  match step loop, Rapier FPS bodies, the climb system (shafts keep a climb-style
  vertical traversal; **ramps are plain walking** — no climb logic on ramps).
- `crates/observed_match/src/full_wfc/bot.rs` — `bot_objective_cell` /
  `bot_command`: route via facility A*, steer to `route.cells[1]`.
- `crates/observed_facility/src/hex_wfc/topology.rs` — hex A* (Phase 90/93).

## Deliverables

- **Routing costs**: per-class travel costs (Door lateral = hall/room tier,
  RampOpen = Climb tier, ShaftOpen = Shaft tier) and the hex+level heuristic;
  routes thread ramp pairs correctly (low cell → head cell → lateral exit).
- **Movement**: `sync_player_from_body` equivalent for hex (world position →
  `HexCoord` via inverse `hex_origin`, level via `TILE_LEVEL_HEIGHT`); ramp cells
  resolve the player's logical level from the walking surface, not floor height;
  shaft climb interaction ported (jump→Up / interact→Down near shaft center).
- **Bots**: steer through hex centers, walk ramps under `step_character`, climb
  shafts; fall recovery on the taller cells (8 m drops are survivable-by-design
  or recovered — decide and document).
- **Thresholds/doors**: `ThresholdKey { room, port }` attachments drive door
  states on blueprint ports; observation frames pin thresholds as on square.
- **Spawn/exit** placement on the rhombus (hex-distance-maximal corners or
  masked-corner equivalent).

## Success criteria

1. **The headless gate**: a bot completes spawn→exit on a pinned seed whose
   route crosses ≥2 ramp levels and ≥1 shaft, deterministically (same tick
   count, twice).
2. Bot soak: N seeds × M bots, no stalls (mirror the Arc K bot-stall standard);
   any stall is a failure, not a note.
3. Route/threshold tests: every blueprint port with a Door has a two-way
   traversable threshold.
4. Movement determinism: headless == interactive digests on a scripted input
   sequence.
5. Workspace green.

## Evidence

Bot-POV capture sequence (GIF via the CLAUDE.md recipe) showing a full
spawn→exit run including a ramp ascent and a shaft transit — agent-viewed and
described. Include the deterministic tick counts in the hand-back.
