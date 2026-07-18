//! Streamed presentation for the canonical hex facility.
//!
//! The authoritative geometry snapshot is the single collision/render source; this
//! module adds style-owned materials, a bounded lighting/post-process rig, and
//! visibility streaming by proximity to the runner.

mod assets;
mod lighting;
mod shell;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexCoord;
use observed_hex::hex_origin;

use self::assets::HexWfcVisualAssets;
use super::sim::HexWfcRuntime;
use crate::view::components::{GameCam, GameSun, MENU_SUN_ILLUMINANCE};

/// Streaming window (metres, plan view) around the runner. Cells farther than this in
/// the horizontal plane, or more than [`STREAM_LEVELS`] levels away, are hidden.
const STREAM_RADIUS: f32 = 30.0;
const STREAM_LEVELS: u8 = 2;

#[derive(Component)]
pub(super) struct HexWfcCell(pub HexCoord);

/// Every shell entity carries this so a relayout can clear the whole facility at once.
#[derive(Component)]
pub(super) struct HexWfcGeometry;

#[derive(Component)]
pub(super) struct HexWfcKeyLight;

#[derive(Component)]
pub(super) struct HexHeadlamp;

pub(super) fn setup_view(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut runtime: ResMut<HexWfcRuntime>,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    let mut assets = HexWfcVisualAssets::load(&asset_server, &mut materials);
    shell::spawn_geometry(&mut commands, &mut assets, &mut meshes, &runtime);
    runtime.pending_full_rebuild = false;

    let architecture = *runtime
        .match_state
        .facility
        .architecture
        .get(&runtime.local().cell)
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    let palette = observed_style::architecture(architecture);
    if let Ok(camera) = camera.single() {
        commands.entity(camera).insert((
            Hdr,
            Bloom {
                intensity: 0.08,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: palette.fog_color,
                falloff: FogFalloff::Linear {
                    start: palette.fog_start,
                    end: palette.fog_end,
                },
                ..default()
            },
            Msaa::Off,
            Fxaa::default(),
            DepthPrepass,
            NormalPrepass,
            ScreenSpaceAmbientOcclusion {
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
                ..default()
            },
        ));
    }
    for mut light in &mut sun {
        light.illuminance = 0.0;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });
    commands.insert_resource(ClearColor(Color::srgb(0.002, 0.006, 0.015)));
    lighting::spawn_rig(&mut commands);
    commands.insert_resource(assets);
}

pub(super) fn sync_changed_geometry(
    mut commands: Commands,
    mut runtime: ResMut<HexWfcRuntime>,
    mut assets: ResMut<HexWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<Entity, With<HexWfcGeometry>>,
) {
    if !runtime.pending_full_rebuild {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    shell::spawn_geometry(&mut commands, &mut assets, &mut meshes, &runtime);
    runtime.pending_full_rebuild = false;
}

pub(super) fn sync_streamed_cells(
    runtime: Res<HexWfcRuntime>,
    mut cells: Query<(&HexWfcCell, &mut Visibility)>,
) {
    let focus = runtime.local();
    for (cell, mut visibility) in &mut cells {
        let origin = Vec3::from_array(hex_origin(cell.0));
        let plan = Vec2::new(origin.x - focus.position.x, origin.z - focus.position.z).length();
        let level_gap = cell.0.level.abs_diff(focus.cell.level);
        *visibility = if plan <= STREAM_RADIUS && level_gap <= STREAM_LEVELS {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) use lighting::{sync_camera_and_headlamp, sync_lighting_and_atmosphere};

pub(super) fn clear_view(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    if let Ok(camera) = camera.single() {
        commands
            .entity(camera)
            .remove::<(
                Hdr,
                Bloom,
                DistanceFog,
                Fxaa,
                ScreenSpaceAmbientOcclusion,
                DepthPrepass,
                NormalPrepass,
            )>()
            .insert(Msaa::Sample4);
    }
    for mut light in &mut sun {
        light.illuminance = MENU_SUN_ILLUMINANCE;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
    commands.insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.065)));
    commands.remove_resource::<HexWfcVisualAssets>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_window_is_bounded() {
        // A cell directly under the runner is streamed; far plan distance or level gap
        // is culled. (Pure arithmetic mirror of `sync_streamed_cells`.)
        let close = Vec2::new(4.0, 3.0).length();
        assert!(close <= STREAM_RADIUS);
        assert!(0u8.abs_diff(1) <= STREAM_LEVELS);
        assert!(3u8.abs_diff(0) > STREAM_LEVELS);
    }
}
