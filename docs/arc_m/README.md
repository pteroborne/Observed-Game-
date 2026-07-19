# Arc M — Breath & Flame

Arc M hardens the continuous hex facility before multiplayer scale work. Its
implementation contract is the approved seven-phase plan recorded in
`ROADMAP.md`:

1. measurement and first-frame lighting;
2. first-class TrenchBroom authoring;
3. geometry-contract enforcement and authored structural kits;
4. the caged anchor lantern and physical Guardian;
5. bounded frontier mutation;
6. incremental physics and presentation;
7. a mutation-aware active-level survivor map and hands-on gate.

## Scope rulings

- `.map` files become human-owned sources; generated catalogues are runtime
  products, not an alternate source of truth.
- Whole-room modules take precedence over cell kits for their complete
  footprint. The two are never projected together.
- Mutation is made cheap by localizing work and applying deltas. Authoritative
  commit timing never depends on wall-clock task completion.
- One procedural caged lantern provides carried objective guidance, Guardian
  warning, and deployed threshold anchoring. Inventory has no cap.
- This arc retains the current player roster. Six teams of four, eliminated-team
  adversary control, and dedicated-server transport are follow-on arcs.

## Closure gates

- TrenchBroom edit → compile → tile lab/game round-trip succeeds without Rust
  edits.
- The first visible facility frame uses the hex palette and passes the
  Legibility Contract.
- Ramps, shafts, seams, and room floors pass automated capsule/coverage tests and
  ordinary traversal cannot fall through the facility.
- Ten production-size mutation commits while moving sustain p95 ≤ 16.7 ms with
  no mutation-attributable frame above 33.3 ms.
- Lantern inventory/deploy/recover, Guardian behavior, per-cell map staleness,
  replay determinism, reset, and exit cleanup are all covered.

Phase-specific as-landed notes and evidence links are added here as each slice is
verified.

Implementation is complete through Phase 101. The final ten-commit release run
passes the complete performance gate: all-frame p95 is 8.628 ms and the worst
mutation-attributed frame is 8.563 ms. Phase 102 remains open only for the
hands-on TrenchBroom round-trip and visual/traversal/TAC-map playtest.

## Phase hand-offs

- [Phase 96 — Measurement & First-Frame Lighting](phase_96_measurement_first_frame.md)
- [Phase 97 — First-Class TrenchBroom Pipeline](phase_97_trenchbroom_pipeline.md)
- [Phase 98 — Geometry Contract & Authored Kit](phase_98_geometry_contract.md)
- [Phase 99 — Caged Anchor Lantern & Guardian](phase_99_lantern_guardian.md)
- [Phase 100 — Local Mutation Simulation](phase_100_local_mutation.md)
- [Phase 101 — Incremental Physics & Presentation](phase_101_incremental_performance.md)
- [Phase 102 — Active-Level TAC Map & Arc Gate](phase_102_tacmap_arc_gate.md)
