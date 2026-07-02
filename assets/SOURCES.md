# Asset sources

All assets added for `ASSET_PLAN.md` are released under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). They were retrieved
on June 20, 2026.

The basic dev/debug wall and floor texture slots were refreshed from ambientCG on
July 1, 2026.

License references: [Kenney support](https://kenney.nl/support),
[ambientCG license](https://docs.ambientcg.com/license/), and
[Poly Haven license](https://polyhaven.com/license).

The selected Kenney models were repacked as self-contained GLB files so their
original `Textures/colormap.png` references cannot collide after renaming. Their
pivots were normalized as well: floor props use a bottom-center pivot and the
ceiling fixture uses a top-center pivot.

| Repository path | Original asset | Source |
| --- | --- | --- |
| `textures/wall.png` | `SheetMetal002_1K-PNG_Color.png` | [ambientCG Sheet Metal 002](https://ambientcg.com/view?id=SheetMetal002) |
| `textures/floor.png` | `Concrete048_1K-PNG_Color.png` | [ambientCG Concrete 048](https://ambientcg.com/view?id=Concrete048) |
| `textures/ceiling.png` | `MetalPlates006_1K-PNG_Color.png` | [ambientCG MetalPlates006](https://ambientcg.com/view?id=MetalPlates006) |
| `models/light_fixture.glb` | `lampSquareCeiling.glb` | [Kenney Furniture Kit](https://kenney.nl/assets/furniture-kit) |
| `models/exit_gate.glb` | `gate.glb` | [Kenney Modular Space Kit](https://kenney.nl/assets/modular-space-kit) |
| `textures/exit_panel.png` | Derived sign using `exitRight.png` and Kenney Future | [Kenney Game Icons](https://kenney.nl/assets/game-icons) and [Kenney UI Pack: Sci-Fi](https://kenney.nl/assets/ui-pack-sci-fi) |
| `models/player.glb` | `astronautA.glb` | [Kenney Space Kit](https://kenney.nl/assets/space-kit) |
| `models/bot.glb` | `alien.glb` | [Kenney Space Kit](https://kenney.nl/assets/space-kit) |
| `models/doorway.glb` | `structure-doorway.glb` | [Kenney Factory Kit](https://kenney.nl/assets/factory-kit) |
| `models/equipment.glb` | `machine_wirelessCable.glb` | [Kenney Space Kit](https://kenney.nl/assets/space-kit) |
| `models/decor_crate.glb` | `container.glb` | [Kenney Space Station Kit](https://kenney.nl/assets/space-station-kit) |
| `models/decor_console.glb` | `computer-wide.glb` | [Kenney Space Station Kit](https://kenney.nl/assets/space-station-kit) |
| `models/hazard.glb` | `warning-orange.glb` | [Kenney Factory Kit](https://kenney.nl/assets/factory-kit) |
| `sounds/footstep.ogg` | `footstep00.ogg` | [Kenney RPG Audio](https://kenney.nl/assets/rpg-audio) |
| `sounds/reroute.ogg` | `doorClose_002.ogg` | [Kenney Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds) |
| `sounds/escape.ogg` | `confirmation_004.ogg` | [Kenney Interface Sounds](https://kenney.nl/assets/interface-sounds) |
| `sounds/ambience.ogg` | `spaceEngineLow_000.ogg` | [Kenney Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds) |
| `textures/environment.hdr` | `empty_warehouse_01_1k.hdr` | [Poly Haven Empty Warehouse 01](https://polyhaven.com/a/empty_warehouse_01) |

## Notes

- `exit_panel.png` is a project-specific CC0 derivative: the icon and font are
  Kenney CC0 assets, arranged on a newly generated dark-green sign.
- `environment.hdr` is the optional advanced environment asset from item 16. It is
  **present but not rendered**: Bevy image-based lighting needs a `.ktx2` cubemap, not
  an equirectangular `.hdr`. To use it, bake `empty_warehouse_01` to a `.ktx2` cubemap
  (diffuse + specular) and wire an `EnvironmentMapLight` + `Skybox`; until then the
  match is lit by the directional sun, ambient, and fixture point lights.
- `prop.glb` predated this sourcing pass and is not covered by this ledger.
