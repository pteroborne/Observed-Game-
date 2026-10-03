# In-match guidance and team map

The Observer/spectator guidance and map portion of Slice 3 from the
[end-to-end audit](end_to_end_audit.md) is implemented on `codex/end-to-end-ux`,
following the [menu and role-entry milestone](menu_implementation.md).

## Behavior

- The Observer HUD adds a contextual next step beside the objective. Prison escape,
  teammate rescue, corruption and completed runs take priority over power or kinetic
  charge. A dark floor names the generator; an empty tool names a powered station.
  Race keeps its own objective progression. The displayed floor remains one-based.
- The Ask panel follows the measured objective height, including at 125% gameplay
  text size. An acknowledgement is labelled **Acknowledged**; it does not certify
  that the requested work is complete. The panel and its input yield to help, menus,
  spectators and the Rogue desk. Spectators do not hear an acknowledgement meant
  for the followed bot.
- Spectator chrome names the followed Observer, team, floor, objective and current
  camera mode. Follow, overview, eyes, rotation and zoom controls are visible for
  keyboard and controller. They yield to maps, pause and captured UI input. Selecting
  eyes from overview returns directly to eyes. The team's map uses the followed
  body's discoveries. Opening it keeps that body's normal bot driver running;
  offline pause still stops the match.
- The team map starts with the objective, current position, discovered floor list
  and essential controls. H or controller west toggles the fuller reading guide.
  Geometry occupies the right half of the measured viewport, leaving room for both
  guide sizes. Labels match the HUD's one-based floors. Font and guide updates do
  not rebuild meshes; discovery, position, occupancy and viewport changes do. The
  map camera matches the facility camera's `Msaa::Off` setting; mismatched sample
  counts produced intermittent blank geometry in the native sweep. Both final
  sweeps retain compact and expanded map geometry.
- The map continues to render only team discoveries. Its Ascent stability language
  follows fixed structure, climbs and known anchors, rather than treating all rooms
  as permanent or occupancy as retraction protection. Global observations and rival
  positions remain outside this projection. Prison coordinates no longer produce a
  facility-position arrow; the map explains that it is a facility sketch, not the
  prison maze.
- Corruption opens the human Rogue desk with a persistent faction/goal message,
  including when a play is refused. Following a corrupted bot keeps a spectator in
  spectator mode. Review role help describes the current Rogue or spectator role,
  with four beats each. These reviews use the existing modal widgets and camera.
  Their completion adds no saved preference fields. Reopened help reads online
  continuity from the active runtime, including after Loading metadata is removed.

The simulation rules, canonical rosters, LAN protocol and progression model are
unchanged. The spectator fix changes which existing bot command the adapter sends
while its map is open.

## Validation and evidence

Final repository gate and capture results are recorded in
[guidance checks](../evidence/ux/guidance_checks.txt) and the
[guidance manifest](../evidence/ux/guidance_manifest.json).

The native game sweep uses production Play/Start and asynchronous Loading for each
run. It stages power, charge, teammate jail, local jail and true void through the
existing rules/evidence seams. Camera and map-guide edges queue normal Bevy keyboard
messages for the production input systems. Review role help and all four pages use
production semantic actions. The driver holds each screen until Bevy reports that
its asynchronous screenshot is captured. It records physical 1280x800 PNGs, widget
bounds and visible UI text bounds; the bounds are sampled at the capture request.

| Evidence | What it demonstrates |
| --- | --- |
| [Observer at 125%](../evidence/ux/guidance_1280x800/03_observer_hud_large.png) | Objective guidance, equipment copy, measured Ask placement and interaction prompt |
| [Power](../evidence/ux/guidance_1280x800/04_power_lost.png), [empty tool](../evidence/ux/guidance_1280x800/05_tool_empty.png) | Contextual recovery instructions |
| [Rescue](../evidence/ux/guidance_1280x800/06_teammate_rescue.png), [prison](../evidence/ux/guidance_1280x800/07_prison_hud.png) | Correct objective priority and Ask affordance |
| [Prison map](../evidence/ux/guidance_1280x800/08_prison_map.png), [expanded guide at 125%](../evidence/ux/guidance_1280x800/09_prison_guide.png) | Facility/maze distinction, no false position arrow, readable guide beside geometry |
| [Rogue desk](../evidence/ux/guidance_1280x800/12_rogue_board.png), [Rogue help](../evidence/ux/guidance_1280x800/12c_rogue_review.png) | Faction change, goal and current-role review over the board |
| [Chase](../evidence/ux/guidance_1280x800/15_spectator_chase.png), [overview](../evidence/ux/guidance_1280x800/16_spectator_overview.png), [eyes](../evidence/ux/guidance_1280x800/17_spectator_eyes.png), [follow](../evidence/ux/guidance_1280x800/18_spectator_next.png) | Followed identity, live objective, current mode and control hints |
| [Spectator map](../evidence/ux/guidance_1280x800/19_spectator_map.png), [guide](../evidence/ux/guidance_1280x800/20_spectator_guide.png), [review](../evidence/ux/guidance_1280x800/22_spectator_review.png) | Discovery remains bounded; optional guide and current-role help |

Assembled-app regressions exercise keyboard/controller follow and view selection,
input ownership behind modals, no spectator Ask or Rogue-desk claim, map-guide
updates without mesh rebuilds, text scaling, reopening and exit cleanup. A 180-tick
comparison verifies that opening a spectator map preserves every body's normal
physical bot run. Pure tests cover objective priority, floor naming, Ascent map
stability and current-role help; existing map privacy/connectivity tests remain.

Reproduce with no other Cargo worktree using the shared cache:

```bash
ux_tree="$(pwd)"
ux_run="$(mktemp -d /tmp/observed-guidance.XXXXXX)"
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
  OBSERVED2_CAPTURE_FRONTEND_GUIDANCE=1 \
  /srv/build-cache/cargo/debug/observed
)
```

## Remaining acceptance

Captures and synthetic controller edges do not establish physical input usability
or a completed human Ascent session. Human movement/aiming, map orientation during
play, prison rescue and Rogue play remain acceptance work. Connected LAN continuity,
role contention and roster compatibility remain in Slice 3. Mode/faction-aware
results, replay/rematch context, cosmetics previews and loading wording remain in
Slice 4; loading cancellation/retry also retains its hands-on gate. The known
extended-suite `hex_full_match_soak` exception is not resolved by this change.
