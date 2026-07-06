# Arc E — Ready to Race (polish, comfort, and the real race)

Planned 2026-07-06, after Arc D ([liminal_arc_plan.md](liminal_arc_plan.md)) took the
facility to generated liminal scale. Arcs C and D made the race *worth racing* and
gave it a body to race in. This arc makes it a **finished game a real person wants to
sit down with** — and then puts a second real person across the table over the LAN.

## The thesis

Everything mechanical is proven: contested observation, door identity, the gantry,
visible collapse, rival legibility, generated maps. What's missing is the difference
between a proving ground and a game someone chooses to play twice: it must **teach
itself**, **feel good in the hands**, **read at a glance**, and — the payoff the whole
project has been building toward — let you race a **real human**, not a bot.

Two user decisions shape this arc (2026-07-06):

1. **Polish & QoL land first.** The first human-vs-human match should feel like a
   finished game, not a prototype with the seams showing.
2. **Multiplayer targets the LAN.** A real socket transport over the proven
   deterministic lockstep spine, for players on one network — no NAT traversal, no
   relay servers, no online matchmaking (those are a later arc). Local/loopback (two
   processes on one machine) is the first testable rung of that same transport.

**The architectural gift:** because the teleport model shows each player only the one
place they occupy, LAN play needs **no split-screen** — each machine runs its own
process and renders its own first-person view of the *shared* lockstep match. And
contested observation, sightings, and collapse all fall out of the shared brain state
for free once first-person actions enter the lockstep stream (Phase 51). The hard part
of networked multiplayer — keeping two simulations identical — is the part this
project has been proving in the labs since the beginning.

## Design principles for the arc

1. **The Legibility Contract is still inviolable.** Polish adds atmosphere, audio,
   and game-feel — it never dims, hides, or delays a gameplay-critical signal. Every
   new sound and effect is verified against the contract, not just felt.
2. **Determinism and lockstep are sacred.** The deterministic in-process transport
   that every test relies on stays. Real sockets enter as an **adapter behind the
   proven `observed_net` lockstep interface** — the simulation is byte-identical
   whether the bytes travelled through memory or an Ethernet cable. Every multiplayer
   step keeps the replay/desync tests green; a divergence is a bug, never a shrug.
3. **Lab-first for the risky pieces.** The action-protocol extension and the real
   transport are proven in labs (`net_match_lab` / `network_lab` / a focused new lab)
   before the assembled game depends on them, per agents.md.
4. **Accessibility is a deliverable, not a footnote.** `observed_style` already owns
   accessibility legend mappings; the settings/onboarding work surfaces them.
5. **Code-as-art holds.** No new authored art. Audio uses the existing drop-in slot
   convention (present → used, absent → silent), so the game ships complete and a
   dropped file only enriches it.

---

## Stages

Three polish/QoL phases, then three staged multiplayer phases (protocol → transport →
the real race). Each lands green (`cargo fmt`, `clippy --workspace`, `test --workspace`)
and, where it touches presentation, refreshes `OBSERVED2_CAPTURE` evidence.

### Phase 48 — Onboarding & Settings (the biggest QoL gap for a real player)

**Core work:**
- **First-run teaching.** A guided introduction to the core loop — observe-to-freeze,
  anchors, the tac-map, the gantry's jump-or-bypass, the collapse — surfaced through
  the existing legend/discovery/hint systems (diegetic where possible; the facility
  teaches by being read, not by a wall of text).
- **A real settings screen.** Master / SFX / music volume, mouse sensitivity, and key
  rebinding (rebind overlays are proven in `control_lab`; input is already abstracted
  through `player_input::PlayerIntent`), plus accessibility/legibility toggles wired
  to `observed_style`'s accessibility mappings. Persisted with the career profile.
- **Control legibility.** Context hints for the current tool/action; a pause-menu
  control reference.

**Verification:** settings persist across sessions; rebinding round-trips through the
intent layer without regressing gamepad support; a new player can complete a match
without external instruction (validated by a scripted "cold" bot-driven walkthrough
capture).

### Phase 49 — Audio & Game Feel

**Core work:**
- **Audio depth.** Spatialised, attenuated cues (footsteps, doors, rival bleed already
  exist — extend to distance/occlusion), per-district ambient beds, and the missing
  dramatic stings (collapse advance, klaxon countdown, escape). UI/menu sounds. All
  behind drop-in slots.
- **Movement & camera juice.** Teleport-transition polish, threshold-crossing feedback,
  gantry jump/land feel, and a restrained camera response to collapse — every effect
  checked against the Legibility Contract (no shake that hides a signal, no flash that
  masks a threat).

**Lab:** reuse `style_lab` / the capture pipeline to review effects in isolation.
**Verification:** the Legibility Contract audit (signals punch through) passes with all
effects live; evidence GIFs refreshed.

### Phase 50 — HUD & Results Clarity

**Core work:**
- **Objective legibility.** At-a-glance answers to "where is the exit, what do I still
  need, who's ahead" — keystone tracking, the tac-map read, the collapse frontier, the
  series standing — tightened into a HUD a first-timer parses in a glance.
- **Post-match clarity.** A results/summary screen that reads the series outcome and
  the run's story (placement, escapes, what the collapse took) legibly, and a cleaner
  path back into the loop.

**Verification:** HUD element counts and readability assertions on the generated
default map; results screen renders every outcome shape (win / placed / absorbed).

### Phase 51 — Shared Actions (`LocalAction::PlaceAnchor`)

The recorded next mechanical step, and the foundation both local and LAN play need.

**Core work:**
- **First-person actions enter the lockstep stream.** Extend the `LocalAction` wire
  protocol so anchor placement (and the other first-person tool actions) are shared,
  deterministic, replayable inputs — not game-local presentation. This is the change
  Arc C deliberately deferred: it turns the local anchor torch into a *real shared
  freeze* every peer computes identically.
- **Replay-format versioning.** The wire/replay format gains a version so old tapes are
  handled deliberately (upgrade or reject, never silently misread).

**Lab:** `net_match_lab` (or a focused successor) proves the extended action stream
stays byte-identical across simulated peers and round-trips through replay.
**Verification:** determinism, replay reproduction, and desync-detection tests hold
with the new action in the stream; contested observation over the shared stream freezes
edges identically for every peer.

### Phase 52 — Real Transport (loopback → LAN)

**Core work:**
- **A real socket transport adapter** behind `observed_net`'s lockstep interface — the
  simulated `NetworkProfile::Hostile` transport stays for tests and adverse-condition
  modelling; this adds a real-socket path selected at runtime.
- **Prove it on loopback first** (two processes, one machine, byte-identical lockstep),
  then across the LAN. Any dependency evaluated against the R11 dependency bar
  (`std::net` first; a minimal crate only if it removes real risk, behind a feature).

**Lab:** `network_lab` / `net_match_lab` gain a real-socket mode; a two-process
loopback test asserts identical final state vs. the in-process run of the same seed.
**Verification:** loopback and LAN runs converge to the same deterministic result the
in-process transport produces; hostile-condition tests (drop/jitter/reconnect) still
pass against the simulated transport.

### Phase 53 — LAN Lobby & the Real Race

**Core work:**
- **LAN discovery + connect.** Wire `observed_progression`'s lobby / session /
  matchmaking (proven in `session_lab` with a simulated peer) to real LAN discovery,
  connection, and reconnect over the Phase-52 transport.
- **The real race.** Two (or more) humans in a full contested first-person match over
  the LAN — each on their own machine, their own view, attacking each other's reality:
  shared observation, anchor denial, the gantry, the collapse, rival legibility, all
  between real people at last.

**Verification:** a two-machine (or two-process) LAN match completes with a consistent
result on both ends; reconnect recovers a dropped peer; the headless==interactive and
solvability invariants hold for networked matches.

---

## Non-goals (this arc)

- **No NAT traversal, relay servers, or online matchmaking.** LAN only. Online play is
  the next arc's horizon; the transport adapter is built so it can grow there.
- **No new gameplay rules.** Contention, collapse, the gantry, and door identity are
  done. This arc is presentation and presence, not new mechanics. (The one exception —
  `PlaceAnchor` — is not a new rule but the *networking* of an existing one.)
- **No new authored art dependencies.** Code-as-art holds; audio is drop-in.
- **No regression of the input abstraction.** Multiple players and input sources were a
  first-class assumption from day one; keep it.

## Horizon (after the arc)

Full **online** play — real transport over the internet with NAT traversal / relay and
online matchmaking — is the next arc, built on the LAN transport this arc proves. The
deterministic lockstep spine has been ready for it since the start; the LAN rung is the
last thing between the game and the open internet.
