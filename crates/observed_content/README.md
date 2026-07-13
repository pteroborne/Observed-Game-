# observed_content

Pure schemas and validation for immutable Observed 2 content manifests. The crate
contains no Bevy, Rapier, renderer, filesystem watcher, or editor integration.

It deliberately exposes two revisions:

- `SimulationContentHash` covers traversal settings, module selection, convex bake
  fingerprints, ports, gameplay sockets, and navigation.
- `PresentationContentHash` covers the complete manifest, including district
  atmosphere and optional visual assets.

Network sessions and deterministic replays require the simulation hash. Presentation
hash mismatches may use declared procedural fallbacks because visual assets never own
gameplay state.
