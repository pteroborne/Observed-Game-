# Phase 56 — Audio Content & Spatial Depth

**Objective:** with the Phase-55 director in place, perfect what the player
*hears*: real distance/occlusion classes, an audited cue set where every
gameplay-critical signal has an audio counterpart, richer district beds, and a
CC0-only audio inventory. Read [README.md](README.md) first. Depends on Phase 55
(the cue table is where rolloff/occlusion classes live).

## Read first

- `game/src/screens/audio.rs` (+ the Phase-55 director/table) — attenuation today:
  `bleed_rival_sound`'s distance pattern is the model to generalize.
- `game/src/screens/match_runtime/ambience.rs` — district beds and cross-fades.
- `crates/observed_assets/src/lib.rs` + README — slot manifest; new slots land
  here (drop-in: absent → silent).
- `game/src/screens/hud.rs` — the debug-HUD legend; any *new* audible signal class
  gets a legend entry there (debug surface only; normal play stays HUD-free).
- `assets/SOURCES.md` — provenance ledger; the CC-BY 3.0 Little Robot Sound
  Factory zips in `assets/oga_25d/raw/` are reference-only and slated for
  replacement/removal this phase.
- `docs/evidence/visual_audit/` + `game/src/evidence/audit.rs` — the audit gate.

## Design rulings (already decided)

- **Coverage before flourish.** First deliverable is an *audio coverage audit*: a
  checked-in table (doc or test fixture) mapping every gameplay-critical signal —
  door freeze/unfreeze, anchor place/expire, keystone pickup, exit unlock,
  guardian proximity/alert, rival enters/leaves your place, collapse frontier
  advance, reroute, countdown — to its cue (or an explicit "intentionally
  silent" ruling). Missing cues found by the audit are the content backlog.
- **Spatial classes, not per-cue hacks:** rolloff (near/room/far) and occlusion
  (same place vs through-threshold vs through-wall) are class parameters in the
  Phase-55 table. Occlusion is a volume/filter approximation — no raycast
  physics, place/threshold topology the presentation already knows is enough.
- **All new sounds are drop-in slots** and CC0 (Kenney libraries and in-repo
  synthesis are the proven sources). The CC-BY zips are deleted from
  `assets/oga_25d/raw/` and their `SOURCES.md` rows moved to a "removed" note,
  closing the license caveat for good.
- **The comfort language is binding:** no jump-scare stingers; guardian proximity
  reads as growing dread (bed/filter shift), not a horror sting. Atmosphere never
  masks a signal — a duck must never make a critical cue inaudible.

## Files you may edit

`game/src/screens/audio.rs` (+ Phase-55 audio module), 
`game/src/screens/match_runtime/ambience.rs`,
`crates/observed_assets/src/lib.rs` + `crates/observed_assets/README.md`,
`assets/sounds/*` (new CC0 files), `assets/SOURCES.md`, `assets/oga_25d/raw/`
(CC-BY removal only), `game/src/screens/hud.rs` (debug legend),
`game/src/tests.rs`, `docs/` (coverage audit doc). Do NOT edit `sim/` or
gameplay rules.

## Implementation

1. **Audio coverage audit.** Enumerate signal→cue mappings; record rulings in
   `docs/arc_f/audio_coverage.md`; add a test that every `MatchAudioCue` variant
   appears in the table (keeps the doc honest as cues are added).
2. **Spatial classes.** Add rolloff + occlusion parameters to the cue table;
   derive listener–source relation (same place / adjacent through threshold /
   elsewhere) from presentation state; generalize `bleed_rival_sound` into the
   class system.
3. **Missing cues.** Implement the audit's gaps as new `MatchAudioCue` variants +
   `observed_assets` slots with CC0 files (or synthesized loops per the district
   beds precedent), each through the director with a sensible class.
4. **Bed depth.** Layer or re-synthesize district beds where flat (interior
   variation, e.g. gantry vs corridor already differ — extend to intensity
   states the ambience module already tracks: drained, klaxon).
5. **License cleanup.** Remove CC-BY zips, update `SOURCES.md`, and add a test or
   check that `assets/` game-ready paths reference only ledgered CC0 rows if
   cheap to assert.
6. **Evidence.** Refresh the bot-POV capture (audio is felt in the GIF's context
   even if unheard) and note a manual listen-through of one full match.

## Success criteria

- The coverage table exists, is test-enforced against the cue enum, and has no
  unruled gaps.
- Rival/guardian/door cues audibly change with distance and place topology in a
  manual run; same-place vs through-wall is distinguishable.
- No CC-BY files remain anywhere under `assets/`; `SOURCES.md` is consistent.
- Every new slot is optional: deleting `assets/sounds/` still boots and plays
  silently without errors or panics.
- Full verification recipe green; visual audit reports zero findings.
