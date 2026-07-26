//! Streamed presentation for the canonical hex facility.
//!
//! The authoritative geometry snapshot is the single collision/render source; this
//! module adds style-owned materials, a bounded lighting/post-process rig, and
//! visibility streaming by proximity to the runner.

use std::time::Instant;

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

/// The full set of grid cells a [`HexWfcCell`]'s fixture group actually
/// occupies. An ordinary tile's footprint is just its own coordinate. A
/// whole-room module is different: `push_room` in
/// `observed_match::hex_wfc::geometry` stamps every hull of the room with
/// `source_cell = anchor`, so all of a room's pieces are grouped under one
/// [`HexWfcCell`] keyed by its anchor — `shell::cell_footprint` recovers the
/// room's true footprint from the solved world's stamped blueprints so this
/// carries every cell the room occupies, not only its anchor. Streaming keys
/// off "any cell of the footprint is in range" so a large multi-cell room
/// stays visible while the player is anywhere inside it. For today's
/// room-less catalog (and every ordinary tile once rooms exist) this is
/// always a single-element list equal to the `HexWfcCell` coordinate, so
/// streaming behavior is unchanged.
#[derive(Component)]
pub(super) struct HexWfcCellFootprint(pub Vec<HexCoord>);

/// Every shell entity carries this so a relayout can clear the whole facility at once.
#[derive(Component)]
pub(super) struct HexWfcGeometry;

#[derive(Component)]
pub(super) struct HexWfcKeyLight;

/// A per-tile downlight fixture (tier 2 of the rig). Carries its cell so the shadow
/// budget can pick the fixtures nearest the runner to cast; the rest stay shadowless
/// fill. Every non-boundary cell gets one, so no tile is left without a light source.
#[derive(Component)]
pub(super) struct HexPractical(pub HexCoord);

#[allow(clippy::too_many_arguments)]
pub(super) fn setup_view(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    runtime: Res<HexWfcRuntime>,
    mut camera: Query<(Entity, &mut Transform), With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
    mut perf: Option<ResMut<super::perf::HexPerfMetrics>>,
) {
    let started = Instant::now();
    let architecture = *runtime
        .match_state
        .facility
        .architecture
        .get(&runtime.local().cell)
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    let palette = observed_style::architecture(architecture);
    let current = runtime.local().cell;
    if let Ok((camera, mut transform)) = camera.single_mut() {
        lighting::prime_camera(&mut transform, runtime.local());
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
                // The authored shell already carries strong semantic edge light and
                // fog separation. Low SSAO preserves local contact depth without
                // consuming the GPU margin needed by a 1440x900 mutation frame.
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
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
    commands.insert_resource(ClearColor(palette.fog_color));
    lighting::spawn_rig(&mut commands, architecture, current, runtime.local());

    // Geometry is deliberately enqueued only after the camera, atmosphere, menu sun,
    // and both semantic lights have their exact initial values. The renderer therefore
    // cannot observe a shell under the outgoing menu rig.
    let mut assets = HexWfcVisualAssets::load(&asset_server, &mut materials);
    shell::spawn_geometry(&mut commands, &mut assets, &mut meshes, &runtime);
    commands.insert_resource(assets);
    super::perf::record_view(
        &mut perf,
        super::perf::ViewTimingKind::Startup,
        &runtime,
        started.elapsed(),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sync_changed_geometry(
    mut commands: Commands,
    mut runtime: ResMut<HexWfcRuntime>,
    mut assets: ResMut<HexWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing: Query<(Entity, &HexWfcCell)>,
    mut perf: Option<ResMut<super::perf::HexPerfMetrics>>,
) {
    if runtime.pending_visual_cells.is_empty() {
        return;
    }
    let started = Instant::now();
    let changed = std::mem::take(&mut runtime.pending_visual_cells);
    for (entity, cell) in &existing {
        if changed.contains(&cell.0) {
            commands.entity(entity).despawn();
        }
    }
    shell::spawn_cells(&mut commands, &mut assets, &mut meshes, &runtime, &changed);
    super::perf::record_view(
        &mut perf,
        super::perf::ViewTimingKind::MutationRebuild,
        &runtime,
        started.elapsed(),
    );
}

/// Whether `coord` alone is within the streaming window of `focus_position`
/// / `focus_level`. Pure arithmetic core of [`sync_streamed_cells`], kept
/// standalone so [`footprint_in_range`] (and its unit tests) can reuse it.
fn cell_in_stream_range(coord: HexCoord, focus_position: Vec3, focus_level: u8) -> bool {
    let origin = Vec3::from_array(hex_origin(coord));
    let plan = Vec2::new(origin.x - focus_position.x, origin.z - focus_position.z).length();
    let level_gap = coord.level.abs_diff(focus_level);
    plan <= STREAM_RADIUS && level_gap <= STREAM_LEVELS
}

/// A cell's fixture group is streamed in when ANY cell of its footprint is
/// in range — for an ordinary tile the footprint is just its own coordinate
/// (identical to the old single-point check); for a whole-room module it is
/// the room's complete stamped footprint, so a large room stays visible
/// while the player is anywhere inside it rather than popping in/out keyed
/// to a single anchor cell that might be far from the player.
fn footprint_in_range(footprint: &[HexCoord], focus_position: Vec3, focus_level: u8) -> bool {
    footprint
        .iter()
        .any(|&coord| cell_in_stream_range(coord, focus_position, focus_level))
}

pub(super) fn sync_streamed_cells(
    runtime: Res<HexWfcRuntime>,
    mut cells: Query<(&HexWfcCellFootprint, &mut Visibility, Option<&Children>)>,
    mut perf: Option<ResMut<super::perf::HexPerfMetrics>>,
) {
    let focus = runtime.local();
    let (mut visible, mut flipped, mut visible_pieces) = (0usize, 0usize, 0usize);
    for (footprint, mut visibility, children) in &mut cells {
        let wanted = if footprint_in_range(&footprint.0, focus.position, focus.cell.level) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        // Guarded, not unconditional: `Mut` deref marks `Changed<Visibility>`, and Bevy's
        // visibility propagation is gated on exactly that filter. Writing every frame
        // re-propagated the whole facility (~5.6k cells, ~109k child pieces) to reach a
        // steady state it was already in. Same guarded-assignment reasoning as
        // `sync_practical_shadow_budget`.
        if wanted == Visibility::Inherited {
            visible += 1;
            // Draw-call proxy: a cell's children are its hull meshes, and hull counts vary
            // by an order of magnitude across the tile corpus, so resident cells are a far
            // worse cost estimate than resident pieces.
            visible_pieces += children.map_or(0, |children| children.len());
        }
        if *visibility != wanted {
            *visibility = wanted;
            flipped += 1;
        }
    }
    super::perf::record_streaming(&mut perf, visible, flipped, visible_pieces);
}

pub(super) use lighting::{
    sync_camera, sync_lighting_and_atmosphere, sync_practical_shadow_budget,
};

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
        // A cell directly under the runner is streamed; far plan distance or
        // level gap is culled.
        let close = HexCoord {
            q: 5,
            r: 5,
            level: 0,
        };
        let focus_position = Vec3::from_array(hex_origin(close));
        assert!(cell_in_stream_range(close, focus_position, 0));

        let far_plan = HexCoord {
            q: 45,
            r: 5,
            level: 0,
        };
        assert!(!cell_in_stream_range(far_plan, focus_position, 0));

        let far_level = HexCoord {
            q: 5,
            r: 5,
            level: 5,
        };
        assert!(!cell_in_stream_range(far_level, focus_position, 0));
    }

    #[test]
    fn footprint_in_range_is_a_no_op_for_single_cell_footprints() {
        // An ordinary tile's footprint is always `[coord]`; the ANY-of-footprint
        // check used for whole-room modules must degrade to exactly the
        // single-cell check it replaced, near or far.
        let coord = HexCoord {
            q: 10,
            r: 10,
            level: 1,
        };
        let near_focus = Vec3::from_array(hex_origin(coord));
        assert_eq!(
            footprint_in_range(&[coord], near_focus, 1),
            cell_in_stream_range(coord, near_focus, 1)
        );

        let far_focus = near_focus + Vec3::new(500.0, 0.0, 0.0);
        assert_eq!(
            footprint_in_range(&[coord], far_focus, 1),
            cell_in_stream_range(coord, far_focus, 1)
        );
    }

    #[test]
    fn footprint_in_range_covers_a_whole_room_footprint() {
        // A whole-room module's single `HexWfcCell` is keyed by its anchor,
        // which can sit far from the player while another footprint cell is
        // close — the room must still stream in, which is exactly the bug
        // this stream fixes (previously only the anchor coordinate was
        // checked, so a large room could vanish while the player stood in
        // one of its far corners).
        let anchor = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        let near_cell = HexCoord {
            q: 5,
            r: 5,
            level: 0,
        };
        let focus_position = Vec3::from_array(hex_origin(near_cell));

        assert!(!cell_in_stream_range(anchor, focus_position, 0));
        assert!(cell_in_stream_range(near_cell, focus_position, 0));
        assert!(footprint_in_range(&[anchor, near_cell], focus_position, 0));
    }
}
