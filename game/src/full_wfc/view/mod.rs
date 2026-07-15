//! Streamed presentation for the canonical continuous full-WFC facility.
//!
//! The authoritative geometry snapshot remains the single collision/render source;
//! this module adds style-owned materials, register dressing, imported threshold
//! silhouettes, and a bounded lighting/post-process rig.

mod assets;
mod lighting;
mod registers;
mod shell;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::light::VolumetricFog;
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_facility::full_wfc::{CellCoord, ThresholdKey};
use observed_style::ThresholdFrameState;

use super::sim::FullWfcRuntime;
use super::{FullWfcCapture, FullWfcCaptureMode};
use crate::content::GameContent;
use crate::view::components::{GameCam, GameSun, MENU_SUN_ILLUMINANCE};
use assets::FullWfcVisualAssets;

#[derive(Component)]
pub(super) struct FullWfcCell(pub CellCoord);

#[derive(Component)]
pub(super) struct CandleLight;

#[derive(Component)]
pub(super) struct ThresholdSignal(ThresholdKey);

#[derive(Component)]
pub(super) struct CellPractical {
    cell: CellCoord,
    architecture: observed_content::ArchitectureRegister,
    phase: f32,
    detail: f32,
}

#[derive(Component)]
pub(super) struct FullWfcKeyLight;

#[derive(Component)]
pub(super) struct FullWfcFogVolume;

#[allow(clippy::too_many_arguments)]
pub(super) fn setup_view(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    content: Res<GameContent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut runtime: ResMut<FullWfcRuntime>,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    let mut assets =
        FullWfcVisualAssets::load(&asset_server, &content, &mut meshes, &mut materials);
    for placement in runtime.match_state.facility.placements.values() {
        shell::spawn_cell(
            &mut commands,
            &mut assets,
            &mut meshes,
            placement,
            &runtime.match_state.facility,
            runtime.match_state.facility.exit(),
            &runtime.match_state.geometry.pieces,
        );
    }
    runtime.pending_visual_changes.clear();

    let palette = runtime
        .match_state
        .facility
        .placement(runtime.local().cell)
        .map(|placement| observed_style::architecture(placement.architecture))
        .unwrap_or_else(|| observed_style::district(observed_style::District::Archive));
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
    mut runtime: ResMut<FullWfcRuntime>,
    mut assets: ResMut<FullWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<(Entity, &FullWfcCell)>,
) {
    if runtime.pending_visual_changes.is_empty() {
        return;
    }
    let changed = std::mem::take(&mut runtime.pending_visual_changes);
    for (entity, cell) in &existing {
        if changed.contains(&cell.0) {
            commands.entity(entity).despawn();
        }
    }
    for coord in changed {
        if let Some(placement) = runtime.match_state.facility.placement(coord) {
            shell::spawn_cell(
                &mut commands,
                &mut assets,
                &mut meshes,
                placement,
                &runtime.match_state.facility,
                runtime.match_state.facility.exit(),
                &runtime.match_state.geometry.pieces,
            );
        }
    }
}

pub(super) fn sync_streamed_cells(
    runtime: Res<FullWfcRuntime>,
    capture: Option<Res<FullWfcCapture>>,
    mut cells: Query<(&FullWfcCell, &mut Visibility)>,
) {
    let center = presentation_focus(&runtime, capture.as_deref());
    for (cell, mut visibility) in &mut cells {
        *visibility = if cell_is_streamed(center, cell.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn presentation_focus(
    runtime: &FullWfcRuntime,
    capture: Option<&FullWfcCapture>,
) -> CellCoord {
    let Some(capture) = capture.filter(|capture| capture.mode == FullWfcCaptureMode::Style) else {
        return runtime.local().cell;
    };
    let index = (usize::from(capture.frame) / 45)
        .min(observed_content::ArchitectureRegister::ALL.len() - 1);
    let register = observed_content::ArchitectureRegister::ALL[index];
    runtime
        .match_state
        .facility
        .placements
        .values()
        .find(|placement| {
            placement.architecture == register
                && placement.space == observed_facility::full_wfc::ModuleSpace::Room
        })
        .or_else(|| {
            runtime
                .match_state
                .facility
                .placements
                .values()
                .find(|placement| {
                    placement.architecture == register
                        && placement.space != observed_facility::full_wfc::ModuleSpace::Void
                })
        })
        .map_or(runtime.local().cell, |placement| placement.coord)
}

fn cell_is_streamed(center: CellCoord, candidate: CellCoord) -> bool {
    center.x.abs_diff(candidate.x) <= 3
        && center.z.abs_diff(candidate.z) <= 3
        && center.level.abs_diff(candidate.level) <= 1
}

pub(super) fn sync_threshold_signals(
    runtime: Res<FullWfcRuntime>,
    assets: Res<FullWfcVisualAssets>,
    mut signals: Query<(&ThresholdSignal, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (signal, mut material) in &mut signals {
        let state = if terminal_face_is_reserved(&runtime, signal.0) {
            ThresholdFrameState::Sealed
        } else if runtime
            .match_state
            .equipment
            .anchors_in(signal.0.room)
            .next()
            .is_some()
        {
            ThresholdFrameState::Anchored
        } else {
            ThresholdFrameState::Mutable
        };
        material.0 = assets.threshold(state);
    }
}

fn terminal_face_is_reserved(runtime: &FullWfcRuntime, threshold: ThresholdKey) -> bool {
    let world = &runtime.match_state.facility;
    if threshold.room == world.exit() {
        return world.reserved_exit_faces.contains(&threshold.face);
    }
    world
        .config
        .neighbor(threshold.room, threshold.face)
        .is_some_and(|next| {
            next == world.exit()
                && world
                    .reserved_exit_faces
                    .contains(&threshold.face.opposite())
        })
}

pub(super) use lighting::{sync_camera_and_candle, sync_lighting_and_atmosphere};
pub(super) use shell::normalize_imported_materials;

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
                VolumetricFog,
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
    commands.remove_resource::<FullWfcVisualAssets>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_window_is_bounded() {
        let center = CellCoord::new(4, 4, 1);
        assert!(cell_is_streamed(center, center));
        assert!(cell_is_streamed(center, CellCoord::new(7, 1, 2)));
        assert!(!cell_is_streamed(center, CellCoord::new(8, 4, 1)));
        assert!(!cell_is_streamed(center, CellCoord::new(4, 4, 3)));
    }
}
