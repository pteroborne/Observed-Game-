# Bevy Assets Research

Source: the official Bevy Assets page at <https://bevy.org/assets/>, accessed
2026-06-26. The page is a community catalogue of third-party Bevy plugins,
resources, and apps. This repository currently uses Bevy 0.18.1, so entries
listed for `^0.18`, `0.18.x`, or older compatible lines are more practical than
entries listed only for `^0.19`.

Follow-up plan: [docs/bevy_asset_integration_roadmap.md](bevy_asset_integration_roadmap.md)
picks the top ten candidates and sequences them into isolated integration-lab
phases.

## Recommendation

Use `bevy_trenchbroom` as the first map-authoring spike.

The official page lists `bevy_trenchbroom` as a TrenchBroom and ericw-tools
integration with `.map` and `.bsp` loading, and marks it for Bevy `^0.18.0`.
That is the best match for this project because the active game direction is
3D, rooms and corridors have distinct jobs, and authored door/port/socket
markers need to remain explicit. TrenchBroom-style blockout maps can express
rooms, corridors, thresholds, elevation, and greybox traversal without forcing a
full art pipeline.

Recommended validation lab:

1. Add a small `map_authoring_lab` or `trenchbroom_lab`, not a production
   dependency first.
2. Import one tiny `.map` containing two rooms, one corridor, two doors, and
   explicit port/socket marker entities.
3. Convert imported editor metadata into existing domain concepts:
   `RoomId`, `PortId`, door state, corridor/room classification, and gameplay
   semantic style.
4. Verify deterministic reset/despawn, collision, and debug overlays.
5. Only promote a reusable importer after the lab proves the authored data can
   stay separate from presentation entities.

Fallbacks:

- `bevy_ecs_ldtk` is the best 2D editor/importer option if the immediate task is
  room graphs, route schematics, or top-down labs.
- `bevy_ecs_tiled` is a solid 2D tilemap option if Tiled's workflow is preferred.
- `bevy-yoleck` is conceptually interesting for an in-game editor, but the
  official page currently lists it for Bevy `^0.19`, so it should wait unless the
  workspace upgrades or an older compatible release is verified.

## High Value Now

| Asset | Official page category | Bevy listed | Why it may help |
| --- | --- | ---: | --- |
| `bevy_trenchbroom` | 3D | `^0.18.0` | Best map-editor fit. Supports 3D greybox rooms/corridors and explicit authored markers through TrenchBroom `.map`/`.bsp` data. |
| `bevy_ecs_ldtk` | 2D | `^0.18` | Strong 2D level editor path for labs, schematic maps, and room graph prototypes. |
| `bevy_ecs_tiled` | 2D | `^0.18` | Tiled map editor support for 2D tilemaps; useful for route/facility schematics or older 2D labs. |
| `bevy_mod_outline` | 3D | `^0.18` | Directly supports the Legibility Contract by making players, doors, hazards, and interactables punch through fog/bloom. |
| `bevy_hanabi` | 3D | `^0.18` | GPU particles for readable hazard, door, reroute, and machinery feedback. Keep semantic color/emission in `observed_style`. |
| `bevy_archie` | Input | `^0.18` | Controller support, remapping, haptics, gyro, touchpad, and multiplayer. Good candidate if `control_lab` grows beyond current input abstraction. |
| `bevy_fix_cursor_unlock_web` | Input | `^0.18.0` | Useful if browser builds matter for FPS cursor capture and release behavior. |
| `bevy_image_export` | Helpers | `^0.18` | Could strengthen the existing screenshot evidence workflow by recording camera output to image sequences. |
| `bevy_mod_config` | Configuration | `^0.18.0` | Config persistence plus editor UI. Candidate for settings, debug flags, and lab knobs if current custom settings become unwieldy. |
| `bevy_event_extras` | Helpers | `^0.18.1` | Small event utilities; possible fit for the event-heavy interaction, doors, progression, and match crates. |
| `bevy_log_events` | Helpers | `^0.18` | Could make invisible event flows inspectable in labs without building one-off logging every time. |
| `bevy_framepace` | Helpers | `^0.18` | Framerate limiting/frame pacing for repeatable local testing and capture. |
| `bevy-panic-handler` | Helpers | `^0.18.0` | Friendlier local prototype failure reporting during lab iteration. |
| `bevy_ggrs` | Networking | `^0.18.0` | Later rollback candidate if the final match model favors deterministic competitive simulation. Defer until networking is active work. |
| `naia` | Networking | `^0.18` | Later option for ECS world sync and FPS-style rollback. Defer until networking is active work. |
| `lightyear` | Networking | `^0.18` | Later full server-client networking candidate. Defer until the local match loop is fun and stable. |
| `bevy_rapier` | Physics | `^0.18.1` | Mature 2D/3D physics option. Only use if custom traversal/collision proves insufficient; do not add casually. |
| `bevy_rts_camera` | Camera | `^0.18` | Useful for debug/spectator/tactical map views, not primary FPS play. |
| `bevy_flycam` | Camera | `^0.18` | Fast debug camera for 3D labs if the repo's existing cameras are not enough. |
| `bevy_customizable_camera_controllers` | Camera | `^0.18.1` | Possible debug or editor camera controller for level-authoring labs. |

## Map And Level Authoring

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_trenchbroom` | `^0.18.0` | Best 3D fit | Use first. Its editor workflow matches explicit room/corridor/door/port authoring without requiring polished assets. |
| `bevy_ecs_ldtk` | `^0.18` | Good 2D fit | ECS-friendly LDtk importer. Best for 2D labs and graph/schematic prototyping. |
| `bevy_ecs_tiled` | `^0.18` | Good 2D fit | Tiled map editor helper. Useful if tile grids become the fastest way to author room plans. |
| `bevy_spritefusion` | `^0.18` | Narrow 2D fit | Sprite Fusion map importer. Likely lower value than LDtk or Tiled unless Sprite Fusion is chosen. |
| `bevy_ecs_tilemap` | `^0.18.0` | Renderer/support | Tilemap rendering where each tile is an entity. Useful under custom editor/data pipelines. |
| `bevy_entitiles` | `^0.14` | Possible 2D support | Tilemap library with algorithms/tools built in. Version lag makes it a lower priority. |
| `bevy_tiled` | `^0.5` | Low | Tiled rendering plugin, but old/non-standard compared with `bevy_ecs_tiled`. |
| `bevy_ldtk` | `^0.5` | Low | LDtk rendering plugin, but old/non-standard compared with `bevy_ecs_ldtk`. |
| `bevy-yoleck` | `^0.19` | Watch | In-game level editor model is attractive, but current official listing is Bevy 0.19. |
| `HillVacuum` | `0.16` | Reference only | 2D Doom/TrenchBroom-inspired map editor app. Good to study, not a direct fit for Bevy 0.18.1 integration. |
| `BerryCode` | `^0.18` | Evaluate | Native Bevy IDE with scene editor and ECS inspector. Could be useful, but should not replace small data-driven labs. |
| `bevy_ai_editor` | `^0.18` | Experimental | Remote level editor for AI agents. Interesting for the agent-development goal, but high process risk until authoring semantics are proven. |
| `skein` | `^0.19.0` | Later | Blender component metadata via glTF. More useful if authored props/scenes return; currently mismatched with code-as-art direction and Bevy version. |
| `bevy_gltf_components` | `^0.14` | Later | Component metadata in glTF. Useful only if Blender/glTF becomes a deliberate content pipeline. |
| `bevy_gltf_blueprints` | `^0.14` | Later | Blueprint/prefab spawning from glTF. Same caveat as above. |
| `blender_bevy_components_workflow` | `^0.14` | Later | Blender-as-editor workflow. Higher asset-curation burden than the current procedural visual direction. |

## Procedural Geometry And Visual Systems

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_ghx_proc_gen` | `^0.18` | Medium | 2D/3D WFC/model-synthesis generation. Could help later route/facility experiments, but only after room/corridor semantics are stable. |
| `bevy_generative` | `^0.16.1` | Medium | Procedural maps/textures/terrain/planets. Evaluate only for isolated labs; avoid replacing explicit room definitions too early. |
| `Noiz` | `^0.19` | Watch | Noise library for procedural texture/geometry signals. Bevy 0.19 listing means wait or verify older compatibility. |
| `bevy_copperfield` | `^0.15` | Low/medium | Procedural mesh editor. Interesting for generated architecture, but likely too general before map-import needs are known. |
| `bevy_aabb_instancing` | `^0.12.1` | Medium | Efficient rendering of many boxes. Useful for massive greybox/debug facilities if current mesh spawning becomes expensive. |
| `bevy_vector_shapes` | `^0.18.0` | Medium | 2D/3D vector shapes for debug overlays, ports, legends, and authored-socket visualization. |
| `bevy_svg` | `^0.18` | Low/medium | SVG loading/drawing for icons or schematic overlays. Keep runtime UI simple unless this removes custom drawing code. |
| `bevy_vello` | `^0.18.0` | Low/medium | Vector rendering integration. Possible future map/schematic renderer. |
| `bevy_vox` | `^0.18` | Low | MagicaVoxel loader. Useful only if voxel blockouts become a chosen asset source. |
| `bevy_voxel_world` | `^0.18` | Low | Voxel world plugin. Probably conflicts with explicit rooms/ports unless isolated in a lab. |
| `bevy_vox_mesh` | `^0.12.0` | Low | MagicaVoxel as meshes; lower priority than procedural primitives or TrenchBroom blockouts. |
| `bevy_fontmesh` | `^0.19` | Watch | 3D text meshes could help diegetic labels/legends, but version mismatch and not urgent. |
| `bevy_sdf_klown` | `^0.17.3` | Low | Raymarching/SDF experiments. Interesting visually, risky for legibility and scope. |
| `bevy-vfx-bag` | `^0.10` | Low | VFX collection; lower priority than `bevy_hanabi` and the shared style module. |
| `bevy_toon_shader` | `^0.12` | Low | Toon shader does not match the chosen neon-noir facility direction. |

## Debugging, Inspection, And Agent Workflow

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy-inspector-egui` | `^0.19.0` | Already useful | The repo already has a Bevy 0.18-compatible optional version in `inspector_lab`. Keep it dev-only. |
| `bevy_mod_debugdump` | `^0.19.0` | Watch | Schedule/render graph visualization. Useful once compatible or after a Bevy upgrade. |
| `bevy_lint` | unlisted version | Evaluate | Static linting for Bevy footguns could be valuable if it runs cleanly on this workspace. |
| `bevycheck` | `^0.10` | Evaluate | Annotates invalid systems with clearer errors. Useful if scheduling errors slow iteration. |
| `bevy_dev_console` | unlisted version | Medium | Runtime command console could help labs reset, toggle overlays, and force states. Keep out of core simulation. |
| `bevy_mod_debug_console` | `^0.8` | Low | ECS console; older and likely less attractive than the dev console or inspector. |
| `bevy-remote-devtools` | `^0.6` | Experimental | Remote UI for entities/assets/logs/systems. Possible debugging aid, but version maturity is a risk. |
| `bevy_inspector.nvim` | unlisted version | Low | Editor-specific remote inspector. Only relevant for users who prefer Neovim. |
| `Bevy Inspector VS Code Extension` | `main` | Low/medium | Useful if VS Code live inspection becomes part of the workflow. |
| `Dexterous Developer` | unlisted version | Evaluate | Hot reload system. Potentially high iteration value, but needs a dedicated compatibility spike. |
| `bevy_simple_subsecond_system` | `^0.16.0` | Evaluate | Hotpatch systems while running. Useful for labs if stable with current Bevy. |
| `bevy_debug_lines` | `^0.12` | Low | Older debug line drawing; prefer Bevy gizmos/current local overlays unless this fills a gap. |
| `bevy_mod_gizmos` | `^0.10` | Low | Visual gizmos; check against Bevy's built-in gizmos before adding. |
| `bevy-debug-text-overlay` | `^0.13` | Low | Convenient text overlay, but this repo already favors explicit debug overlays. |
| `bevy_image_export` | `^0.18` | High | Useful for automated screenshot/evidence capture and visual regression loops. |
| `bevy-autoplay` | `^0.13.2` | Medium | Recorded play-session integration testing could fit manual-lab verification later. |
| `bevy_local_commands` | `^0.18` | Low | Running local shell commands from Bevy may help tooling labs, but avoid broad shell surfaces in game runtime. |

## Input, Camera, And Controls

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `leafwing_input_manager` | `^0.19` | Watch | Excellent action-mapping candidate, but current official listing is Bevy 0.19. This repo already has `player_input`, so adoption would require a deliberate migration. |
| `bevy_enhanced_input` | `^0.19.0` | Watch | Unreal-style contextual mappings. Same version and migration caveat as Leafwing. |
| `bevy_archie` | `^0.18` | High if needed | Strongest current-version input candidate for controllers, remapping, haptics, and local multiplayer support. |
| `Virtual Joystick` | `^0.18` | Low | Mobile/touch support only if web/mobile prototypes become important. |
| `bevy_input_prompts` | `^0.16.0` | Medium | Could help display correct prompt icons once rebinding/controller support matures. |
| `bevy_advanced_input` | `0.5.0` | Low | Hotkeys/chords. Useful mostly for editor/debug shortcuts. |
| `keyseq` | `^0.18` | Low | Key chord notation for debug/editor commands. |
| `bevy_ineffable` | `^0.16.0` | Low/medium | Alternative input manager focused on accessibility. Compare only if `player_input` becomes insufficient. |
| `bevy_pancam` | `^0.19` | Watch | Panning/zooming 2D map camera. Useful for tactical maps after compatibility is solved. |
| `bevy_smooth_pixel_camera` | `^0.18.1` | Low | Good for pixel-perfect 2D labs, less relevant to the first-person game. |
| `bevy_flycam` | `^0.18` | Medium | Useful debug camera for 3D labs and map-authoring inspection. |
| `bevy_rts_camera` | `^0.18` | Medium | Spectator/tactical map camera candidate. |
| `bevy_third_person_camera` | `^0.18` | Low/medium | Possible traversal debugging view, not the main first-person target. |
| `bevy_customizable_camera_controllers` | `^0.18.1` | Medium | General debug/editor camera controls. |
| `bevy_dolly` | `^0.15` | Low/medium | Smooth cinematic/debug camera composition; useful for evidence videos, not core gameplay. |

## Physics, Navigation, And Movement

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_rapier` | `^0.18.1` | Evaluate carefully | Mature physics, but the project explicitly avoids adding physics casually. Spike only if custom collision/traversal blocks fun. |
| `avian` | `^0.19.0` | Watch | ECS-driven 2D/3D physics, but official listing is Bevy 0.19. |
| `bevy-tnua` | `^0.19` | Watch | Physics-based floating character controller. Interesting only if the project adopts a physics backend. |
| `bevy_mod_wanderlust` | `^0.11` | Low | Floating character controller, but likely old relative to current Bevy. |
| `bevy_fpc` | `^0.13` | Low | First-person controller plugin. The repo already owns FPS/traversal logic, so importing a controller may fight the architecture. |
| `avian_pickup` | `^0.18.0` | Low | Gravity-gun style pickup for Avian. Only relevant if Avian is adopted. |
| `bevy_rerecast` | `^0.18` | Later | Recast navmesh generation. Useful for bots/AI navigation after map geometry is stable. |
| `vleue_navigator` | `^0.18.0` | Later | Fast 2D/3D navmesh with live updates and layers. Good candidate for bot route testing. |
| `oxidized_navigation` | `^0.15` | Later | Runtime 3D navmesh generation. Lower priority than `bevy_rerecast`/`vleue_navigator`. |
| `seldom_map_nav` | `^0.17.2` | Later 2D | Tilemap navmesh/pathfinding for 2D map-editor paths. |

## Data, Assets, Persistence, And Settings

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_common_assets` | `^0.19.0` | Watch | JSON/YAML/RON/TOML/MessagePack assets. Good fit for room definitions and style legends after compatibility is solved. |
| `bevy_asset_loader` | `^0.19.0` | Watch | Asset collections during app states. Useful for menu/game loading states after compatibility is solved. |
| `bevy_embedded_assets` | `^0.19` | Watch | Embedding assets in the binary. Useful for demos/releases, not current labs. |
| `bevy_full_asset_path` | `^0.16.1` | Low/medium | Could help debug asset provenance and dropped-in assets. |
| `bevy_titan` | `^0.18` | Low | Texture atlas loading from RON. Only useful if 2D/UI sprite atlases grow. |
| `bevy_asset_ron` | `^0.7` | Low | Custom RON asset loading. Likely superseded by local data parsing or `bevy_common_assets`. |
| `bevy_mod_config` | `^0.18.0` | Medium/high | Settings/debug config candidate with editor UI. |
| `bevy-persistent` | `^0.19` | Watch | Persistent resources across sessions. Useful for settings after compatibility is solved. |
| `bevy-settings` | `^0.19.0-rc.2` | Watch | Struct-based persistent settings. Useful later; version mismatch now. |
| `bevy_simple_prefs` | `0.19.0` | Watch | WASM-compatible resource preferences in one RON file. |
| `bevy_pkv` | `^0.19` | Watch | Persistent key/value storage. |
| `moonshine-save` | `^0.19` | Watch | Save/load framework. Later only. |
| `bevy_save` | `^0.16.1` | Later | Game-state save/load. Useful only after game state stabilizes. |
| `bevy-persistent-windows` | `^0.17` | Low | Remembering tool/editor windows may help dev tools, not gameplay. |

## UI, Accessibility, And Player-Facing Feedback

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_color_blindness` | `^0.8.0` | High for checks | Helps preview color-blindness impact. Strong fit with the Legibility Contract even if used only in a lab/tool. |
| `bevy_egui` | `^0.19.0` | Dev UI/watch | Great for inspectors and dev panels, but keep player HUD in project style unless there is a clear reason. |
| `bevy_immediate` | `^0.19.0` | Watch | Immediate-mode-like UI on retained ECS. Potential dev UI option after compatibility. |
| `Bevy Extended UI` | `^0.19.0-rc.2` | Watch | HTML/CSS support for Bevy UI. Useful only if UI complexity grows. |
| `bevy_hui` | `^0.19` | Watch | HTML to Bevy UI. Same caveat. |
| `bevy_flair` | `^0.19` | Watch | CSS-style UI. Consider only if UI styling becomes a major bottleneck. |
| `bevy_cobweb_ui` | `0.17` | Evaluate | Productive UI framework; version and architecture need a spike. |
| `Bevy Lunex` | `^0.18` | Evaluate | ECS-first UI library. Possible option for stronger HUD/menu systems. |
| `bevy_quickmenu` | `^0.11.0` | Low/medium | Nested menu system with gamepad/keyboard/mouse navigation. Could help menus but is older. |
| `bevy-ui-navigation` | `^0.12` | Low/medium | Menu navigation components. Older, but conceptually aligned with controller menus. |
| `bevy-alt-ui-navigation-lite` | `0.17.0` | Low/medium | Lightweight fork of UI navigation. Possible menu input candidate. |
| `bevy_simple_text_input` | `^0.19.0` | Watch | Simple line input. Useful for settings/debug consoles after compatibility. |
| `bevy_text_edit` | `>=0.19` | Watch | Text input editing. |
| `bevy_cosmic_edit` | `^0.15` | Low | Multiline editing; mostly editor/debug tools. |
| `pyri_tooltip` | `^0.18` | Medium | Tooltips for dev tools or menus. Avoid using it as a substitute for in-world readability. |
| `bevy_screen_diagnostics` | `^0.16.0` | Low | The repo already notes this is not 0.18-compatible in `inspector_lab`; use built-in diagnostics for now. |
| `bevy_text_popup` | `^0.17` | Low | Event-driven UI text popups. Use sparingly; gameplay state should stay diegetic/readable. |
| `bevy_mod_bbcode` | `0.15` | Low | Rich text formatting if UI copy grows. |
| `bevy_simple_rich_text` | `0.19.0` | Watch | BBCode-like rich text, version mismatch. |
| `bevy_mod_ui_sprite` | `^0.8` | Low | Sprites in Bevy UI; old. |
| `bevy_ui_exact_image` | `^0.9` | Low | Exact image sizing. Useful only for asset-heavy UI. |
| `bevy_ui_borders` | `^0.10` | Low | UI borders. Likely unnecessary. |
| `bevy-ui-gradients` | `^0.16` | Low | Gradients in UI. Avoid if it pushes the project toward decorative one-note styling. |
| `bevy_fluent` | `^0.19` | Later | Localization with Fluent. Not needed until player-facing text stabilizes. |
| `bevy_simple_i18n` | `^0.19` | Later | Simple localization. Same timing as above. |

## Networking Candidates For Later

Networking remains explicitly deferred, but the official page has several
entries worth tracking once the local match is fun and stable.

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `lightyear` | `^0.18` | Strong later candidate | Complete server-client networking library. Probably the first serious evaluation target for authoritative play. |
| `naia` | `^0.18` | Strong later candidate | ECS world sync plus FPS-style rollback. Good conceptual fit for competitive traversal if authority/rollback tradeoffs check out. |
| `bevy_ggrs` | `^0.18.0` | Strong later candidate | P2P rollback integration. Better for deterministic local-state games than heavily physics/network-authoritative FPS. |
| `bevy_matchbox` | `^0.18.0` | Later | WebRTC peer-to-peer networking, often paired with rollback experiments. |
| `bevy_quinnet` | `^0.18.0` | Later | QUIC client/server networking. Possible authoritative transport candidate. |
| `bevy_replicon` | `^0.19` | Watch | High-level replication/events, but version mismatch now. |
| `bevy_renet` | `^0.19` | Watch | Server/client networking via renet, version mismatch now. |
| `bevy_renet2` | unlisted version | Later | Cross-platform server-authoritative renet2 wrapper; verify Bevy compatibility before considering. |
| `bevy_eventwork` | `^0.15` | Later | Message-based async server/client networking. |
| `bevy_simplenet` | `^0.16` | Later | Cross-platform websockets with simple interface. |
| `aeronet` | `^0.19.0` | Watch | Low-level networking, version mismatch now. |

## Audio And Feedback

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `bevy_audio_controller` | `^0.16` | Medium | Channel/track management. Useful if ambience, door, reroute, and hazard cues need more structure than Bevy audio. |
| `bevy_kira_audio` | `^0.19.0` | Watch | Alternative Kira-backed audio. Consider only after compatibility and if native Bevy audio blocks required behavior. |
| `bevy_fundsp` | `^0.11` | Low/medium | Procedural/synth audio. Possible fit for code-as-art sound cues, but not urgent. |
| `bevy_oddio` | `^0.11` | Low | Alternative audio integration. Lower priority than Kira or built-in audio. |

## AI, Bots, And Director Experiments

| Asset | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `seldom_state` | `^0.18.1` | Medium | Component state machines for AI/animation/controllers. Could help bots or interactable state if local patterns become complex. |
| `moonshine-behavior` | `^0.19` | Watch | Minimal state machine; wait for compatibility. |
| `big-brain` | `^0.15.0` | Later | Utility AI. Candidate for facility-director or bots, after gameplay rules are stable. |
| `bevy_observed_utility` | `^0.15` | Later | Utility AI powered by ECS observers. Interesting name collision aside, evaluate only for director/bot prototypes. |
| `bevior_tree` | `^0.18` | Later | Behavior tree plugin. Use only if bots need authored decision trees. |
| `bevy_dogoap` | `^0.16` | Later | GOAP planning. Probably too heavy until AI needs are clearer. |

## Reference Resources And Example Apps

These are useful from the official page as references, not direct dependencies.

| Asset/resource | Bevy listed | Fit | Notes |
| --- | ---: | --- | --- |
| `Official Bevy Examples` | `0.20.0-dev` | Ongoing reference | Use for current API patterns, but adapt to this workspace's pinned Bevy 0.18.1. |
| `Official Migration Guides` | unlisted | Ongoing reference | Required reading before any Bevy upgrade, especially because many promising assets are listed for Bevy 0.19. |
| `Shadplay` | `0.19.0-rc.2` | Watch/reference | Useful WGSL shader playground for neon-noir experiments after compatibility is checked. |
| `Bevy Noisemap Example` | `0.12` | Reference | Procedural noise reference for textures/debug maps, not a current dependency. |
| `A walkthrough of bevy 0.11 rendering` | `0.11` | Reference | Rendering-pipeline context that can help maintain the style/legibility modules. |
| `Choosing your networking architecture` | `0.20.0-dev` | Later reference | Useful once networking moves from non-goal to active design. |
| `Bevy Cheatbook` | `0.16` | Reference with caution | Practical background, but version-lagged; verify against Bevy 0.18.1 docs/API. |
| `Block Breaker` | `0.18` | Reference | Workshop includes states, ray casting, collision response; useful for lab patterns. |
| `Drone Agility Challenge` | `0.10.1` | Inspiration | Traversal-through-levels example. Old Bevy version, so use only as design reference. |
| `Combine Racers` | `0.14` | Inspiration | Racing/traversal prototype reference. Old version, not dependency material. |
| `Pixie Wrangler` | `0.17` | Inspiration | Puzzle game inspired by circuit-board design software; relevant to cable/route puzzle thinking. |
| `Nodus` | unlisted | Inspiration | Logic gate simulator; useful as a machinery/puzzle interaction reference, not as game code. |

## Items To Avoid For Now

- Large physics/network/UI frameworks should not be added directly to production
  crates. They need a focused lab first.
- Asset-heavy Blender/glTF workflows conflict with the current code-as-art
  direction unless they are used only for editor metadata or a small number of
  props.
- Procedural generation plugins should not replace the explicit room/port model.
  They are useful only after rooms, corridors, doors, sockets, and observation
  rules have stable data contracts.
- Bevy 0.19-only entries should be tracked, not adopted, while the workspace is
  pinned to Bevy 0.18.1.

## Next Smallest Step

Create a tiny `bevy_trenchbroom` feasibility lab that imports a single authored
map and proves these facts:

- Room and corridor geometry can be loaded and reset without leaked entities.
- Door thresholds are explicit, inspectable, and convertible into current domain
  IDs.
- Imported collision is good enough for first-person traversal tests.
- Visual presentation still flows through `observed_style`; map materials do not
  introduce ad-hoc colors that violate the Legibility Contract.
- The lab can generate screenshot evidence like the existing 3D/FPS labs.
