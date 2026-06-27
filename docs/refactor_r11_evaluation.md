# Refactor Phase R11 — Bevy Ecosystem Dependency Evaluation

Decision artifact for **Roadmap Phase R11**. Now that the refactor graph is clean
(R0–R10: the game depends only on `crates/*`, Rule 1 holds, 490 workspace tests green),
this phase evaluates the Bevy Assets candidates catalogued in
[architecture.md](../architecture.md) against the project's **Dependency Policy** and
decides accept / defer for each.

Evaluated: 2026-06-26.

## The bar (from architecture.md)

Before adopting any third-party Bevy asset:

1. **Prove the problem exists in this codebase** (not hypothetically).
2. **Check Bevy `0.18.1` compatibility** or plan a deliberate Bevy upgrade.
3. Add the dependency **behind the smallest adapter crate**.
4. Keep **domain state independent** of the plugin's component/resource types.
5. Add **one proving lab** + **one guard test** for the adapter.
6. Document **maintenance cost and fallback** behavior.

A candidate is **accepted** only if it clears step 1 (a real, current problem) and the
adapter/lab/fallback work is justified by the value. Otherwise it is **deferred** with a
concrete *adopt-when* trigger, so the decision is revisited when the problem actually
appears — not carried as speculative complexity.

## Cross-cutting gate: Bevy `0.18.1` compatibility (verified)

The workspace is pinned to `bevy = 0.18.1` (released 2026-03-04). The latest stable Bevy
is `0.19.0` (2026-06-19) — one minor ahead, a week old. Most ecosystem crates track the
latest Bevy and frequently lag a release, so each candidate's compatibility was checked
against crates.io, with this conclusion:

- The accepted candidate (**`bevy-inspector-egui`**) **is compatible with `0.18.1`**:
  version `0.36.0` requires `bevy ^0.18.0` (pulling `bevy_egui ^0.39`). This was verified
  not just by version strings but by **actually building it** — `bevy_egui 0.39.1` +
  `bevy-inspector-egui 0.36.0` compile cleanly against the pinned `bevy 0.18.1`
  (`cargo check -p inspector_lab --features dev_tools`). **So no Bevy upgrade is needed.**
- A Bevy upgrade `0.18.1 → 0.19` was therefore evaluated and **rejected**: it is a large
  breaking change across the whole 40+-crate, 490-test workspace, undertaken for no
  benefit, since the one accepted dependency already supports `0.18.1`.
- `bevy_screen_diagnostics` is **not** `0.18`-compatible (latest `0.8.1` pins
  `bevy ^0.16.0` and is unmaintained for newer Bevy) — but it is unnecessary: Bevy ships a
  built-in `FrameTimeDiagnosticsPlugin` for FPS/frame timing, used instead with zero
  external dependency.

For the deferred candidates, the same gate stands: verify a `0.18.1`-compatible release
(or plan an upgrade) before adopting.

## Per-area decisions

| Area | Candidate(s) | Problem exists now? | Decision | Adopt-when trigger |
| --- | --- | --- | --- | --- |
| Asset loading | `bevy_asset_loader`, `bevy_common_assets`, `bevy_embedded_assets` | **No** | **Defer** | state-scoped async loading / progress screens exceed the manifest |
| Debug inspection | `bevy-inspector-egui` (✓ 0.18.1) — *not* `bevy_screen_diagnostics` (0.16-stuck) | Yes (dev-only) | **Accept** — [`inspector_lab`](../labs/inspector_lab), default-off `dev_tools` feature | — (adopted) |
| Vector / tac-map drawing | `bevy_vector_shapes`, `bevy_svg`, `bevy_mod_gizmos` | **No** | **Defer** | the tac-map needs richer vector graphics than Node boxes + gizmos |
| Input mapping | `leafwing_input_manager`, `bevy_enhanced_input` | **No** | **Defer** | rebinding / controller config becomes a requirement (a current non-goal) |
| Persistence | `bevy-persistent`, `bevy_pkv`, `bevy-settings` | **No** | **Defer** | the game needs disk-backed saves/settings beyond the serialize string |
| Network transport | `lightyear`, `bevy_quinnet`, `naia`, `aeronet` | **No** | **Defer** | online play with a real transport (explicit production-hardening, a non-goal) |

### 1. Asset loading — Defer

*Current state.* [`observed_assets`](../crates/observed_assets/src/lib.rs) (promoted in
R2) is a flat slot manifest with synchronous presence checks (`slot_present`,
`slot_full_path`); the game loads each present file or falls back procedurally. There is
no async pipeline, no loading-state machine, no progress UI.

The architecture's own note: *"consider `bevy_asset_loader` only if state-scoped loading
becomes more complex than the current manifest."* It has not. Adopting it now would add a
plugin + states for a problem that does not exist.

*Fallback / trigger.* Keep the manifest. Adopt when the game gains a real async loading
state (e.g. a streaming open world or large authored asset set with a progress screen).

### 2. Debug inspection — **Accept** (`bevy-inspector-egui` behind `dev_tools`)

*Current state.* Every lab ships a bespoke debug overlay with a `[PASS]`/`[FAIL]`
entity-health line and gizmo visualizations; the game has an in-match HUD + tac-map.
These encode *required debug semantics* (the documented per-lab success conditions) that a
generic inspector does not replace — so the inspector is **additive**, not a replacement.

This is the one candidate that clears the policy: a live ECS inspector genuinely helps
during development, it is purely additive, and — critically — **`bevy-inspector-egui`
`0.36.0` is compatible with the pinned Bevy `0.18.1`** (verified by building, see the gate
above). It is adopted with the full policy treatment:

- **Smallest adapter + proving lab.** [`labs/inspector_lab`](../labs/inspector_lab) is the
  single seam where the dependency enters: a `DevToolsPlugin` adds Bevy's built-in
  `FrameTimeDiagnosticsPlugin` always, and — only behind the **default-off `dev_tools`
  cargo feature** — `bevy_inspector_egui::quick::WorldInspectorPlugin`. The optional dep
  is `dep:bevy-inspector-egui`, so the normal workspace build/test pulls **no egui** at
  all; `cargo run -p inspector_lab --features dev_tools` launches with the inspector.
- **Domain state independent of the plugin's types** (policy rule 4). The lab's entities
  are plain Bevy entities; the inspector reads them generically. Removing the feature
  removes nothing but the inspector.
- **Guard test + fallback.** A headless test boots the fallback path (no feature → no
  egui/window) and asserts the built-in diagnostics + the inspectable entities; the
  `dev_tools` path is guarded by `cargo check -p inspector_lab --features dev_tools`,
  which proves the optional dep compiles against `0.18.1`. The fallback (custom overlay) is
  fully functional without the inspector.
- **No `bevy_screen_diagnostics`.** It is 0.16-stuck; Bevy's built-in
  `FrameTimeDiagnosticsPlugin` covers FPS/frame timing with zero external dependency.

*Maintenance cost.* `bevy-inspector-egui` pulls `bevy_egui` + `egui` — a real surface, but
it is **dev-only and feature-gated off**, so it never ships in the game and never affects
the default build/test. The cost is paid only by developers who opt in.

### 3. Vector / tac-map drawing — Defer

*Current state.* The tac-map ([`screens/hud.rs`](../game/src/screens/hud.rs)) draws rooms,
route bars, and markers as absolutely-positioned `Node` boxes (`tac_box`); debug lines use
Bevy `Gizmos`. Both are sufficient and dependency-free.

*Fallback / trigger.* Keep Node boxes + gizmos. Adopt `bevy_vector_shapes` if the tac-map
(or killcam) needs anti-aliased strokes, curves, or filled polygons that Node/gizmo
drawing handles poorly.

### 4. Input mapping — Defer

*Current state.* `PlayerIntent` is now pure (R4); each lab/the game samples
keyboard/mouse into it directly (e.g. [`screens/input.rs`](../game/src/screens/input.rs)).
Rebinding and controller configuration are **explicit non-goals**. A mapping crate would
add a layer with no consumer.

*Fallback / trigger.* Keep hand-rolled sampling behind the pure `PlayerIntent` boundary —
which is exactly the seam a mapping crate would plug into later, at zero cost now. Adopt
`leafwing_input_manager` as the Bevy input adapter when rebinding / multi-device config
becomes a requirement.

### 5. Persistence — Defer

*Current state.* [`observed_progression`](../crates/observed_progression/src/progression.rs)
already serializes a `Profile` to a compact round-tripping string (`serialize`/`parse`);
the model is storage-agnostic. There is no disk save or settings file yet.

*Fallback / trigger.* Keep the serializable domain structs. Adopt `bevy-persistent` /
`bevy_pkv` when the game needs durable on-disk saves/settings — it slots behind the
existing serializable types without changing them.

### 6. Network transport — Defer

*Current state.* [`observed_net`](../crates/observed_net/src/network.rs) proves
deterministic lockstep over an in-process `SimulatedNetwork` (with real loss / delay /
duplication / reordering). Real sockets, relays, and NAT traversal are **explicit
production-hardening / non-goals**.

The architecture is emphatic: *"Keep the deterministic lockstep model pure. When online
networking becomes active, choose topology first, then spike one transport behind
`observed_net`."* The lockstep model is already isolated in a crate, ready for that spike.

*Fallback / trigger.* Keep `SimulatedNetwork` for tests/feasibility. Adopt a transport
(`lightyear` / `bevy_quinnet` / `aeronet`) behind `observed_net` when building actual
online play — topology decision first.

## Conclusion

**One dependency accepted, five deferred.** The live ECS inspector (`bevy-inspector-egui`)
is adopted behind the default-off `dev_tools` feature in [`inspector_lab`](../labs/inspector_lab)
— verified compatible with the pinned **Bevy 0.18.1** (so the evaluated `0.18.1 → 0.19`
upgrade was rejected as unnecessary), feature-gated so the default build/test pulls no
egui, with an adapter + guard test + working fallback. The other five candidate areas are
deferred because the problem they solve does not yet exist in the codebase (synchronous
asset manifest, Node/gizmo tac-map, hand-rolled input behind the pure `PlayerIntent` seam,
string-serializable progression, in-process lockstep) — adopting them now would
re-introduce the speculative complexity the R0–R10 refactor removed. Each carries a
concrete *adopt-when* trigger and a documented fallback, so the decision is revisited on
real need.

The refactor arc (R0–R11) is complete: the dependency graph expresses the intended
architecture, the one justified ecosystem dependency is adopted under the policy, and the
policy for growing the set further is recorded.
