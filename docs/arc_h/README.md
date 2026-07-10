# Arc H — Ground Truth: implementation plans

One hand-off document per phase (61–66), each self-contained enough to give a fresh
implementation agent. Arc design and staging:
[../ground_truth_arc_plan.md](../ground_truth_arc_plan.md). Bug provenance:
[../bug_backlog.md](../bug_backlog.md).

## Hand-off model

```text
Wave 1: [61]              (landing/bookkeeping — parent session or one agent)
Wave 2: [62 ∥ 63 ∥ 64]    (place shell/materials ∥ settings/pause ∥ teleport geometry)
Wave 3: [65 ∥ 66]         (place/monitors.rs ∥ menu/results + ship gate)
```

- The **parent session** dispatches one agent per phase with the phase doc as the
  brief, reviews against the success criteria, runs the full verification itself,
  and commits — **agents do not commit**; concurrent agents use isolated worktrees.
- **Every phase lands as its own commit** before the next wave dispatches. The
  multi-week uncommitted working tree that ended Arc F must not recur.
- Shared-file rule: `game/src/tests.rs` is append-only for concurrent phases; the
  parent resolves overlap at commit time.

## The falsifiable-evidence rule (new, binding for this arc)

Arc F's lesson: success criteria satisfiable by tests alone will be satisfied by
tests alone, and `docs/evidence/oga_25d_rivals.png` shipped showing an empty floor.
Therefore, in this arc:

1. Every phase's success criteria include **at least one capture that visibly shows
   the claimed behavior** (a rival on screen, a threshold in a wall gap, a tinted
   room). "The capture ran" is not the bar; "the capture shows it" is.
2. The **agent must look at its own capture** (read the image) before reporting
   completion, and say in its report what the image shows.
3. The parent session views every capture during review and rejects the phase if
   the evidence does not visibly demonstrate the claim.

## Standing invariants — these apply to EVERY phase

1. **The Legibility Contract is inviolable**, and `observed_style` owns every
   player-facing color/emission/contrast decision. Imported art is an input the
   style layer treats — never the other way around.
2. **The comfort language is binding:** no camera shake, no full-screen flash, no
   jump-scares; decoherence and collapse stay diegetic.
3. **The immersion ruling holds:** normal play is HUD-free (`OBSERVED2_DEBUG_HUD`
   for debug surfaces); the tac-map stays a fog-of-war survivor's sketch; the
   observation-room work must respect `MapKnowledge` consistency rules.
4. **`game/` architecture rules** (`arch_check.rs` ratchets): no glob re-exports,
   no non-test `use super::*`, `sim/` never imports `view/`/`screens/`. Fix the
   dependency direction, never the ratchet.
5. **Determinism hygiene:** seeded randomness only (`observed_core::SplitMix`),
   no `HashMap` iteration reliance, headless == interactive; the deterministic
   brain (`observed_net`, `observed_match`, `sim/director.rs` rules) is untouched
   in this arc — every phase is presentation, input, geometry projection, or
   bookkeeping.
6. **Drop-in slots with fallbacks:** deleting every imported asset leaves a
   playable, legible game.
7. **Match resource lifecycle:** any new Match-scoped resource goes in
   `for_each_match_resource!` and the no-leak test.
8. **Every bug fix lands with a regression gate** (audit rule, ratchet, or test)
   so the class of defect, not just the instance, is closed.

## Verification recipe (every phase, zero warnings)

```
cargo fmt --all
cargo clippy --workspace --all-targets       # zero warnings — resolve, don't suppress
cargo test --workspace                        # all green; note count + wall time
```

Presentation changes refresh the relevant `OBSERVED2_CAPTURE*` / `OBSERVED2_VIS_AUDIT`
evidence — and per the falsifiable-evidence rule, the capture must be *viewed*.

## After each phase (parent session)

Review captures with eyes, run verification, commit with the house style, tick the
ROADMAP `[x]` + Recent-Milestones entry, append the "As landed" note (this is not
optional — Arc F skipped five of seven), update memory ([[ground-truth-arc]]).

## Phase index

| Phase | Doc | Wave | Closes |
| --- | --- | --- | --- |
| 61 — Land Arc F (commits, ledger, evidence) | [phase_61_land_arc_f.md](phase_61_land_arc_f.md) | 1 | Arc F bookkeeping debt |
| 62 — Style Reconciliation | [phase_62_style_reconciliation.md](phase_62_style_reconciliation.md) | 2 | Bug backlog #2 |
| 63 — Control Rebind, For Real | [phase_63_control_rebind.md](phase_63_control_rebind.md) | 2 | Bug backlog #1 |
| 64 — Threshold Geometry Integrity | [phase_64_threshold_integrity.md](phase_64_threshold_integrity.md) | 2 | Bug backlog #3 |
| 65 — Observation Rooms Made Real | [phase_65_observation_rooms.md](phase_65_observation_rooms.md) | 3 | Bug backlog #4 |
| 66 — Post-Match Summary & Ship Gate | [phase_66_summary_ship_gate.md](phase_66_summary_ship_gate.md) | 3 | Arc E Phase 50 remainder + hygiene |
