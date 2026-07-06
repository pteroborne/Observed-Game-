# Phase 48 — Onboarding & Settings

**Objective:** a new player can sit down, understand the core loop, tune the game to
their hands, and complete a match without external instruction. This is the biggest QoL
gap between "proving ground" and "game." Read [README.md](README.md) (standing
invariants) first.

## Read first (confirm current state before editing)

- `game/src/screens/input.rs` and `game/src/screens/match_runtime/input.rs` — input is
  currently **hardcoded `KeyCode` constants inline** with a `MOUSE_SENSITIVITY` const;
  there is no settings resource and no rebinding in the game yet.
- `crates/player_input/src/lib.rs` — `PlayerIntent`, and `RebindCapture` (the rebind
  primitive proven in `labs/control_lab/src/controls.rs` — read that lab for the working
  pattern: `BeginJumpRebind`, `capture_rebind`, cancel).
- `game/src/flow.rs` — `Career`/`Profile`; **confirm whether the profile persists to
  disk** or only lives for the app session. `game/Cargo.toml` already depends on
  `serde_json`, so file persistence is available.
- `crates/observed_style/src/lib.rs` — the accessibility legend mappings the settings
  screen surfaces (grep `accessib`); confirm the actual API.
- `game/src/screens.rs` (the `GameState` machine + `ScreensPlugin`), `screens/menu.rs`,
  `screens/loadout.rs` — the menu-screen pattern (`MenuAction`, `MenuCursor`,
  `menu_button`, state-scoped `DespawnOnExit`) a settings screen mirrors.
- `game/src/screens/hud.rs` (`spawn_match_hud`) + the legend — where first-run/context
  hints live.

## Design rulings (already decided — don't re-litigate)

- Settings is a new `GameState::Settings` screen reachable from Main Menu and the pause
  menu, mirroring `Loadout`'s structure (state-scoped, keyboard/gamepad navigable).
- Teaching is **diegetic-first**: reuse the legend/discovery/glyph systems and short
  contextual hints; NOT a wall of modal text. A first-run guided beat is acceptable but
  the facility should teach by being read (observe-to-freeze, anchors, the tac-map, the
  gantry jump-or-bypass, the collapse).
- Bindings and settings persist to disk (serde_json) alongside/within the profile.
- Keep full gamepad support — settings are edited with keyboard OR gamepad.

## Files you may edit

`game/src/screens/{input.rs, menu.rs}`, a new `game/src/screens/settings.rs`,
`game/src/screens/match_runtime/input.rs`, `game/src/screens.rs` (state + plugin
registration), `game/src/flow.rs` or a new `game/src/settings.rs` (the settings/bindings
resource + persistence), `game/src/screens/hud.rs` (hints), `game/src/tests.rs`. Do NOT
touch `sim/`, `crates/` (except reading), or the match brain.

## Implementation

1. **Settings resource + bindings.** Introduce a `Settings` resource (a new
   `game/src/settings.rs`): master/sfx/music volume (0..1), mouse sensitivity, and a
   keybinding table replacing the inline `KeyCode` consts (movement, jump, sprint,
   interact, torch, pad, activate, tac-map, pause). `Default` = today's exact bindings
   and sensitivity, so behaviour is unchanged until the player edits. `#[derive(Serialize,
   Deserialize)]`; load on startup from a settings file (next to the profile), save on
   change. Persist accessibility toggles here too.
2. **Route input through it.** `screens/input.rs` and `match_runtime/input.rs` read the
   binding table + sensitivity from `Settings` instead of literals — still producing the
   same `PlayerIntent`/`ItemIntent` (invariant 4). Gamepad mapping unchanged.
3. **Settings screen** (`screens/settings.rs`): a `GameState::Settings` with rows for
   each slider/toggle/binding; rebinding uses the `RebindCapture` pattern from
   `control_lab`. Register `OnEnter(Settings)` setup + its Update systems in
   `ScreensPlugin`; add a Main-Menu entry (`MenuAction`) and a pause-menu route. Volume
   changes apply live (bevy audio `Volume`).
4. **Accessibility toggles** wired to `observed_style`'s accessibility mappings (e.g. a
   high-contrast/legend-emphasis mode) — surfaced, legend-backed, Legibility-Contract-safe.
5. **Onboarding.** A first-run flag in `Settings`; on a player's first match, surface the
   core-loop hints through the HUD/legend (short, contextual, dismissable), and a
   pause-menu control reference. Keep it diegetic and skippable.

## Tests (name them so intent reads)

- `default_settings_reproduce_the_shipped_bindings_and_sensitivity` — `Settings::default`
  yields today's exact keys/sensitivity (no behaviour change out of the box).
- `settings_round_trip_through_serde` — serialize → deserialize is identity.
- `rebinding_a_key_changes_the_intent_it_produces` — rebind through the intent layer,
  assert the new key drives the action and gamepad still works.
- `settings_screen_is_state_scoped_and_navigable` — enter/exit despawns cleanly (no
  leak), cursor navigates rows.
- First-run onboarding shows on first match and not after (flag flips + persists).

## Verification

Per the README recipe. Add: a "cold" bot-driven walkthrough capture
(`OBSERVED2_CAPTURE_BOT` or the tour) still completes with settings routed through the
new table (proves no input regression). Report: bindings/settings API as landed,
persistence location, accessibility toggles wired, test list, verification results, and
whether the profile already persisted to disk or you added it.
