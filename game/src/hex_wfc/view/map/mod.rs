//! The full-screen isometric survivor map (Tab).
//!
//! Promoted from `iso_observer_lab`, which is where the projection was proven.
//! The lab renders ground truth because its job is to audit what the WFC
//! composed; this one renders **only what the team has discovered**, because its
//! job is to be a survivor's sketch.
//!
//! A tile is a pixel. On its own it means very little, so the map is built to
//! show what the tiles *compose*, on four channels that do not compete:
//!
//! | channel | carries |
//! |---|---|
//! | colour | the district register |
//! | height | the archetype |
//! | footprint width | room / hallway / vertical — rooms fill their hex and meet with no seam, corridors are ribbons |
//! | link bars | connectivity: one bar per port pair the survivor has seen from both sides |
//! | cap plate | a cell the survivor is holding: an anchor, or a teammate standing on it |
//! | literal label | an entered or locally surveyed room's function |
//!
//! Signal-tier cells (you, the exit) are recoloured outright so they punch
//! through the district palette the way the Legibility Contract requires.

mod build;
mod cell;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_match::hex_wfc::HexMapDiscovery;
use observed_style::MarkerRole;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

use self::build::MapCensus;

/// The map draws on its own layer so the facility never bleeds into it and it
/// never bleeds into the facility. Layer 1 belongs to the portal previews.
pub(super) const MAP_RENDER_LAYER: usize = 2;
/// Drawn after the world camera, with an opaque clear, so it covers the frame.
const MAP_CAMERA_ORDER: isize = 1;
const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 900.0;
/// True isometric pitch: `atan(1 / sqrt(2))`.
const ISO_PITCH: f32 = -0.615_479_7;

/// Every entity the map spawns, for despawn and leak checks.
#[derive(Component)]
pub(crate) struct HexMapVisual;

/// Only the per-hex solids. Links ride at midpoints between cells and caps ride
/// above them, so a gate that asks "does every drawn thing stand over a hex the
/// team knows" has to be able to ask about cells specifically.
#[derive(Component)]
pub(crate) struct HexMapCell;

#[derive(Component)]
pub(crate) struct HexMapCamera;

#[derive(Component)]
pub(crate) struct HexMapLegend;

/// Change-detection cache, so the map rebuilds on a real change rather than
/// every frame — the same contract the corner sketch held.
#[derive(Resource, Default)]
pub(crate) struct HexMapProjection {
    signature: u64,
    built: bool,
}

pub(in crate::hex_wfc) fn setup(mut commands: Commands) {
    commands.insert_resource(HexMapProjection::default());
    let camera = commands
        .spawn((
            HexMapCamera,
            DespawnOnExit(GameState::HexWfc),
            Camera3d::default(),
            Camera {
                order: MAP_CAMERA_ORDER,
                is_active: false,
                clear_color: Color::srgb(0.008, 0.010, 0.020).into(),
                ..default()
            },
            Projection::Orthographic(OrthographicProjection {
                scale: 1.0,
                ..OrthographicProjection::default_3d()
            }),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::default(),
            Name::new("Hex survivor map camera"),
        ))
        .id();
    // The map carries its own key. Borrowing the world's rig would make the
    // survivor's sketch change brightness with whatever district the runner
    // happens to be standing in, and would leave it unlit wherever the world's
    // lights are not on this render layer.
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        DirectionalLight {
            illuminance: 3_200.0,
            shadows_enabled: false,
            ..default()
        },
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.95, 0.0)),
        Name::new("Hex survivor map key"),
    ));
    commands.spawn((
        HexMapLegend,
        // The legend belongs to the map's own camera, which draws over the world with its
        // own clear colour. On the default UI camera it would be wiped by that clear.
        UiTargetCamera(camera),
        DespawnOnExit(GameState::HexWfc),
        Visibility::Hidden,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(observed_style::marker(MarkerRole::You).base_color),
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(18),
            ..default()
        },
        GlobalZIndex(60),
    ));
}

pub(in crate::hex_wfc) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HexMapProjection>();
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    mut projection: ResMut<HexMapProjection>,
    mut camera: Query<(&mut Camera, &mut Projection, &mut Transform), With<HexMapCamera>>,
    mut legend: Query<(&mut Visibility, &mut Text), With<HexMapLegend>>,
    existing: Query<Entity, With<HexMapVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let open = runtime.map_open;
    if let Ok((mut camera, _, _)) = camera.single_mut() {
        camera.is_active = open;
    }
    if let Ok((mut visibility, _)) = legend.single_mut() {
        *visibility = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if !open {
        // Keep the built geometry: reopening the map is a frequent, cheap action
        // and a survivor should not pay a full rebuild for a glance.
        return;
    }
    let signature = signature(&runtime);
    if projection.built && projection.signature == signature {
        return;
    }
    projection.signature = signature;
    projection.built = true;

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let census = build::build(&mut commands, &runtime, &mut meshes, &mut materials);

    if let (Some((min, max)), Ok((_, mut projection, mut transform))) =
        (census.bounds, camera.single_mut())
    {
        let (framed, scale, far) = frame_map(min, max);
        *transform = framed;
        *projection = Projection::Orthographic(OrthographicProjection {
            scale,
            near: 0.1,
            // The 3D default far plane is 1000 m, which a production facility's
            // diagonal exceeds on its own — leave it and the map is clipped.
            far,
            ..OrthographicProjection::default_3d()
        });
    }

    if let Ok((_, mut text)) = legend.single_mut() {
        **text = legend_text(&census, runtime.map_level);
    }
}

/// Frame the discovered facility in an orthographic isometric view, returning
/// (transform, world-units-per-pixel, far plane).
#[must_use]
pub(super) fn frame_map(min: Vec3, max: Vec3) -> (Transform, f32, f32) {
    let rotation = Quat::from_euler(EulerRot::YXZ, std::f32::consts::FRAC_PI_4, ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        extent = extent.max((inverse * (corner - centre)).truncate().abs());
    }
    let scale = (extent.x * 2.0 / WINDOW_WIDTH)
        .max(extent.y * 2.0 / WINDOW_HEIGHT)
        .max(f32::MIN_POSITIVE)
        * 1.12;
    let diagonal = (max - min).length().max(1.0);
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * diagonal).with_rotation(rotation);
    (transform, scale, diagonal * 2.0)
}

/// Every channel on screen gets a named line. Atmosphere never carries meaning
/// alone, so the counts are here in words as well as in the geometry.
fn legend_text(census: &MapCensus, focus: u8) -> String {
    let known = census.traversed + census.glimpsed + census.stale;
    let floors = census
        .floors
        .iter()
        .map(|level| {
            if *level == focus {
                format!("[{level}]")
            } else {
                format!("{level}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let rooms = if census.rooms.is_empty() {
        "none yet".to_string()
    } else {
        census.rooms.iter().cloned().collect::<Vec<_>>().join(", ")
    };
    format!(
        "SURVIVOR MAP   floor {focus}   {known} cells known   floors {floors}\n\
         what you know    {} traversed | {} glimpsed | {} stale\n\
         what it composes  {} room | {} hallway | {} vertical      rooms: {rooms}\n\
         how it connects   {} lateral | {} vertical links seen from both sides\n\
         what can change   {} hallway cells rewire | {} permanent | {} held right now\n\
         rooms and vertical links are permanent; a hallway rewires unless held\n\
         colour = district   width = room/hallway   height = archetype   capped = held by you\n\
         text = entered or locally surveyed room function\n\
         PageUp/PageDown change floor    Tab close",
        census.traversed,
        census.glimpsed,
        census.stale,
        census.room_cells,
        census.hall_cells,
        census.vertical_cells,
        census.lateral_links,
        census.vertical_links,
        census.mutable,
        census.permanent,
        census.held,
    )
}

/// Rebuild only when something a survivor would see has actually changed.
fn signature(runtime: &HexWfcRuntime) -> u64 {
    let mut hasher = DefaultHasher::new();
    runtime.map_level.hash(&mut hasher);
    let local = runtime.local().cell;
    (local.q, local.r, local.level).hash(&mut hasher);
    // Teammate positions drive the "held" cap, so they belong in the signature.
    let team = runtime.local().team;
    for player in runtime.match_state.players.values() {
        if player.team == team {
            (player.cell.q, player.cell.r, player.cell.level).hash(&mut hasher);
        }
    }
    if let Some(knowledge) = runtime.match_state.player_map(runtime.local_player) {
        knowledge.cells.len().hash(&mut hasher);
        let world = &runtime.match_state.facility;
        for (&cell, known) in &knowledge.cells {
            (cell.q, cell.r, cell.level).hash(&mut hasher);
            known.anchored.hash(&mut hasher);
            known.known_ports.0.hash(&mut hasher);
            (known.discovery == HexMapDiscovery::Traversed).hash(&mut hasher);
            known.room_role.map(|role| role.label()).hash(&mut hasher);
            known.is_stale(world, cell).hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_framing_fits_the_discovered_bounds_inside_the_viewport() {
        let min = Vec3::new(-140.0, 0.0, -120.0);
        let max = Vec3::new(260.0, 80.0, 240.0);
        let (transform, scale, far) = frame_map(min, max);
        let inverse = transform.rotation.inverse();
        let centre = (min + max) * 0.5;
        for i in 0..8u8 {
            let corner = Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            );
            let projected = (inverse * (corner - centre)).truncate();
            assert!(projected.x.abs() <= scale * WINDOW_WIDTH * 0.5 + 1e-3);
            assert!(projected.y.abs() <= scale * WINDOW_HEIGHT * 0.5 + 1e-3);
        }
        assert!(
            far > (max - min).length(),
            "the far plane must clear the facility diagonal"
        );
    }

    #[test]
    fn the_legend_names_every_channel_it_draws() {
        let census = MapCensus {
            traversed: 4,
            glimpsed: 9,
            room_cells: 3,
            hall_cells: 8,
            vertical_cells: 2,
            lateral_links: 11,
            vertical_links: 1,
            permanent: 5,
            mutable: 8,
            floors: [0u8, 1].into_iter().collect(),
            rooms: ["decision".to_string()].into_iter().collect(),
            ..MapCensus::default()
        };
        let text = legend_text(&census, 1);
        // Atmosphere never carries meaning alone: every geometric channel has a
        // named counterpart in the legend.
        for channel in [
            "colour = district",
            "width = room/hallway",
            "height = archetype",
            "capped = held by you",
        ] {
            assert!(text.contains(channel), "legend must document {channel}");
        }
        assert!(text.contains("decision"), "known rooms are named");
        assert!(text.contains("[1]"), "the focus floor is marked");
        assert!(text.contains("11 lateral"), "connections are counted");
    }
}
