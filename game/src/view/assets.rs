//! The match's drop-in asset registry: slot paths from the shared `observed_assets`
//! manifest, presence checks, the neon-noir material builders, and [`MatchAssets`] —
//! the shared meshes/materials/scenes/sounds resource the place renderer draws from.
//! Drop a file at a slot's path and the match uses it; leave it absent and the match
//! falls back procedurally.

use std::path::PathBuf;

use bevy::prelude::*;
use observed_match::facility::TEAM_COUNT;
use observed_style::{self as style, MarkerRole, SurfaceRole};

use super::actor_metadata::SpriteMetadata;
use super::environment::{load_content_scene, load_repeating_texture};
use crate::layout::{HALL_WIDTH, PLACE_TILE, WALL_HEIGHT};
use crate::view::theme::TEAM_COLORS;

pub(crate) const DISTRICT_COUNT: usize = observed_style::District::ALL.len();

// Drop-in asset slots — the paths are *not* re-declared here; they come from the
// shared `observed_assets` manifest (the single source of truth `asset_lab` also
// reads).
const WALL_TEX: &str = observed_assets::WALL.path;
const FLOOR_TEX: &str = observed_assets::FLOOR.path;
const CEILING_TEX: &str = observed_assets::CEILING.path;
const EXIT_PANEL_TEX: &str = observed_assets::EXIT_PANEL.path;
// Present in the asset inventory but not rendered: proper HDRI image-based lighting
// needs a `.ktx2` cubemap, not an equirectangular `.hdr` (see assets/SOURCES.md).
const ENVIRONMENT_HDR: &str = observed_assets::ENVIRONMENT.path;
const LIGHT_FIXTURE_MODEL: &str = observed_assets::LIGHT_FIXTURE.path;
const EXIT_GATE_MODEL: &str = observed_assets::EXIT_GATE.path;
const PLAYER_MODEL: &str = observed_assets::PLAYER.path;
const BOT_MODEL: &str = observed_assets::BOT.path;
const EQUIPMENT_MODEL: &str = observed_assets::EQUIPMENT.path;
const HAZARD_MODEL: &str = observed_assets::HAZARD.path;
const RUNNER_STAND_SPRITE: &str = observed_assets::RUNNER_STAND.path;
const RUNNER_WALK1_SPRITE: &str = observed_assets::RUNNER_WALK1.path;
const RUNNER_WALK2_SPRITE: &str = observed_assets::RUNNER_WALK2.path;
const RIVAL_STAND_SPRITE: &str = observed_assets::RIVAL_STAND.path;
const RIVAL_WALK1_SPRITE: &str = observed_assets::RIVAL_WALK1.path;
const RIVAL_WALK2_SPRITE: &str = observed_assets::RIVAL_WALK2.path;
const GUARDIAN_STAND_SPRITE: &str = observed_assets::GUARDIAN_STAND.path;
const RIVAL_ACTOR_SPRITE: &str = observed_assets::RIVAL_ACTOR.path;
const GUARDIAN_ACTOR_SPRITE: &str = observed_assets::GUARDIAN_ACTOR.path;
const DECOR_COLUMN_SPRITE: &str = observed_assets::DECOR_COLUMN.path;
const DECOR_TORCH_SPRITE: &str = observed_assets::DECOR_TORCH.path;
const DECOR_LAB_CRATE_SPRITE: &str = observed_assets::DECOR_LAB_CRATE.path;
const DECOR_LAB_TABLE_SPRITE: &str = observed_assets::DECOR_LAB_TABLE.path;
const WALL_ALBEDO_LAB_TEX: &str = observed_assets::WALL_ALBEDO_LAB.path;
const CONTROL_DEVICE_SPRITE: &str = observed_assets::CONTROL_DEVICE.path;
const KEYSTONE_CARD_SPRITE: &str = observed_assets::KEYSTONE_CARD.path;
const KEYSTONE_CORE_SPRITE: &str = observed_assets::KEYSTONE_CORE.path;
const EXIT_ACCESS_CARD_SPRITE: &str = observed_assets::EXIT_ACCESS_CARD.path;
const ANCHOR_TORCH_SPRITE: &str = observed_assets::ANCHOR_TORCH.path;
const ROUTE_CELL_SPRITE: &str = observed_assets::ROUTE_CELL.path;
const RELAY_DEVICE_SPRITE: &str = observed_assets::RELAY_DEVICE.path;
const BATTERY_CHARGE_SPRITE: &str = observed_assets::BATTERY_CHARGE.path;
const REPAIR_TOKEN_SPRITE: &str = observed_assets::REPAIR_TOKEN.path;
const FOOTSTEP_SOUND: &str = observed_assets::FOOTSTEP.path;
const REROUTE_SOUND: &str = observed_assets::REROUTE.path;
const ESCAPE_SOUND: &str = observed_assets::ESCAPE.path;
const AMBIENCE_SOUND: &str = observed_assets::AMBIENCE.path;
// Optional (not in the required asset plan): a door open/close thunk on entering or
// leaving a place. Silent until a file is dropped here.
const DOOR_SOUND: &str = observed_assets::DOOR.path;
const KLAXON_SOUND: &str = observed_assets::KLAXON.path;
const COLLAPSE_STING_SOUND: &str = observed_assets::COLLAPSE_STING.path;
const UI_CLICK_SOUND: &str = observed_assets::UI_CLICK.path;
const UI_HOVER_SOUND: &str = observed_assets::UI_HOVER.path;
const JUMP_SOUND: &str = observed_assets::JUMP.path;
const LAND_SOUND: &str = observed_assets::LAND.path;
const TOOL_INTERACT_SOUND: &str = observed_assets::TOOL_INTERACT.path;
const KEYSTONE_SOUND: &str = observed_assets::KEYSTONE.path;
const EXIT_UNLOCK_SOUND: &str = observed_assets::EXIT_UNLOCK.path;
const GUARDIAN_DREAD_SOUND: &str = observed_assets::GUARDIAN_DREAD.path;
const AMBIENCE_CORRIDOR_SOUND: &str = observed_assets::AMBIENCE_CORRIDOR.path;
const AMBIENCE_GANTRY_SOUND: &str = observed_assets::AMBIENCE_GANTRY.path;

// Procedural neon doorways (code-as-art; no GLB). A closed leaf hides the corridor
// beyond (mystery) and slides up into the lintel as the player approaches. The frame
// spans the FULL hall width ([`crate::layout::HALL_WIDTH`]) so doorways and hallways
// line up by design.
pub(crate) const DOOR_POST_W: f32 = 0.22;
pub(crate) const DOOR_POST_D: f32 = 0.5;
pub(crate) const DOOR_LINTEL_H: f32 = 0.34;
pub(crate) const DOOR_LEAF_D: f32 = 0.14;

#[derive(Clone)]
pub(crate) struct ContentScene {
    pub scene: Handle<Scene>,
    pub scale: f32,
}

/// The workspace `assets/` directory (where `cargo run` resolves Bevy's asset root).
pub(crate) fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("game crate lives directly under the workspace root")
        .join("assets")
}

pub(crate) fn asset_present(relative: &str) -> bool {
    assets_dir().join(relative).is_file()
}

/// Build a neon-noir `StandardMaterial` from the shared visual language's treatment,
/// so the match never invents ad-hoc surface colours (see `observed_style`).
pub(crate) fn neon_material(t: &style::Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: t.base_color,
        emissive: t.emissive,
        perceptual_roughness: 0.85,
        ..default()
    }
}

pub(crate) fn textured_neon_material(
    t: &style::Treatment,
    texture: Option<Handle<Image>>,
) -> StandardMaterial {
    let has_texture = texture.is_some();
    let mut material = neon_material(t);
    material.base_color_texture = texture;
    if has_texture {
        material.base_color = t.base_color;
        material.unlit = t.signal;
    }
    material
}

fn srgb_rgb(color: Color) -> [f32; 3] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue]
}

/// The district tint applied to a structural surface before any albedo texture is
/// sampled. Bevy multiplies `base_color` by `base_color_texture`, so this keeps
/// imported albedo under the style palette instead of letting it replace the palette.
pub(crate) fn palette_tint_for_surface(
    t: &style::Treatment,
    palette: &style::DistrictPalette,
) -> Color {
    if t.signal {
        return t.base_color;
    }

    let role = srgb_rgb(t.base_color);
    let light = srgb_rgb(palette.light_color);
    let ambient = srgb_rgb(palette.ambient_color);
    let brightness = (palette.ambient_brightness / 75.0).clamp(0.45, 1.1);
    let mut rgb = [0.0; 3];
    for i in 0..3 {
        let palette_channel = light[i] * 0.68 + ambient[i] * 0.32;
        rgb[i] = (role[i] * 0.28 + palette_channel * 0.72) * 0.55 * brightness;
    }
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn palette_emissive_for_surface(
    t: &style::Treatment,
    palette: &style::DistrictPalette,
) -> LinearRgba {
    if t.signal {
        return t.emissive;
    }

    // Keep the district cast well below the base treatment's glow (luminance ~0.07).
    // Structural surfaces are *lit* — by the district-tinted fixtures — never light
    // sources themselves: any stronger flat emissive swamps the lit, tinted albedo,
    // which is exactly the wash that hid the textures and the palette in the first
    // Phase 62 captures.
    LinearRgba::rgb(
        t.emissive.red * 0.25 + palette.accent.red * 0.06,
        t.emissive.green * 0.25 + palette.accent.green * 0.06,
        t.emissive.blue * 0.25 + palette.accent.blue * 0.06,
    )
}

pub(crate) fn apply_surface_palette(
    material: &mut StandardMaterial,
    t: &style::Treatment,
    palette: &style::DistrictPalette,
) {
    material.base_color = palette_tint_for_surface(t, palette);
    material.emissive = palette_emissive_for_surface(t, palette);
    material.unlit = t.signal;
}

pub(crate) fn palette_tinted_neon_material(
    t: &style::Treatment,
    palette: &style::DistrictPalette,
    texture: Option<Handle<Image>>,
) -> StandardMaterial {
    let mut material = textured_neon_material(t, texture);
    apply_surface_palette(&mut material, t, palette);
    material
}

pub(crate) const PLANNED_ASSET_PATHS: [&str; 15] = [
    WALL_TEX,
    FLOOR_TEX,
    CEILING_TEX,
    LIGHT_FIXTURE_MODEL,
    EXIT_GATE_MODEL,
    EXIT_PANEL_TEX,
    PLAYER_MODEL,
    BOT_MODEL,
    EQUIPMENT_MODEL,
    HAZARD_MODEL,
    FOOTSTEP_SOUND,
    REROUTE_SOUND,
    ESCAPE_SOUND,
    AMBIENCE_SOUND,
    ENVIRONMENT_HDR,
];

pub(crate) fn all_planned_assets_present() -> bool {
    PLANNED_ASSET_PATHS.iter().all(|path| asset_present(path))
}

/// Shared meshes/materials for the solid maze, resolved with the drop-in convention:
/// textured/models/audio if present, procedural fallbacks otherwise.
#[derive(Resource)]
// Some drop-in slots (GLB scenes, panel/halo meshes, team materials) are retained for
// the asset plan and future per-place props but aren't read by the teleport renderer.
#[allow(dead_code)]
pub struct MatchAssets {
    pub(crate) floor_mesh: Handle<Mesh>,
    pub(crate) wall_mesh: Handle<Mesh>,
    pub(crate) ceiling_mesh: Handle<Mesh>,
    pub(crate) panel_mesh: Handle<Mesh>,
    pub(crate) placeholder_mesh: Handle<Mesh>,
    pub(crate) halo_mesh: Handle<Mesh>,
    /// Unit-radius torus used for exact gameplay-radius rings (for example anchors).
    pub(crate) radius_ring_mesh: Handle<Mesh>,
    pub(crate) door_post_mesh: Handle<Mesh>,
    pub(crate) door_lintel_mesh: Handle<Mesh>,
    pub(crate) door_leaf_mesh: Handle<Mesh>,
    pub(crate) objective_beam_mesh: Handle<Mesh>,
    pub(crate) rival_body_mesh: Handle<Mesh>,
    pub(crate) floor_material: Handle<StandardMaterial>,
    pub(crate) spine_floor_material: Handle<StandardMaterial>,
    pub(crate) safe_floor_material: Handle<StandardMaterial>,
    pub(crate) trap_active_material: Handle<StandardMaterial>,
    pub(crate) trap_idle_material: Handle<StandardMaterial>,
    /// The gantry's raised jump-map deck surface (upper route).
    pub(crate) gantry_deck_material: Handle<StandardMaterial>,
    /// Grey concrete for the wellshaft pillar, ledges, treads and guard rails.
    pub(crate) wellshaft_stone_material: Handle<StandardMaterial>,
    /// The gantry deck's lit rim — the readable jump/fall commitment line.
    pub(crate) gantry_edge_material: Handle<StandardMaterial>,
    /// The gantry's lower understory landing — "where a fall puts you".
    pub(crate) understory_material: Handle<StandardMaterial>,
    /// Collapse-sealed doorway fill. Signal-tier rubble from the shared style module.
    pub(crate) rubble_material: Handle<StandardMaterial>,
    pub(crate) wall_material: Handle<StandardMaterial>,
    pub(crate) ceiling_material: Handle<StandardMaterial>,
    pub(crate) exit_panel_material: Handle<StandardMaterial>,
    pub(crate) fixture_glow_material: Handle<StandardMaterial>,
    /// A warm, glowing lamp body for the per-place ceiling fixtures.
    pub(crate) lamp_material: Handle<StandardMaterial>,
    /// Emissive wall-trim materials, one per district (indexed by `District::index`), so
    /// the structural baseboard/cornice linework carries the neighbourhood's accent.
    pub(crate) district_accent_materials: [Handle<StandardMaterial>; 6],
    pub(crate) placeholder_material: Handle<StandardMaterial>,
    pub(crate) doorframe_material: Handle<StandardMaterial>,
    pub(crate) spine_doorframe_material: Handle<StandardMaterial>,
    pub(crate) door_leaf_material: Handle<StandardMaterial>,
    pub(crate) objective_material: Handle<StandardMaterial>,
    pub(crate) rival_material: Handle<StandardMaterial>,
    pub(crate) anchor_torch_material: Handle<StandardMaterial>,
    pub(crate) teleport_pad_material: Handle<StandardMaterial>,
    pub(crate) team_materials: [Handle<StandardMaterial>; TEAM_COUNT],
    pub(crate) light_fixture: Option<Handle<Scene>>,
    pub(crate) exit_gate: Option<Handle<Scene>>,
    pub(crate) player: Option<Handle<Scene>>,
    pub(crate) bot: Option<Handle<Scene>>,
    pub(crate) equipment: Option<Handle<Scene>>,
    pub(crate) hazard: Option<Handle<Scene>>,
    /// Canonical CC0 Modular Space dressing selected by the committed manifest.
    pub(crate) threshold_gate: Option<ContentScene>,
    pub(crate) cable_bundle: Option<ContentScene>,
    pub(crate) runner_stand: Option<Handle<Image>>,
    pub(crate) runner_walk1: Option<Handle<Image>>,
    pub(crate) runner_walk2: Option<Handle<Image>>,
    pub(crate) rival_stand: Option<Handle<Image>>,
    pub(crate) rival_walk1: Option<Handle<Image>>,
    pub(crate) rival_walk2: Option<Handle<Image>>,
    pub(crate) guardian_stand: Option<Handle<Image>>,
    pub(crate) control_device: Option<Handle<Image>>,
    pub(crate) keystone_card: Option<Handle<Image>>,
    pub(crate) keystone_core: Option<Handle<Image>>,
    pub(crate) exit_access_card: Option<Handle<Image>>,
    pub(crate) anchor_torch: Option<Handle<Image>>,
    pub(crate) route_cell: Option<Handle<Image>>,
    pub(crate) relay_device: Option<Handle<Image>>,
    pub(crate) battery_charge: Option<Handle<Image>>,
    pub(crate) repair_token: Option<Handle<Image>>,
    pub(crate) footstep: Option<Handle<AudioSource>>,
    pub(crate) reroute: Option<Handle<AudioSource>>,
    pub(crate) escape: Option<Handle<AudioSource>>,
    pub(crate) ambience: Option<Handle<AudioSource>>,
    pub(crate) door: Option<Handle<AudioSource>>,
    pub(crate) klaxon: Option<Handle<AudioSource>>,
    pub(crate) collapse_sting: Option<Handle<AudioSource>>,
    pub(crate) click_sound: Option<Handle<AudioSource>>,
    pub(crate) hover_sound: Option<Handle<AudioSource>>,
    pub(crate) jump: Option<Handle<AudioSource>>,
    pub(crate) land: Option<Handle<AudioSource>>,
    pub(crate) tool_interact: Option<Handle<AudioSource>>,
    pub(crate) keystone: Option<Handle<AudioSource>>,
    pub(crate) exit_unlock: Option<Handle<AudioSource>>,
    pub(crate) guardian_dread: Option<Handle<AudioSource>>,
    pub(crate) district_ambience: [Option<Handle<AudioSource>>; DISTRICT_COUNT],
    pub(crate) ambience_corridor: Option<Handle<AudioSource>>,
    pub(crate) ambience_gantry: Option<Handle<AudioSource>>,
    pub(crate) rival_actor_sheet: Option<Handle<Image>>,
    pub(crate) rival_actor_layout: Option<Handle<TextureAtlasLayout>>,
    pub(crate) rival_actor_meta: Option<SpriteMetadata>,
    pub(crate) guardian_actor_sheet: Option<Handle<Image>>,
    pub(crate) guardian_actor_layout: Option<Handle<TextureAtlasLayout>>,
    pub(crate) guardian_actor_meta: Option<SpriteMetadata>,
    pub(crate) decor_column: Option<Handle<Image>>,
    pub(crate) decor_torch: Option<Handle<Image>>,
    pub(crate) decor_lab_crate: Option<Handle<Image>>,
    pub(crate) decor_lab_table: Option<Handle<Image>>,
    pub(crate) wall_albedo_lab: Option<Handle<Image>>,
}

fn png_dimensions<P: AsRef<std::path::Path>>(path: P) -> Option<(u32, u32)> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header).ok()?;
    if header[0..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return None;
    }
    if header[12..16] != [73, 72, 68, 82] {
        return None;
    }
    let w = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let h = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    Some((w, h))
}

impl MatchAssets {
    /// Resolve every slot once at Match entry: build the procedural meshes, derive the
    /// neon materials from `observed_style`, and load whichever drop-in files are
    /// present (absent slots stay `None` and fall back procedurally).
    pub(crate) fn load(
        asset_server: &AssetServer,
        content_manifest: &observed_content::ContentManifest,
        texture_atlases: &mut Assets<TextureAtlasLayout>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        let load_texture =
            |path: &'static str| asset_present(path).then(|| asset_server.load::<Image>(path));
        // Structural surfaces (walls/floors) get world-unit UVs that run well past 1.0,
        // so their albedo must sample with a repeating address mode. Bevy's default
        // sampler clamps to the edge, which smears the border texel across the whole
        // surface — the flat, texture-less look bug backlog #2 captured.
        let wall_texture = load_repeating_texture(asset_server, WALL_TEX);
        let floor_texture = load_repeating_texture(asset_server, FLOOR_TEX);

        let floor_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::Plain),
            floor_texture.clone(),
        ));
        let spine_floor_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::Spine),
            floor_texture.clone(),
        ));
        let safe_floor_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::SafeBypass),
            floor_texture.clone(),
        ));
        let trap_active_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::TrapArmed),
            floor_texture.clone(),
        ));
        let trap_idle_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::TrapIdle),
            floor_texture.clone(),
        ));
        let gantry_deck_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::GantryDeck),
            floor_texture,
        ));
        let wellshaft_stone_material =
            materials.add(neon_material(&style::surface(SurfaceRole::WellshaftStone)));
        // The commitment line and the understory landing are both `signal: true`
        // treatments (Legibility Contract: gameplay-critical reads stay unlit/emissive
        // like the district accents, not modulated by scene lighting).
        let gantry_edge_treatment = style::surface(SurfaceRole::GantryEdge);
        let gantry_edge_material = materials.add(StandardMaterial {
            base_color: gantry_edge_treatment.base_color,
            emissive: gantry_edge_treatment.emissive,
            unlit: true,
            ..default()
        });
        let understory_treatment = style::surface(SurfaceRole::Understory);
        let understory_material = materials.add(StandardMaterial {
            base_color: understory_treatment.base_color,
            emissive: understory_treatment.emissive,
            unlit: true,
            ..default()
        });
        let rubble_treatment = style::surface(SurfaceRole::Rubble);
        let rubble_material = materials.add(StandardMaterial {
            base_color: rubble_treatment.base_color,
            emissive: rubble_treatment.emissive,
            unlit: true,
            ..default()
        });
        let wall_material = materials.add(textured_neon_material(
            &style::surface(SurfaceRole::Wall),
            wall_texture,
        ));
        let ceiling_material = materials.add(StandardMaterial {
            cull_mode: None,
            double_sided: true,
            ..textured_neon_material(&style::surface(SurfaceRole::Ceiling), None)
        });
        let exit_panel_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: asset_present(EXIT_PANEL_TEX)
                .then(|| asset_server.load(EXIT_PANEL_TEX)),
            emissive: LinearRgba::rgb(0.08, 5.0, 0.35),
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let fixture_glow_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.9, 1.0),
            emissive: LinearRgba::rgb(4.0, 7.0, 10.0),
            unlit: true,
            ..default()
        });
        let lamp_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.85, 0.72),
            emissive: LinearRgba::rgb(3.2, 2.7, 1.8),
            unlit: true,
            ..default()
        });
        let district_accent_materials = std::array::from_fn(|i| {
            let accent = style::district(style::District::ALL[i]).accent;
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.02, 0.03, 0.05),
                emissive: LinearRgba::rgb(
                    accent.red * 10.0,
                    accent.green * 10.0,
                    accent.blue * 10.0,
                ),
                unlit: true,
                ..default()
            })
        });
        let placeholder_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.10, 0.14),
            emissive: LinearRgba::rgb(0.10, 0.30, 0.45),
            perceptual_roughness: 0.7,
            ..default()
        });
        let doorframe_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.07, 0.11),
            emissive: LinearRgba::rgb(0.35, 1.9, 2.5),
            perceptual_roughness: 0.5,
            ..default()
        });
        let spine_doorframe_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.08, 0.03),
            emissive: LinearRgba::rgb(2.6, 1.7, 0.5),
            perceptual_roughness: 0.5,
            ..default()
        });
        let door_leaf_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.06, 0.10),
            emissive: LinearRgba::rgb(0.10, 0.32, 0.5),
            perceptual_roughness: 0.55,
            ..default()
        });
        let objective = style::marker(MarkerRole::NextRoom);
        let objective_material = materials.add(StandardMaterial {
            base_color: objective.base_color,
            emissive: objective.emissive,
            unlit: true,
            ..default()
        });
        let rival = style::marker(MarkerRole::Rival);
        let rival_material = materials.add(StandardMaterial {
            base_color: rival.base_color,
            emissive: rival.emissive,
            perceptual_roughness: 0.6,
            ..default()
        });
        let anchor = style::marker(MarkerRole::Control);
        let anchor_torch_material = materials.add(StandardMaterial {
            base_color: anchor.base_color,
            emissive: anchor.emissive,
            unlit: true,
            ..default()
        });
        let pad = style::marker(MarkerRole::You);
        let teleport_pad_material = materials.add(StandardMaterial {
            base_color: pad.base_color,
            emissive: pad.emissive,
            unlit: true,
            ..default()
        });
        let team_materials = TEAM_COLORS.map(|color| {
            materials.add(StandardMaterial {
                base_color: color.with_alpha(0.58),
                emissive: color.to_linear() * 1.5,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })
        });
        let load_scene = |path: &'static str| {
            asset_present(path)
                .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset(path)))
        };
        let load_sound = |path: &'static str| {
            asset_present(path).then(|| asset_server.load::<AudioSource>(path))
        };

        let mut load_actor_sheet = |path_png: &'static str| -> (
            Option<Handle<Image>>,
            Option<Handle<TextureAtlasLayout>>,
            Option<SpriteMetadata>,
        ) {
            if !asset_present(path_png) {
                return (None, None, None);
            }
            let img_handle = asset_server.load::<Image>(path_png);
            let path_json = path_png.replace(".png", ".json");
            let json_full = assets_dir().join(&path_json);
            if let Ok(meta) = SpriteMetadata::load_from_path(json_full) {
                let img_full = assets_dir().join(path_png);
                if let Some((w, h)) = png_dimensions(&img_full) {
                    let mut layout = TextureAtlasLayout::new_empty(UVec2::new(w, h));
                    for f in &meta.frames {
                        layout.add_texture(URect::new(f.x, f.y, f.x + f.w, f.y + f.h));
                    }
                    let layout_handle = texture_atlases.add(layout);
                    return (Some(img_handle), Some(layout_handle), Some(meta));
                }
            }
            (Some(img_handle), None, None)
        };

        let (rival_actor_sheet, rival_actor_layout, rival_actor_meta) =
            load_actor_sheet(RIVAL_ACTOR_SPRITE);
        let (guardian_actor_sheet, guardian_actor_layout, guardian_actor_meta) =
            load_actor_sheet(GUARDIAN_ACTOR_SPRITE);

        debug_assert_eq!(
            observed_assets::DISTRICT_AMBIENCE.len(),
            observed_style::District::ALL.len(),
            "district ambience manifest must stay aligned with observed_style::District::ALL",
        );

        Self {
            floor_mesh: meshes.add(Plane3d::default().mesh().size(PLACE_TILE, PLACE_TILE)),
            wall_mesh: meshes.add(Cuboid::new(PLACE_TILE, WALL_HEIGHT, PLACE_TILE)),
            ceiling_mesh: meshes.add(Plane3d::default().mesh().size(PLACE_TILE, PLACE_TILE)),
            panel_mesh: meshes.add(Rectangle::new(4.4, 2.2)),
            placeholder_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            halo_mesh: meshes.add(Cylinder::new(0.46, 0.025)),
            radius_ring_mesh: meshes.add(Torus::new(0.98, 1.0)),
            door_post_mesh: meshes.add(Cuboid::new(DOOR_POST_W, WALL_HEIGHT, DOOR_POST_D)),
            door_lintel_mesh: meshes.add(Cuboid::new(HALL_WIDTH, DOOR_LINTEL_H, DOOR_POST_D)),
            door_leaf_mesh: meshes.add(Cuboid::new(
                HALL_WIDTH - 2.0 * DOOR_POST_W,
                WALL_HEIGHT - DOOR_LINTEL_H,
                DOOR_LEAF_D,
            )),
            objective_beam_mesh: meshes.add(Cylinder::new(0.16, 9.0)),
            rival_body_mesh: meshes.add(Capsule3d::new(0.32, 1.0)),
            floor_material,
            spine_floor_material,
            safe_floor_material,
            trap_active_material,
            trap_idle_material,
            gantry_deck_material,
            wellshaft_stone_material,
            gantry_edge_material,
            understory_material,
            rubble_material,
            wall_material,
            ceiling_material,
            exit_panel_material,
            fixture_glow_material,
            lamp_material,
            district_accent_materials,
            placeholder_material,
            doorframe_material,
            spine_doorframe_material,
            door_leaf_material,
            objective_material,
            rival_material,
            anchor_torch_material,
            teleport_pad_material,
            team_materials,
            light_fixture: load_scene(LIGHT_FIXTURE_MODEL),
            exit_gate: load_scene(EXIT_GATE_MODEL),
            player: load_scene(PLAYER_MODEL),
            bot: load_scene(BOT_MODEL),
            equipment: load_scene(EQUIPMENT_MODEL),
            hazard: load_scene(HAZARD_MODEL),
            threshold_gate: load_content_scene(asset_server, content_manifest, "kenney_gate"),
            cable_bundle: load_content_scene(asset_server, content_manifest, "kenney_cables"),
            runner_stand: load_texture(RUNNER_STAND_SPRITE),
            runner_walk1: load_texture(RUNNER_WALK1_SPRITE),
            runner_walk2: load_texture(RUNNER_WALK2_SPRITE),
            rival_stand: load_texture(RIVAL_STAND_SPRITE),
            rival_walk1: load_texture(RIVAL_WALK1_SPRITE),
            rival_walk2: load_texture(RIVAL_WALK2_SPRITE),
            guardian_stand: load_texture(GUARDIAN_STAND_SPRITE),
            control_device: load_texture(CONTROL_DEVICE_SPRITE),
            keystone_card: load_texture(KEYSTONE_CARD_SPRITE),
            keystone_core: load_texture(KEYSTONE_CORE_SPRITE),
            exit_access_card: load_texture(EXIT_ACCESS_CARD_SPRITE),
            anchor_torch: load_texture(ANCHOR_TORCH_SPRITE),
            route_cell: load_texture(ROUTE_CELL_SPRITE),
            relay_device: load_texture(RELAY_DEVICE_SPRITE),
            battery_charge: load_texture(BATTERY_CHARGE_SPRITE),
            repair_token: load_texture(REPAIR_TOKEN_SPRITE),
            footstep: load_sound(FOOTSTEP_SOUND),
            reroute: load_sound(REROUTE_SOUND),
            escape: load_sound(ESCAPE_SOUND),
            ambience: load_sound(AMBIENCE_SOUND),
            door: load_sound(DOOR_SOUND),
            klaxon: load_sound(KLAXON_SOUND),
            collapse_sting: load_sound(COLLAPSE_STING_SOUND),
            click_sound: load_sound(UI_CLICK_SOUND),
            hover_sound: load_sound(UI_HOVER_SOUND),
            jump: load_sound(JUMP_SOUND),
            land: load_sound(LAND_SOUND),
            tool_interact: load_sound(TOOL_INTERACT_SOUND),
            keystone: load_sound(KEYSTONE_SOUND),
            exit_unlock: load_sound(EXIT_UNLOCK_SOUND),
            guardian_dread: load_sound(GUARDIAN_DREAD_SOUND),
            district_ambience: std::array::from_fn(|i| {
                load_sound(observed_assets::DISTRICT_AMBIENCE[i].path)
            }),
            ambience_corridor: load_sound(AMBIENCE_CORRIDOR_SOUND),
            ambience_gantry: load_sound(AMBIENCE_GANTRY_SOUND),
            rival_actor_sheet,
            rival_actor_layout,
            rival_actor_meta,
            guardian_actor_sheet,
            guardian_actor_layout,
            guardian_actor_meta,
            decor_column: load_texture(DECOR_COLUMN_SPRITE),
            decor_torch: load_texture(DECOR_TORCH_SPRITE),
            decor_lab_crate: load_texture(DECOR_LAB_CRATE_SPRITE),
            decor_lab_table: load_texture(DECOR_LAB_TABLE_SPRITE),
            wall_albedo_lab: load_repeating_texture(asset_server, WALL_ALBEDO_LAB_TEX),
        }
    }

    pub(crate) fn rival_sprite(
        &self,
        images: &Assets<Image>,
        frame: usize,
    ) -> Option<Handle<Image>> {
        let image = match frame % 3 {
            1 => &self.rival_walk1,
            2 => &self.rival_walk2,
            _ => &self.rival_stand,
        };
        crate::view::sprites::ready_image(images, image)
    }

    pub(crate) fn guardian_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.guardian_stand)
    }

    pub(crate) fn control_device_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.control_device)
    }

    pub(crate) fn keystone_card_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.keystone_card)
    }

    pub(crate) fn keystone_core_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.keystone_core)
    }

    pub(crate) fn exit_access_card_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.exit_access_card)
    }

    pub(crate) fn anchor_torch_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.anchor_torch)
    }

    pub(crate) fn route_cell_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.route_cell)
    }

    pub(crate) fn relay_device_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.relay_device)
    }

    #[allow(dead_code)]
    pub(crate) fn battery_charge_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.battery_charge)
    }

    #[allow(dead_code)]
    pub(crate) fn repair_token_sprite(&self, images: &Assets<Image>) -> Option<Handle<Image>> {
        crate::view::sprites::ready_image(images, &self.repair_token)
    }
}
