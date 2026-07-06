# Phase 53 — LAN Lobby & the Real Race

**Objective:** two or more humans, each on their own machine, in a full contested
first-person match over the LAN — attacking each other's reality at last. Read
[README.md](README.md) first. **Depends on Phases 51 (shared actions) and 52 (real
transport).** This is the arc's payoff.

## Read first

- `crates/observed_progression/src/session/*` — `Session`/`SessionLabWorld` (lobby.rs:
  `formed`, `participant`, `all_connected`, `all_ready`, `team_rating`), `connection.rs`
  (`set_ready`, `disconnect`, `reconnect`, `tick`, `finish_match`), matchmaking. All
  proven **in-process with a simulated peer** (`session_lab`) — this phase gives them a
  real peer.
- `game/src/screens/lobby.rs` (`setup_lobby`, `LobbyRuntime` holding `SessionLabWorld`) —
  the current lobby renders the formed session at spawn; it becomes the real
  connect/ready surface.
- Phase 52's socket adapter + runtime transport selection; `game/src/sim/director.rs` /
  `flow.rs` (match setup from a session + seed).
- `game/src/map_catalog.rs` — both peers must agree on map + seed (they already generate
  deterministically from a shared seed).

## Design rulings (already decided)

- LAN only: discovery + connect + reconnect on one network. **No** NAT traversal, relay,
  or online matchmaking (next arc's horizon).
- Reuse `observed_progression`'s session/lobby *logic* (ready/connect/reconnect/teams)
  unchanged — this phase gives it a real transport and real discovery, not new rules.
- Each peer renders its own first-person view of the shared match (no split-screen). Both
  peers run the identical deterministic lockstep; the shared seed fixes the map.
- Reconnect recovers a dropped peer via the existing `reconnect` + lockstep resync — a
  dropped player rejoins the same deterministic race.

## Files you may edit

`game/src/screens/lobby.rs` + a new discovery module (`game/src/lan.rs` or in
`observed_net`'s `net_io` feature — decide from the code), `game/src/screens.rs`
(lobby↔match wiring for a networked session), `game/src/flow.rs`/`sim/director.rs` (start
a match from a real session), `crates/observed_progression/*` only if a real-peer seam is
genuinely missing (prefer reusing the existing API), `game/src/tests.rs`. Keep
`session_lab` and existing progression tests green.

## Implementation

1. **LAN discovery.** A lightweight discovery over the Phase-52 socket layer (UDP
   broadcast / a tiny announce-and-listen — no external mDNS dependency unless it clears
   the R11 bar): a host advertises an open session on the LAN; clients list and pick it.
2. **Connect + lobby.** A client joins over the real transport; the lobby uses the
   existing `Session`/`connection` API (`set_ready`, `all_ready`, team assignment,
   `team_rating`) driven by real peers instead of the simulator. Host and clients agree on
   map name + seed (deterministic generation → identical facility on every machine).
3. **Launch the shared match.** On all-ready, every peer starts the same
   `MatchDirector`/`LiveNetMatch` over the real transport with the shared seed; each
   renders its own first-person view. Contested observation, anchors (Phase 51), the
   gantry, and the collapse all resolve from the shared lockstep — identically on both
   ends.
4. **Reconnect.** A dropped peer rejoins via `reconnect` + lockstep resync and continues
   the same deterministic race; the remaining peer(s) keep playing (the existing
   wait/absorb rules cover a missing runner).

## Tests (automated portion)

- `two_process_lan_match_reaches_a_consistent_result` (feature-gated, loopback) — two
  local endpoints run a full session→match and agree on the result and final state hash.
- `a_dropped_peer_reconnects_into_the_same_deterministic_race` — disconnect + reconnect
  recovers and converges (extends the progression reconnect + lockstep resync tests).
- Discovery announce/listen round-trips on loopback.
- headless==interactive and solvability invariants hold for a networked match.

## Verification

Per README, plus the feature-gated networked tests. **Live-testing caveat — this is
mandatory before "done":** the true acceptance is a **human two-machine LAN match** —
two people on one network completing a contested game, seeing each other's anchors/
freezes/presence, with a consistent result and a successful reconnect. Document the exact
manual procedure (host launch, client join, play, drop-and-rejoin) and mark the phase —
and Arc E — incomplete until that human playtest passes. Report: the discovery + connect
design, the lobby→match wiring, the automated loopback results, the manual LAN
procedure + its outcome, verification results.
