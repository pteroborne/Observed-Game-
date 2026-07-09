# Asset sources

All assets added for `ASSET_PLAN.md` are released under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). They were retrieved
on June 20, 2026.

The basic dev/debug wall and floor texture slots were refreshed from ambientCG on
July 1, 2026.

The Phase 49 ambience loop variants were procedurally synthesized in-repo on
July 6, 2026 and use no external source material.

The 2.5D dev placeholder sprites were selected from Kenney CC0 packs on
July 9, 2026. Only the curated PNG files listed below are checked in, not the full
source archives.

The OpenGameArt 2.5D FPS raw intake was downloaded on July 9, 2026 into
`assets/oga_25d/raw/`. These are source artifacts for the roadmap in
`sprite_roadmap.md`, not game-ready asset slots yet. Most selected visual sources
are CC0; the two Little Robot Sound Factory sound libraries are CC-BY 3.0 and must
remain raw/reference-only unless attribution support is added or they are replaced.

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
| `sounds/ambience.ogg` | Procedural low-transient facility loop | In-repo synthesis |
| `sounds/klaxon.ogg` | `computerNoise_002.ogg` | [Kenney Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds) |
| `sounds/collapse_sting.ogg` | `explosionCrunch_004.ogg` | [Kenney Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds) |
| `sounds/ui_click.ogg` | `click_001.ogg` | [Kenney Interface Sounds](https://kenney.nl/assets/interface-sounds) |
| `sounds/ui_hover.ogg` | `tick_001.ogg` | [Kenney Interface Sounds](https://kenney.nl/assets/interface-sounds) |
| `sounds/jump.ogg` | `phaseJump1.ogg` | [Kenney Digital Audio](https://kenney.nl/assets/digital-audio) |
| `sounds/land.ogg` | `footstep04.ogg` | [Kenney RPG Audio](https://kenney.nl/assets/rpg-audio) |
| `sounds/ambience_archive.ogg` | Procedural archive district loop | In-repo synthesis |
| `sounds/ambience_reactor.ogg` | Procedural reactor district loop | In-repo synthesis |
| `sounds/ambience_atrium.ogg` | Procedural atrium district loop | In-repo synthesis |
| `sounds/ambience_foundry.ogg` | Procedural foundry district loop | In-repo synthesis |
| `sounds/ambience_hollow.ogg` | Procedural hollow district loop | In-repo synthesis |
| `sounds/ambience_spillway.ogg` | Procedural spillway district loop | In-repo synthesis |
| `sounds/ambience_junction.ogg` | Procedural junction ambience loop | In-repo synthesis |
| `sounds/ambience_monument.ogg` | Procedural monument ambience loop | In-repo synthesis |
| `sounds/ambience_gantry.ogg` | Procedural gantry ambience loop | In-repo synthesis |
| `sounds/ambience_corridor.ogg` | Procedural corridor ambience loop | In-repo synthesis |
| `sounds/ambience_void.ogg` | Procedural void ambience loop | In-repo synthesis |
| `textures/environment.hdr` | `empty_warehouse_01_1k.hdr` | [Poly Haven Empty Warehouse 01](https://polyhaven.com/a/empty_warehouse_01) |
| `sprites/runner_stand.png` | `PNG/Player/Poses/player_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/runner_walk1.png` | `PNG/Player/Poses/player_walk1.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/runner_walk2.png` | `PNG/Player/Poses/player_walk2.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_stand.png` | `PNG/Soldier/Poses/soldier_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_walk1.png` | `PNG/Soldier/Poses/soldier_walk1.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_walk2.png` | `PNG/Soldier/Poses/soldier_walk2.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/guardian_stand.png` | `PNG/Zombie/Poses/zombie_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/control_device.png` | `PNG/Default size/Structure/scifiStructure_12.png` | [Kenney Sci-Fi RTS](https://kenney.nl/assets/sci-fi-rts) |

## OpenGameArt 2.5D raw intake

| Repository path | Original asset | Source | License |
| --- | --- | --- | --- |
| `oga_25d/raw/nmn_items/items_paletted.png` | `items_paletted.png` | [Items - armor, health, ammo](https://opengameart.org/content/items-armor-health-ammo) by Nmn | CC0 |
| `oga_25d/raw/nmn_decorations/decorations_a_paletted.png` | `decorations_a_paletted.png` | [Decorations - torches, trees, some corpses](https://opengameart.org/content/decorations-torches-trees-some-corpses) by Nmn | CC0 |
| `oga_25d/raw/mutantleg_lab_sprites/lab_sprite.zip` | `lab_sprite.zip` | [LAB sprites](https://opengameart.org/content/lab-sprites) by mutantleg | CC0 |
| `oga_25d/raw/mutantleg_lab_textures/lab_texture.zip` | `lab_texture.zip` | [LAB textures](https://opengameart.org/content/lab-textures) by mutantleg | CC0 |
| `oga_25d/raw/nmn_human_guard/enforcer_palette.png` | `enforcer_palette.png` | [Human guard for Sprite Based FPS](https://opengameart.org/content/human-guard-for-sprite-based-fps) by Nmn | CC0 |
| `oga_25d/raw/knekko_oldschool_decorations/oldschool_fps_decoration_sprites.zip` | `oldschool_fps_decoration_sprites.zip` | [Oldschool FPS decoration sprites](https://opengameart.org/content/oldschool-fps-decoration-sprites) by knekko | CC0 |
| `oga_25d/raw/knekko_guard/guard_spritesheet.png` | `guard_spritesheet.png` | [The Guard](https://opengameart.org/content/the-guard) by knekko | CC0 |
| `oga_25d/raw/xcvg_keycards/xcvg_cardkeys_premade.zip` | `xcvg_cardkeys_premade.zip` | [Key/Credit Cards](https://opengameart.org/content/keycredit-cards) by XCVG | CC0 |
| `oga_25d/raw/xcvg_keycards/xcvg_cardkeys_doom.zip` | `xcvg_cardkeys_doom.zip` | [Key/Credit Cards](https://opengameart.org/content/keycredit-cards) by XCVG | CC0 |
| `oga_25d/raw/pasmate_crosshairs64/crosshairs64.png` | `crosshairs64.png` | [64 crosshairs pack](https://opengameart.org/content/64-crosshairs-pack) by para | CC0 |
| `oga_25d/raw/pasmate_crosshairs64/pasmateRs_crosshairs64.zip` | `pasmateRs_crosshairs64.zip` | [64 crosshairs pack](https://opengameart.org/content/64-crosshairs-pack) by para | CC0 |
| `oga_25d/raw/pasmate_crosshairs64_split/Crosshairs_64.zip` | `Crosshairs 64.zip` | [64 crosshairs pack - Split](https://opengameart.org/content/64-crosshairs-pack-split) by LoneCoder, based on para's CC0 pack | CC0 |
| `oga_25d/raw/cursor_pack/*` | `cursor.png` page files | [Cursor Pack](https://opengameart.org/content/cursor-pack) by Ivan Voirol | CC0 |
| `oga_25d/raw/fps_weapons_overlay/fps_weapons.png` | `fps_weapons.png` | [FPS Weapons Overlay](https://opengameart.org/content/fps-weapons-overlay) by knekko | CC0 |
| `oga_25d/raw/fps_weapons_overlay/fps_weapons_0.png` | `fps_weapons.png` alternate downloaded URL | [FPS Weapons Overlay](https://opengameart.org/content/fps-weapons-overlay) by knekko | CC0 |
| `oga_25d/raw/sci_fi_sound_library/Sci-Fi_Sound_Library.zip` | `Sci-Fi Sound Library.zip` | [Sci-Fi Sound Effects Library](https://opengameart.org/content/sci-fi-sound-effects-library) by Little Robot Sound Factory | CC-BY 3.0 |
| `oga_25d/raw/ui_sound_library/UI_Sound_Library.zip` | `UI Sound Library.zip` | [UI Sound Effects Library](https://opengameart.org/content/ui-sound-effects-library) by Little Robot Sound Factory | CC-BY 3.0 |

## Notes

- `exit_panel.png` is a project-specific CC0 derivative: the icon and font are
  Kenney CC0 assets, arranged on a newly generated dark-green sign.
- `environment.hdr` is the optional advanced environment asset from item 16. It is
  **present but not rendered**: Bevy image-based lighting needs a `.ktx2` cubemap, not
  an equirectangular `.hdr`. To use it, bake `empty_warehouse_01` to a `.ktx2` cubemap
  (diffuse + specular) and wire an `EnvironmentMapLight` + `Skybox`; until then the
  match is lit by the directional sun, ambient, and fixture point lights.
- `prop.glb` predated this sourcing pass and is not covered by this ledger.
