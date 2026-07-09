# Arc F — Sight & Sound (the MVP-closing presentation arc)

Planned 2026-07-09, revising Arc E ([ready_to_race_arc_plan.md](ready_to_race_arc_plan.md)).
Arc E's polish phases (48–50) made the game teach itself and read at a glance. This
arc finishes the MVP by perfecting the two remaining presentation systems — **audio**
and the **2.5D sprite layer** — and formally moves everything beyond the MVP (LAN
multiplayer, world interaction) onto a documented post-MVP backlog instead of an
implicit "next".

## The 2026-07-09 scope ruling

**All LAN multiplayer work is deferred to post-MVP.** Arc E's Phases 51–53 (Shared
Actions, Real Transport, LAN Lobby) do not run; their design docs
(`docs/arc_e/phase_51..53_*.md`) are retained as design input for the post-MVP
"true LAN multiplayer with dedicated servers" backlog entry. Arc E therefore ends
when Phase 50 lands. The MVP is the polished, self-teaching, single-machine game.

## The thesis

Everything mechanical is proven and the game now teaches itself. What separates the
current build from a *finished* MVP is presentation depth in exactly two places:

1. **Audio is functional, not perfected.** Phase 49 landed spatial cues, district
   beds, and stings — but cue behavior is scattered across spawn sites, mixing is
   ad-hoc (no ducking, no priority), occlusion is minimal, and several gameplay
   signals still have no audio counterpart. The fix is architectural first
   (one audio director, buses, data-driven cue table), then content.
2. **The sprite layer is a placeholder pass, not a presentation layer.** The Kenney
   dev placeholders proved `bevy_sprite3d` end-to-end; the OpenGameArt CC0 intake is
   already on disk with provenance. What's missing is the metadata/atlas pipeline
   and the staged integration — objects, directional actors, room dressing —
   designed in [../sprite_roadmap.md](../sprite_roadmap.md).

## Design principles

1. **The Legibility Contract is inviolable.** Sprites and sounds add atmosphere and
   readability; they never dim, hide, delay, or mask a gameplay-critical signal.
   `observed_style` owns semantics; the visual audit is the acceptance gate.
2. **The immersion ruling holds.** Normal play stays HUD-free; the tac-map stays a
   fog-of-war sketch. No new HUD surfaces enter through this arc.
3. **Drop-in or fallback.** Every sprite/texture/sound slot follows the
   `observed_assets` convention: present → used, absent → procedural fallback or
   silence. Deleting every imported asset must leave a playable, legible game.
4. **Determinism hygiene.** Frame selection, cue scheduling, and any derive script
   are deterministic and tested. No wall-clock or iteration-order dependence.
5. **CC0 only in game-ready paths.** The CC-BY reference libraries are replaced,
   not attributed.
6. **Built for sub-agents.** The arc runs as two file-disjoint tracks (audio /
   sprites) so two implementation agents can work concurrently; every phase has a
   self-contained hand-off doc in [arc_f/](arc_f/README.md) and the parent session
   reviews, verifies, and commits.

## Stages

**Foundation (wave 1, before/alongside the tracks):**

- **Phase 54 — True Singleplayer (bot & guardian toggles).** Menu/settings
  toggles that disable rival teams, own AI teammates, and the guardian
  *separately*, as deterministic match configuration (populations never spawn in
  the sim — no presentation suppression), following the `OBSERVED2_MAP`
  precedent with an `OBSERVED2_BOTS` override. Serves gameplay (a true-solo
  exploration of the facility) and testing (isolate any system without bot
  noise) alike; the all-on default stays byte-identical, pinned by a
  characterization test, and a solo match ends and reports sanely.

**Track A — Audio perfection** (sequential within the track):

- **Phase 55 — Audio Architecture (the mixer).** One audio director owns cue
  scheduling: bus model (master/music/sfx/ambience over the Phase-48 settings
  channels), priority and ducking (stings duck beds), per-cue cooldowns (the
  Geiger-counter lesson made structural), and a single data-driven cue table
  replacing scattered constants. Regression tests for bus math, gating, and spam.
- **Phase 56 — Audio Content & Spatial Depth.** Distance-rolloff classes and
  through-wall occlusion on the director's table; an audio coverage audit (every
  gameplay-critical signal has a legend-backed audio counterpart) and the missing
  cues it reveals; richer district beds; CC-BY reference libraries replaced with
  CC0 and retired from intake.

**Track B — The sprite layer** (sequential within the track; design reference:
[../sprite_roadmap.md](../sprite_roadmap.md)):

- **Phase 57 — Sprite Metadata Pipeline & the OGA Lab.** Frame metadata format,
  deterministic crop/derive script into `assets/oga_25d/derived/`,
  `TextureAtlasLayout` support, and the `oga_25d_lab` (grown from
  `sprite3d_placeholder_lab`) demonstrating actors, objects, decorations, and
  direction/clip debugging with capture output.
- **Phase 58 — Gameplay Object Sprites.** Semantic object slots (keystones, cards,
  anchor torch, route cells, equipment) driven by existing match state in the
  played game, with style halos preserved and clean despawn on reset/exit.
- **Phase 59 — Directional Actors.** Rivals and guardian move to directional
  sheets with semantic clips; camera-relative direction selection; deterministic,
  tested frame selection; capsule fallbacks intact; guardian light/halo signal
  independent of the art.
- **Phase 60 — Room Dressing, Textures & Interaction Read.** Role-driven props
  under the dressing rules, optional LAB wall/floor albedo variants under style
  tint, and the reticle decision (diegetic interaction read only — no HUD);
  refreshed visual audit and evidence.

**Track scheduling:** [54 ∥ 57] → [55 ∥ 58] → [56 ∥ 59] → [60] (Phase 57 is
lab/assets-only, so it runs beside 54's game-side work). The only regularly shared
files are `crates/observed_assets/src/lib.rs` (+ README) and `game/src/tests.rs`;
the hand-off README gives the conflict rule.

## Horizon (post-MVP backlog — list, don't build)

Recorded in ROADMAP.md as the post-MVP backlog:

1. **True LAN multiplayer with dedicated servers.** The deferred Arc E designs
   (shared lockstep actions, socket transport, LAN lobby) plus a dedicated-server
   deployment model — a headless deterministic host peers connect to, replacing
   pure peer-to-peer session ownership. Online play (NAT/relay/matchmaking)
   remains beyond even that.
2. **World interaction.** Players act on the facility graph itself: explorers
   "hacking" a room console to connect it to a specified room ID (player-driven
   rerouting under the solvability invariant), and fallen/absorbed teams
   connecting nodes from a top-down view — extending "eliminated teams join the
   adversary" into an active graph-editing role that keeps everyone playing.
3. **Carried follow-ups from Arcs C/D:** a third hall endpoint so the gantry's
   understory exit reaches a genuinely different neighbour; the decoherence
   counter-tool.
