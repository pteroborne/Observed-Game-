# Menu and role-entry implementation

The menu milestone from the [end-to-end audit](end_to_end_audit.md) is implemented
on `codex/end-to-end-ux`. This record supplements the historical audit and captures.

## Behavior

- Main Menu explains Observer/Architect cooperation, initially focuses Play, and
  routes Cosmetics, Settings and explicit Quit through the existing actions.
  Profile progress remains in Cosmetics. Decorative facility geometry uses UI
  nodes and the shared theme; it has no focus targets or new asset dependency.
- Play presents direct rules and role choices beside an actual local-run summary.
  Start and Back fit alongside the presets. Ascent uses “Team competition”;
  Facility race keeps “Team race”. Spectate explains its bot viewpoint and disables
  role controls while remembering the last playable role. Advanced preserves the
  rules/role and describes local versus hosted rosters separately.
- Fresh setup defaults to Solo/Ascent/Observer. Explicit saved Race and roles remain
  intact; missing rules in older setup JSON still means Race. Preset body counts,
  bot behavior, the sixteen-body limit and the setup save schema are unchanged.
  Summaries count Observer bodies and dedicated Architect desks separately.
- LAN discovery/hosting remains on the existing route. Copy explains that joining
  adopts host rules and that team/Architect claims happen in the server's lobby.
- Ascent Observer help teaches the summit, Ask, protection and recovery. Architect
  help teaches the map, card aim/confirmation, requests and team goal. Race retains
  its existing lesson. Copy follows actual bindings and implemented controls.
  Completion is recorded independently for the two Ascent roles. Old race
  completion does not suppress new role help. Pause offers Review role help.
  Offline help freezes play; online help retains the neutral-input policy.
  Scoped widget capture permits keyboard/controller help actions and blocks the
  underlying screen through dismissal; exclusive rebind capture remains intact.
  A dedicated final UI camera makes help and pause visible above the Architect's
  opaque board camera and is removed on match exit.
- Legacy saves with an Anchor key but no Ask binding assign Ask an unused key,
  preferring T then Y. Explicit bindings remain intact, including explicit conflicts
  that the existing Controls warning exposes. The hand reserves room for multiline
  card detail so Station text clears the control strip.

## Validation

The menu capture mode covers every shipped preset under both Ascent roles and
Facility race, advanced setup, every help beat, the Architect desk and pause.
Each role enters through the production Play action and asynchronous Loading
handoff. Captures use isolated fresh preferences and 1280×800 physical pixels.
Per-shot widget bounds accompany the images for checking clipping and focus margin.

The [implementation manifest](../evidence/ux/implementation_manifest.json) records
32 inspected images, checksums and capture provenance. All images are 1280×800;
all visible semantic widget rectangles leave at least four pixels at the viewport
edge for focus treatment. Geometry checks supplement visual inspection of text,
overlap and camera layering. The original audit captures are retained separately.

| Evidence | What was checked |
| --- | --- |
| [Main Menu](../evidence/ux/implemented_1280x800/00_main_menu.png), [Cosmetics](../evidence/ux/implemented_1280x800/00b_cosmetics.png) | Premise, initial Play focus, renamed route and retained profile progress |
| [Observer Solo](../evidence/ux/implemented_1280x800/01_observer_solo.png), [Architect teams](../evidence/ux/implemented_1280x800/07_architect_teams.png), [Spectate](../evidence/ux/implemented_1280x800/04_observer_spectate.png), [Race](../evidence/ux/implemented_1280x800/09_race_solo.png) | Direct rules/roles, actual local roster, disabled roles, goal and Start within the viewport |
| [Advanced](../evidence/ux/implemented_1280x800/13_advanced.png) | Maximum configured roster and local versus LAN explanation |
| [Observer help](../evidence/ux/implemented_1280x800/16_observer_help_2.png), [Architect help](../evidence/ux/implemented_1280x800/21_architect_help_2.png), [Race help](../evidence/ux/implemented_1280x800/28_race_help_4.png) | All four pages for each introduction inspected after actual prepared entry |
| [Architect desk](../evidence/ux/implemented_1280x800/23b_architect_desk.png), [Architect pause](../evidence/ux/implemented_1280x800/23c_architect_pause.png) | Station detail clears controls; help/pause render over the board |

Reproduce after the repository gates, with no other Cargo worktree using the cache:

```bash
ux_tree="$(pwd)"
ux_run="$(mktemp -d /tmp/observed-menu-implementation.XXXXXX)"
mkdir -p "$ux_run/cwd" "$ux_run/config" "$ux_run/capture"
CARGO_TARGET_DIR=/srv/build-cache/cargo cargo build -p observed_game --features bevy/dynamic_linking
ux_rustlib="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/lib"
(
  cd "$ux_run/cwd"
  LD_LIBRARY_PATH="/srv/build-cache/cargo/debug/deps:$ux_rustlib" \
  OBSERVED2_ASSET_ROOT="$ux_tree/assets" \
  OBSERVED2_CONFIG_DIR="$ux_run/config" \
  OBSERVED2_SEED=0xF011FAC11177 \
  OBSERVED2_CAPTURE_FRONTEND="$ux_run/capture" \
  OBSERVED2_CAPTURE_FRONTEND_MENUS=1 \
  /srv/build-cache/cargo/debug/observed
)
```

Automated regressions cover keyboard completion of all help pages, direct selection,
disabled-role activation and focus
eligibility, remembered role, finalized local no-fill roster and Spectate viewpoint,
old/new preference migration, independent help completion, reopening and repeated
state cleanup. The integration test also verifies that offline help holds the
simulation tick, targets a camera above the Architect board, and removes that
camera and input capture on exit. See [implementation checks](../evidence/ux/implementation_checks.txt)
for the final repository gate results.

## Remaining audit work

The ordered backlog still includes the loading cancellation/retry hands-on gate,
real controller use, live Observer/spectator play, map orientation/floor labels,
connected LAN continuity/role contention, roster compatibility, mode/faction-aware
results and replay/rematch, cosmetics previews and loading wording. Automated
launches and captured views do not establish physical input usability or an actual
completed human Ascent session. The historical human acceptance matrix remains
open for those scenarios.
