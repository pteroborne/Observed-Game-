# Cosmetics comparison and loading clarity

Cosmetics now compares the equipped profile with a selected design before saving a
choice. Loading describes the selected match, perspective, phase and next action
in player language. This completes the representative-preview and wording work
from UX-11/12 in the [end-to-end audit](end_to_end_audit.md), following the
[recorded world replay replacement](replay_implementation.md).

## Cosmetics

The screen opens on the equipped color. All ten items can be inspected with the
shared pointer, keyboard and controller widgets, including locked items. Each
locked row states its unlock requirement. The selected item changes only its slot
in the comparison; the other equipped slots remain visible. Previewing never
changes the profile or writes a save.

The separate Equip action becomes available only for an unlocked, unequipped
selection. It uses the existing profile equip/save path. Locked or already-equipped
choices have a disabled Equip control that is skipped by focus navigation. The
screen retains a distinct selected label and the shared focus indicator.

The comparison is a code-drawn eye, trail and badge. Ash, Ember, Cobalt and Void
have shared semantic colorways in `observed_style::cosmetics`; Spark and Comet
have distinct trail shapes, and Rookie, Veteran and Champion have distinct badge
shapes. No Trail draws no trail. The screen names all three slots beside each
preview, so appearance is not explained by hue alone.

These are **representative designs**. The canonical match does not currently
consume cosmetic profile choices, and the screen explicitly says that match
appearance is not applied yet. This slice does not recolor gameplay-critical team
or threat signals, claim an in-match trail, or add new unlocks. Applying cosmetics
to canonical presentation remains separate work.

The preview uses ordinary state-scoped UI entities, with no render target, asset
loading or live simulation dependency. Exiting removes both the comparison tree
and its selection resource. Reentry reads the saved equipped color.

## Loading

The screen shows Local run, Rematch or LAN match; the selected rules; Observer,
Architect, Explorer or Spectator perspective; the actual finalized body count;
and facility floors. Architect Ascent lists its extra Architect per team explicitly.
It no longer shows grid dimensions, worker terminology, request IDs or solve-time
explanations as routine player information.

Progress remains truthful and coarse: preparing rooms/routes, waiting for players,
ready, failed or cancelled. Elapsed time and attempt number remain visible. There
are no percentages. Status and recovery text explain when the player can go back,
retry or leave a LAN host. Typed failures distinguish missing setup, installation
files, host/client file mismatch, an unavailable layout, stopped preparation and a
silent host. Full technical failure details remain in the existing diagnostic logs.

The launch request now snapshots rules and seat alongside its existing immutable
configuration and spectator/context fields. Local, rematch and authoritative LAN
issuers provide those facts; retries retain them. Local canonical entry uses the
same frozen rules/seat even if the menu draft has changed. LAN continues to use its
authoritative launch. Worker ownership, cancellation, stale-result rejection and
LAN start-barrier behavior remain in their existing loading boundary. There is no
wire protocol or persisted profile format change.

## Verification

Focused checks cover inspecting a locked item, rejecting its Equip action, profile
immutability while previewing, saving an unlocked selection, reentry, scene cleanup,
one-slot comparison, finalized loading metadata/retries and actual canonical entry
when the editable menu role changes. Existing async loading and real-UDP tests
continue to cover cancellation, stale completion, server silence and launch readiness.
The complete game suite also exercises button activation outside Cosmetics, so its
screen-scoped selection cannot break Play, Settings, help, Results or Replay.

[Native evidence](../evidence/ux/polish_1280x800/README.md) contains 17 native
1280x800 screenshots, widget/text bounds, validation and source/artifact hashes.
The cosmetics profile and loading phases/errors are staged fixtures using the
production screens. The LAN waiting view has an unserved loopback socket and staged
launch descriptor; it is not evidence of two connected graphical clients.

The final required fmt, clippy and workspace test results are recorded beside the
evidence. Human keyboard/pointer/controller acceptance and two graphical LAN clients
remain open, as does the audit's separate roster compatibility work. The known
ignored asserting `hex_full_match_soak` remains unresolved; this slice does not
claim the extended suite is green.

## Reproduce screenshots

Build serially with the shared cache, following the repository runbook:

```bash
CARGO_TARGET_DIR=/srv/build-cache/cargo cargo build -p observed_game --bin observed --features bevy/dynamic_linking
```

Run the binary from a temporary working directory with a separate temporary config
directory. Set `OBSERVED2_ASSET_ROOT` to this worktree's `assets`,
`OBSERVED2_CONFIG_DIR` to that config directory, `OBSERVED2_CAPTURE_FRONTEND` to the
output directory, and `OBSERVED2_CAPTURE_FRONTEND_POLISH=1`. On Linux the dynamic
build needs `/srv/build-cache/cargo/debug/deps` and the Rust toolchain's
`lib/rustlib/x86_64-unknown-linux-gnu/lib` on `LD_LIBRARY_PATH`. The capture driver
forces 1280x800 at scale factor 1.0 and exits after all readbacks complete.
