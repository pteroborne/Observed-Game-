# Lab Observability Lab

**Phase A7** of the [Bevy asset-integration roadmap](../../docs/bevy_asset_integration_roadmap.md):
the lab-config and event-trace candidates,
[`bevy_mod_config`](https://crates.io/crates/bevy_mod_config) and
[`bevy_log_events`](https://crates.io/crates/bevy_log_events).

It answers one question: **can labs expose useful knobs and event traces without
making debug state part of the game simulation?** The lab runs a deterministic
event simulation (doors, observation, interaction, match rounds, pickups,
reroutes), exposes its launch seed plus a set of debug knobs through a typed,
JSON-persisted config, mirrors events to the log through a config-gated tracer,
and renders the otherwise-invisible event flow and live config in a neon-noir
overlay.

## Compatibility Gate & Asset Decision

- **`bevy_mod_config 0.6.2` — adopted (egui off).** It is the Bevy `0.18` line
  (core deps `bevy_app`/`bevy_ecs ^0.18.0`); the `bevy_egui` editor UI and `serde`
  persistence are *optional* features. The lab pins it `default-features = false`
  with `["std", "serde", "serde_json"]`: it takes the typed schema, change
  detection, and JSON persistence, and **leaves the egui editor off**, rendering
  its own overlay instead. (`Json` has no `Default`, so the app constructs the
  manager with `init_config_with(.., Json::new)`.)
- **`bevy_log_events 0.7.0` — not adopted.** `0.7.0` is the only Bevy `0.18`
  release (`0.6.0` targets Bevy `0.17`), but its only logging-capable feature,
  `enabled`, force-pulls `bevy_egui`, and `LogEventsPlugin::build` does
  `assert!(app.is_plugin_added::<EguiPlugin>())` — so the crate cannot be used at
  all without adding an egui UI stack. That violates this project's dependency
  rule (don't add a UI dependency unless it removes substantial technical risk;
  egui has been kept off in A3 and A6). The capability it would have provided —
  registration-based, runtime-toggleable event logging — is implemented
  **lab-locally** over Bevy's own `tracing` log in
  [`trace_events`](src/lab.rs). This mirrors A3's `bevy_color_blindness`
  rejection: adopt the asset that fits, preserve the capability of the one that
  does not.
- Dependency impact from `cargo tree -p lab_observability_lab -e normal --depth 1`:
  the lab depends directly on `bevy 0.18.1`, `bevy_mod_config 0.6.2`,
  `observed_style`, and `player_input`. `bevy_mod_config` adds a small,
  render-free stack on top of the existing Bevy graph (`bevy_mod_config_macros`,
  `derivative`, `hashbrown`, `serde`, `serde_json`, `variadics_please`); **no egui
  and no extra GPU/render crates**.

## The Architectural Spine: manifest vs. debug knobs

Every knob lives in one config root, but they play two different roles, and the
whole lab is built to keep that line honest:

- **`seed` is a candidate launch input.** The running simulation is driven by an
  `ActiveManifest`, not by the live config seed. Editing the seed (`-`/`=`) only
  marks a *pending relaunch*; it cannot change deterministic state. Pressing
  `Enter` **commits** the candidate seed into the manifest — the single, explicit
  path by which a config value enters deterministic state — and rebuilds the
  event stream. At startup, a persisted seed becomes the launch manifest the same
  deliberate way.
- **Every other knob is debug/presentation only.** Fog, bloom, overlay
  visibility, color-vision preview, and trace verbosity are applied live and are
  *structurally incapable* of perturbing the simulation: the pure
  [`model::simulate`](src/model.rs) takes only a `LaunchManifest`. A test runs the
  same seed twice — once loud with all knobs at one setting, once silent with
  every knob changed — and asserts the stream checksum and step count are
  identical while only the log-line count differs.

## What It Demonstrates

- **Config-backed knobs**: seed, fog, bloom, overlay, color-vision preview, trace
  verbosity, and capture warm-up, all typed `bevy_mod_config` fields with
  defaults, edited live from the keyboard (writes go to the scalar data and bump
  the change generation).
- **JSON persistence**: `F2` saves the config through the Serde JSON manager;
  `F3` reloads it; a fresh launch loads it and promotes the persisted seed into
  the manifest. Path defaults to a temp file (override with `OBSERVED2_CONFIG`),
  so the lab never litters the repo.
- **Config vs. deterministic manifest**: editing the seed is pending until
  committed; debug knobs never touch the sim (proven by tests).
- **Event trace**: a lab-local tracer mirrors semantic events to Bevy's `tracing`
  log, gated by the `trace_verbosity` knob (0 off / 1 key events / 2 all).
  Reading always drains the messages, so disabling logging changes nothing about
  the simulation.
- **The bug-hunting workflow**: the overlay shows a live, colour-coded ring of the
  most recent events with their rooms — readable at a glance, faster than
  scrolling console output — alongside the active manifest, checksum, and every
  knob value.
- **Legibility**: every colour comes from `observed_style`, including the
  color-vision preview transform applied to the map tiles.

## Controls

- `[` / `]`: fog strength down / up
- `;` / `'`: bloom strength down / up
- `V`: cycle the color-vision preview (normal → … → achromatopsia)
- `T`: cycle trace verbosity (0 → 1 → 2)
- `-` / `=`: decrement / increment the candidate seed
- `Enter`: commit the candidate seed into the launch manifest (relaunch)
- `F1`: toggle the overlay   `F2`: save config   `F3`: load config
- `R`: reset the scene (manifest unchanged)

## Success Conditions

1. The overlay shows `[PASS]` (one camera, one UI root, eight room tiles, focused,
   and the live stream checksum equals the pure model's checksum for the active
   manifest).
2. Editing a debug knob (`[`, `]`, `;`, `'`, `V`, `T`) changes the presentation /
   log only; the `stream` checksum and `sim tick` cadence are unaffected.
3. Editing the seed shows `<- PENDING`; pressing `Enter` updates the active seed
   and the checksum, and the pending marker clears.
4. `F2` then relaunching (or a second instance) loads the saved config and adopts
   the persisted seed as the manifest.
5. `R` reset leaves exactly one camera, one UI root, and eight tiles with no
   leaks; config entities (owned by `bevy_mod_config`) survive reset.

## Decision

`bevy_mod_config` is a clean fit for lab/debug knobs and persistence and stays
**lab-local** until a second consumer needs the same typed-config + persistence
shape (per roadmap A9); keep runtime game settings separate from this debug
configuration. `bevy_log_events` is **rejected** for direct integration on
egui-coupling grounds; the lab-local `tracing` tracer covers the capability with
zero added dependencies. Revisit `bevy_log_events` only if a future release drops
the mandatory egui plugin.

## Regenerating The Evidence

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/lab_observability_lab.png"
cargo run -p lab_observability_lab
```
