# Asset sources

All assets added for `ASSET_PLAN.md` are released under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). They were retrieved
on June 20, 2026.

The basic dev/debug wall and floor texture slots were refreshed from ambientCG on
July 1, 2026.

All `sounds/*.ogg` files were regenerated on July 9, 2026 by the deterministic
in-repo synthesizer `tools/generate_audio.py` (numpy + ffmpeg); they use no
external source material and replace the earlier Kenney one-shots and the
July 6/9 ad-hoc ambience synthesis passes.

The 2.5D dev placeholder sprites were selected from Kenney CC0 packs on
July 9, 2026. Only the curated PNG files listed below are checked in, not the full
source archives.

The OpenGameArt 2.5D FPS raw intake was downloaded on July 9, 2026 into
`assets/oga_25d/raw/`. These are source artifacts for the roadmap in
`sprite_roadmap.md`, not game-ready asset slots yet. Phase 56 removed the two
attribution-required raw sound-library archives from that intake; the remaining
checked-in raw intake listed below is CC0.

The content-manifest feasibility lab downloaded Kenney's Modular Space Kit from
OpenGameArt on July 12, 2026. Only two GLBs and the original license file are checked
in. Both files are CC0 and remain presentation-only dressing with procedural fallbacks.

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
| `content_manifest_lab/kenney_modular_space/cables.glb` | `Models/GLB format/cables.glb` | [Kenney Modular Space Kit on OpenGameArt](https://opengameart.org/content/modular-space-kit), CC0 |
| `content_manifest_lab/kenney_modular_space/gate.glb` | `Models/GLB format/gate.glb` | [Kenney Modular Space Kit on OpenGameArt](https://opengameart.org/content/modular-space-kit), CC0 |
| `content_manifest_lab/kenney_modular_space/Textures/colormap.png` | Shared GLB `Textures/colormap.png` | [Kenney Modular Space Kit on OpenGameArt](https://opengameart.org/content/modular-space-kit), CC0 |
| `content_manifest_lab/kenney_modular_space/LICENSE.txt` | Original pack license | [Kenney Modular Space Kit on OpenGameArt](https://opengameart.org/content/modular-space-kit), CC0 |
| `sounds/footstep.ogg` | soft contact thump (movement family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/reroute.ogg` | detuned reality-shift shimmer (structure family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/escape.ogg` | rising bell ladder and bloom (progress family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience.ogg` | neutral facility room-tone loop | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/door.ogg` | pneumatic hiss and clunk (structure family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/klaxon.ogg` | short restrained two-tone alarm loop (threat family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/collapse_sting.ogg` | rumble swell, crack, falling drone (structure family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ui_click.ogg` | two-step glass confirm tick (UI family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ui_hover.ogg` | small glass tick (UI family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/jump.ogg` | airy upward sweep (movement family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/land.ogg` | low landing thump (movement family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/tool_interact.ogg` | servo chirp acknowledgement (tool family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/keystone.ogg` | three rising bell notes (progress family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/exit_unlock.ogg` | chord bloom and sub pulse (progress family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/guardian_dread.ogg` | low inharmonic dread swell (threat family) | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_archive.ogg` | archive district bed: dry stillness, dust shimmer | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_reactor.ogg` | reactor district bed: warm beating hum stack | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_atrium.ogg` | atrium district bed: wandering wind, high sparkle | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_foundry.ogg` | foundry district bed: rumble, metallic comb | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_hollow.ogg` | hollow district bed: sweeping empty mid band | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_spillway.ogg` | spillway district bed: burbling water bands | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_gantry.ogg` | gantry hall bed: deep swells, wind whistle | In-repo synthesis (`tools/generate_audio.py`) |
| `sounds/ambience_corridor.ogg` | corridor hall bed: duct air, vent hum | In-repo synthesis (`tools/generate_audio.py`) |
| `textures/environment.hdr` | `empty_warehouse_01_1k.hdr` | [Poly Haven Empty Warehouse 01](https://polyhaven.com/a/empty_warehouse_01) |
| `sprites/runner_stand.png` | `PNG/Player/Poses/player_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/runner_walk1.png` | `PNG/Player/Poses/player_walk1.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/runner_walk2.png` | `PNG/Player/Poses/player_walk2.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_stand.png` | `PNG/Soldier/Poses/soldier_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_walk1.png` | `PNG/Soldier/Poses/soldier_walk1.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/rival_walk2.png` | `PNG/Soldier/Poses/soldier_walk2.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/guardian_stand.png` | `PNG/Zombie/Poses/zombie_stand.png` | [Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters) |
| `sprites/control_device.png` | `PNG/Default size/Structure/scifiStructure_12.png` | [Kenney Sci-Fi RTS](https://kenney.nl/assets/sci-fi-rts) |
| `sprites/keystone_card.png` | `oga_25d/derived/keystone_card.png` | Game-ready 2.5D keystone access card | CC0 |
| `sprites/keystone_core.png` | `oga_25d/derived/keystone_core.png` | Game-ready 2.5D keystone power core | CC0 |
| `sprites/exit_access_card.png` | `oga_25d/derived/exit_access_card.png` | Game-ready 2.5D exit authorization card | CC0 |
| `sprites/anchor_torch.png` | `oga_25d/derived/anchor_torch.png` | Game-ready 2.5D anchor torch body | CC0 |
| `sprites/route_cell.png` | `oga_25d/derived/route_cell.png` | Game-ready 2.5D route/mesh power cell | CC0 |
| `sprites/relay_device.png` | `oga_25d/derived/relay_device.png` | Game-ready 2.5D portable relay node | CC0 |
| `sprites/battery_charge.png` | `oga_25d/derived/battery_charge.png` | Game-ready 2.5D battery unit charge | CC0 |
| `sprites/repair_token.png` | `oga_25d/derived/repair_token.png` | Game-ready 2.5D subsystem repair token | CC0 |
| `sprites/rival_actor.png` | `oga_25d/derived/rival_actor.png` | Game-ready 2.5D directional rival sheet | CC0 |
| `sprites/guardian_actor.png` | `oga_25d/derived/guardian_actor.png` | Game-ready 2.5D directional guardian sheet | CC0 |

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

## Notes

- `exit_panel.png` is a project-specific CC0 derivative: the icon and font are
  Kenney CC0 assets, arranged on a newly generated dark-green sign.
- `environment.hdr` is the optional advanced environment asset from item 16. It is
  **present but not rendered**: Bevy image-based lighting needs a `.ktx2` cubemap, not
  an equirectangular `.hdr`. To use it, bake `empty_warehouse_01` to a `.ktx2` cubemap
  (diffuse + specular) and wire an `EnvironmentMapLight` + `Skybox`; until then the
  match is lit by the directional sun, ambient, and fixture point lights.
- `prop.glb` predated this sourcing pass and is not covered by this ledger.

## Derived 2.5D assets (`assets/oga_25d/derived/`)

| Repository path | Source raw asset | Notes | License |
| --- | --- | --- | --- |
| `oga_25d/derived/rival_actor.png` | `oga_25d/raw/knekko_guard/guard_spritesheet.png` | Copied directly (8x7 grid of 64x64 sprites) | CC0 |
| `oga_25d/derived/guardian_actor.png` | mutantleg robot frames inside `lab_sprite.zip` | Stitched robot walk/attack/hit frames (6x1 grid of 64x64 sprites) | CC0 |
| `oga_25d/derived/keystone_card.png` | `full_id.png` inside `xcvg_cardkeys_premade.zip` | Extracted keycard | CC0 |
| `oga_25d/derived/keystone_core.png` | `full_stripe.png` inside `xcvg_cardkeys_premade.zip` | Extracted stripe keycard representing the core | CC0 |
| `oga_25d/derived/exit_access_card.png` | `slim_id.png` inside `xcvg_cardkeys_premade.zip` | Extracted exit access card | CC0 |
| `oga_25d/derived/battery_charge.png` | `items_paletted.png` (224, 193, 33, 40) | Cropped battery item with cyan made transparent | CC0 |
| `oga_25d/derived/route_cell.png` | `items_paletted.png` (216, 143, 28, 21) | Cropped route cell with cyan made transparent | CC0 |
| `oga_25d/derived/repair_token.png` | `items_paletted.png` (109, 257, 26, 27) | Cropped repair token with cyan made transparent | CC0 |
| `oga_25d/derived/relay_device.png` | `items_paletted.png` (263, 60, 33, 19) | Cropped relay device with cyan made transparent | CC0 |
| `oga_25d/derived/anchor_torch.png` | `items_paletted.png` (128, 36, 15, 17) | Cropped anchor torch with cyan made transparent | CC0 |
| `oga_25d/derived/column.png` | `decorations_a_paletted.png` (328, 25, 27, 88) | Cropped column with cyan made transparent | CC0 |
| `oga_25d/derived/torch_wall.png` | `decorations_a_paletted.png` (219, 30, 27, 83) | Cropped wall torch with cyan made transparent | CC0 |
| `oga_25d/derived/lab_crate.png` | `LAB/sprites/crate.png` inside `lab_sprite.zip` | Extracted crate sprite | CC0 |
| `oga_25d/derived/lab_table.png` | `LAB/sprites/d_table.png` inside `lab_sprite.zip` | Extracted table sprite | CC0 |
| `oga_25d/derived/lab_wall_tile.png` | `LAB/wall/tile000.png` inside `lab_texture.zip` | Extracted wall texture sampler | CC0 |

