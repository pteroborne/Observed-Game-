# Arc T — Somewhere To Go

**Opened 2026-08-09 from the first Deck-to-Deck playtest.** Findings are
[bug_backlog.md](../bug_backlog.md) entries **29–37**, plus playtest verdicts
recorded against the four entries (20, 21, 22, 25) that had been sitting on
"fix landed, awaiting human verification".

The arc's name is its acceptance test. A player should be able to say where they
are, where they are going, and how they would get back.

---

## 1. What the findings actually are

Ten reports, five problems. Getting this collapse right is most of the plan,
because three of the ten cannot be fixed by working on them directly.

| | Finding | Reading |
| --- | --- | --- |
| **Blocker** | Crash between solve and map load, no error recorded | Nothing else matters until LAN survives loading. The missing diagnosis is a second defect inside the first. |
| **Local defect** | Lanterns and plates do not stay where dropped | Plates root-caused; lanterns not. Cheap, isolated. |
| **The root** | Corpus too small to compose distinguishable places | Directly causes "nothing to move toward", and *indirectly* causes three more. |
| **Opportunity** | Spectator cutaway is good | The only feature that drew unprompted praise. |
| **Broken rule** | Events are a screen-space banner | Not a missing feature — a regression against a directive the project wrote down repeatedly and then stopped following. |

The three findings that are **symptoms of the root**, and must not be scheduled
as independent work:

- *"Gameplay is shallow"* — the objective model (two keystones, a two-operator
  station, a regroup) is implemented and reachable. The choices are not legible
  as choices because the places they are made in are not distinguishable.
- *"Players can't tell the map is changing"* — a change between two
  indistinguishable layouts is imperceptible by construction. Improving the cue
  cannot fix this while the layouts read the same.
- *"Tac-map is useless"* — a map of indistinguishable places cannot orient
  anyone. Some of this is genuine map-design debt and some is downstream; the
  split is only visible after the corpus improves.

**Consequence for scheduling:** the corpus comes before the legibility work that
depends on it, or that work gets done twice.

### 1a. The process finding, which is worth more than any single fix

Entries 20, 21, 22 and 25 were all marked *fix landed, awaiting human
verification*. This playtest was that verification and **all four failed**.

Entry 21 is the sharp one. Its deterministic gate **passes** — production
collapse provably rejects any solve lacking a connected seven-cell, three-exit
open volume — and players still did not experience the facility as having open
places. A measurable proxy was satisfied without the thing it stood for.

Arc T therefore holds one rule: **no phase closes on a proxy.** A gate may prove
a necessary condition; only a person closes a perceptual claim.

---

## 2. What can actually run in parallel

The hope going in was that Arc S left the codebase decoupled enough to task out
without shotgun surgery. Checked rather than assumed, and the answer is split.

**The code is decoupled.** The presentation workstreams and the crash touch
disjoint trees, and none of them moves a pinned identity:

| Packet | Exclusive files | Moves a pin? |
| --- | --- | --- |
| T-1 crash | `game/src/hex_wfc/loading.rs`, `loading_tests.rs` | no |
| T-2 dropped tools | `crates/observed_match/src/hex_wfc/model/pad.rs`, `pad_tests.rs` | no — the gate bot never deploys, so `deployed_pads` is empty in its digest |
| T-3 cutaway | `game/src/screens/match_runtime/spectator.rs`, cutaway paths under `game/src/hex_wfc/view/` | no |
| T-9 diegetic events | `game/src/hex_wfc/feedback.rs`, `cues.rs`, `audio.rs` | no |

**Two workstreams are serializing, and not because of coupling.** Facility size
and corpus authoring each move `compiled_catalog` → `simulation_content_hash`,
and with it the headless-gate tick, the gate digest, the perimeter-tower trace,
and the 24-seed survey. They cannot run beside each other or beside anything that
re-pins, and by the Arc S rule **only the integrator regenerates content**
(`.map`, catalog, manifest, FGD, `.sha256`, certificates).

So: four-way parallel at the front, single-file through the middle, parallel
again at the back once the corpus is settled.

### 2a. The shared `target/` is the real hazard, and it has a fix

`.cargo/config.toml` sets an **absolute** `target-dir = "O:/Observed 2/target"`,
shared by every sibling worktree. Arc S lost real time to this: concurrent cargo
runs give `rust-lld: permission denied`, and the half-written artifact then
produces convincing `unresolved import` errors for symbols that plainly exist.
It reads exactly like a cross-packet incompatibility and is not one.

**Every worker in this arc sets its own target directory:**

```powershell
$env:CARGO_TARGET_DIR = "O:/Observed 2-t-a/target"
```

Cost: one full cold build per worktree. Pay it — it is cheaper than one
misdiagnosis. If a worker ever sees an impossible unresolved import,
`cargo clean -p <crate>` before believing anything about another packet.

---

## 3. Waves

### Wave 0 — unblock, and the cheap wins `[ ]`

Four workers, fully parallel, disjoint files, no pins moved.

- **T-1 — The load crash produces a diagnosis, then stops happening.**
  Backlog #29. *Order matters*: make the failure report itself first, then fix
  what it reports. The solve runs on a worker thread and a panic there can be
  swallowed rather than surfacing through the panic hook; a failed load must
  reach an error screen rather than continuing. Reproduce with two peers — a
  listen host that never crashes solo points at the client's
  replay-from-tick-one path. Suspected interaction with T-4.
- **T-2 — Dropped tools stay dropped.** Backlog #32. Plates are confirmed:
  `step_pad_actions` records `player.position`, which is the body-box centre
  (`half_height = 0.9`), so every plate hangs at chest height. Fix in the
  **sim**, not the renderer, so the snapshot agrees, and add the height
  assertion whose absence let this ship. Lanterns are a **different, unproven**
  cause — diagnose before changing.
- **T-3 — The cutaway says what it is cutting.** Backlog #36. Clarity only:
  what is cut, where the viewer is, which bodies are which. Do not promote it
  into the match yet; that is T-7's question and it wants this work done first.
- **T-9 — Events are seen and heard in the world, not read off the screen.**
  Backlog #37. A standing directive says a player should see and hear every
  event diegetically; what ships is a banner reading "X rooms changed". Both
  halves are wrong in the code's own words — `feedback.rs` is "Screen-space
  semantic event feedback" and the cue audio is a **non-spatial** `AudioPlayer`
  one-shot, which is a UI sting rather than a sound in the room.

  **This is smaller than it looks:** `HexMatchEvent` already carries
  `cell: Option<HexCoord>`, so the simulation already reports where every event
  happened — presentation discards it. Nothing new is needed from the sim.

  **This is also not a re-render.** `cues.rs` models an event as
  `glyph`/`label`/`marker`/`sound` — a string to display. A diegetic vocabulary
  answers *what does the world do when this happens?* Replace that model, or the
  text just moves. Start with the events that are already local and provable —
  a lantern placed, a plate set, a coherence lock taken — and let the mutation
  beats follow; telling two *layouts* apart depends on Wave 2 and is T-7's
  remainder.

### Wave 1 — size `[ ]`

**Integrator only.** Serializes against everything.

- **T-4 — A facility the corpus can fill.** Backlog #33. `HexWfcConfig`
  (`cols`, `rows`, `levels`) and the production room quotas. This is a
  measurement, not a taste call: `hex_wfc_lab`'s production-corpus mode and the
  Studio coverage tab both report how much of a facility the corpus can
  meaningfully fill. Re-pin the gate tick, gate digest and survey in one commit,
  and say in the message which pins moved and why.

### Wave 2 — the corpus `[ ]`

**The actual work of this arc, and it is authoring, not engineering.** Arc S
closed the shotgun-surgery smell as a code problem and said in as many words
that what remained was content debt. This is that bill.

- **T-5 — Places that differ.** Backlog #30. The target is not variety for its
  own sake: a player crossing a threshold should be able to tell *where they are*
  and *which way they came from*. Instruments already exist — the Studio's
  coverage tab for what the corpus cannot fill, the neighbour explorer (`N`) for
  what a tile actually composes with.
- **T-6 — Geometry that does not mislead.** Backlog #34, overlaps #25.
  Confusing is worse than plain: a shape implying a route that is not there costs
  more than a dull shape. The 2026-08-07 human note stands — the kit will need
  hand-made tiles; everything in the corpus today is forge-generated.

### Wave 3 — legibility, once there is something to be legible about `[ ]`

Parallel again. Deliberately after Wave 2.

- **T-7 — Two layouts can be told apart.** Backlog #31, **reduced by T-9**.
  T-9 makes each mutation beat perceptible where it happens; what is left here
  is the part that genuinely needs Wave 2 — a player noticing that the facility
  is now *different*, which is impossible while every layout reads the same.
- **T-8 — A map that answers where am I, where am I going, how do I get back.**
  Backlog #35. Decide first whether it should show *rooms and connections*
  rather than cells. T-3's cutaway is the strongest candidate input here — it is
  already the clearest picture of the facility the project has produced, and an
  in-match version would attack #31 and #35 together.

---

## 4. Worktree protocol

Carried unchanged from Arc S, which used it successfully across seven waves.

```
O:\Observed 2         integrator, integration branch
O:\Observed 2-t-a     worker
O:\Observed 2-t-b     worker
O:\Observed 2-t-c     worker
```

- Each worker gets a **packet ID, base SHA, exclusive file list, invariants,
  hash policy, exact test commands, and a handoff block**. Workers never merge.
- A worker needing an edit outside its list **stops at a compiling leaf and hands
  it to the integrator**. This worked in Arc S and is the reason packets stayed
  clean.
- Workers do **not** regenerate content. Say it explicitly; they otherwise will.
- Waves branch from the **published integration SHA**, never a moving branch.
- **`git add -A` and commit a compiling leaf immediately and at every
  milestone.** Arc S lost three agents mid-packet to session limits with ~130 KB
  uncommitted, most of it untracked and one checkout away from gone.
- Per-worktree `CARGO_TARGET_DIR` — see §2a.

**Verifying a handoff — do not take it on trust.** Check the changed-file list
against the exclusive list; confirm a claimed-inert path really is inert by
grepping the call site rather than believing the summary; and check that **pin
test files do not appear in the diff.** A pin that passes because it was edited
is worse than a red one.

---

## 5. Gates

Per packet: `cargo fmt --all`, `cargo dev-clippy` (warnings are errors),
`cargo dev-test` — and capture cargo's **real** exit code, not a pipeline's.
`cargo test ... | tail` reports `tail`'s status and hides the result; a probe
inside a *passing* test prints nothing without `--nocapture`.

Per wave: the union gate, plus the 24-seed spectator survey at zero stalls.

**Arc close is a human gate, and it is one sentence:** a player who has not seen
the facility before can say where they are, where they are going, and how they
would get back. No proxy closes it — see §1a.
