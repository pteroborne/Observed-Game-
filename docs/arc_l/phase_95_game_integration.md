# Phase 95 — Game Integration & Square Demotion

**Wave 6, serial, parent-heavy, user playtest gate.** This phase ends the arc:
hex becomes the canonical game map; the square lattice is demoted to
regression-only. This is the one phase allowed to touch `game/src/full_wfc/`
(wiring only) and game-level docs. Arc context:
[../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Ship the hex facility as the Play flow with the full match experience — visual
language, lighting, audio hooks, relayout in play — and land the arc's
documentation and evidence refresh. Absorbs the **Phase 85 playtest debt**: the
Arc K hands-on gate deferred at Phase 86 is discharged here, on the hex game.

## Source material

- `game/src/full_wfc/` — the shell being paralleled: `sim.rs` (driver),
  `view/shell.rs` (StructurePiece → mesh/material via `observed_style`),
  `view/lighting.rs` (deterministic local-light budget), `view/entities.rs`,
  `input.rs`.
- The `OBSERVED2_MAP` env precedent (map override) and the legacy-match
  regression pattern from Arc K Phase 85.

## Deliverables

- `game/src/hex_wfc/`: sim driver over `observed_match::hex_wfc`, view shell
  instancing authored tile render geometry with `observed_style` materials and
  register dressings, lighting within the deterministic budget, streaming by
  visibility (mirror the full_wfc chunk streaming), relayout warnings/feedback
  wired to the existing AudioDirector + visual cue tables.
- Menu/Play flow switches to hex; Spectate AI works on hex; tac-map renders the
  hex survivor sketch (hex outlines in the sketch style).
- **Square demotion**: `full_wfc` reachable only via regression tests and an env
  override (`OBSERVED2_MAP` style); no new features; its tests stay green.
- Docs: ROADMAP arc record, `Catalogue.md` (new crates/labs/modules),
  `agents.md` if any standing rule changed, as-landed notes across
  `docs/arc_l/`, evidence refresh under `docs/evidence/hex_wfc/`.
- arch_check ratchets extended to the new modules (hex sim never imports view;
  600-line ratchet covers `hex_wfc` paths).

## Success criteria (hard gate)

1. Full workspace green (fmt/clippy/test, zero warnings); square regression
   suite still green.
2. Determinism: same seed + inputs ⇒ same digest, headless and interactive, on
   hex.
3. Full-match soak on hex (mirror the Arc K 36,000-tick autonomous soak).
4. Evidence refresh: gameplay capture, tac-map capture, a wellshaft descent, a
   ramp chain — all agent-viewed.
5. **Hands-on user playtest**: ramps, a silo descent, an observed relayout, a
   complete match. The arc does not close without it. Tuning findings triaged
   into the backlog or fixed in-phase.

## Evidence

The refreshed evidence set plus the playtest verdict recorded in this doc's
as-landed notes.
