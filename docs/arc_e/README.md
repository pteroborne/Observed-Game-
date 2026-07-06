# Arc E — Ready to Race: implementation plans

One hand-off document per phase (48–53), each self-contained enough to give a fresh
implementation agent. Arc design and staging: [../ready_to_race_arc_plan.md](../ready_to_race_arc_plan.md).

## Hand-off model

Phases run **sequentially with review gates**, not fire-and-forget — 52 depends on 51,
53 on 52, and each lands a reviewed commit before the next starts. Polish phases 48–50
are largely independent and *could* run in parallel if their file sets don't overlap
(they mostly don't; each doc lists its files). The parent session dispatches the agent,
reviews the returned work against the doc's success criteria, runs the full verification
itself, and commits — the agent does not commit.

**Live-testing caveat (52–53):** real sockets and a two-machine LAN match cannot be
fully verified headlessly. Those docs mark exactly which assertions are automated
(loopback, determinism, desync) and which need a human running two endpoints. Do not
mark those phases "done" on headless green alone.

## Standing invariants — these apply to EVERY phase

Restated in each doc's critical points, but binding everywhere:

1. **Determinism & lockstep are sacred.** The deterministic in-process transport and
   every seeded model stay byte-identical. `cargo test --workspace` includes the
   replay/desync/characterization gates; a divergence is a bug, never a shrug. Anything
   networked is an *adapter behind* the proven `observed_net` interface.
2. **The Legibility Contract is inviolable.** Atmosphere (audio, effects, polish) never
   dims, hides, delays, or masks a gameplay-critical signal. New coloured/audio signals
   are legend-backed; `observed_style` owns the semantic palette — presentation asks it,
   never invents ad-hoc colours.
3. **`game/` architecture rules (enforced by `game/src/arch_check.rs`):** no glob
   re-exports between modules; no `use super::*` outside `#[cfg(test)]`; `sim/` never
   imports `view/` or `screens/`. Presentation (`view`, screen systems) reads simulation
   (`sim`), never the reverse. Fix the dependency direction, never the ratchet test.
4. **Input abstraction:** gameplay reads `player_input::PlayerIntent`, never hardware
   directly; multiple players/input sources were a day-one assumption — never regress it.
5. **Match resource lifecycle:** every Match-scoped resource is enumerated once in
   `screens::match_runtime::session::for_each_match_resource!` and covered by the no-leak
   test. A new Match resource goes in that macro.
6. **Match-end invariant** (do not re-break): the match ends on the *visible run*
   (`director.live.finished()`), never on the elimination series alone — the series is
   the placement authority (`outcome()`), not the terminator. See `sim/director.rs`.
7. **Code-as-art / no new art:** audio uses the drop-in slot convention (present → used,
   absent → silent); no new authored-asset dependencies. New third-party deps clear the
   R11 bar (problem proven, smallest adapter, fallback, lab + guard test) and go behind a
   feature where reasonable.
8. **Determinism hygiene:** no `HashMap` iteration-order reliance (BTreeMap/Vec), all
   randomness through `observed_core::SplitMix` or a seeded RNG. The three keyed hash
   finalizers (`game/hallway.rs`, `teleport/geom.rs`, `observed_style::district_for`) are
   NOT PRNG duplicates — never "unify" them.

## Verification recipe (every phase, zero warnings)

```
cargo fmt --all
cargo clippy --workspace --all-targets       # zero warnings — resolve, don't suppress
cargo test --workspace                        # all green; note count + wall time
```
Feature-gated crates also get `--features <feat>` clippy+test. Presentation changes
refresh the relevant `OBSERVED2_CAPTURE*` evidence (one attempt, 3-min timeout, note if
it fails — do not iterate on captures).

## After each phase (parent session)

Commit with the house message style (Co-Authored-By trailer), tick the ROADMAP `[x]` +
add a Recent-Milestones entry, append an "As landed" note to the phase doc / arc plan,
and update memory ([[ready-to-race-arc]]).
