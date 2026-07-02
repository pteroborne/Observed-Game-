# Contention Lab

The Contention Lab is the Phase 38 **Contested Observation** feasibility probe — it isolates the
core mechanic: *shared, objective observation over a deterministically-rewiring decoherence graph,
layered with team-attributed anchors and team-local knowledge*.

The technical question: **does shared, team-attributed observation with anchors create real
competitive interaction while preserving determinism and solvability?**

The model (`observed_observation::contention::ContentionWorld`) extends
`ObservationWorld` with team membership, anchors (hard freezes on rooms), and per-team knowledge
ledgers (fog of war over truth, not geometry). Observation is shared: when any team occupies or
anchors a room, every team's doorways freeze there. Knowledge is private: each team maintains its
own ledger of which doorway links it has personally observed and when.

## What it demonstrates

- **Shared observation, private knowledge**: one team's presence or anchor freezes a room for
  all teams, but only the observing team records what it sees.
- **Anchor-based competition**: a team can freeze a room without occupying it, creating a
  strategic resource (e.g., locking a choke point) independent of member presence.
- **Solvability preservation**: even as teams strategically rewire the unobserved graph via their
  movements and placement, the guard ensures no member is ever stranded from the exit.
- **Determinism under fog**: the shared graph rewires deterministically; each team's diverging
  ledger creates asymmetric knowledge (one team may see a new topology before the other) even
  though the truth is one and shared.

## Controls

- **Arrows** (↑↓←→): move Team 0's member through doors (traverse)
- **D**: decohere now (rewire all unpinned doorways)
- **A**: toggle Team 0's anchor in Team 0's current room (place if absent, remove if present)
- **K**: cycle knowledge view: OFF → Team 0 → Team 1 → Team 2 → Team 3 → OFF
  - When a team is selected, draw only that team's known edges (from `known_edges`), faded by staleness
  - Demonstrates "reality is shared, knowledge is not"
- **R**: reset (rebuild the entire world; no leaked state per agents.md reset discipline)
- **F1**: toggle on-screen help text

## Layout

- **Rooms**: 3×3 grid (0–8), exit at room 8 (top-right)
- **Starting positions**: Teams 0, 1, 2 in corner rooms (0, 2, 6); Team 3 in the center room (4)
  to avoid the exit being trivially observed at rest
- **Colors**: Team 0 (red), Team 1 (blue), Team 2 (purple), Team 3 (orange)

## Visual language

- **Rooms**: squares at their world center, exit room outlined brighter
- **Links**: lines between door positions; pinned links draw thicker/brighter and are tinted
  by their first pin source's team color
- **Members**: filled circles in team colors at their room centers
- **Anchors**: small squares at room corners in team colors
- **Knowledge view**: known edges drawn faded, with older observations dimmed further

## Text overlay

Shows: tick, decoherence count, last decoherence attempts/revert status, anchor count,
and current knowledge view mode.

## Seed-corpus experiments

Experiments (seed-driven probe tests) live in `src/experiment.rs` (added by a separate task).

## Manual verification

1. Run `cargo run -p contention_lab`.
2. Watch the colored members and anchors interact: a member in a room freezes its doors for all teams.
3. Place an anchor (`A`) to test team-independent freezing: the anchor freezes its room even when
   the member leaves.
4. Press `K` to cycle knowledge views: notice that each team's ledger diverges as members move and
   the unobserved graph rewires—reality is shared, but what each team *knows* is not.
5. Press `D` to decohere and watch the solvability guard in action: the graph rewires while keeping
   all members reachable from the exit (room 8).
6. Press `R` to reset and confirm no leaked entities or state persist.
