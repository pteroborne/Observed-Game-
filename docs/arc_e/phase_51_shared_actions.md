# Phase 51 — Shared Actions (`LocalAction::PlaceAnchor`)

**Objective:** first-person tool actions — anchor placement first — become **shared,
deterministic, replayable lockstep inputs** instead of game-local presentation. This
turns the local anchor torch into a *real shared freeze every peer computes identically*
— the change Arc C deliberately deferred, and the foundation both loopback and LAN play
need. Read [README.md](README.md) first; determinism/replay are sacred here.

## Read first

- `crates/observed_match/src/hybrid/mod.rs` — `enum LocalAction { Advance, Seize, Wait }`
  (the per-round player intent) and how it feeds `force_round`/`advance_round`.
- `crates/observed_net/src/netmatch.rs` — `local_action_to_wire`/`wire_to_local_action`
  (0/1/2 encoding), `LiveNetMatch`, `NetMatch`, how a frame's action is serialized and
  applied; and `crates/observed_net/src/network.rs` — `LockstepFrame`/`LockstepTape`/
  `LockstepWorld`/`PeerSession`, the frame-input and hashing model.
- `crates/observed_match/src/facility.rs` — `place_team_anchor`/`remove_team_anchor`/
  `pin_sources` (Phase 38b: the brain already applies team-keyed anchors; the pins are
  already globally shared). `crates/observed_observation/src/contention.rs` — the
  `Anchor` model.
- `game/src/items.rs` (`ItemsState::drop_anchor_torch` — the current **game-local** torch,
  nav/presentation only, never touches the brain), `game/src/screens/match_runtime/mod.rs`
  `item_actions` (where the torch is dropped), `game/src/sim/replay.rs` +
  `crates/observed_match/src/hybrid/replay.rs` (replay/tape).

## Design rulings (already decided)

- Anchor placement enters the **shared lockstep input stream** — every peer applies it via
  `place_team_anchor` and computes the identical frozen graph. It is a real rule now, not
  a game-local tool.
- The wire/replay format gains an explicit **version**; old tapes are handled deliberately
  (upgrade or reject with a clear error — never silently misread).
- Prove it lab-first in `net_match_lab` before the assembled game depends on it.
- The game-local torch's *presentation* (the dropped prop, the frame tint) stays; what
  changes is that dropping it now emits a shared action that the brain applies, so rivals
  see the freeze too.

## Design decision for the agent (recommend + justify in report)

`LocalAction` today is a *round* action (advance/seize/wait). Anchor placement is a
*tool* action with a location, a different kind. **Recommended:** extend the per-frame
lockstep input with an optional tool action — e.g. `ToolAction::PlaceAnchor { room }` /
`RemoveAnchor { room }`, team-keyed by the acting peer — carried alongside the round
action in `LockstepFrame`, wire-encoded, versioned. Do NOT overload the 3-value
`LocalAction` enum in a way that breaks its wire contract silently. Confirm the exact
seam from the code and document the choice.

## Files you may edit

`crates/observed_match/src/{hybrid/*, facility.rs}` (the shared action type + application
— reuse `place_team_anchor`), `crates/observed_net/src/{netmatch.rs, network.rs}` (wire
encoding + frame input + version), `labs/net_match_lab/*` (the proving lab), and — only
after the crates + lab are green — `game/src/items.rs` + `game/src/screens/match_runtime/
mod.rs` `item_actions` + `game/src/sim/replay.rs` (emit the shared action from the
first-person torch through the director/live match). Keep `contention_lab`/existing tests
green.

## Implementation

1. **Shared action type + application** in `observed_match`: the tool action, applied
   deterministically to the competitive facility via the existing `place_team_anchor`
   (idempotent, team-keyed — Phase 38b). Attribution via `pin_sources` unchanged.
2. **Wire + frame + version** in `observed_net`: encode the optional tool action in
   `LockstepFrame`'s input; add a format version constant; `wire_to_*` rejects/updates
   unknown versions with a typed error. Keep the round-action encoding backward-shaped.
3. **Prove in `net_match_lab`:** two simulated peers where one places an anchor mid-race —
   assert byte-identical `state_hash` across peers every frame, the freeze applies for
   both, replay reproduces it exactly, and desync detection still fires on injected
   divergence.
4. **Game wiring (last):** `item_actions`' torch drop emits the shared action through the
   live match instead of only mutating `ItemsState`; the presentation (prop, tint,
   sighting) now reflects a brain-applied freeze. The headless==interactive
   characterization and the no-leak/solvability tests stay green.

## Tests (the phase's proof)

- `observed_net`: shared-action frames are byte-identical across peers; replay reproduces
  a match with an anchor action exactly; version mismatch is a typed error, not a
  misread; desync detection still triggers on injected divergence.
- `observed_match`: a shared anchor action freezes the edge for every team through the
  facility's decohere path (extends the Phase-38 override/attribution tests).
- `game`: dropping the first-person torch now freezes the edge in the shared brain (a
  rival computes the same frozen graph); characterization + no-leak hold.

## Verification

Per README, plus `cargo test -p observed_net -p observed_match -p net_match_lab`. This is
the highest-determinism-risk phase — the workspace replay/desync/characterization gates
are the acceptance bar. Report: the shared-action type + wire/version design and why, the
lab proof numbers, the game-wiring seam, any replay-format migration handling,
verification results.
