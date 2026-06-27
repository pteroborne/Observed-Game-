# Archie Input Lab

**Phase A6** of the [Bevy asset-integration roadmap](../../docs/bevy_asset_integration_roadmap.md):
the controller / local-multiplayer input candidate,
[`bevy_archie`](https://crates.io/crates/bevy_archie).

It answers one question: **can richer device support feed the existing
`player_input::PlayerIntent` boundary without rewriting gameplay systems?** The lab
adds archie's real `ControllerPlugin` (device detection, controller ownership,
haptics, and the data-driven `ActionMap`) and routes keyboard, gamepad, scripted,
and replayed control for four local players into the *same* `PlayerIntent`.
Gameplay never reads a device directly.

## Compatibility Gate

- `bevy_archie 0.2.4` is the Bevy `0.18` line (`bevy ^0.18`); the latest `0.3.0`
  targets Bevy `0.19` and the `0.1.x` line targets Bevy `0.17`, so both are
  rejected for this workspace. Pinned exactly with `default-features = false`.
- **`bevy_archie 0.2.x` declares MSRV `1.94`.** Building the lab required updating
  the workspace's Rust toolchain to `>= 1.94` (it was on `1.92`). This is the one
  real cost of the dependency and is recorded here as the adoption finding.
- Dependency impact from `cargo tree -p archie_input_lab -e normal --depth 1`: the
  lab depends directly on `bevy 0.18.1`, `bevy_archie 0.2.4`, `player_input`, and
  `observed_style`. archie adds `dirs`, `log`, `serde`, and `serde_json` on top of
  the existing Bevy graph (no extra GPU/render stack), and pulls a wider set of
  Bevy sub-features it needs (`bevy_gilrs`, `bevy_render`, `bevy_ui`, `bevy_text`,
  `bevy_sprite`, `bevy_state`).

## Architecture Finding

archie ships a global `ActionState` resource: its `update_action_state` system
merges **every** connected gamepad and the keyboard into one set of action
booleans. That is ideal for a single-player menu but cannot express "what does
*player 2's* controller want?", which this project's four-local-player rule
requires.

So the lab keeps archie's genuinely valuable pieces — the remappable `ActionMap`
binding table, the `GameAction` vocabulary, the `ControllerConfig` deadzone /
sensitivity model, the `ControllerOwnership` assignment store, and the
`RumbleRequest` haptics — and evaluates them against a **single device sample at a
time** in a pure adapter ([`src/adapter.rs`](src/adapter.rs)). The result is
per-player isolation through one code path, ending in `player_input::PlayerIntent`.
This mirrors how the trenchbroom lab uses archie's importer but not its scene: we
consume the asset's data model, not the part that fights the architecture.

## What It Demonstrates

- **One `PlayerIntent` for every source**: keyboard, gamepad, scripted fallback,
  and replayed tape all produce the same `PlayerIntent`. Gameplay branches on none
  of them.
- **Four local players, no single-player assumptions**: P1 is a keyboard human;
  P2–P4 start scripted and a connected controller claims the next free slot
  through archie's `ControllerOwnership`.
- **Assignment + disconnection + scripted fallback**: a connected controller
  claims a slot; a disconnect (archie's `ControllerUnassigned`) reverts that player
  to its deterministic scripted pattern.
- **Remapping**: `F7` rebinds the interact action by editing archie's `ActionMap`;
  the next evaluation reads the new binding with no code change.
- **Focus loss**: losing window focus neutralizes every intent and freezes input.
- **Recording / replay at the intent layer**: `F5`/`F6` record and loop a player's
  `PlayerIntent` stream — the exact layer the project's replay and networking use.
- **Presentation-only haptics**: a hazard pulse (`G`) rumbles gamepad players via
  archie; tests assert it never touches `PlayerIntent`.
- **Legibility**: every probe and panel colour comes from `observed_style`.

## Controls

- `1`–`4`: select a player
- `Tab`: toggle the selected player between keyboard and scripted
- P1 keyboard: `WASD` move, `IJKL` look, `Space` jump, `Shift` sprint, `F`
  interact, `C` climb
- `F5` record / `F6` replay the selected player's intent
- `F7` rebind interact   `H` toggle haptics   `G` hazard pulse
- `R` reset   `F1` toggle overlay

## Success Conditions

1. The overlay shows `[PASS]` (one camera, one UI root, four probes, four players,
   focused).
2. Each player's row shows a live `PlayerIntent`; a connected controller's row
   reads `gamepad` while the keyboard and scripted rows keep working independently.
3. Disconnecting a controller reverts its player to a scripted pattern.
4. `R` reset leaves exactly one camera, one UI root, and four probes with no leaks.

## Decision

`bevy_archie` is a viable controller/remapping/haptics provider for the
`PlayerIntent` boundary, but it stays **lab-local**. Its global `ActionState` is
not adopted (it is not per-player), and its only real cost is the Rust `1.94` MSRV
bump. Promote a narrow device-to-intent adapter only when controller support
becomes a committed feature (per roadmap A9); do not replace `player_input`.

## Regenerating The Evidence

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/archie_input_lab.png"
cargo run -p archie_input_lab
```
