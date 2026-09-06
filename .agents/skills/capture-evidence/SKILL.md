---
name: capture-evidence
description: Authoring hex tiles (the forge, tilec validate/build, headless preview) and capturing evidence from labs or the game — showcase PNG screenshots and bot-POV walkthrough GIFs via FFmpeg. Use when regenerating .map sources, recompiling the tile catalog, or producing evidence images for docs/evidence.
---

# Authoring & Evidence Capture

## Authoring Hex Tiles

Full workflow (forge primitives, contract cheat sheet, lab preview scripts, gotchas): see [docs/tile_authoring.md](../../../docs/tile_authoring.md).

```powershell
cargo run -p observed_authoring --bin tilec -- gen-tiles                   # regenerate authored .map sources
cargo run -p observed_authoring --bin tilec -- validate <map>              # contract check
cargo run -p observed_authoring --bin tilec -- build                       # recompile assets/tiles catalog
$env:OBSERVED2_SCRIPT = "scratch/<view>.json"; cargo dev-run -p hex_tile_lab   # headless preview capture
```

## Capture Evidence (Showcase screenshot)

Renders the lab/showcase, saves a PNG, and exits:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/<lab_name>.png"; cargo dev-run -p <lab_name>
```

## Capture Bot POV walkthrough GIF

To build interactive loopable GIFs for evidence using FFmpeg:

1. Run bot POV capture:
   ```powershell
   $env:OBSERVED2_CAPTURE_BOT = "docs/evidence/bot_pov"; cargo dev-run -p observed_game
   ```
2. Compile frames into an optimized GIF using FFmpeg:
   ```powershell
   ffmpeg -y -i docs/evidence/bot_pov/bot_pov_%03d.png -vf "palettegen" docs/evidence/bot_pov/palette.png
   ffmpeg -y -framerate 10 -i docs/evidence/bot_pov/bot_pov_%03d.png -i docs/evidence/bot_pov/palette.png -filter_complex "[0:v][1:v]paletteuse" docs/evidence/bot_pov/bot_pov.gif
   ```
