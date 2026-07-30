# Phase 123 — 1280×800 Human UX Gate

Status: **machine half complete, hands-on half open.**

Phase 123 is a human gate. This note records what the automated pass established, the
defects it found and fixed, and exactly what a person still has to do at the keyboard
before the phase can be ticked.

## The capture harness

`OBSERVED2_CAPTURE_FRONTEND=<dir>` (see
[`game/src/evidence/capture/frontend.rs`](../../game/src/evidence/capture/frontend.rs))
pins the primary window to the baseline viewport and photographs the frontend states in
contract order:

```bash
OBSERVED2_CAPTURE_FRONTEND="docs/evidence/arc_q/frontend_1280x800" cargo run -p observed_game
```

The driver refuses to photograph anything until the surface actually reports 1280×800.
The first attempt at this silently produced 2558×1368 images: the window opens maximized,
so a logical `WindowResolution::set` is ignored, and on a high-DPI display logical pixels
are not the pixels a screenshot records. It now clears the maximized state, pins the
scale factor to 1.0, sets the physical resolution, and re-asserts until the surface
agrees.

Captures live in [`docs/evidence/arc_q/frontend_1280x800/`](../evidence/arc_q/frontend_1280x800/).

## What the captures established

At 1280×800, with no clipping, overlap, or horizontal scrolling:

| Screen | Reads |
| --- | --- |
| Main Menu, Play hub, Advanced setup | fit with margin |
| Settings — Preferences | 8 targets, fits |
| Settings — Controls | 15 bindings in 2 columns + Pause + Back, ends ~730 px |
| Loadout | locked cosmetics dimmed, each stating its unlock condition |
| LAN Browser | disabled paging and Join each stating *why* they are disabled |
| Lobby | roster panel, legend, hierarchical Back |
| Loading | honest failure path: no percentage, explicit error, real Retry/Back |
| Results | finalized roster and spectator perspective, not legacy profile flags |
| Replay | states "focus unavailable" rather than inventing a subject |

Disabled controls are legible and explain themselves, and focus is carried by a `>`
chevron plus an outline, so neither depends on colour.

## Defects the gate found

**1. The shipped font renders only ASCII, so every non-ASCII glyph was a blank box.**
The game ships no font asset, so labels are drawn with Bevy's embedded default — a subset
with no geometric shapes, dashes, bullets, or degree sign. At the baseline this made:

- the Play hub's `◆`/`◇` preset markers invisible, so **which preset was selected could
  not be read at all** — the most serious of the three, because selection is the one
  thing the preset-first hub exists to communicate;
- the Lobby seat legend read `HUMAN ▯ BOT ▯ RESERVED ▯ PREPARING ▯ EMPTY`;
- the field-of-view row read `Field of view: 80▯`;
- the Results subtitle read `4 teams ▯ 1`.

Fixed by spelling rendered strings in ASCII (`[*]`/`[ ]` for selection, `|` separators,
` deg`, `x`). `arch_check::rendered_ui_strings_stay_within_the_shipped_font` now fails the
build on any non-ASCII character in a non-test string literal.

**2. The onboarding gate silently stopped the simulation in every Bevy test fixture.**
`test_app` inherited real user preferences through `load_settings`, so whether onboarding
ran — and therefore whether `simulation_policy` returned `Stop` — depended on the
developer's own save file. `hex_headless_and_interactive_agree` was reporting a
determinism divergence that was really the interactive side never advancing past tick 0.
Fixtures now insert a hermetic `Settings` with onboarding complete.

**3. The interactive determinism gate drove bots with a superseded API.** The headless
fork used `bot_command` (intent only) while production uses `bot_player_command`
(objective-aware, and it presses the action buttons a keystone/station beat needs). The
fork now mirrors `step_runtime` exactly.

**4. Four hex modules had grown past the 600-line review ratchet** (`view/mod.rs` at 876).
Test modules moved to sibling `_tests.rs` files, and the bounded-residency systems moved
to [`view/residency.rs`](../../game/src/hex_wfc/view/residency.rs) — a real Phase 122
boundary rather than an arbitrary cut.

**5. The Controls-page layout assertion was a stale magic number** (`rows <= 7` against an
actual 9). It now asserts the height the page needs against the 800 px baseline, so
adding a binding fails the test instead of silently clipping.

## Still required before Phase 123 can be ticked

The automated pass cannot substitute for these:

- [ ] Traverse every reachable screen with **keyboard only**, then repeat the primary
      route with **pointer** and with a **controller**. Confirm Back never escapes a
      modal or an input capture unexpectedly.
- [ ] Confirm disabled controls cannot *receive focus or activate* — the captures show
      they are legible, not that they are unreachable.
- [ ] Launch all four presets and confirm the summary, roster, spectator perspective, and
      result copy describe the match that actually ran.
- [ ] Cancel and retry a **real** local preparation. The captured Loading screen is the
      no-request error path, because the driver jumps straight to the state; the normal
      progress view and a genuine cancel/retry are unverified.
- [ ] Enter offline and LAN pause; confirm offline stops the tick while LAN continues
      neutrally, and that Leave requires the explicit confirmation page.
- [ ] Onboarding, pause, and the first playable frame at 1280×800 — these are in-match
      overlays and are not part of the frontend sweep.
- [ ] **Two real LAN processes** with one client's preparation deliberately delayed.

On that last item: the protocol guarantee is already proven over real UDP sockets in
[`server/src/lib.rs`](../../server/src/lib.rs)
(`authoritative_tick_one_waits_for_every_connected_human` holds the server at tick 0 for
16 ticks while one of two connected clients withholds `mark_launch_ready`). What remains
is the same exercise between two real game processes.
