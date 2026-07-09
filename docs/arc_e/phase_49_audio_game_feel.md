# Phase 49 — Audio & Game Feel

**Objective:** the facility *sounds* alive and the body *feels* good to move — without
ever violating the Legibility Contract or the established comfort language. Read
[README.md](README.md) first.

## Read first

- `game/src/screens/audio.rs` — `MatchAudioCue { Ambience, Footstep, Door, Escape,
  Reroute, RivalBleed }`, `play_one_shot`, `sync_match_audio` (footstep cadence from
  stride), `bleed_rival_sound`; the drop-in sound slots come from `observed_assets`
  (present → used, absent → silent).
- `game/src/screens/match_runtime/ambience.rs` — district atmosphere, `drained`/`klaxon`
  palette transitions, `flicker_lights`, `sync_decohere_fx` (the **diegetic** decoherence
  feedback: lights + sound, deliberately **no camera shake, no full-screen flash** — see
  the comfort-design memory).
- `game/src/screens/place/*` (renderer), `sim/state.rs` `TeleportAnimation`, the gantry
  deck/land path, `present_match_camera`.
- `crates/observed_style/src/lib.rs` (districts, `klaxon`, `drained`) and
  `game/src/evidence/audit.rs` — the Legibility Contract audit (signals punch through the
  atmosphere) is the acceptance gate.
- `crates/observed_assets/src/lib.rs` — the asset-slot manifest; add new sound slots here
  if needed (they stay optional).

## Design rulings (already decided)

- **The comfort language is binding:** no camera shake, no full-screen flash, no effect
  that masks a threat or a signal. Decoherence and collapse are felt *diegetically*
  (flickering fixtures, audio, palette drain), never through screen-space violence. Juice
  is subtle and readable.
- All new audio is **drop-in** (new `observed_assets` slots, silent until a file is
  dropped) — the game ships complete; assets only enrich.
- Every new sound/effect is verified against the Legibility Contract audit, not just
  eyeballed.

## Files you may edit

`game/src/screens/audio.rs`, `game/src/screens/match_runtime/ambience.rs`,
`game/src/screens/place/{lighting.rs, factory.rs, shell.rs, preview.rs}` (effect spawns),
`game/src/view/{components.rs, assets.rs}` (new cue variants / material or sound handles),
`crates/observed_assets/src/lib.rs` + its README (new optional slots),
`game/src/screens/hud.rs` (legend for any new signal), `game/src/tests.rs`. Do NOT change
the deterministic brain, `sim/director.rs`, or gameplay rules.

## Implementation

1. **Spatial/attenuated audio.** Give the existing one-shots distance attenuation and
   simple occlusion (quieter through a wall/threshold), building on `bleed_rival_sound`'s
   attenuation pattern. Volume respects the Phase-48 `Settings` sfx/music channels.
2. **Ambient beds.** A looping per-district ambient bed (drop-in slot per district or one
   slot modulated by district), cross-fading as the player crosses places — mirrors how
   `apply_place_atmosphere` eases the palette.
3. **Missing stings.** Collapse-advance, klaxon-countdown, and escape stings as
   `MatchAudioCue` variants (add to the cue enum + the no-leak-safe audio path), fired
   from the existing collapse/countdown/escape state the presentation already reads.
4. **UI/menu sounds.** Cursor move / confirm / back on the menu screens (drop-in,
   Settings-volume-gated).
5. **Movement & camera juice (diegetic only).** Polish the teleport transition
   (`TeleportAnimation`), threshold-crossing feedback, gantry jump/land response, and a
   restrained collapse camera cue — all within the comfort language (subtle FOV/level
   easing at most; NO shake/flash). Keep `present_match_camera` deterministic.

## Tests

- New `MatchAudioCue` variants fire on their trigger state and exactly once per event
  (mirror the `RivalBleed` one-cue-per-appearance test).
- `legibility_contract_holds_with_all_effects_live` — the `observed_style`/diagnostics
  signal-punch-through assertion passes with the new atmosphere active.
- Audio volume respects the Settings channels (a muted channel produces no playing cue).
- No new Match audio/effect resource leaks past `OnExit(Match)` (the enumerated lifecycle).

## Verification

Per the README recipe, plus the Legibility Contract audit run
(`OBSERVED2_VIS_AUDIT`) and a refreshed bot-POV evidence GIF. Report: new cues/slots
added, the attenuation/occlusion model, the game-feel effects and how each stays inside
the comfort language, Legibility audit result, verification results.

## As landed

- Added manifest-owned per-district ambience slots in `observed_assets::DISTRICT_AMBIENCE`
  and loaded them from `MatchAssets`, eliminating district sound path literals from the
  game layer. The UI sound slots now point at the checked-in `ui_click` and `ui_hover`
  files and are loaded through named manifest constants.
- Diagnosed the audio stutter risk in the WIP path as stream pause/resume churn plus
  always-on diagnostics logging. The landed path keeps district ambience loop entities
  warm and cross-fades only their volume, while muted SFX channels no longer spawn
  inaudible one-shot cue entities.
- Follow-up diagnosis confirmed the remaining "Geiger counter" stutter was a rapid
  one-shot stream, not faulty media playback: an idle match repeatedly spawned `Land`
  cue entities because exact floor/deck contact could toggle the traversal body from
  grounded to airborne and back each fixed step. `observed_traversal` now keeps exact
  resting support grounded and reports `landed` only on an actual fall-to-support
  transition.
- Collapse/klaxon stings are SFX-volume-gated and fire once per event/active loop.
  Rival bleed remains distance-attenuated through the existing threshold-distance model.
- Camera feedback now uses smooth jump/land/teleport height easing and a tiny steady
  collapse level offset. The old teleport/collapse jitter was removed to keep the
  comfort ruling binding: no camera shake and no full-screen flash/jolt for route
  instability or collapse.
- Added regressions covering district manifest alignment, UI slot loading, muted cue
  gating, one-shot/loop sting behavior, idle-match audio cue stability, and traversal
  resting support.
- Verification passed: `cargo fmt --all`, `cargo test -p observed_traversal` (17 tests),
  `cargo test -p observed_game` (195 tests),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `OBSERVED2_VIS_AUDIT=docs/evidence/visual_audit cargo run -p observed_game` (zero
  findings), `OBSERVED2_CAPTURE_BOT=docs/evidence/bot_pov cargo run -p observed_game`,
  and the refreshed FFmpeg palette/GIF pass for `docs/evidence/bot_pov/bot_pov.gif`.
