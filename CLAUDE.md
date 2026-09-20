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

The gate deliberately skips a few long instrumentation tests. Each is marked
`#[ignore = "..."]` with its cost and the reason, and they are **evidence rather than
regression cover** — they print playtest measurements and assert nothing. Left in the
gate they cost ~25 minutes a run, and a gate nobody runs protects nothing.

Run everything periodically, and whenever you change the simulation they measure:

```powershell
cargo dev-test-all
```

Anything that *asserts* belongs in `dev-test`, however slow. If you find yourself
wanting to `#[ignore]` a test with assertions in it, make the test faster instead.
*Note: Make sure resetting the lab removes all of its Bevy entities/resources without leaking state.*

### The shared build cache, and why builds cannot overlap

`.cargo/config.toml` keeps a Windows `target-dir` deliberately and tells you to override
it per machine with `CARGO_TARGET_DIR`, because mold's `mmap(MAP_SHARED)` output cannot
land on the ntfs3 mount this repo lives on. That override is **one absolute cache shared
by the primary checkout and every worktree** — which is the intended trade, one warm
cache instead of a cold one per tree, and it has a sharp edge.

**Do not run cargo in two worktrees at the same time.** Concurrent builds overwrite each
other's artifacts for the same crate names, and the errors that come out are fiction
that *reproduces* — a `pub fn` the compiler says is missing while it sits in front of
you, or an `extern location ... does not exist` for a crate that built fine:

```
error[E0432]: no `step_character_in_query` in `rapier_controller`
error: extern location for observed_game does not exist: .../libobserved_game-*.rlib
```

Both have been seen on branches that were entirely healthy, and one nearly had a
gate-green branch reported broken. Because it repeats, running it twice does not tell
you whether it is real.

**Never `cargo clean` while another tree may be building.** In a shared cache it
destroys their build too, and two people each clearing a phantom that way will
invalidate each other indefinitely.

To build two trees at once, give each its own cache and accept a cold first build:

```bash
CARGO_TARGET_DIR=/srv/build-cache/cargo-<worktree> cargo dev-test
```

Check `df -h` on the cache filesystem first; a full cache dir is tens of gigabytes and
a disk-full mid-link is a worse afternoon than a slow build.

#### When the cache filesystem fills up, it is cargo, not logs

It has filled twice. Both times the instinct was to look for runaway logging, and both
times that was wrong: journald, `/var/log` and every Claude transcript together come to
roughly **300 MB**. Measured on 2026-09-19, `debug/deps` alone held **51 GB across 16,366
files**.

Cargo never garbage-collects. Every time a dependency version, feature flag or profile
changes, the old hashed artifact stays in `deps/` forever, so a long-lived shared cache
accumulates indefinitely whether or not anyone is building. On that day 41 GB of the 51
had not been touched in a day, and 19.5 GB not in a week.

**Check nothing is building first** (`ps -eo args | grep rustc`), then prune in
increasing order of boldness. Everything here costs a rebuild at worst — cargo
regenerates whatever it finds missing:

```bash
# 1. Example binaries. Nothing depends on these persisting.       (~11 GB)
rm -rf /srv/build-cache/cargo/debug/examples

# 2. Incremental state. Regenerated automatically.                (~1.6 GB)
rm -rf /srv/build-cache/cargo/debug/incremental

# 3. Dependency artifacts nobody has touched in a week.           (~19 GB)
#    This is what `cargo-sweep --time 7` does, without the dependency.
find /srv/build-cache/cargo/debug/deps -maxdepth 1 -type f -mtime +7 -delete
```

That took the cache from 67 GB to 37 GB with the warm path intact — a `cargo check` on an
untouched crate still finished in 0.15s.

**Still never `cargo clean`.** It destroys every tree's build, not only yours. The prune
above is the safe version: it removes what is stale rather than what is current.

Per-worktree caches are the other half of the budget. Each is 13-15 GB, so **delete a
worktree's cache the moment its branch merges** rather than letting them stack up.

### Authoring Hex Tiles & Capturing Evidence
Tileforge/tilec workflow, showcase PNG capture, and bot-POV GIF capture: see the
`capture-evidence` skill ([.claude/skills/capture-evidence/SKILL.md](.claude/skills/capture-evidence/SKILL.md))
and [docs/tile_authoring.md](docs/tile_authoring.md).
