# Asset Integration Phase A9 — Game Integration Decision

Decision artifact for **asset roadmap Phase A9** of
[bevy_asset_integration_roadmap.md](bevy_asset_integration_roadmap.md). Phases A1–A8
each evaluated one Bevy ecosystem asset in an isolated lab; every one closed with a
"keep lab-local … per A9" promotion decision. A9 is where those deferrals come due:
**which lab-proven assets should enter the assembled `game`?**

Evaluated: 2026-06-27. No code changed in this phase — it is a triage decision plus
the documentation updates its exit criteria require. The eight asset labs, their
tests, READMEs, and evidence remain as the proving harnesses.

## The bar (same policy as R11)

A9's scope says: *promote only assets with at least one passing lab **and a concrete
game use**; prefer small adapter crates over direct `game` integration when the
behavior has multiple consumers.* That restates the project's standing dependency
policy ([architecture.md](../architecture.md), and the R11 evaluation
[refactor_r11_evaluation.md](refactor_r11_evaluation.md)):

1. **Prove the problem exists in the game today** (not hypothetically).
2. **Verify Bevy `0.18.1` compatibility** (already done per lab in A0–A8).
3. Add the dependency **behind the smallest adapter**, with **domain state independent**
   of the plugin's types.
4. A reusable behavior with **multiple consumers** earns a small adapter crate; a
   single consumer does not.
5. Carry a **passing lab + guard/fallback** and document **maintenance cost**.

A candidate is **promoted** only if it clears step 1 — a real, current need in the
assembled game that the existing code does not already meet. Otherwise it stays
**lab-local** with a concrete *adopt-when* trigger, so the call is revisited on real
need rather than carried as speculative complexity. The eight labs already proved
*feasibility and compatibility*; A9 asks the separate question of *committed game
value now*.

## Decision summary

**Zero promotions into `game` at this time; all eight assets stay lab-local, each
with a concrete adopt-when trigger and a recorded owner boundary.** This matches every
lab's own promotion decision and the R11 precedent (the refactor accepted only the one
dependency — `bevy-inspector-egui`, dev-only — that solved a real current problem and
deferred the other five). No asset evaluated in A1–A8 clears the "concrete game use
now" bar, and one (`bevy_trenchbroom`) actively conflicts with the game's deliberately
procedural geometry direction.

This is not "do nothing": each lab is a *banked, compatible, tested* integration that
can be promoted the day its trigger fires, behind the adapter shape its lab already
established. The decision records that trigger so the work is cheap and obvious later.

| # | Asset | Lab | Passing lab? | Concrete game use today? | Decision | Adopt-when trigger |
| --- | --- | --- | :---: | --- | --- | --- |
| A1 | `bevy_trenchbroom` | `trenchbroom_lab` | ✅ | **No — conflicts.** Game geometry is deliberately procedural (`game/src/maze.rs`, `hallway.rs`, `teleport.rs`, polygon rooms) per the teleport-hallway pivot. | **Defer** | the design pivots from seeded/procedural layouts back to hand-authored fixed maps |
| A2 | `bevy_ecs_ldtk` | `ldtk_schematic_lab` | ✅ | **No.** No second schematic consumer; the in-game tac-map renders from match state, not LDtk. | **Defer** | a design-time tactical-map / route-plan authoring workflow is committed |
| A3 | `bevy_mod_outline` | `outline_legibility_lab` | ✅ | **Partial.** Legibility is a real playtest gap, but the game already routes treatments through `observed_style`; mesh outlines need a committed legibility-integration pass with >1 presenter. Color-vision is already covered by pure `observed_style` matrices. | **Defer (next-up)** | the paused legibility arc resumes and the match presentation commits to mesh outlines for gameplay-critical signals |
| A4 | `bevy_hanabi` | `semantic_vfx_lab` | ✅ | **No.** Readability gain does not yet exceed the GPU-particle render/maintenance cost; events are already legible via style + lights. | **Defer** | a specific event reads poorly without particles and the style/light treatment is exhausted |
| A5 | `bevy_image_export` | `capture_pipeline_lab` | ✅ | **No.** The existing `OBSERVED2_CAPTURE*` hooks already produce evidence across every lab and the game; no second adopter. | **Defer** | visual-regression sequences (not single stills) become a committed CI/evidence requirement |
| A6 | `bevy_archie` | `archie_input_lab` | ✅ | **No.** Controller support / rebinding is an explicit non-goal; `player_input` already owns the boundary. | **Defer** | controller support or runtime rebinding becomes a committed feature |
| A7 | `bevy_mod_config` | `lab_observability_lab` | ✅ | **No.** No second typed-config/persistence consumer; runtime game settings stay separate from debug knobs. | **Defer** | the game needs disk-backed user settings beyond the progression serialize-string |
| A8 | `vleue_navigator` | `navigation_probe_lab` | ✅ | **No.** The current match brain uses no physical pathfinding; bots resolve in graph space. | **Defer** | bots/AI need physical routing in the first-person facility |

Two assets researched alongside the labs were already rejected on compatibility and
remain rejected: **`bevy_color_blindness`** (A3 — only release pins Bevy `0.8`; the
color-vision check stays in pure `observed_style` preview matrices) and
**`bevy_log_events`** (A7 — force-pulls `bevy_egui` via a hard `assert!`, against the
no-egui-in-default-build rule; event tracing stays lab-local over Bevy `tracing`).

## Per-asset rationale

### A1 — `bevy_trenchbroom`: Defer (conflicts with current direction)

`trenchbroom_lab` proved an authored `.map` can be imported *as data* and projected
into `RoomId`/`PortId`/door-state/collision without editor entities becoming the game
model — a clean result. But A1's own gate was conditional: promote *"if authored maps
beat the current procedural/hard-coded geometry workflow."* They do not, because the
game's committed direction is the opposite. The teleport-hallway pivot
([ROADMAP.md](../ROADMAP.md)) builds match space from **seeded, procedural** parts:
`game/src/maze.rs` generates randomized-DFS grid mazes per hallway, `hallway.rs`
jitters connector lengths, `teleport.rs` clamps convex **polygon rooms**, all re-rolled
deterministically when an edge decoheres. Authored fixed maps would *fight* the
"changes when unobserved" generation, not improve it. The importer adapter stays
lab-local (no `observed_authoring_trenchbroom` crate) until the design itself asks for
hand-authored fixed geometry.

### A2 — `bevy_ecs_ldtk`: Defer (fallback only, no second consumer)

A2 explicitly closed as "keep LDtk lab-local and live as a fallback until a second
consumer needs editable tactical-map or route-plan data." Nothing changed: the game's
in-match tac-map renders from live match state through `observed_style`, not from an
LDtk project, and no design-time schematic authoring workflow is committed. The lab
remains a proven 2D data path ready for that future, distinct from TrenchBroom's 3D
geometry path.

### A3 — `bevy_mod_outline`: Defer, but the **first** promotion candidate

This is the closest call. The 2026-06-21 playtest named *presentation/legibility* as
the game's weak link, and `outline_legibility_lab` proved that gameplay-critical
signals (doors, interactables, hazards, rivals, objective beacons, pickups, the player
proxy) can punch through fog/bloom with outline colour/width selected entirely from
`observed_style::outline`. That is genuine, on-direction value.

It is still deferred for three reasons: (1) the legibility/visual-language arc is
*paused behind the now-complete architecture refactor* and has not formally resumed, so
there is no committed integration milestone to attach the outline pass to; (2) the game
already routes every gameplay colour through `observed_style`, and the in-match markers
were made diegetic (next-room beacon, exit gate/light, hazard beacons, rival avatars),
so outlines are an *enhancement*, not a gap-filler; (3) A3's own gate requires multiple
presenters before promoting style-facing helper APIs, and only the one lab consumes the
outline table today. **Recommendation:** when the legibility arc resumes, promote
`bevy_mod_outline` first — behind a thin presentation adapter driven by
`observed_style::outline`, used by both the match and the tac-map — making it the lab
with the strongest standing claim. The `observed_style` outline table is already
promoted (R1), so half the adapter exists.

### A4 — `bevy_hanabi`: Defer

`semantic_vfx_lab` proved particles can be deterministic, toggle-off, style-driven event
projections that never hide gameplay signals. But A4's gate ("unless readability
improves more than the added render complexity costs") is not met today: the match's
events (door open/slam, reroute flash, pickups, exit unlock) already read through style
treatments, lights, the route-shift flash, and audio. GPU particles add a real shader
stack (`naga`/`wgpu`/`naga_oil`) for an enhancement, not a fix. Defer until a specific
event is shown to read poorly with the style/light treatment exhausted.

### A5 — `bevy_image_export`: Defer

`capture_pipeline_lab` proved repeatable still + short-sequence offscreen capture
without perturbing fixed-step timing. But the existing `OBSERVED2_CAPTURE*` screenshot
hooks already produce all current evidence across every lab and the game, and only one
lab adopts the export helper. A5's gate ("if it makes evidence capture more reliable")
is not met for single stills. Adopt when multi-frame **visual-regression sequences**
become a committed evidence/CI requirement — the shape the lab already proved.

### A6 — `bevy_archie`: Defer

`archie_input_lab` proved richer device support can feed the existing `PlayerIntent`
boundary through a pure per-device adapter (archie's global `ActionState` deliberately
*not* adopted). But controller support and runtime rebinding are explicit project
non-goals, and `player_input` already owns the boundary cleanly. The cost is also
concrete: archie's MSRV bumped the toolchain to `1.96`. Promote only the
device-to-intent adapter, only when controller support is a committed feature.

### A7 — `bevy_mod_config`: Defer

`lab_observability_lab` proved typed config + JSON persistence can stay strictly
separate from the deterministic launch manifest (config edits only mark a pending
relaunch; `Enter` commits a seed into the manifest), and that event tracing can be
disabled with zero behaviour change. Valuable as a *lab* dev-tool, but there is no
second config consumer and no committed need for disk-backed **user settings** in the
game (progression already serializes to a round-tripping string). Keep runtime game
settings separate from debug configuration, as the lab's design itself argues. Adopt
when the game needs durable on-disk settings.

### A8 — `vleue_navigator`: Defer

`navigation_probe_lab` proved a navmesh can be a *derived consumer* of facility geometry
(it never owns the room graph: a closed door becomes a solid obstacle and the route
detours or fails, cross-checked against the graph over all 16 door configs × every room
pair). But A8's gate is unchanged: the current match brain resolves bots in graph space
and needs no physical routing. Defer until bots/AI need to physically navigate the
first-person facility.

## Owner boundaries (unchanged, recorded for completeness)

Every asset dependency stays isolated to its lab's `Cargo.toml`; no production crate or
`game` gains an asset dependency in A9. The production/lab split is intact:
`game -> crates/* -> pure domain crates`, with **zero `game -> labs/*` and zero
`crates/* -> labs/*`** edges. The eight asset labs are debug/feasibility projections,
exactly like the foundation and FPS labs.

## Exit criteria

- **`ROADMAP.md` updated with proven outcomes, not intentions.** ✅ The Asset-integration
  arc section records A2–A9 as complete with this decision (zero promotions, eight
  triggers), not as a promotion plan.
- **`Catalogue.md` updated for new permanent labs.** ✅ The two labs created after the
  2026-06-26 catalogue generation (`lab_observability_lab`, A7; `navigation_probe_lab`,
  A8) are added; this decision doc is added to the evaluations section. The other six
  asset labs were already catalogued.
- **Every promoted asset has tests, README, evidence, and a matching owner boundary.**
  ✅ Vacuously satisfied — nothing is promoted. As verification of the underlying claim,
  all eight labs carry passing tests, a README naming the question/controls/reset/
  verification, and screenshot evidence under `docs/evidence/` (the
  `capture_pipeline_lab` writes a still + a six-frame sequence under its own evidence
  subdirectory).

## Conclusion

A9 closes the asset-integration arc with a deliberate, rules-consistent decision:
**all ten researched candidates evaluated, eight proven in isolated labs, zero promoted
into the game today, each carrying a concrete adopt-when trigger.** The strongest
standing claim is `bevy_mod_outline`, queued as the first promotion when the legibility
arc resumes. The labs are banked, compatible, and tested, so each promotion is a small,
obvious step the day its trigger fires — the same discipline that kept the R0–R11
refactor's dependency graph clean.
