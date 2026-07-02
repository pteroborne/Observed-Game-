# CLAUDE.md

Day-to-day working reference and command runbook for agents and developers.

## Core Documentation Map
- **Long-range Design & Active Rules:** See [agents.md](agents.md) (strictly governs architecture, simulation/presentation separation, stable IDs, styling).
- **Workspace Inventory & Crate/Lab Descriptions:** See [Catalogue.md](Catalogue.md).
- **Milestones & Next Phases:** See [ROADMAP.md](ROADMAP.md).

---

## Workspace Orientation
```text
/
├── agents.md                 # Core design rules and AI/developer constraints
├── CLAUDE.md                 # Streamlined command cheat sheet (this file)
├── Catalogue.md              # Crate/lab architecture descriptions and bloated files list
├── ROADMAP.md                # Completed milestones and active phase details
├── Cargo.toml                # Workspace manifest listing crates and labs
├── crates/                   # Promoted production crates (observed_core, observed_doors, etc.)
├── labs/                     # Independently runnable prototype labs
├── game/                     # Assembled first-person game entry point
└── docs/                     # Documentation archives, decisions, and evidence
```

---

## Developer Commands

### Running Labs
Each prototype lab launches independently:
```powershell
cargo run -p movement_lab      # ...or any other lab listed in Catalogue.md
```

### Running the Game
To run the main game:
```powershell
cargo run -p observed_game
```

### Verifying Changes
Run these commands before claiming completion of any task (warnings must be resolved, not suppressed):
```powershell
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```
*Note: Make sure resetting the lab removes all of its Bevy entities/resources without leaking state.*

### Capture Evidence (Showcase screenshot)
Renders the lab/showcase, saves a PNG, and exits:
```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/<lab_name>.png"; cargo run -p <lab_name>
```

### Capture Bot POV walkthrough GIF
To build interactive loopable GIFs for evidence using FFmpeg:
1. Run bot POV capture:
   ```powershell
   $env:OBSERVED2_CAPTURE_BOT = "docs/evidence/bot_pov"; cargo run -p observed_game
   ```
2. Compile frames into an optimized GIF using FFmpeg:
   ```powershell
   ffmpeg -y -i docs/evidence/bot_pov/bot_pov_%03d.png -vf "palettegen" docs/evidence/bot_pov/palette.png
   ffmpeg -y -framerate 10 -i docs/evidence/bot_pov/bot_pov_%03d.png -i docs/evidence/bot_pov/palette.png -filter_complex "[0:v][1:v]paletteuse" docs/evidence/bot_pov/bot_pov.gif
   ```
