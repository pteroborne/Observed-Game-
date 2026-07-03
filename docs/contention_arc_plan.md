# Arc C — Contention & Depth (the "secret sauce" roadmap)

Planned 2026-07-02, immediately after Refactor Arc G
([refactor_game_arc_plan.md](refactor_game_arc_plan.md)) left the game layer clean,
one-brained, and ratchet-tested. This arc is a *design* arc: it exists to answer
"why is this game worth playing" with mechanics, not content.

## The thesis

The game's differentiator is already built and mechanically honest: **geography that
is real while observed and fluid while unobserved**, realized rigorously (a doorway's
preview *is* its destination, frozen at place-entry; decoherence is diegetic). What
is missing is not more world — it is **friction between teams**. Today observation
only affects your own route; rivals are HUD numbers. The arc's single most important
change is one rule:

> **Observation is objective and shared.** When *any* team observes or anchors a
> connection, it is frozen for *every* team. Your certainty becomes my wall — or my
> highway.

Everything else in the arc (door identity, verticality, visible pressure, rival
legibility) exists to give that rule a board worth fighting over and a body to feel
it through. No combat is ever added: you attack someone by *rearranging what is
true*.

## Design principles for the arc

1. **Chosen, readable irreversibility.** Corridors are the commitment beat. One-way
   moves (drops, crossings, collapses) are welcome *when the player can read the
   commitment before making it* — a ledge visibly too tall to climb back, a gap
   whose far side has no return path in sight. Irreversibility that reveals itself
   only afterwards is a Legibility Contract violation, not tension.
2. **Setback, not death.** Consistent with the no-combat pillar and the comfort
   pass: failure costs *route and time*, never a corpse run. Falling reroutes you
   through the graph; it does not kill you.
3. **Solvability is inviolable.** The discovery feasibility work proved
   "shift-but-always-solvable"; contention must inherit it. No combination of rival
   freezes, anchors, and collapse may leave a living team without *some* path to the
   exit. This is the arc's central validator and every phase's regression test.
4. **Lab-first, per agents.md.** Each phase proves its rules in an isolated lab
   with observable success criteria before the game integrates it.

---

## Phase 38 — Contested Observation (the sauce activator)

**One question:** does shared observation create real competitive interaction while
preserving determinism and solvability?

**Rules to prove (initial set — deliberately minimal):**
- *Presence observes.* A room occupied by any team has its visible thresholds
  frozen — for everyone. Rival positions become spatial facts, not HUD trivia.
- *Anchors are hard freezes.* A team's anchor torch pins a room's threshold set for
  all teams until removed. Team-keyed (ownership already exists in the brain).
- *Anchor-vs-anchor is idempotent.* Two anchors on overlapping thresholds agree —
  freezing is a fact, not a vote. The conflict lives at the **route** level: you
  deny me by freezing the topology in a shape that favors you.
- *No unfreezing — yet.* A counter-tool (a "decoherence charge" that re-fluidizes an
  edge) is the obvious escalation, but it doubles the rule surface. Phase 38 ships
  observation + anchors only; the lab measures whether denial is *too* strong and
  the counter-tool is added only if stalemates appear in bot-vs-bot series.
- *Knowledge is team-local.* The tac-map shows only what your team has observed,
  with staleness. Reality is shared; *information about it* is not. (Fog of war
  over truth — the inverse of most games.)

**Lab:** `contention_lab` — pure rules over `observed_observation` +
`observed_match` (no rendering): N bot teams race seeded graphs under shared
freezing. Success criteria: (a) route denial measurably changes series outcomes vs
a no-sharing control across a seed corpus; (b) determinism/replay byte-identical;
(c) zero solvability violations across the corpus; (d) no degenerate
freeze-everything stalemate (else revisit the counter-tool).

**Game integration:** the brain change is small (pins become global, sources stay
team-keyed); presentation reuses tether lights (rival-team colors on frames they
froze) and the existing threshold-status vocabulary.

## Phase 39 — Doors With Identity (discovery surfaced)

**One question:** can a player standing at two doorways make an *informed gamble*?

The semantic board already exists (`RoomRole`: Start/Exit/Decision/DecoherenceFork/
AnchorCheckpoint/TeleportRelay/… and the discovery_lab typed-room feasibility). The
gap is entirely player-facing:
- **Threshold reads:** a glyph language on doorframes (via `observed_style`, added
  to the legend), plus sensory bleed through open thresholds — district light
  temperature, room-type ambience — so a read is a *skill*, not a menu.
- **Type-true payoffs:** the room behind a Reactor read must behave like a Reactor
  (hazard/pressure), a Sensor room reveals map knowledge (feeds the team-local
  tac-map from Phase 38), a Decoy advertises an exit signal that lies.
- **Lab:** extend `discovery_lab` with the read layer; success = a scripted reader
  bot outperforms a random-door bot by a target margin on seeded runs.

## Phase 40 — The Gantry (verticality as a jump map)

**One question:** does a fall-reroutes-you platform hall create the
risky-shortcut-vs-safe-bypass beat without death or frustration?

New `HallwayFlavor::Gantry` (the flavor enum and the `Climb` elevation precedent
already exist): a two-level hallway piece —
- **Upper route:** floating/raised platforms spanning the hall; jumping them is the
  fast path to the intended exit threshold.
- **Understory:** falling drops you to the lower floor — *no damage* — where the
  only exits are different threshold nodes: back the way you came, or a slower side
  route to a different neighbor. In graph terms, gravity rewires your crossing.
  The setback is time and (post-38) the observation churn of re-entering.
- **Readable commitment:** platform edges are lit (style module), the understory is
  visible from above (you can see where failure puts you *before* you jump), and
  drop-downs are one-way by visible height. This is the arc's canonical example of
  chosen irreversibility.
- Builds on `fps_elevation_lab` (AABB elevation, step limits) and the existing
  interior-wall render/collision path. Grapple sockets remain a *later* verb —
  Gantry ships first because it needs zero new controller mechanics beyond jump.

**Lab:** `gantry_lab` — one hall, both routes, a bot that attempts the jumps with a
tunable failure rate; success = fall recovery always lands in a navigable
understory with at least one exit (solvability), and timing spread between
clean-jump / fall-recover / safe-bypass hits design targets (fast / medium / slow).

**As landed:** Phase 40 shipped the pure model (`observed_traversal::gantry`), `gantry_lab`, and initially only a flat footprint projection in the game; the review flagged that gap, and Phase 40b (2026-07-03) closed it — height-gated `DoorGap.floor_y` crossings, `DeckSeg` decks extruded into the arena, the four-threshold projection (two exits to the destination at different heights, understory side exit home), mount stairs, and GantryDeck/GantryEdge/Understory rendering with a legend entry. Note the remaining deliberate deferral: the side exit targets the origin room (`from`); routing it to a genuinely *different* neighbor awaits a hall contract that carries a third endpoint, which is Phase 41/42 territory if wanted.

## Phase 41 — Pressure Made Flesh (collapse in the world)

**One question:** can the director's pressure be *seen*, not read?

- Collapse claims territory diegetically: a dying district's lights fail and its
  palette drains ahead of closure; a collapsed edge's hallway variation returns as
  rubble (sealed threshold with debris, not a silent absence).
- The first-escape countdown gets a facility-wide state change (klaxon lighting
  tier via the style module) instead of only HUD text.
- Interaction with Phase 38: collapse *overrides* freezing — the one force anchors
  cannot hold back. (The guardian remains the embodied threat; collapse is the
  environmental one. Whether the guardian's gaze *observes* — freezing thresholds
  as it patrols — is this phase's design experiment.)

## Phase 42 — The Race Reads as a Race (rival legibility)

**One question:** does a solo player *feel* three other teams existing?

- Rival traces in the world: their team-colored anchor torches and frame lights
  (Phase 38 makes these meaningful), doors left open, pads left behind.
- Sound through thresholds: footsteps/decohere stings bleeding from occupied
  neighbors — presence you can hear before you see.
- Bot-team personalities in `teamplay` (the freezer, the sprinter, the saboteur) so
  strategies are recognizable across a series; the HUD names them.
- Tac-map inference: rival-frozen edges you encounter get attributed ("Team 3 was
  here"), turning the map into a detective board.

---

## Horizon (explicitly after the arc)

- **Human multiplayer.** The lockstep spine is proven; humans slot in when
  Phases 38–42 make the race worth racing. Not before.
- **Counter-observation tools** (decoherence charge, observation jamming) — only if
  Phase 38's lab shows denial dominance.
- **Second traversal verb** (grapple sockets) — after Gantry proves the corridor
  beat.

## Non-goals (unchanged from agents.md, restated for this arc)

No combat or direct harm, ever — contention stays informational and spatial. No
new art-asset dependencies (code-as-art holds). No procedural mesh geometry beyond
the authored hall/platform primitives. No changes to the deterministic
brain/transport contract — every new rule must survive the replay and lockstep
tests unchanged.
