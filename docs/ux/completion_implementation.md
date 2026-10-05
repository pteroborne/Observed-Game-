# Results, replay and playing again

The Results/replay portion of Slice 4 from the [UX audit](end_to_end_audit.md)
is implemented on `codex/end-to-end-ux`, following the
[in-match guidance milestone](in_match_implementation.md).

## Behavior

Ascent results retain the rules' outcome, summit team, local team, role and final
Observer counts before the match is removed. Observer and Architect results name
the team and summit condition. A corrupted Observer receives the Rogue faction's
outcome; its former team's summit victory does not count as that player's victory.
An Architect remains loyal when a bot Observer falls. Spectate describes the bots'
outcome without claiming a player victory. Rogue capture and the alternate Darkness
objective have different explanations. An unfinished outcome never invents a winner.

The legacy career envelope remains for progression. Its first-place flag now follows
the player's actual faction win: a winning Rogue receives first place; a corrupted
Observer whose former team wins does not. This uses the existing progression award
and does not change XP amounts, unlocks, simulation rules or the LAN protocol.

Replay retains its own Ascent context, read independently of live match resources.
Its samples distinguish loyal, jailed, corrupted and summit Observers. Prison and
void coordinates do not place a marker in a facility room. Ascent room markers use
neutral styling rather than race objective colors. Rooms have outlines; actor markers
are solid so focus remains distinct. Architect and Spectate label
the followed body as a bot. The screen calls its timeline ticks and facility
revisions, and explains the format's limit: a recorded room trace, without reconstructed
card plays, prison layouts or facility rewrites. The existing race presentation remains
available for race tapes. Replay does not simulate or mutate the completed match.

Absent and empty tapes disable Watch replay. Local **Play again** restores the frozen
launch choices, including rules, starting seat, roster, Guardian and Spectate; a Rogue
starts the new run in the original Observer seat. It chooses a new seed and uses the
existing Loading handoff. LAN **Next match** and both screens' **Return to lobby**
actions return to the lobby, leaving the next launch to the host. Architect desk and
Ask resources are now removed on match exit alongside the runtime.

## Validation and evidence

[Completion checks](../evidence/ux/completion_checks.txt) and the
[manifest](../evidence/ux/completion_manifest.json) record the final gates and capture
provenance. The screenshot sweep stages synthetic completed outcomes over a real
prepared hex room trace. Its LAN pictures stage a local UDP client object; they do
not establish a connected server or another game process. Screenshots establish
rendered layout and wording, not a completed human session.

The assembled-app tests stage jail, void and standable summit positions through the
existing physical evidence seams, then let the production Ascent rules decide the
outcome. They exercise Observer, Architect, corrupted and Spectate perspectives,
including an Architect's bot falling, final-fact preservation after cleanup, unavailable
replay, immutable replay navigation, once-only rewards on returning from replay,
distinct identities in four-seat co-op, restoration of launched settings, and LAN return routing. The LAN routing test uses
a loopback UDP client without a server handshake; connected continuity remains open.

| Evidence | What it demonstrates |
| --- | --- |
| [Observer summit](../evidence/ux/completion_1280x800/00_observer_summit.png), [other team wins](../evidence/ux/completion_1280x800/01_observer_other_team.png) | Loyal win and loss with the actual summit condition |
| [Architect summit](../evidence/ux/completion_1280x800/02_architect_summit.png), [Rogue wins](../evidence/ux/completion_1280x800/03_architect_rogue_loss.png) | Architect role and loyal faction outcome |
| [Rogue victory](../evidence/ux/completion_1280x800/04_rogue_victory.png), [summit loss](../evidence/ux/completion_1280x800/05_rogue_summit_loss.png), [Darkness](../evidence/ux/completion_1280x800/08_rogue_darkness.png) | Faction change and distinct completion reasons |
| [Spectator summit](../evidence/ux/completion_1280x800/06_spectator_summit.png), [Rogue outcome](../evidence/ux/completion_1280x800/07_spectator_rogue.png) | Observation outcome instead of a player victory |
| [No tape](../evidence/ux/completion_1280x800/09_replay_absent.png), [empty tape](../evidence/ux/completion_1280x800/10_replay_empty.png) | Disabled replay with an explanation |
| [Observer replay](../evidence/ux/completion_1280x800/11_replay_observer.png), [Architect](../evidence/ux/completion_1280x800/12_replay_architect.png), [Rogue](../evidence/ux/completion_1280x800/13_replay_rogue.png), [Spectate](../evidence/ux/completion_1280x800/14_replay_spectator.png) | Stored role/outcome, correct actor identity and stated trace limitations |
| [LAN results](../evidence/ux/completion_1280x800/15_lan_results.png), [LAN replay](../evidence/ux/completion_1280x800/16_lan_replay.png) | Lobby destinations; synthetic client presence only |
| [Legacy race results](../evidence/ux/completion_1280x800/17_race_results.png) | Existing race fixture remains supported |

Reproduce from the feature checkout, with no other Cargo worktree using the shared
cache:

```bash
ux_tree="$(pwd)"
ux_run="$(mktemp -d /tmp/observed-completion.XXXXXX)"
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
  OBSERVED2_CAPTURE_FRONTEND_COMPLETION=1 \
  /srv/build-cache/cargo/debug/observed
)
```

## Remaining acceptance

The audit's hands-on matrix remains open: actual human summit/Rogue completion,
physical controller navigation, complete keyboard/pointer sessions, cancellation
and retry, and two real LAN game processes. The automated completion tests and
synthetic screenshots do not close those gates. Slice 3's connected LAN continuity,
role contention and roster compatibility remain. Cosmetics previews and loading
wording are the remaining Slice 4 implementation work. The known extended-suite
`hex_full_match_soak` failure is unchanged; the extended suite is not claimed green.
