# Arc Q — Clear Intent

Arc Q replaces the assembled game's prototype menu collection with one coherent
front end for the canonical hex facility. It makes every choice explicit, every
wait truthful, and every interaction usable with pointer, keyboard, or controller.
The deprecated isolated-Place match and demoted square-WFC fixture remain regression
surfaces rather than player-facing destinations.

## Design contract

- **One control means one action.** Screens own small typed actions. There is no
  global menu enum, numeric cursor, or dispatcher that must learn every feature.
- **Focus is semantic and visible.** Stable widget IDs, explicit order, remembered
  focus, a shape marker, and an outline serve pointer, keyboard, and controller
  through the same activation path. Disabled choices remain readable and explain why.
- **The common path is shallow.** Main Menu opens a preset-first Play hub. Solo,
  Co-op, Team race, and Spectate take one selection plus an explicit Start; Custom
  rules live one level deeper.
- **Preferences are not match rules.** Audio, display, controls, and onboarding are
  per-user preferences. Team shape, bot fill, and Guardian pressure are a separate
  persisted play-setup draft.
- **Transitions tell the truth.** Loading reports coarse work and elapsed time,
  never a fabricated percentage. Retry and cancel are real operations. LAN clients
  wait for a matching server start after every connected human is prepared.
- **Pause has explicit policy.** Offline pause stops authoritative ticks. Online
  pause and the survivor map send neutral input while the server continues. Leaving
  a run requires confirmation.
- **The first frame is bounded.** Preparation constructs the deterministic match off
  the main thread, then presentation admits nearby cells under a per-frame budget and
  removes distant cells with hysteresis. A production facility is never spawned in
  one blocking frame.
- **The baseline viewport is 1280×800.** Dense screens use paging, columns, and
  compact summaries rather than relying on clipping or tiny text.

## Player-facing hierarchy

| From | Primary choice | Destination / behavior |
| --- | --- | --- |
| Main Menu | Play | Preset hub |
| Play | Solo / Co-op / Team race / Spectate | Prepared canonical match |
| Play | Advanced | Custom roster, bot fill, and Guardian rules |
| Play | LAN | Discovery/direct-address browser, then authoritative lobby |
| Main Menu | Loadout | Local cosmetic selection with locked choices explained |
| Main Menu | Settings | Preferences page, then Controls page |
| Match | Pause | Resume, Preferences, Controls, or confirmed leave |
| Results | Rematch / Replay / Return | New prepared seed, available replay, or prior hub |

`Escape` and controller East activate the active focus scope's semantic Back action
when that scope defines one; the root menu deliberately requires explicit Quit.
Tab, arrows, D-pad, and a latched left stick move through the same explicit order;
Enter, Space, controller South, and pointer click activate the same widget observer.

## As-landed architecture

`game/src/screens/widgets/` is the shared interaction and presentation layer. A
screen contributes `WidgetId`, `FocusScope`, ordering, label, enabled state, and its
own action component. The widget layer owns focus restoration, input parity,
accessibility metadata, non-colour focus treatment, and frontend feedback sounds.
An architecture ratchet rejects a return of the former global menu action/cursor.
`labs/menu_lab` remains the small resettable lifecycle and input-parity proof.

`game/src/play_setup.rs` owns `PlaySetupDraft`, shipped presets, roster validation,
and the actual launched-session description used by Results. `game/src/settings.rs`
owns versioned per-user preferences, binding conflicts, onboarding completion, safe
normalization, and migration from the former workspace-local save files. Runtime
files resolve to the platform user configuration directory (or an explicit
`OBSERVED2_CONFIG_DIR`) and are ignored by source control.

`game/src/hex_wfc/loading.rs` owns immutable launch requests, monotonic request IDs,
async preparation, stale-result rejection, retry, cancellation, and the one-shot
prepared hand-off consumed by simulation setup. LAN adds a reliable ready/start
barrier around the same request: the server cannot tick early and a client cannot
enter a different generation. `game/src/hex_wfc/view/` separately owns bounded
presentation residency, so simulation size does not dictate a blocking render spawn.

`game/src/hex_wfc/overlay.rs` and `game/src/screens/onboarding.rs` are canonical-match
overlays with higher-priority focus scopes. Onboarding is binding-aware and versioned;
pause exposes its online/offline simulation rule. Results and replay retain run facts,
but never pretend an unavailable replay or a bot-controlled spectator team is “you.”

## Phase hand-offs

- Phase 119 — Semantic Widget Foundation `[x]`
- Phase 120 — Preset-First Front End & Preferences `[x]`
- Phase 121 — Honest Loading, Pause & Onboarding `[x]`
- Phase 122 — LAN Barrier, Secondary Screens & Bounded Residency `[x]`
- Phase 123 — 1280×800 Human UX Gate `[ ]`
  ([phase_123_human_ux_gate.md](phase_123_human_ux_gate.md)) — the automated half is
  done: a baseline capture harness, eleven inspected screens, and five defects fixed
  (ASCII-only font, onboarding stopping the tick in test, a superseded bot driver in the
  determinism gate, four modules over the review ratchet, a stale layout bound). The
  hands-on traversal, a real cancel/retry, the in-match overlays, and the two-process LAN
  barrier remain.

## Phase 123 checklist

- At 1280×800, inspect Main Menu, Play, Advanced, both Settings pages, Loadout,
  LAN Browser, Lobby, Loading, onboarding, Pause, Results, and Replay.
- Traverse every reachable screen using only keyboard; repeat the primary route with
  pointer and controller. Confirm Back never escapes a modal or capture unexpectedly.
- Verify the focus chevron and outline remain visible without relying on colour, and
  disabled controls cannot receive focus or activation.
- Launch all four presets and confirm the summary, roster, spectator perspective, and
  result copy describe the match that actually ran.
- Cancel and retry a local preparation; cancel a LAN preparation; confirm no stale
  worker or old launch generation can enter the match afterward.
- With two real LAN processes, delay one client's preparation and verify the server
  remains at tick zero until every connected human reports ready.
- Enter offline and LAN pause. Confirm offline simulation stops while LAN continues
  neutrally, and that Leave requires the explicit confirmation page.
- Record screenshots of the densest screens and the first playable frame; treat any
  clipping, overlap, unreadable disabled state, or unexplained wait as a failed gate.
