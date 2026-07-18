# Arc L — Hex Tile Facility: implementation plans

One hand-off document per phase (87–95), each self-contained enough to give a
fresh implementation agent. Arc design and staging:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md). Phase 86 (closeout/baseline)
was parent-executed and has no separate brief.

## Hand-off model

```text
Wave 0: [86]         (Arc K closeout, clean committed baseline — parent, done)
Wave 1: [87]         (observed_hex foundation crate — everything depends on it)
Wave 2: [88 ∥ 89]    (2D hex WFC solver + animated step lab ∥ TrenchBroom tile pipeline + tile lab)
Wave 3: [90 ∥ 91]    (solver verticality + room blueprints ∥ tile library authoring, done)
Wave 4: [92 ∥ 93]    (3D hex facility lab ∥ relayout/decoherence parity, done)
Wave 5: [94]         (match-layer parity: bots, routing, thresholds)
Wave 6: [95]         (game integration; hex replaces square; playtest gate)
```

- The **parent session** dispatches one agent per phase with the phase doc as the
  brief, reviews against the success criteria, runs the full verification itself,
  and commits — **agents do not commit**; concurrent agents use isolated worktrees.
- **Every phase lands as its own commit** before the next wave dispatches.
- Shared-file rules for concurrent phases:
  - Wave 2: 88 owns `crates/observed_facility/src/hex_wfc/` + `labs/hex_wfc_lab/`;
    89 owns `crates/observed_authoring/` + `labs/hex_tile_lab/` + `assets/tiles/`.
    Both may append (never edit) `crates/observed_hex/` tests; neither edits the
    other's tree. Workspace `Cargo.toml` member additions are resolved by the
    parent at commit time.
  - Wave 3: 90 owns solver code; 91 owns `assets/tiles/**` + validator additions
    in `observed_authoring`. The manifest schema is frozen at the end of Wave 2 —
    schema changes require the parent to serialize the phases.
  - Wave 4: 92 owns `crates/observed_match/src/hex_wfc/` + lab 3D modules; 93 owns
    `hex_wfc/{relayout,topology}.rs` + lab 2D relayout mode; the shared
    `hex_wfc/tests.rs` is append-only.
- **The square lattice is frozen until Phase 95:** no phase edits
  `observed_facility::full_wfc`, `observed_match::full_wfc`, or
  `game/src/full_wfc/`.

## Standing rules (carried from Arcs H–K, binding)

The **falsifiable-evidence rule** applies verbatim: agents run their lab with
`OBSERVED2_CAPTURE`, view their own captures, and describe them; the parent
rejects phases whose evidence doesn't visibly show the claim. The Legibility
Contract and `observed_style` ownership of player-facing color are inviolable.
Architecture ratchets (sim never imports presentation, no glob re-exports, files
under 600 lines), determinism hygiene (SplitMix keyed streams, headless ==
interactive), lab reset discipline (tag + dirty-flag rebuild, no leaked
entities/resources), and regression gates all hold.

Arc L additions:

1. **One source of truth for hex metrics.** Solver, geometry projector, importer,
   and the TrenchBroom template all read `observed_hex` constants; no local
   copies of corner/pitch/height numbers.
2. **Exact-snap authoring.** Tile footprints must match the canonical quantized
   corners exactly; the importer hard-fails otherwise. Author from the locked
   template map (`assets/tiles/template.map`, shipped in Phase 89).
3. **Manifest coverage is a gate.** From Phase 90 on, the coverage validator
   (every demandable port signature has ≥1 tile) must be green at every commit.

## Verification recipe (every phase, zero warnings)

```
cargo fmt --all
cargo dev-clippy                              # dynamic Bevy linking; zero warnings
cargo dev-test                                # dynamic Bevy linking; all green
```

## Phase index

| Phase | Doc | Wave | Notes |
| --- | --- | --- | --- |
| 86 — Arc K Closeout & Clean Baseline | (parent-executed) | 0 | carries Phase 85 playtest into 95 |
| 87 — observed_hex Foundation | [phase_87_observed_hex_foundation.md](phase_87_observed_hex_foundation.md) | 1 | blocks everything |
| 88 — 2D Hex WFC Solver + Animated Step Lab | [phase_88_hex_wfc_2d_lab.md](phase_88_hex_wfc_2d_lab.md) | 2 | ∥ 89 |
| 89 — TrenchBroom Tile Pipeline + Tile Lab | [phase_89_tile_pipeline_lab.md](phase_89_tile_pipeline_lab.md) | 2 | ∥ 88; fires the A9 adopt-when trigger |
| 90 — Solver Verticality & Room Blueprints | [phase_90_verticality_blueprints.md](phase_90_verticality_blueprints.md) | 3 | complete; ∥ 91 |
| 91 — Tile Library Authoring | [phase_91_tile_library.md](phase_91_tile_library.md) | 3 | complete; ∥ 90; content-only |
| 92 — 3D Hex Facility Lab | [phase_92_hex_facility_3d.md](phase_92_hex_facility_3d.md) | 4 | complete; ∥ 93; collider budget measured |
| 93 — Relayout / Decoherence Parity | [phase_93_relayout_parity.md](phase_93_relayout_parity.md) | 4 | complete; ∥ 92; production yield measured |
| 94 — Match-Layer Parity | [phase_94_match_parity.md](phase_94_match_parity.md) | 5 | serial |
| 95 — Game Integration & Square Demotion | [phase_95_game_integration.md](phase_95_game_integration.md) | 6 | serial; user playtest gate |
