# Phase 54 — True Singleplayer (bot & guardian toggles)

**Objective:** menu/settings toggles that disable each bot population separately —
rival teams, own AI teammates, and the guardian — so a "True Singleplayer" match
(alone in the facility, or any combination) exists for both gameplay and testing.
The toggles are *deterministic match configuration*, not presentation suppression.
Read [README.md](README.md) first. This is the arc's wave-1 foundation phase.

## Read first

- `game/src/sim/director.rs` + `game/src/sim/state.rs` — how the match is
  configured and populated today (teams, the elimination series, guardian
  pressure). **Read-only for orientation**: this phase is the sanctioned
  exception to invariant 9 in that it may extend match *configuration* through
  the setup path, but it must not change any rule of a fully-populated match.
- `game/src/screens/match_runtime/{mod.rs, session.rs}` — match setup/entry, the
  `for_each_match_resource!` lifecycle macro.
- `game/src/screens/menu.rs` + `game/src/screens/match_runtime/pause_settings.rs`
  — the Phase-48 settings screen and career-profile persistence; the menu path
  into a match (where per-match options belong).
- `game/src/map_catalog.rs` (or `game/src/` equivalent) — the `OBSERVED2_MAP`
  pattern: a validated, env-overridable selection that headless and interactive
  paths share. The bot toggles follow this exact pattern.
- ROADMAP Phase 33/35 milestones — how the elimination series and teamplay
  model use teams, so you know what a zero-rival series must still satisfy.

## Design rulings (already decided)

- **Three independent toggles:** `rival_teams`, `ai_teammates`, `guardian` — each
  on/off, all combinations valid. Default: all on (current behavior, byte-
  identical; a characterization test pins this).
- **Configuration, not suppression.** A disabled population is *never spawned in
  the simulation* — not spawned-then-hidden, not spawned-then-frozen. The config
  enters the deterministic match setup exactly like map selection, so
  headless == interactive holds and the same config + seed always produces the
  same match digest.
- **Surfaced in the menu flow** as pre-match options (with the Phase-48 settings
  conventions: persisted with the career profile, readable labels), plus an
  `OBSERVED2_BOTS` env override (e.g. `all|none|no_guardian|no_rivals|no_teammates`,
  combinable) for capture/testing parity with the `OBSERVED2_MAP` precedent.
- **A solo match must be complete and winnable.** With zero rival teams the
  elimination series degenerates gracefully: the match still ends on the visible
  run (`director.live.finished()` — the match-end invariant), escape still
  produces a result screen (placement trivially 1st), no stalemate, no division
  by zero in standings. With the guardian off, pressure systems that read
  guardian state read "absent", not a dummy entity.
- **Solvability and collapse rules are untouched** — the facility behaves
  identically; only who else is in it changes.
- **If a rule change looks unavoidable** (e.g. the series hard-assumes ≥2 teams
  somewhere deep), stop and report to the parent session with the specific site
  — do not silently rewrite series rules.

## Files you may edit

`game/src/sim/` match setup/config surface only (a `MatchConfig`-style input to
the director — populations, not rules), `game/src/screens/menu.rs`,
`game/src/screens/match_runtime/{mod.rs, session.rs, pause_settings.rs}`,
career-profile persistence for the new fields, `game/src/tests.rs`,
`game/src/evidence/` only if captures need the env override plumbed. Do NOT edit
`observed_match`, `observed_net`, collapse/decoherence/keystone rules, or the
match-end rule.

## Implementation

1. **Config type.** A plain `BotPopulations { rival_teams: bool, ai_teammates:
   bool, guardian: bool }` (or equivalent) carried in the match setup input;
   default all-true; parsed from `OBSERVED2_BOTS` with validation and from the
   persisted profile.
2. **Setup plumbing.** Thread it through interactive match setup and headless
   `play_match()` identically; populations the config disables are skipped at
   spawn/registration time in the setup path.
3. **Series degeneracy.** Audit the elimination series and standings for solo
   correctness; fix at the *setup/reporting* level (team count in, results out),
   not the rule level.
4. **Menu + persistence.** Pre-match toggles in the menu flow; persisted with
   the career profile; labels explicit ("Rival teams", "AI teammates",
   "Guardian"); the pause screen shows the active configuration read-only.
5. **Tests.** Characterization: default config produces today's match digest.
   Per-toggle: headless matches for all 8 combinations complete without panic;
   solo escape yields a well-formed result; guardian-off spawns no guardian
   entity/resources (no-leak test extended); env parsing round-trips; profile
   persistence round-trips.

## Success criteria

- All 8 toggle combinations run headless to a valid outcome; the all-on digest
  is unchanged from before the phase (pinned test).
- A human can start a true-solo match from the menu, explore, and escape; the
  result screen is sane. Toggles persist across app restarts.
- `OBSERVED2_BOTS=none` + a capture run produces evidence of an empty facility
  (one screenshot under `docs/evidence/`).
- No presentation-side "hide the bot" code anywhere in the diff; disabled
  populations simply never exist in the sim.
- Full verification recipe green (fmt, clippy zero warnings, workspace tests).

## As landed (2026-07-09; note written 2026-07-10 during Arc H Phase 61)

- `BotPopulations { rival_teams, ai_teammates, guardian }` lives in
  `game/src/sim/director.rs` with `from_env()`/`from_str()` parsing `OBSERVED2_BOTS`
  (`all|none|no_guardian|no_rivals|no_teammates`, combinable). Config threads
  through `MatchDirector::new(seed, map_spec, config)`; lobby toggles persist via
  the career profile; the env override wins; `MATCH_START` logs the configuration.
- Guardian off = the `Guardian` resource is never inserted; guardian systems take
  `Option<ResMut<Guardian>>` and bail. No presentation-side hiding anywhere.
- Deviations: (1) `apply_population_config` rebuilds host/remote team lists after
  construction rather than skipping at spawn — behavior-preserving for the all-on
  default per the seed-corpus/career tests, but the doc's literal "all-on digest
  pinned unchanged" characterization test was not added (scheduled: Phase 66).
  (2) `from_str` panics on an unknown token instead of warning (scheduled: 66).
- Tests: all 8 toggle combinations headless to valid outcomes, guardian-off spawns
  no guardian resources, env parsing + persistence round-trips. Evidence:
  `docs/evidence/empty_facility.png` (OBSERVED2_BOTS=none).
