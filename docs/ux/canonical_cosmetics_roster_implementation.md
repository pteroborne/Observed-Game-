# Canonical cosmetics and Ascent roster compatibility

Saved cosmetics now dress the canonical floating Observer and survive recorded-world
replay. Architect Ascent permits one-to-three Observer bodies per team across the
Play Hub, advanced setup, saved-setup migration, dedicated/listen hosts and LAN launch
validation. Facility race retains its existing four-body co-op and sixteen-seat cap.

## Cosmetic presentation

The equipped profile supplies stable color, trail and badge IDs. Each canonical
launch freezes those choices by `PlayerId`; subsequent profile changes affect the
next launch. Local bots wear the default look. Gimbals carry Ash, Ember, Cobalt or
Void, while the iris and hover haze retain the shared You/Teammate/Rival signals.
Globe, pupil, geometry, ownership, visibility and simulation behavior retain their
existing meanings. The menu comparison now shows a role-colored iris with the
selected decorative color around it, and explains that designs apply at match start.

Rookie uses one badge bar, Veteran three equal bars and Champion a taller center
bar. Spark has three small trail points; Comet six larger tapered points; No Trail
allocates none. Trails keep at most seven position samples and reuse a fixed set of
entities. They clear across large position jumps, hide with the body being viewed
through, and disappear with the match. Badge geometry and trail shape/spacing helpers
are also used by the recorded world viewer.

`ReplayTape` retains the frozen cosmetic metadata. The viewer reads that recording,
including past positions for trails, rather than the current career or live match.
Cosmetics never enter authoritative commands, gameplay decisions or state digests.
No profile or persisted setup schema changed; their existing equipped IDs remain the
source of truth.

## LAN compatibility and ownership

LAN protocol **16** carries a validated look in Hello and a bounded, seat-ordered
look list in Launch. The host freezes the list when the match starts and repeats the
same list during preparation, late join and reconnect. Empty/bot seats use defaults;
a late joiner's chosen look applies at the next match. Peers reject malformed slot
IDs, mismatched look counts, oversized rosters and Ascent teams above three bodies.
Repeated or stale launch packets cannot change the accepted look or transport
progress. Hosts and clients must be updated together: protocol 15 datagrams fail the
version check.

Cosmetic unlocks remain local progression policy. The server validates catalog-slot
IDs, not the client's earned XP or wins. Cosmetic metadata has no gameplay authority.

Lobby snapshots now expose the host's rules and empty-seat policy. Facility race
disables Architect claims; a teammate's claimed desk disables a competing claim.
The roster shows each stable body seat on its own bounded line, with ready/occupancy
facts and explicit `ARCHITECT + BOT BODY` ownership. No-fill requires all configured
connection seats to have humans; a human at a desk still delegates its body to a bot.

The existing LAN transport model is preserved: **one connection per Observer body**.
Claiming a desk gives that human the separate non-embodied rules seat and makes their
body bot-driven. It does not add another body or another connection. Consequently,
a three-body team with a human Architect can have two human Observers and one bot
Observer. An independently connectable Architect outside the body quota would need
a separate transport/identity slice; this implementation does not claim that capacity.

A real reconnect race was also fixed: a delayed packet could reconnect the reserved
old socket just before a valid resume Hello arrived on a new socket. The host now
accepts that migration when token, account and existing seat all match. The new
socket resumes the original launch metadata; its new look is retained for a future
match.

## Roster selection and migration

Selecting Ascent co-op creates three bodies plus the rules' Architect. Advanced
team-size cycling stays within 1–3 and the existing total body cap. Invalid runtime
setup drafts produce a concrete error rather than launching a different roster.
Saved Ascent co-op/custom setups that previously requested four or more bodies are
normalized to the supported count; saved Facility race counts are preserved.
Switching co-op back to Facility race restores its four-body preset explicitly.
Dedicated hosts reject invalid Ascent body counts before loading facility content.
`members_per_team` continues to mean bodies, never a total including the Architect.

## Verification and evidence

Regression tests cover role iris preservation against the actual dressed mesh
materials, launch/replay snapshot immutability, cleanup/reentry, bounded trails and
teleport clearing, migration, retained Facility race capacity, invalid LAN metadata,
and stale/conflicting descriptors. A real UDP test launches three bodies with two
humans and a claimed desk, compares both peers' cosmetic metadata, then expires and
resumes a connection with a different look while preserving the accepted launch.

[Native evidence](../evidence/ux/canonical_1280x800/README.md) includes menu/roster
screens, a real loopback lobby with one graphical client and a second UDP client,
canonical cosmetic portraits and a recorded-world view. Portrait positions, camera
and movement are staged with simulation paused to expose the small decorative parts.
Replay uses the existing recorded rules/physics fixture with staged look metadata.
These are renderer/layout checks, not human traversal or controller acceptance.
Bounds, checks and source/artifact hashes are recorded alongside the images.

Human keyboard/pointer/controller acceptance, two connected graphical clients and
physical-machine/firewall validation remain open. The known ignored asserting
`hex_full_match_soak` remains unresolved; the extended suite is not claimed green.

To reproduce, build the native game with `bevy/dynamic_linking`, use a temporary
working/config directory and set `OBSERVED2_ASSET_ROOT`, `OBSERVED2_CONFIG_DIR`,
`OBSERVED2_CAPTURE_FRONTEND` and `OBSERVED2_CAPTURE_FRONTEND_FINISH=1`. On Linux set
`LD_LIBRARY_PATH` to the shared cache's `debug/deps` plus the Rust toolchain library
directory, as described in the [previous capture instructions](cosmetics_loading_implementation.md#reproduce-screenshots).
