# Capture Pipeline Lab

**Phase A5** of the [Bevy asset-integration roadmap](../../docs/bevy_asset_integration_roadmap.md):
the evidence capture candidate, [`bevy_image_export`](https://crates.io/crates/bevy_image_export).

It answers one question: **can the existing screenshot evidence loop become more
repeatable?** The lab keeps `bevy_image_export 0.16.0` isolated here, renders a
deterministic offscreen camera target, and exports both a still and a short image
sequence under lab-owned evidence directories.

Compatibility gate: `bevy_image_export 0.16.0` is the Bevy `0.18` line and is
pinned exactly because the latest `0.17.0` line targets Bevy `0.19`. The lab
enables only the `png` feature.

## Functionality Evidence

The capture command writes:

- `docs/evidence/capture_pipeline_lab/still/00001.png`
- `docs/evidence/capture_pipeline_lab/sequence/00001.png` through `00006.png`

The scene is a tiny neon-noir corridor with a two-panel door, a route spine, a
runner marker, and a reroute flash. The sequence samples fixed ticks across the
door opening and reroute flash.

## What It Demonstrates

- **Offscreen export**: the exported images come from a render target image, not
  from the primary window screenshot hook.
- **Still plus sequence**: one command writes a single deterministic still and a
  six-frame transition sequence.
- **Lab-scoped output paths**: exports go under
  `docs/evidence/capture_pipeline_lab/`, so they do not overwrite unrelated lab
  screenshots.
- **Presentation-only capture**: capture frames sample `CaptureSnapshot` values;
  tests assert sampling does not advance or pause the fixed-step timeline.
- **Clean reset**: `R` despawns and rebuilds the scene without leaking cameras,
  UI roots, or door panels.

## Controls

- `R`: reset/rebuild the scene
- `Space`: restart the interactive transition
- `F1`: toggle the debug overlay

## Success Conditions

1. The overlay shows `[PASS]`.
2. Running the capture command produces one still frame and six sequence frames.
3. The sequence includes the closed door, opening door, and reroute flash states.
4. Reset leaves one window camera, one export camera, one UI root, and two door
   panels.

## Decision

`bevy_image_export` remains a live candidate for repeatable evidence capture,
but it stays lab-local. Promote shared capture helpers only after at least two
labs adopt the same offscreen export shape.

Dependency impact from `cargo tree -p capture_pipeline_lab -e normal --depth 1`:
the lab depends directly on `bevy 0.18.1`, `bevy_image_export 0.16.0`, and
`observed_style`. The export crate adds its GPU readback/image sequence stack
(`futures`, `futures-lite`, `image`, `thiserror`, `bytemuck`, and `wgpu`) on top
of the existing Bevy graph.

## Regenerating The Evidence Frames

```powershell
$env:OBSERVED2_CAPTURE_PIPELINE = "docs/evidence/capture_pipeline_lab"
cargo run -p capture_pipeline_lab
```
