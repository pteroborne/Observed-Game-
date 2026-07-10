# Arc H — Ground Truth (harden & ship the MVP)

Planned 2026-07-10, after Arc F ([sight_and_sound_arc_plan.md](sight_and_sound_arc_plan.md))
landed all seven phases. The Arc F review found the *code* claims hold (189 green
suites, clippy clean, tested pure functions everywhere they were promised) but the
*game* does not yet match its claims: the imported textures visually overpowered the
neon-noir style layer, two evidence captures are missing or show nothing, four
playtest defects sit in the bug backlog, Phase 50's post-match summary never landed,
and none of it is committed. This arc closes that gap. **The MVP ships when what the
player sees, hears, and reads matches what the docs and tests claim.**

## The thesis

Arc F proved a lesson about agent-built phases: success criteria that can be
satisfied by tests alone will be satisfied by tests alone. `oga_25d_rivals.png` shows
an empty floor; the visual audit passed while the district palette disappeared under
a beige albedo. Ground Truth therefore adds a new standing rule: **every phase in
this arc ends with evidence a human can falsify** — a capture that visibly shows the
claimed behavior, checked by the parent session against the phase doc before commit.

## Design principles

1. **The Legibility Contract and comfort language are the arbiters.** The
   style-reconciliation phase does not "tune" textures — it restores the rule that
   `observed_style` owns what the player sees; albedo is an input the palette tints.
2. **Every fix lands with a regression gate.** The threshold bug extends the map
   audit; the style regression extends the visual audit; the rebind bug gets a
   round-trip test. A bug fixed without a gate is a bug scheduled to return.
3. **Commit discipline returns.** Phase 61 lands the entire Arc F working tree in
   reviewed commits before anything else changes; every later phase lands as its own
   commit. No more multi-week uncommitted trees.
4. **No new features.** World interaction and LAN stay on the Post-MVP Backlog;
   this arc only makes existing promises true. The one scope-flavored deliverable
   (the post-match summary) is Arc E Phase 50's remaining half, not new scope.
5. **Sub-agent structure carries over from Arc F:** self-contained phase docs in
   [arc_h/](arc_h/README.md), file-disjoint waves, parent-session review gates —
   plus the new falsifiable-evidence rule above.

## Stages

- **Phase 61 — Land Arc F (commits, ledger, honest evidence).** Stage the working
  tree into logical reviewed commits; tick ROADMAP and add Recent-Milestones
  entries; append the missing as-landed notes (54, 55, 58, 59, 60 — including the
  reticle ruling); update memory; re-capture the evidence that is missing or shows
  nothing (rivals with rivals in frame, dressing before/after).
- **Phase 62 — Style Reconciliation.** Textures go back under the Contract:
  district palette tint modulates every albedo, UVs scale in world units (fixes
  stretching), the triangulated ceiling-tile geometry is removed or replaced with a
  flat treated ceiling, and the visual audit gains a **style-presence check** so an
  albedo overpowering the palette can never pass silently again. Subsumes bug
  backlog #2.
- **Phase 63 — Control Rebind, For Real.** Replace the custom rebind capture with
  the proven `control_lab` overlay machinery; the capture ignores its own
  activation press; round-trip tests through `player_input`. Bug backlog #1.
- **Phase 64 — Threshold Geometry Integrity.** Reproduce and fix hallway thresholds
  overlapping walls or landing in corners; the map audit learns "no threshold may
  intersect an interior wall or another threshold" as a permanent gate across the
  seed corpus. Bug backlog #3.
- **Phase 65 — Observation Rooms Made Real.** The tether/guardian observation
  panels become a legible per-room feed (schematic "camera view" consistent with
  the fog-of-war rules) instead of a number, a jutting object, and a blue screen.
  Bug backlog #4.
- **Phase 66 — Post-Match Summary & the Ship Gate.** The results screen reads the
  run's story (placement, escapes, what the collapse took) with a clean path back
  into the loop — closing Arc E Phase 50 — then the arc ends with a scripted
  full-loop evidence set plus the backlog hygiene items (`OBSERVED2_BOTS` panic →
  warning, the all-on digest characterization test), and a human playtest checklist
  the user runs before the MVP is declared shipped.

**Waves:** [61] → [62 ∥ 63 ∥ 64] → [65 ∥ 66]. 62 owns the place renderer's
shell/materials, 63 owns settings/pause, 64 owns teleport geometry — disjoint. 65
owns `place/monitors.rs`, 66 owns menu/results — disjoint.

## Horizon (unchanged)

The Post-MVP Backlog (ROADMAP.md) still holds: true LAN multiplayer with dedicated
servers, world interaction (explorer room-hacking, fallen-team top-down node
connecting), and the carried Arc C/D follow-ups. The next arc after Ground Truth
draws from there — with an actually-shipped MVP underneath it.
