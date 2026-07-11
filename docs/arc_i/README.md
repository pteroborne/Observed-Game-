# Arc I — Light & Line: implementation plans

One hand-off document per phase (67–72), each self-contained enough to give a
fresh implementation agent. Arc design and staging:
[../light_and_line_arc_plan.md](../light_and_line_arc_plan.md). Bug provenance:
[../bug_backlog.md](../bug_backlog.md).

## Hand-off model

```text
Wave 1: [67 ∥ 68]    (audio mix + bot-POV fix ∥ new lighting_lab crate)
Wave 2: [69]         (game light staging + luminance gate — touches place spawners + audit)
Wave 3: [70 ∥ 71]    (district light data ∥ shell/hallway geometry grammar)
Wave 4: [72]         (arc gate: evidence refresh + user playtest)
```

- The **parent session** dispatches one agent per phase with the phase doc as the
  brief, reviews against the success criteria, runs the full verification itself,
  and commits — **agents do not commit**; concurrent agents use isolated worktrees.
- **Every phase lands as its own commit** before the next wave dispatches.
- Shared-file rule: `game/src/tests.rs` is append-only for concurrent phases; the
  parent resolves overlap at commit time.

## Standing rules (carried from Arc H, binding)

The **falsifiable-evidence rule** and all eight **standing invariants** of
[../arc_h/README.md](../arc_h/README.md) apply verbatim to every phase: agents
view their own captures and describe them; the Legibility Contract and comfort
language are inviolable; `observed_style` owns player-facing color; architecture
ratchets, determinism hygiene, drop-in fallbacks, Match resource lifecycle, and
regression gates all hold.

Arc I additions:

1. **Lab before game.** No game-side lighting change lands unless the register it
   implements was first demonstrated in `lighting_lab` and its capture viewed.
2. **Capture environment is standardized** (Arc H Phase 66 findings): evidence
   runs set `OBSERVED2_BOTS=all`; world/HUD captures run on default Vulkan;
   `WGPU_BACKEND=dx12` only for Results-screen captures. Capture drivers must
   keep the phase-0 state re-assert pattern (`game/src/evidence/capture/mod.rs`).
3. **The luminance corridor is the arc's signature gate:** once Phase 69 lands
   it, every subsequent phase's captures must pass it.

## Verification recipe (every phase, zero warnings)

```
cargo fmt --all
cargo clippy --workspace --all-targets       # zero warnings — resolve, don't suppress
cargo test --workspace                        # all green; note count + wall time
```

## Phase index

| Phase | Doc | Wave | Closes |
| --- | --- | --- | --- |
| 67 — Hygiene Opener: Audio Mix & Bot-POV Stall | [phase_67_audio_mix_bot_stall.md](phase_67_audio_mix_bot_stall.md) | 1 | Bug backlog #5, #7 |
| 68 — lighting_lab: Nine Liminal Dioramas | [phase_68_lighting_lab.md](phase_68_lighting_lab.md) | 1 | — (feasibility) |
| 69 — Light Staging in the Game + Luminance Gate | [phase_69_light_staging_gate.md](phase_69_light_staging_gate.md) | 2 | Bug backlog #6 |
| 70 — District Light Identities | [phase_70_district_light_identities.md](phase_70_district_light_identities.md) | 3 | — |
| 71 — District Geometry Language | [phase_71_district_geometry_language.md](phase_71_district_geometry_language.md) | 3 | Post-MVP idea #4 |
| 72 — Arc Gate: Evidence Refresh & Playtest | [phase_72_arc_gate.md](phase_72_arc_gate.md) | 4 | — |
