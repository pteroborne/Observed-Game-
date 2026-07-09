# Arc F — Sight & Sound: implementation plans

One hand-off document per phase (54–60), each self-contained enough to give a fresh
implementation agent. Arc design and staging:
[../sight_and_sound_arc_plan.md](../sight_and_sound_arc_plan.md); sprite design
reference: [../../sprite_roadmap.md](../../sprite_roadmap.md).

## Hand-off model (foundation phase + two parallel tracks)

The arc is built for **concurrent sub-agents**. A standalone foundation phase (54,
True Singleplayer) and two tracks with disjoint core file sets run in waves; phases
within a track are sequential with review gates:

```text
Foundation:         54 (bot/guardian toggles — game setup, menu, settings)
Track A (audio):    55 ──► 56
Track B (sprites):  57 ──► 58 ──► 59 ──► 60
Dispatch waves:     [54 ∥ 57] → [55 ∥ 58] → [56 ∥ 59] → [60]
```

Phase 54 pairs with 57 in wave 1 because 57 is lab/assets-only and touches no
`game/src` file; 54 owns the menu/settings/sim-setup surface for its wave.

- The **parent session** dispatches one implementation agent per phase with that
  phase's doc as the brief, reviews the returned work against the doc's success
  criteria, runs the full verification itself, and commits — **agents do not
  commit**, and concurrent agents work in isolated worktrees.
- **Shared-file rule:** the only files both tracks touch are
  `crates/observed_assets/src/lib.rs` (+ its README) and `game/src/tests.rs`.
  Both are append-friendly (new slots, new test fns). Agents keep additions
  self-contained (no reformatting of neighbouring entries); the parent resolves
  any overlap at commit time and lands one track's commit before merging the
  other's worktree.
- **Review gates are real.** A phase is not "done" because an agent says so; the
  parent re-runs the verification recipe and checks each success criterion. Any
  criterion the agent could not meet must be reported, not papered over.

## Standing invariants — these apply to EVERY phase

Restated in each doc's critical points, but binding everywhere:

1. **The Legibility Contract is inviolable.** Sprites, textures, and audio never
   dim, hide, delay, or mask a gameplay-critical signal. New signals are
   legend-backed; `observed_style` owns the semantic palette and treatments —
   presentation asks it, never invents ad-hoc colours or materials.
2. **The immersion ruling holds (Phase 50, 2026-07-09).** Normal play is HUD-free
   (debug HUD only behind `OBSERVED2_DEBUG_HUD`); the tac-map is a fog-of-war
   survivor's sketch. No phase adds HUD surfaces to normal play.
3. **`game/` architecture rules (enforced by `game/src/arch_check.rs`):** no glob
   re-exports; no `use super::*` outside `#[cfg(test)]`; `sim/` never imports
   `view/` or `screens/`. Presentation reads simulation, never the reverse. Fix
   the dependency direction, never the ratchet test.
4. **Drop-in slots with fallbacks.** All assets go through `observed_assets` named
   slots: present → used, absent → procedural fallback / silence. Simulation stays
   asset-free. Deleting every imported asset leaves a playable, legible game.
5. **Match resource lifecycle:** every Match-scoped resource is enumerated once in
   `screens::match_runtime::session::for_each_match_resource!` and covered by the
   no-leak test. New Match resources go in that macro.
6. **Determinism hygiene:** no `HashMap` iteration-order reliance, all randomness
   through `observed_core::SplitMix` or a seeded RNG, no wall-clock-dependent
   gameplay-visible behavior. Derive scripts are deterministic and rerunnable.
   The three keyed hash finalizers (`game/hallway.rs`, `teleport/geom.rs`,
   `observed_style::district_for`) are NOT PRNG duplicates — never "unify" them.
7. **The comfort language is binding:** no camera shake, no full-screen flash, no
   audio jump-scare stingers; decoherence and collapse stay diegetic.
8. **Licensing:** CC0 only in game-ready asset paths; every new file gets a
   provenance row in `assets/SOURCES.md`. CC-BY sources stay reference-only.
9. **Deterministic brain untouched.** No phase in this arc edits `observed_net`,
   `observed_match`, or any gameplay rule. Sanctioned exception: Phase 54 may
   extend the match *configuration* surface (bot populations into the setup
   path) — deterministically, headless == interactive, with the all-on default
   pinned byte-identical by a characterization test. Rules of a fully-populated
   match never change.

## Verification recipe (every phase, zero warnings)

```
cargo fmt --all
cargo clippy --workspace --all-targets       # zero warnings — resolve, don't suppress
cargo test --workspace                        # all green; note count + wall time
```

Feature-gated crates also get `--features <feat>` clippy+test. Presentation changes
refresh the relevant `OBSERVED2_CAPTURE*` / `OBSERVED2_VIS_AUDIT` evidence (one
attempt, 3-min timeout, note if it fails — do not iterate on captures).

## After each phase (parent session)

Commit with the house message style (Co-Authored-By trailer), tick the ROADMAP `[x]`
+ add a Recent-Milestones entry, append an "As landed" note to the phase doc, and
update memory ([[sight-and-sound-arc]]).

## Phase index

| Phase | Doc | Track | Wave |
| --- | --- | --- | --- |
| 54 — True Singleplayer (bot & guardian toggles) | [phase_54_true_singleplayer.md](phase_54_true_singleplayer.md) | Foundation | 1 |
| 55 — Audio Architecture (the mixer) | [phase_55_audio_architecture.md](phase_55_audio_architecture.md) | A | 2 |
| 56 — Audio Content & Spatial Depth | [phase_56_audio_content_spatial.md](phase_56_audio_content_spatial.md) | A | 3 |
| 57 — Sprite Metadata Pipeline & OGA Lab | [phase_57_sprite_pipeline_lab.md](phase_57_sprite_pipeline_lab.md) | B | 1 |
| 58 — Gameplay Object Sprites | [phase_58_gameplay_object_sprites.md](phase_58_gameplay_object_sprites.md) | B | 2 |
| 59 — Directional Actors | [phase_59_directional_actors.md](phase_59_directional_actors.md) | B | 3 |
| 60 — Room Dressing, Textures & Interaction Read | [phase_60_dressing_textures_reticle.md](phase_60_dressing_textures_reticle.md) | B | 4 |
