# CLAUDE.md

Day-to-day working reference and command runbook for agents and developers.

## Core Documentation Map
- **Long-range Design & Active Rules:** See [agents.md](agents.md) (strictly governs architecture, simulation/presentation separation, stable IDs, styling).
- **Workspace Inventory & Crate/Lab Descriptions:** See [Catalogue.md](Catalogue.md).
- **Milestones & Next Phases:** See [ROADMAP.md](ROADMAP.md).

---

## Developer Commands

### Running Labs
Each prototype lab launches independently:
```powershell
cargo dev-run -p movement_lab      # ...or any other lab listed in Catalogue.md
```

### Running the Game
To run the main game:
```powershell
cargo dev-run -p observed_game
```

`dev-run` enables Bevy dynamic linking for faster iteration. Use ordinary
`cargo run` when validating a standalone/release-style executable.

### Running LAN Multiplayer
```powershell
cargo run -p observed_server -- --bind 0.0.0.0:47624 --name "Workshop"
cargo dev-run -p observed_game
cargo dev-run -p lan_lab       # resettable real-UDP loopback proof
```
The game discovers hosts by LAN broadcast and also accepts direct `IP:port` entry.
See [docs/lan_integration.md](docs/lan_integration.md) for protocol and deployment details.

### Verifying Changes
Run these commands before claiming completion of any task (warnings must be resolved, not suppressed):
```powershell
cargo fmt --all
cargo dev-clippy
cargo dev-test
```
*Note: Make sure resetting the lab removes all of its Bevy entities/resources without leaking state.*

### Authoring Hex Tiles & Capturing Evidence
Tileforge/tilec workflow, showcase PNG capture, and bot-POV GIF capture: see the
`capture-evidence` skill ([.claude/skills/capture-evidence/SKILL.md](.claude/skills/capture-evidence/SKILL.md))
and [docs/tile_authoring.md](docs/tile_authoring.md).
