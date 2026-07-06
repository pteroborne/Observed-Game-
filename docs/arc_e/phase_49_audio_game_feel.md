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
