# Phase 65 — Observation Rooms Made Real

**Objective:** the tether/guardian observation rooms promise a "camera view" of
each room and currently deliver a room number, an object jutting through the
panel, and a blue screen. Make each panel a legible, live, in-fiction feed.
Closes bug backlog #4. Read [README.md](README.md) first.

## Read first

- `game/src/screens/place/monitors.rs` — the 3×3 panel walls for the tether
  camera room and guardian observation room (Phase 31 origins), and what
  currently feeds them (district accent materials + state colors).
- ROADMAP Phase 31 + Arc D "living monitors" notes — the intended read:
  tether room = cyan when that room has an active anchor; guardian room =
  red flash when the guardian is in that room. Those *state* semantics already
  work and must survive; what's missing is the "camera feed" body.
- `game/src/sim/knowledge.rs` (`MapKnowledge`) + the Phase 50 immersion ruling —
  fog-of-war consistency: what a camera wall may reveal is a design decision
  this phase must take explicitly (see rulings).
- `game/src/screens/place/factory.rs` — panel geometry (fix the jutting object).

## Design rulings (already decided)

- **Schematic feed, not render-to-texture.** Each panel renders a stylized
  miniature of its room from the simulation's own data: the room's footprint
  outline, its doorway stubs, dots for occupants (rival presence the shared
  observation model already exposes, the guardian, anchors as halos) — in
  `observed_style` semantics on the panel. Render-to-texture cameras (one per
  room) are rejected: cost, and the schematic reads better at panel size.
- **Fog-of-war ruling:** the observation rooms are *facility infrastructure* —
  standing in one is the in-fiction act of looking at the cameras. Panels may
  therefore show live room contents for rooms the cameras cover, but they do
  **not** write into the player's `MapKnowledge` tac-map (the sketch stays
  personally-witnessed only). The panel is diegetic knowledge you must read and
  remember — exactly the Betrayal-style payoff these rooms were designed for.
- **The existing signals stay:** cyan = anchored (tether room), red = guardian
  present (guardian room), layered on the new feed, legend-backed in the debug
  HUD legend.
- **Fix the geometry:** nothing may jut through a panel face; panel content
  renders flush (decal/child quads slightly proud of the wall, or the console
  moved out of the panel wall).

## Files you may edit

`game/src/screens/place/{monitors.rs, factory.rs}`, `game/src/view/{assets.rs,
components.rs}` (panel materials/meshes), `crates/observed_style` (panel
treatment helper only if missing), `game/src/screens/hud.rs` (debug legend
entries), `game/src/tests.rs`. Do NOT touch `sim/` state or the sighting/
knowledge rules — the panels *read* existing simulation data only.

## Success criteria

- A capture inside each observation room shows 3×3 panels with legible room
  miniatures — footprints, doorways, occupant dots — with anchored rooms cyan
  and the guardian's room flagged; viewed per the falsifiable-evidence rule,
  and the report names which panel shows what.
- Nothing intersects a panel; the visual audit passes with zero findings.
- Panels update live (guardian moves → its dot moves rooms within a second).
- Tests: panel-content generation is a pure function of sim state (unit-tested:
  given a match state, panel N shows room X's occupants); tac-map `MapKnowledge`
  is asserted unchanged by standing in an observation room.
- Full verification recipe green.
