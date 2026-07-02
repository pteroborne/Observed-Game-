# observed_diagnostics

Pure, engine-independent schemas and checks for visual diagnostics.

This crate turns rendered game state into agent-readable evidence:

- geometry footprints and preview AABBs for overlap checks
- threshold frame/leaf/light/label counts keyed by stable threshold IDs
- light and material brightness records for the legibility contract
- TAC-map expected/rendered element counts
- camera/monitor luminance checks for black-screen regressions

It deliberately has no Bevy dependency. Bevy-facing adapters live in the game crate and
serialize snapshots into this crate's `DiagnosticSnapshot` format.
