# Refactor Phase R0 — Guardrails & Dependency Audit

Architectural evidence for **Roadmap Phase R0**. This is the captured baseline the
rest of the refactor arc (R1–R11) is measured against. The long-range plan and the
hard rules live in [architecture.md](../architecture.md); this file records the
*current* dependency reality and the rules that govern every later extraction.

Captured: 2026-06-25. Reproduce with the command below — the numbers should only
ever go **down** as labs are promoted into `crates/*`.

## Dependency guardrails (the rules)

These are the [architecture.md](../architecture.md) hard rules, restated as the
checks this audit enforces:

1. **`crates/*` must never depend on `labs/*` or `game`.** Production crates are the
   stable bottom of the graph.
2. **`game` must depend on `crates/*`, not `labs/*`.** This is the rule the refactor
   exists to satisfy; it is *currently violated* (10 edges, below) and is cut in R10.
3. **Labs may depend on `crates/*`** and, for now, on other labs — but production
   code must not depend on labs. Cross-lab edges are deleted as the shared behavior
   is promoted (R5–R9).
4. **Pure simulation crates avoid Bevy**; Bevy enters through adapter crates/modules.
5. **Domain identity stays in stable newtypes** (`observed_core`), never Bevy `Entity`.

Target dependency direction (from [architecture.md](../architecture.md) and the
roadmap's refactor arc):

```text
game  -> crates/* (reusable Bevy adapters) -> pure domain crates -> observed_core / player_input
labs  -> crates/* + lab-only presentation/debug harnesses
```

## Reproduction

```powershell
cargo metadata --no-deps --format-version 1
```

Classify each workspace member by its manifest directory (`crates/`, `labs/`,
`game/`) and read the `dependencies[].path` of each package. The audit scripts used
to capture the snapshot below live in the scratchpad; the raw command is the
exit-criterion artifact.

## Finding 1 — `game -> labs/*` (the violation R10 must cut)

`observed_game` currently has **10 direct path dependencies on `labs/*`** and only
**2 on `crates/*`**:

| `game -> labs/*` (10) | `game -> crates/*` (2) |
| --- | --- |
| competitive_facility | observed_core |
| fps_controller_lab | player_input |
| fps_hybrid_match_lab | |
| fps_maze_lab | |
| mutable_facility | |
| net_match_lab | |
| network_lab | |
| progression_lab | |
| session_lab | |
| style_lab | |

**Transitive closure:** through those 10 edges the production build pulls in **14
labs** — the direct 10 plus `observation_lab`, `competition_lab`, `constraint_lab`,
and `director_lab` (reached via `competitive_facility` / `fps_hybrid_match_lab` /
`mutable_facility`). That 14-lab surface is what the refactor incrementally drains
into `crates/*`.

## Finding 2 — Rule 1 holds today (no `crates/* -> labs/*`)

The two existing production crates are clean:

| crate | path deps |
| --- | --- |
| `observed_core` | `player_input` (crate) |
| `player_input` | none |

No `crates/*` depends on any `labs/*` or on `game`. The bottom of the graph is
already correct; the refactor is about *growing* it, not repairing it.

## Finding 3 — cross-lab dependency map (deleted as behavior is promoted)

Labs that depend on other labs today. These edges are the entanglement the
promotion phases (R5–R9) unwind — each disappears once the shared behavior lives in
a crate and both labs import the crate instead.

| lab | depends on labs |
| --- | --- |
| competitive_facility | competition_lab, constraint_lab, director_lab, mutable_facility, observation_lab |
| constraint_lab | observation_lab |
| door_lab | observation_lab |
| facility_sandbox | climbing_lab |
| fps_elevation_lab | fps_controller_lab |
| fps_facility_lab | fps_controller_lab, observation_lab, room_lab |
| fps_hybrid_match_lab | competition_lab, competitive_facility, director_lab, fps_controller_lab, fps_maze_lab, mutable_facility, observation_lab |
| fps_match_lab | competition_lab, competitive_facility, director_lab, fps_facility_lab, mutable_facility, observation_lab, room_lab |
| fps_maze_lab | constraint_lab, fps_controller_lab, observation_lab |
| fps_observation_lab | observation_lab |
| fps_reroute_lab | constraint_lab, fps_controller_lab, fps_maze_lab, observation_lab |
| fps_rewire_lab | fps_visibility_lab, observation_lab |
| fps_visibility_lab | observation_lab |
| match_replay | competition_lab, competitive_facility, director_lab, observation_lab |
| mutable_facility | constraint_lab, observation_lab |
| net_match_lab | competitive_facility, fps_hybrid_match_lab, fps_maze_lab, network_lab |
| network_lab | fps_controller_lab |
| progression_lab | competitive_facility |
| replay_lab | competition_lab |
| route_lab | observation_lab |

`observation_lab` is the most-depended-upon lab (the root of the simulation), which
is why the architecture sequence promotes it early (R5). `fps_controller_lab` is the
next hub (R7 / traversal).

## Finding 4 — first extraction target selected: `observed_style` (R1)

`style_lab` is the cleanest leaf in the graph:

- It is a **direct** `game -> labs/*` dependency (so promoting it removes a real
  production-on-lab edge).
- It has **no workspace path dependencies** (depends only on Bevy) and **no other lab
  depends on it** — it appears nowhere in the cross-lab map above. Extracting it
  cannot ripple into another lab.
- It already isolates the reusable abstraction in its own module
  ([labs/style_lab/src/style.rs](../labs/style_lab/src/style.rs)): semantic state → visual
  treatment, with legibility tests.

R1 moves that module into `crates/observed_style`, leaves `style_lab` as the visual
proof app, and points `game` at the crate. (`observed_assets` from `asset_lab` is the
R2 follow-up; it is not yet a `game` dependency, so it does not reduce the edge count
but removes duplicated asset-path strings.)

## R0 exit criteria — met

- ✅ `cargo metadata --no-deps` lists the current `game -> labs/*` dependencies (10,
  enumerated above; 14 transitively).
- ✅ The target dependency direction and the guardrail rules are documented (here and
  in [architecture.md](../architecture.md)).
- ✅ The first extraction target is selected: `observed_style` from `style_lab` (R1).
