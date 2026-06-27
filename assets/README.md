# assets/ — drop-in placeholder assets

This folder is Bevy's asset root. The shared **`observed_assets`** manifest defines a
set of "slots"; the **asset showcase lab** (`asset_lab`) reads it and, for each,
**uses a file if it's here, or falls back to a magenta placeholder if it isn't** — no
code changes. Run it to see exactly which slots are filled:

```powershell
cargo run -p asset_lab     # the overlay lists every slot + its exact path + status
```

## Where to drop what

| Slot    | Drop a file at            | Format        | Good CC0 source |
| ------- | ------------------------- | ------------- | --------------- |
| `wall`  | `assets/textures/wall.png`  | PNG / JPG   | [ambientCG](https://ambientcg.com/), [Poly Haven](https://polyhaven.com/textures) |
| `floor` | `assets/textures/floor.png` | PNG / JPG   | [ambientCG](https://ambientcg.com/) |
| `prop`  | `assets/models/prop.glb`    | glTF / GLB  | [Kenney](https://kenney.nl/assets), [Quaternius](https://quaternius.com/), [Poly Pizza](https://poly.pizza/) |
| `chime` | `assets/sounds/chime.ogg`   | OGG / WAV   | [Kenney audio](https://kenney.nl/assets?q=audio), [Freesound](https://freesound.org/) (filter to CC0) |

The larger first-person asset set is listed in
[`ASSET_PLAN.md`](ASSET_PLAN.md). Its completed selections and exact CC0
provenance are recorded in [`SOURCES.md`](SOURCES.md).

Drop the file, re-run, and the placeholder is replaced. To add a new drop-in point,
add a named `AssetSlot` const and a `SLOTS` row in
[`crates/observed_assets/src/lib.rs`](../crates/observed_assets/src/lib.rs) — both
`asset_lab` and `observed_game` then see it.

The **assembled game** (`cargo run -p observed_game`) consumes the full
[`ASSET_PLAN.md`](ASSET_PLAN.md): structure, fixtures, characters, gameplay props,
decor, hazard markers, four audio cues, and the optional HDR environment. Missing
files retain procedural mesh/colour or silent fallbacks.

> Note: both `asset_lab` and `observed_game` point Bevy's asset reader at this
> workspace `assets/` directory. (By default Bevy resolves `assets/` relative to the
> crate under `cargo run`, so a file dropped here would be ignored — the labs override
> that so drop-in works from the repo root.)

## Licensing

Prefer **CC0** (public domain — no attribution, commercial-OK). If you use CC-BY
assets, keep an `ATTRIBUTION.md` here listing the source and author. Don't commit
assets you don't have the rights to redistribute.

## Format notes

- **Textures:** PNG and JPG work out of the box; HDR is enabled for the optional
  panoramic environment.
- **Models:** Bevy's native format is **glTF 2.0** — prefer `.glb` (single file).
  FBX/OBJ are not loaded; convert to glTF (Blender exports glTF) first.
- **Sounds:** **OGG (Vorbis)** or **WAV**. MP3 is not enabled.

The reusable slot manifest lives in `crates/observed_assets` (consumed by both
`asset_lab` and `observed_game`); copy `asset_lab`'s present/placeholder projection
pattern into another lab to give it drop-in slots too.
