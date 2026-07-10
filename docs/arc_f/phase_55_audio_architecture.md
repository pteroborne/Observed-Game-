# Phase 55 — Audio Architecture (the mixer)

**Objective:** one audio director owns every cue decision — buses, priority,
ducking, cooldowns — so audio behavior lives in a single data-driven table instead
of scattered spawn sites. This is the *architecture* half of audio perfection;
Phase 56 adds content on top of it. Read [README.md](README.md) first.

## Read first

- `game/src/screens/audio.rs` — `MatchAudioCue`, `play_one_shot`,
  `sync_match_audio`, `bleed_rival_sound`; the Phase-49 patterns this phase
  consolidates: cue entities, muted-channel suppression (suppress spawn, don't
  spawn silent), attenuation constants.
- `game/src/screens/match_runtime/ambience.rs` — ambience beds are stable loop
  entities that cross-fade **volume only** (never restart streams — the Phase-49
  stutter lesson); district atmosphere transitions.
- `game/src/screens/match_runtime/pause_settings.rs` + the Phase-48 `Settings`
  resource — the existing volume channels the bus model formalizes.
- `crates/observed_assets/src/lib.rs` — sound slot manifest (`FOOTSTEP`,
  `REROUTE`, `ESCAPE`, `DOOR`, `KLAXON`, `COLLAPSE_STING`, `UI_CLICK`,
  `UI_HOVER`, `JUMP`, `LAND`, `DISTRICT_AMBIENCE`).
- ROADMAP.md "Phase 49" milestone entry — the Geiger-counter artifact and its
  fix; that class of bug (event-edge spam becoming audible noise) is what
  structural cooldowns prevent.

## Design rulings (already decided)

- **One director, data-driven.** A single `AudioDirector` (Match-scoped resource,
  registered in `for_each_match_resource!`) receives cue requests and applies a
  per-cue-class config table: bus, base volume, cooldown, max concurrent
  instances, duck behavior. Spawn sites *request*; the director *decides*.
- **Buses over ad-hoc scaling:** `Master → {Music/Ambience, Sfx, Ui}` mapping onto
  the existing Settings channels. Ducking (e.g. stings duck ambience beds) is a
  bus-level, time-eased operation — no stream restarts, volume easing only.
- **Cooldowns are structural**, not per-bug: every one-shot class has a default
  cooldown and an instance cap in the table, so the next contact-toggle-style
  spam bug is inaudible by construction rather than patched after diagnosis.
- **No behavior regressions:** existing cues keep their audible identity (same
  files, same rough loudness); this phase moves *where decisions live*, not how
  the game sounds. The comfort language and Legibility Contract are binding.
- No new sound files this phase; slots and content are Phase 56.

## Files you may edit

`game/src/screens/audio.rs` (the director lives here or in a new sibling
`audio/` module), `game/src/screens/match_runtime/ambience.rs`,
`game/src/screens/match_runtime/{mod.rs, session.rs}` (resource registration),
`game/src/view/{components.rs, assets.rs}` (cue/bus components if needed),
`game/src/screens/menu.rs` (UI sounds route through the Ui bus),
`game/src/tests.rs`. Do NOT edit `sim/`, `observed_style`, or gameplay rules;
`crates/observed_assets` should not need changes this phase.

## Implementation

1. **Cue table.** Define `CueClass` config (bus, base volume, attenuation class
   placeholder for Phase 56, cooldown, max instances, duck spec) as plain data —
   one `const`/static table, unit-testable without Bevy.
2. **AudioDirector resource.** Owns per-class last-fire ticks and live instance
   counts; exposes `request(cue, params)`; enforces cooldowns/caps; spawns via
   the existing one-shot path; suppressed (muted channel) requests spawn nothing.
3. **Bus state & ducking.** Per-bus gain derived from Settings channels; duck
   requests ease bus gain down/up over configured times; ambience cross-fades
   compose with bus gain (multiply, don't fight).
4. **Migrate every spawn site** (footsteps, doors, reroute, escape, rival bleed,
   stings, klaxon, jump/land, UI) to director requests; delete site-local
   volume/cooldown constants as they move into the table.
5. **Tests:** table sanity (every `MatchAudioCue` variant has a class), cooldown
   and instance-cap enforcement, muted-channel suppression, duck easing math,
   bus gain composition, and the no-leak macro covering the new resource.

## Success criteria

- Every match/UI cue routes through the director; `rg 'play_one_shot'` outside
  the director shows no direct call sites.
- A synthetic spam test (N requests in one tick) produces at most the configured
  instance cap and respects cooldown on subsequent ticks.
- Ambience beds still never restart streams (volume-only assertion holds), and a
  sting audibly ducks the beds in a capture run without any stream churn.
- Idle-match audio stability test from Phase 49 still passes unchanged.
- Full verification recipe green (fmt, clippy zero warnings, workspace tests).

## As landed (2026-07-09; note written 2026-07-10 during Arc H Phase 61)

- `AudioDirector` (Match-scoped, in `for_each_match_resource!`) owns the
  data-driven cue table (`get_config`: bus, base volume, cooldown, max instances,
  duck spec, attenuation class), `request`/`request_spatial`, bus ducking with
  eased in/out, and per-class cooldowns/instance caps. All spawn sites migrated;
  `play_one_shot` has no call sites outside the audio module.
- Tests: cue-table sanity, cooldown/instance-cap enforcement, duck math and
  director ducking, muted-channel suppression.
- Post-landing review fix (2026-07-09): `update_audio_director`'s blanket sink
  write also drove the district ambience beds to full volume every frame, fighting
  the fade system between writes — beds are now excluded
  (`Without<AmbienceBed>`); the fade system is their only volume writer. See
  Phase 56's "Review fixes".
