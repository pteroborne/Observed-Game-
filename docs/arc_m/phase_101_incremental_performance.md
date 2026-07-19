# Phase 101 — Incremental Physics & Presentation

**Status:** complete; ten-commit release gate passed.

Rapier now updates stable query-BVH leaves directly and reuses dormant slots.
The match applies only collider IDs whose actual specs changed. Presentation
despawns/respawns only changed cell parents, while per-cell revisions keep the
view and survivor map synchronized. The hex view uses low-quality SSAO because
the authored shell, semantic edge light, and fog already provide depth; bloom,
fog, HDR, and the lantern/Guardian lights remain enabled.

The final release capture at
`docs/evidence/arc_m/phase_101_final/timings.json` observed ten moving production
commits. Fixed commits were 8.334–12.950 ms, view deltas 2.608–3.551 ms, and
every mutation-attributed frame was 8.540–8.563 ms. All-frame p95 was 8.628 ms.
The report therefore records `passed: true`, closing both the player-reported
mutation freeze and the strict Arc M performance gate.

The earlier low-SSAO capture is retained as diagnostic history: it showed that
mutation work was already bounded while a transient present-cadence problem
still doubled unrelated frames. The final run verifies that issue no longer
reproduces under the release gate.
