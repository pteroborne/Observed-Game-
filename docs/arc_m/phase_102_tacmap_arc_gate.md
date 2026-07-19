# Phase 102 — Active-Level TAC Map & Arc Gate

**Status:** implementation complete; final hands-on gate open.

Survivor knowledge is authoritative, per-player simulation state. It records
glimpsed and traversed cells, known ports, and the revision last observed. A cell
becomes stale only when its own revision changes. Rival occupancy never enters
another player's ledger.

The TAC map displays one active floor with discovered-floor browsing
(`PageUp`/`PageDown` or D-pad), adjacent-level hints, known connections, and
distinct style-owned states for current, exit, traversed, glimpsed, stale,
anchored, and vertical cells. It never reconstructs topology from render
entities.

Remaining arc gate:

- install/open the repo TrenchBroom config, edit a copy, run `tilec build`, and
  inspect it through tile-lab hot reload and the game;
- hands-on ramp/shaft/seam and ordinary-fall-through pass;
- lighting/lantern/Guardian/TAC-map legibility pass;
- user playtest and reset/exit-cleanup confirmation.

The Phase 101 release gate now passes across ten commits (8.628 ms all-frame
p95, 8.563 ms worst mutation-attributed frame). The deterministic simulation,
replay boundary, match soak, and headless-versus-interactive relayout gates have
also been re-run successfully.

Multiplayer transport, six teams of four, and eliminated-team adversary control
remain explicitly deferred until these gates close.
