# Phase 102 — Active-Level TAC Map & Arc Gate

**Status:** `[x]` — complete (2026-07-20).

Survivor knowledge is authoritative, per-player simulation state. It records
glimpsed and traversed cells, known ports, and the revision last observed. A cell
becomes stale only when its own revision changes. Rival occupancy never enters
another player's ledger.

The TAC map displays one active floor with discovered-floor browsing
(`PageUp`/`PageDown` or D-pad), adjacent-level hints, known connections, and
distinct style-owned states for current, exit, traversed, glimpsed, stale,
anchored, and vertical cells. It never reconstructs topology from render
entities.

### Arc Gate Verification (Completed 2026-07-20)

- `tilec audit` (1335 valid sources, content hash `690b0b1b...`) and `tilec build` verified; TrenchBroom configuration, tile-lab hot reload, and runtime catalog emission operate deterministically without Rust edits.
- Hands-on ramp/shaft/seam coverage verified; player capsule clearance and floor coverage hold across vertical ramps and full-height wellshafts with no fall-through defects.
- Lighting, lantern guidance, Guardian chase/capture recovery, and active-level TAC map legibility verified under `observed_style` contracts.
- Deterministic match soak, replay boundary, headless-versus-interactive relayout, and match reset/exit cleanup verified green.

The Phase 101 release gate passes across ten commits (8.628 ms all-frame p95, 8.563 ms worst mutation-attributed frame). Arc M is fully shipped.

