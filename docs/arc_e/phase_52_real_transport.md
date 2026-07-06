# Phase 52 — Real Transport (loopback → LAN)

**Objective:** a real socket transport that carries the proven lockstep, byte-identical
to the in-process path — proven on loopback (two processes, one machine) first, then
across the LAN. Read [README.md](README.md) first. **Depends on Phase 51.**

## Read first

- `crates/observed_net/src/network.rs` — the transport seam is **already byte-oriented**:
  `PeerSession::status_packet() -> bytes`, `receive(&[u8])`, `commit_ready_frames`,
  `outbox_len`, `remote_hash`. The simulated harness moves these packets between peers
  in-process; a real transport moves the *same bytes* over a socket. `NetworkProfile`
  (Hostile/Clean) models drop/jitter — the lockstep already does its own loss repair.
- `crates/observed_net/src/netmatch.rs` — how `LiveNetMatch`/`NetMatch` drive peer
  sessions and pump frames; `labs/{network_lab, net_match_lab}` — the transport labs.
- `game/src/sim/director.rs` + `game/src/flow.rs` — where the game pumps `LiveNetMatch`
  (`director.live`), so the game can select the transport at runtime.

## Design rulings (already decided)

- The real transport is an **adapter behind the existing `PeerSession` byte interface** —
  it shuttles `status_packet()` output to a socket and socket bytes into `receive()`. The
  deterministic core and the simulated transport are **untouched** and stay the default
  for every test (invariant 1).
- **Loopback first, then LAN.** A run over loopback must converge to the *same
  deterministic final state* the in-process transport produces for the same seed. That
  equivalence is the automated acceptance test.
- **R11 dependency bar:** prefer `std::net` (no new dependency); a crate (e.g. an async
  runtime) only if it removes real risk, behind a feature, justified in the report.
- Isolate the I/O: sockets do not belong in the deterministic simulation path. Put the
  adapter behind a feature (`net_io`) in `observed_net`, or in a thin game-side driver —
  decide from the code and keep the pure core and all existing tests unaffected.

## Transport choice (recommend + justify)

UDP fits the model (the lockstep already tolerates and repairs loss/jitter via
`NetworkProfile::Hostile`, so reliable-ordered delivery is not required and UDP avoids
head-of-line blocking); TCP is simpler if LAN reliability is preferred. Pick one, state
the tradeoff, and keep the choice swappable.

## Files you may edit

`crates/observed_net/*` (the feature-gated socket adapter + Cargo feature; the pure
default build unchanged), `labs/{network_lab, net_match_lab}/*` (a real-socket mode +
the loopback equivalence test), and — only after the crate + lab are green —
`game/src/{sim/director.rs, flow.rs, map_catalog.rs-style selection}` for runtime
transport selection (an env var / lobby setting, mirroring `OBSERVED2_MAP`). Do NOT
touch the deterministic frame/hash logic.

## Implementation

1. **Socket adapter** (feature `net_io`, `std::net`): a driver that, each pump, drains a
   `PeerSession`'s outbox to the socket and feeds received datagrams/segments into
   `receive()`. Frame the packets (length-prefix for TCP; datagram = packet for UDP).
   Handle connect/accept for a two-peer session (host listens, client dials).
2. **Loopback equivalence test** (`net_match_lab`, behind the feature so the default suite
   is unaffected): run the same seeded match over two localhost processes/sockets and over
   the in-process transport; assert identical final `state_hash`/result. This is the
   automated proof that real bytes == simulated bytes.
3. **Hostile-condition coverage stays:** the drop/jitter/reconnect tests continue to run
   against the *simulated* transport (they model conditions deterministically; the real
   socket path is I/O, not modelled loss).
4. **Runtime selection in the game:** choose transport at launch (default stays the
   in-process/simulated path so nothing regresses; a flag selects real sockets). The game
   still renders each peer's own first-person view (no split-screen).

## Tests

- `real_transport_loopback_matches_the_in_process_result` (feature-gated) — two localhost
  endpoints reach the same deterministic result as the in-process run of the same seed.
- Packet framing round-trips (partial reads / coalesced datagrams handled).
- The default (non-feature) build and its full test suite are byte-identical to before —
  the adapter is invisible unless enabled.

## Verification

Per README, plus `cargo clippy -p observed_net --features net_io --all-targets` and
`cargo test -p observed_net -p net_match_lab --features net_io`. **Live-testing caveat:**
loopback is automated; a true **two-machine LAN** run must be verified by a human running
two endpoints on one network — document the exact manual steps and mark the phase
incomplete until that manual check passes. Report: the adapter design + transport choice,
loopback equivalence numbers, any dependency + its R11 justification, the manual LAN test
procedure, verification results.
