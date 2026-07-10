# Phase 63 — Control Rebind, For Real

**Objective:** the rebind flow says "press Enter to remap" and then binds the
control to Enter — the capture listens while its own activation press is still
live. Replace the custom capture with the proven `control_lab` overlay machinery
(user ruling), and make the capture immune to its activation key. Closes bug
backlog #1. Read [README.md](README.md) first.

## Read first

- `labs/control_lab/` — the proven rebind overlay flow this phase adopts; note
  how it separates "arm the capture" from "capture a key".
- `game/src/screens/match_runtime/pause_settings.rs` — the current (buggy)
  capture: `pause_settings_capture_rebind` and the activation path around it.
- `game/src/screens/settings.rs` + `game/src/settings.rs` — the settings screen
  rebind entry point and binding persistence.
- `player_input::PlayerIntent` — bindings must round-trip through the intent
  layer; gamepad support must not regress (a Phase 48 criterion).

## Design rulings (already decided)

- **Adopt, don't rewrite:** promote/reuse the `control_lab` overlay logic rather
  than patching the custom capture. If promotion needs a shared home, prefer the
  smallest move (a module the game imports) over a new crate.
- **The activation press can never be captured.** Structurally: the capture arms
  on the activation key's *release* (or explicitly swallows the first press),
  so this bug class is impossible, not just patched. Escape cancels; the
  activation key can still be *deliberately* bound by pressing it again after
  arming.
- Rebinds persist with the career profile exactly as today; conflicts (key
  already bound) surface visibly rather than silently double-binding.

## Files you may edit

`game/src/screens/match_runtime/pause_settings.rs`, `game/src/screens/settings.rs`,
`game/src/settings.rs`, whatever minimal shared home the control_lab overlay logic
moves to (+ `labs/control_lab` to re-export instead of duplicate), `game/src/tests.rs`,
`Catalogue.md` if the lab's role changes. Do NOT touch `sim/` or unrelated screens.

## Success criteria

- Automated: a test drives the rebind flow (arm with Enter, then press K) and
  asserts the binding is K, not Enter; a second test arms and presses the
  activation key deliberately and asserts it *can* be bound; cancel works;
  persistence round-trips; gamepad bindings unaffected.
- Manual + capture: a screenshot of the rebind overlay mid-capture (prompt
  visible), viewed per the falsifiable-evidence rule; the report describes the
  full manual flow working in the running game.
- `control_lab` and the game share one implementation (no drifted copy).
- Full verification recipe green.
