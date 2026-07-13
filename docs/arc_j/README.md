# Arc J — Solid Connections: implementation plans

One hand-off document per phase (73–76), each self-contained enough to brief a
fresh implementation agent. Arc thesis and scope live in
[../../ROADMAP.md](../../ROADMAP.md) under **Arc J — Solid Connections
(Rapier3D Integration)**.

Arc J is the traversal-ground-truth arc: move the played game's collision onto
deterministic raw `rapier3d`, and make thresholds explicit enough that rendered
apertures, physical apertures, and graph connections **cannot disagree**. A
corridor becomes a stable *place* with authored threshold sockets, not an
implicit `(from, to)` room pair; every live connection is one reciprocal
room-socket ↔ corridor-socket attachment.

## Hand-off model

```text
Wave 1: [73]   Deterministic Rapier foundation + stable identity types   [DONE — 8cd5f63]
Wave 2: [74]   First-class corridor & threshold runtime (the contract)   ← next
Wave 3: [75]   Room/hall variation parity + consumer migration (fan-out)
Wave 4: [76]   Multi-exit junction gate + user playtest
```

- The waves are **strictly serial**: each phase rewrites the contract the next
  one builds on. There is no cross-phase parallelism. The *only* place multiple
  agents genuinely help is **Phase 75**, whose ~12 consumer-subsystem migrations
  fan out once Phase 74 has redefined the contract.
- The **parent session** dispatches the agent(s) with the phase doc as the brief,
  reviews against the success criteria, runs the full verification itself, and
  commits — **agents do not commit**. Concurrent Phase-75 agents use isolated
  worktrees.
- **Every phase lands as its own commit** before the next wave dispatches.

## Standing rules (carried from Arcs H/I, binding)

The **falsifiable-evidence rule** and all standing invariants of
[../arc_h/README.md](../arc_h/README.md) apply verbatim: the Legibility Contract
is inviolable, `observed_style` owns player-facing color, architecture ratchets
(`arch_check`) hold, determinism hygiene is mandatory, drop-in fallbacks stay
intact, the Match resource lifecycle leaks nothing on reset/exit, and every phase
ends with evidence a human can falsify.

Arc J additions:

1. **One collision authority.** Production movement resolves against Rapier only.
   Any retained custom traversal maths is an isolated reference/test utility, not
   a second live controller. (Full enforcement is Phase 76's gate; do not
   *add* a second authority in the meantime.)
2. **Simulation owns topology.** Presentation reads simulation-owned connectivity;
   it never authors it. Persistent identity uses domain IDs
   (`PlaceId`/`CorridorId`/`ThresholdId`), never Bevy `Entity`.
3. **Rendered structure and Rapier structure derive from the same place geometry**
   (already true via `place_structural_primitives`; keep it true).

## Verification (run by the parent before each commit)

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets   # warnings are failures
cargo test --workspace
```

Plus the phase-specific evidence named in each doc.
