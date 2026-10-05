# UX audit evidence

Captured on 3 October 2026 from `aede5646`; findings and interpretation live in
[the audit](../../ux/end_to_end_audit.md). Review
[the menu concepts](../../ux/menu_concepts.html) alongside the existing game,
not as implemented game screens. [manifest.json](manifest.json) records each
image's dimensions, checksum, provenance, configured role and inspection status.

The subsequent [menu implementation record](../../ux/menu_implementation.md)
tracks the changed screens and a separate set of implementation captures. These
original audit images remain a record of `aede5646`.

The implemented screens are in `implemented_1280x800/` (32 PNGs plus widget bounds).
Their [separate manifest](implementation_manifest.json) records inspection,
checksums and production-launch versus staged-state provenance;
[implementation checks](implementation_checks.txt) records the code gates.

The subsequent [in-match implementation record](../../ux/in_match_implementation.md)
tracks Observer/spectator guidance, Rogue help and the team map. Its native
captures, bounds and checks have their own [guidance manifest](guidance_manifest.json).

## Runs and retained images

| Run | Setup | Retained evidence | What it establishes |
| --- | --- | --- | --- |
| Legacy baseline | Empty config directory; worktree cwd migrates tracked legacy saves | `frontend_1280x800/00`–`14` (15 images) | Current staged screen layouts, migrated settings collision, rendered pause/map |
| True defaults | Empty config directory **and** empty external cwd | `defaults_1280x800/` (3 images) | Solo/Race default, conflict-free F/T defaults, first onboarding beat |
| Returning Observer | Explicit Solo/Ascent/Observer; help version 1 complete | `ascent_1280x800/observer_*` (2 images) | Ascent setup clipping/copy and a bot-driven scene; no live Observer input proof |
| Returning Architect | Explicit Solo/Ascent/Architect; help version 1 complete | `ascent_1280x800/architect_*` (2 images) | Ascent setup clipping/copy and the rendered desk |
| Concepts | Standalone HTML; browser stage captured at 1280×800 | `concept_*.jpg` (3 images) | Proposed Main Menu and Play layouts with both roles |

All PNGs are **1280×800 physical pixels** and are fresh game renders, not copied
historical evidence. The original game build completed successfully, warning-free.
The existing frontend driver stages its career/result/replay fixtures, so its XP,
run seed and result story do not describe an actual completed audit match.

The baseline uses the repository's tracked legacy saves. A fresh config directory
alone does not bypass those saves. The default run deliberately changes cwd to an
empty external directory. Additional role runs write only isolated audit saves.
No user's normal preferences or profile were used or modified.

The sweep's Loading error (`MissingRequest`) is expected: it enters Loading without
issuing a request. That screenshot does **not** test a failure during a real solve.
The driver jumps into a bot-driven match without Loading and injects the captured
pause/map overlays. First-frame images therefore do **not** establish a human
launch, mouse capture, input parity or an end-to-end successful session.
True-default onboarding is rendered, but no Next/Skip interaction was exercised.
Only the first of four beats is photographed. The observer capture intentionally
uses the sweep's bot adapter, which hides the normal player HUD.

## Reproduce the baseline on Linux

Run from the feature worktree. Do not overlap Cargo with another checkout or clean
the shared cache. Check free space before building.

```bash
ux_worktree="$(pwd)"
ux_run="$(mktemp -d /tmp/observed-ux.XXXXXX)"
mkdir -p "$ux_run/legacy-config" "$ux_run/legacy-capture"
CARGO_TARGET_DIR=/srv/build-cache/cargo \
OBSERVED2_CONFIG_DIR="$ux_run/legacy-config" \
OBSERVED2_CAPTURE_FRONTEND="$ux_run/legacy-capture" \
cargo dev-run -p observed_game
```

The driver pins the physical viewport and exits after fifteen captures. Inspect
all screenshots; entering a state is not the same as using its navigation.
Do not overwrite historical Arc Q evidence when rerunning this audit.

For true defaults, reuse that exact built binary from an external empty cwd:

```bash
ux_rustlib="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/lib"
mkdir -p "$ux_run/empty-cwd" "$ux_run/fresh-config" "$ux_run/fresh-capture"
(
  cd "$ux_run/empty-cwd"
  LD_LIBRARY_PATH="/srv/build-cache/cargo/debug/deps:$ux_rustlib" \
  OBSERVED2_ASSET_ROOT="$ux_worktree/assets" \
  OBSERVED2_CONFIG_DIR="$ux_run/fresh-config" \
  OBSERVED2_CAPTURE_FRONTEND="$ux_run/fresh-capture" \
  /srv/build-cache/cargo/debug/observed
)
```

For either saved Ascent role, use an isolated config directory containing these
files before running the same binary/capture command. Replace `observer` with
`architect` for the second run. Unspecified JSON fields use existing defaults.

`settings.json`:

```json
{"completed_onboarding_version": 1, "first_run": false}
```

`play_setup.json`:

```json
{"preset": "solo", "rules": "ascent", "seat": "observer"}
```

This test intentionally retains the current serialized shape and defaults. It does
not simulate returning through the menus or prove save writes on user selection.

## Checks performed

Existing tests were run serially with `CARGO_TARGET_DIR=/srv/build-cache/cargo`.
These support source findings; they do not close human UX gates.

| Command | Result |
| --- | --- |
| `cargo test -p observed_game --lib --features bevy/dynamic_linking hex_wfc::loading::tests` | 12 passed; 1 existing ignored production-size measurement |
| `cargo test -p observed_game --lib --features bevy/dynamic_linking hex_wfc::overlay::tests` | 5 passed |
| `cargo test -p observed_game --lib --features bevy/dynamic_linking screens::` | 55 passed |
| `cargo test -p observed_game --lib --features bevy/dynamic_linking play_setup::tests` | 8 passed |
| `cargo test -p observed_server --lib authoritative_tick_one_waits_for_every_connected_human` | 1 passed; server plus two real UDP clients in one test process |
| `cargo test -p observed_ui --lib --features bevy/dynamic_linking` | 4 passed |
| `cargo test -p observed_game --lib --features bevy/dynamic_linking settings::tests` | 17 passed |

Compact test results are retained in [checks.txt](checks.txt). Full workspace Cargo
gates were not run: this milestone adds documentation, screenshots and an HTML review
artifact without changing production code or configuration.

The HTML concept was checked in the browser for Enter entry, Escape/Back, both role
selections, each preset, live summary updates, disabled Spectate roles, and remembered
Spectate focus. These are concept interactions, not native game checks. For clean
concept screenshots, open `menu_concepts.html?capture=main` (or `?capture=play`) at
1280×800; the capture view hides surrounding review notes. The default URL includes
those notes. Three JPEGs record the complete stage.

The managed worktree creation failed with no space on `/home`. The feature checkout
was created using ordinary Git on BigFastDrive; Codex app attachment rejected it
because it is not a managed checkout. The primary checkout was left unchanged.

## Completion implementation evidence (2026-10-05)

[Results and replay implementation](../../ux/completion_implementation.md) records
this separate 18-screen native set. See [completion checks](completion_checks.txt),
[manifest](completion_manifest.json) and [1280x800 captures](completion_1280x800/).
Outcomes are synthetic fixtures over a prepared hex room trace; LAN captures stage
client presence without a server handshake. They are layout/wording evidence,
not a completed human match or connected multiplayer acceptance.

## Recorded world replay evidence (2026-10-05)

[Replay replacement](../../ux/replay_implementation.md) documents the canonical
world viewer. The [native evidence set](replay_1280x800/README.md) includes nine
1280x800 views, bounds sidecars, a playback video and provenance. The fixture runs
real rules and physics with explicitly staged card/catch/corruption events;
it does not establish completed human or graphical LAN acceptance.
