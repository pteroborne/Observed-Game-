# Phase 98 — Geometry Contract & Authored Kit

**Status:** complete; compiler/runtime contract verified.

Strict v2 sources describe either one seam-safe cell or one whole-room module.
The compiler rejects disconnected footprints, off-boundary or internal ports,
centre-only floors, insufficient capsule headroom, missing/steep ramp surfaces,
and collider budgets above 32 hulls per cell or 128 per room. Floor coverage is
sampled at the centre and six inset corners of every declared solid cell.

Whole-room modules are now production-consumable. Runtime selection requires an
exact room role, rotated footprint, every external port class, and every named
threshold. A match projects the selected room once from its anchor and never
also projects the per-cell fallback kit. Incremental mutation replaces the
whole footprint atomically.

The existing 1,332 v1 maps remain the played fallback kit; they were not
silently bulk-rewritten. Their established six-direction ramp walk, blueprint
coverage, exact footprint, collider-ID, and incremental/full-projection parity
tests remain green. New content receives the stronger v2 gates.
