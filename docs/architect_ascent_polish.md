# Architect Ascent integration

**Partial implementation. The main Play runtime remains `HexWfcMatch`; it does
not yet run the shared Ascent session.**

## Baseline

Branch `codex/architect-ascent-polish` starts from main `0d9eb7ea`, merges
`asymmetry-lab` `02739be8`, and snapshots its uncommitted architect-hand prototype.
The original checkouts were left untouched. The inherited equipment, window,
moonlight, and open-air work is included.

## Implemented

- Promoted the pure Architect lab rules into `observed_match::ascent`; desktop
  and browser lab views re-export those rules rather than maintaining a fork.
- Added a versioned, fixed-tick seat boundary with role authorization, isolated
  loyal Architect hands, shared Rogue authority, corruption transitions, and
  neutral human Observer input that never silently invokes a bot.
- Added read-only card inspection that checks the acting seat's knowledge, hand,
  and cooldown using the same eligibility rules as commit.
- Team knowledge stores last-seen geometry with explicit freshness. A rival's
  observation no longer updates another team's map. Seat snapshots allowlist
  visible information and omit Guardian targeting internals.
- Added bounded, expiring team requests and team-only acknowledgment. These are
  simulation commands; no main-game request UI or network encoding is wired yet.
- Added main-game contextual interaction prompts for implemented nearby actions,
  solo-aware station guidance, synchronization progress, objective/equipment
  readouts, and transient confirmations. Map, pause, onboarding, and spectator
  modes hide this HUD.
- Fixed the main-game adapter dropping the controller's one-shot interact action.
  Keyboard rebinding is reflected in prompt labels; controller actions have text
  equivalents alongside keyboard labels.
- Added persisted gameplay text scaling (90–125%) and reduced hand motion. The
  latter removes hand sway/spin, not world animation.

## Visual evidence

The HUD capture enters through the ordinary asynchronous launch handoff and uses
normal fixed-tick input for interaction and station holding. Staging teleports
only the evidence runner between authored mechanisms.

```bash
OBSERVED2_HEX_FACILITY=28x20x10 \
OBSERVED2_CAPTURE_HEX_WFC_HUD=docs/evidence/ascent-polish \
cargo dev-run -p observed_game
```

This larger fixture requests the objective quotas; compact fixtures may omit
objectives and cannot verify station progress. The production `arc_default`
currently uses 24x17x8, below the older quota activation threshold of 28x20x10;
this work does not silently change its objective-generation policy.

- [Keystone prompt, 1280x800](evidence/ascent-polish/interaction-1280x800.png)
- [Station, 125% text, 1280x800](evidence/ascent-polish/station-large-text-1280x800.png)

## Remaining integration

1. Reserve and generate a physically connected prison core in the authored
   facility. The lab's escape graph currently differs from its placement ports.
   An experiment could project internally stamped maze cells, but that alone
   proved neither reciprocal exterior passages nor room-footprint safety; it
   was not retained as a production adapter.
2. Connect continuous Observer bodies, observation, physical falls, jail/rescue,
   and corruption to the one authoritative Ascent world. Do not run a second
   race simulation beside the rules or convert cell steps into teleporting FPS
   movement. Lab actors still advance on their original beat cadence.
3. Add the dedicated Architect role, mixed ascent/station hand, map placement
   UX, team request UI, power/tool HUD, role transitions, summit/results, and
   controller navigation in the main game. Architect bot seats are not driven
   by the new session yet; existing lab Rogue and Observer bots remain intact.
4. Add authoritative LAN encoding, authenticated seat routing, content identity,
   replay/resync support, and local/remote parity tests for the Ascent session.
   The versioned in-memory input struct is not a transport codec.
5. Prove full playable local and LAN matches, including physical prison rescue,
   lower-floor landings and true-void corruption, before replacing main Play.

The shared-rule extraction and polished current-loop HUD are independently
reviewable foundations, not completion of the full approved plan.
