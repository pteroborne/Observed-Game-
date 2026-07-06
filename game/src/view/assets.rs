//! The match's drop-in asset registry: slot paths from the shared `observed_assets`
//! manifest, presence checks, the neon-noir material builders, and [`MatchAssets`] —
//! the shared meshes/materials/scenes/sounds resource the place renderer draws from.
//! Drop a file at a slot's path and the match uses it; leave it absent and the match
//! falls back procedurally.

use std::path::PathBuf;

use bevy::prelude::*;
use observed_match::facility::TEAM_COUNT;
use observed_style::{self as style, MarkerRole, SurfaceRole};

use crate::layout::{HALL_WIDTH, PLACE_TILE, WALL_HEIGHT};
use crate::view::theme::TEAM_COLORS;

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

// Procedural neon doorways (code-as-art; no GLB). A closed leaf hides the corridor
// beyond (mystery) and slides up into the lintel as the player approaches. The frame
// spans the FULL hall width ([`crate::layout::HALL_WIDTH`]) so doorways and hallways
// line up by design.
pub(crate) const DOOR_POST_W: f32 = 0.22;
pub(crate) const DOOR_POST_D: f32 = 0.5;
pub(crate) const DOOR_LINTEL_H: f32 = 0.34;
pub(crate) const DOOR_LEAF_D: f32 = 0.14;

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
        material.base_color = Color::WHITE;
        material.unlit = true;
    }
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
    pub(crate) district_ambience: [Option<Handle<AudioSource>>; 6],
}

impl MatchAssets {
    /// Resolve every slot once at Match entry: build the procedural meshes, derive the
    /// neon materials from `observed_style`, and load whichever drop-in files are
    /// present (absent slots stay `None` and fall back procedurally).
    pub(crate) fn load(
        asset_server: &AssetServer,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        let load_texture =
            |path: &'static str| asset_present(path).then(|| asset_server.load::<Image>(path));
        let wall_texture = load_texture(WALL_TEX);
        let floor_texture = load_texture(FLOOR_TEX);
        let ceiling_texture = load_texture(CEILING_TEX);

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
            ..textured_neon_material(&style::surface(SurfaceRole::Ceiling), ceiling_texture)
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

        Self {
            floor_mesh: meshes.add(Plane3d::default().mesh().size(PLACE_TILE, PLACE_TILE)),
            wall_mesh: meshes.add(Cuboid::new(PLACE_TILE, WALL_HEIGHT, PLACE_TILE)),
            ceiling_mesh: meshes.add(Plane3d::default().mesh().size(PLACE_TILE, PLACE_TILE)),
            panel_mesh: meshes.add(Rectangle::new(4.4, 2.2)),
            placeholder_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            halo_mesh: meshes.add(Cylinder::new(0.46, 0.025)),
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
            district_ambience: [
                load_sound("sounds/ambience_archive.ogg"),
                load_sound("sounds/ambience_reactor.ogg"),
                load_sound("sounds/ambience_atrium.ogg"),
                load_sound("sounds/ambience_foundry.ogg"),
                load_sound("sounds/ambience_hollow.ogg"),
                load_sound("sounds/ambience_spillway.ogg"),
            ],
        }
    }
}
