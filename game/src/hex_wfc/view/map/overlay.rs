//! Overlay landmarks, compass orientation frame, and player beacon for the survivor map.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_facility::map_spec::RoomRole;
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use observed_match::hex_wfc::HexPlayerMapKnowledge;
use observed_style::MarkerRole;

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

use super::build::{MapAssets, MapCensus};
use super::{
    HexMapLandmark, HexMapOrientationFrame, HexMapPlayerBeacon, HexMapVisual, ISO_PITCH,
    MAP_RENDER_LAYER,
};

pub(super) fn room_marker_role(role: RoomRole) -> MarkerRole {
    match role {
        RoomRole::Exit => MarkerRole::Exit,
        RoomRole::Keystone | RoomRole::AnchorCheckpoint | RoomRole::GuardianControl => {
            MarkerRole::Control
        }
        RoomRole::Start
        | RoomRole::Decision
        | RoomRole::DecoherenceFork
        | RoomRole::DualStation
        | RoomRole::Monitor
        | RoomRole::Recovery
        | RoomRole::TeleportRelay => MarkerRole::NextRoom,
    }
}

pub(super) fn room_label(role: RoomRole) -> &'static str {
    match role {
        RoomRole::Start => "START",
        RoomRole::Decision => "DECIDE",
        RoomRole::DecoherenceFork => "FORK",
        RoomRole::Keystone => "KEY",
        RoomRole::DualStation => "SYNC",
        RoomRole::Monitor => "SURVEY",
        RoomRole::AnchorCheckpoint => "ANCHOR",
        RoomRole::GuardianControl => "GUARD",
        RoomRole::Recovery => "RECOVER",
        RoomRole::Exit => "EXIT",
        RoomRole::TeleportRelay => "RELAY",
    }
}

pub(super) fn rooms_present(
    commands: &mut Commands,
    world: &HexWfcWorld,
    knowledge: &HexPlayerMapKnowledge,
    assets: &mut MapAssets,
    census: &mut MapCensus,
) {
    let camera_rot = Quat::from_euler(EulerRot::YXZ, std::f32::consts::FRAC_PI_4, ISO_PITCH, 0.0);
    for blueprint in &world.blueprints {
        let known_cell = blueprint
            .cells
            .iter()
            .filter_map(|cell| {
                knowledge
                    .cells
                    .get(cell)
                    .filter(|known| known.room_role == Some(blueprint.role))
                    .map(|_| *cell)
            })
            .min();
        let Some(cell) = known_cell else { continue };
        census.rooms.insert(blueprint.role.label().to_string());
        let marker = room_marker_role(blueprint.role);
        let pillar_mesh = assets.bar(1.0, 4.5, 1.0);
        let material = assets.signal(marker);
        let origin = Vec3::from_array(hex_origin(cell));
        commands.spawn((
            HexMapVisual,
            HexMapLandmark,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(pillar_mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(origin + Vec3::Y * 3.0),
            Name::new(format!(
                "Hex map landmark {} pillar",
                blueprint.role.label()
            )),
        ));

        commands.spawn((
            HexMapVisual,
            HexMapLandmark,
            DespawnOnExit(GameState::HexWfc),
            Text2d::new(room_label(blueprint.role)),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(observed_style::marker(marker).base_color),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(origin + Vec3::Y * 6.5).with_rotation(camera_rot),
            Name::new(format!("known {} room label", blueprint.role.label())),
        ));
    }
}

pub(super) fn spawn_player_marker(
    commands: &mut Commands,
    runtime: &HexWfcRuntime,
    cell_height: f32,
    assets: &mut MapAssets,
    census: &mut MapCensus,
) {
    let local = runtime.local();
    if local.escaped {
        return;
    }
    let origin = Vec3::from_array(hex_origin(local.cell));
    let base_y = cell_height;
    let material = assets.signal(MarkerRole::You);
    let camera_rot = Quat::from_euler(EulerRot::YXZ, std::f32::consts::FRAC_PI_4, ISO_PITCH, 0.0);
    let facing_rot = Quat::from_rotation_y(-local.yaw);

    // 1. Player Beacon Pin (elevated vertical pillar, 6m tall)
    let pin_height = 6.0;
    let pin_mesh = assets.bar(0.8, pin_height, 0.8);
    commands.spawn((
        HexMapVisual,
        HexMapPlayerBeacon,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(pin_mesh),
        MeshMaterial3d(material.clone()),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(origin + Vec3::Y * (base_y + pin_height * 0.5)),
        Name::new("Hex map player beacon"),
    ));

    // 2. Heading Pointer: central arrow shaft + angled chevron barbs
    let shaft_len = 5.5;
    let shaft_mesh = assets.bar(0.8, 0.5, shaft_len);
    let forward_dir = facing_rot * (-Vec3::Z);
    let shaft_pos = origin + Vec3::Y * (base_y + 0.3) + forward_dir * (shaft_len * 0.5);
    commands.spawn((
        HexMapVisual,
        HexMapPlayerBeacon,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(shaft_mesh),
        MeshMaterial3d(material.clone()),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(shaft_pos).with_rotation(facing_rot),
        Name::new("Hex map player heading shaft"),
    ));

    // Left and right chevron barbs at the tip to form a clear arrowhead
    let barb_len = 2.4;
    let barb_mesh = assets.bar(0.6, 0.4, barb_len);
    let tip_pos = origin + Vec3::Y * (base_y + 0.3) + forward_dir * shaft_len;

    let left_rot = facing_rot * Quat::from_rotation_y(std::f32::consts::FRAC_PI_4 * 0.8);
    let left_dir = left_rot * Vec3::Z;
    commands.spawn((
        HexMapVisual,
        HexMapPlayerBeacon,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(barb_mesh.clone()),
        MeshMaterial3d(material.clone()),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(tip_pos + left_dir * (barb_len * 0.5)).with_rotation(left_rot),
        Name::new("Hex map player heading barb left"),
    ));

    let right_rot = facing_rot * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4 * 0.8);
    let right_dir = right_rot * Vec3::Z;
    commands.spawn((
        HexMapVisual,
        HexMapPlayerBeacon,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(barb_mesh),
        MeshMaterial3d(material),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(tip_pos + right_dir * (barb_len * 0.5))
            .with_rotation(right_rot),
        Name::new("Hex map player heading barb right"),
    ));

    // 3. Floating "YOU" Text Badge at the top of the beacon pin
    commands.spawn((
        HexMapVisual,
        HexMapPlayerBeacon,
        DespawnOnExit(GameState::HexWfc),
        Text2d::new("YOU"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(observed_style::marker(MarkerRole::You).base_color),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(origin + Vec3::Y * (base_y + pin_height + 1.2))
            .with_rotation(camera_rot),
        Name::new("Hex map player you label"),
    ));

    census.see(
        origin + Vec3::Y * (base_y + pin_height + 2.0),
        Vec3::new(6.0, pin_height, 6.0),
    );
}

pub(super) fn spawn_orientation_frame(
    commands: &mut Commands,
    census: &mut MapCensus,
    focus_level: u8,
    assets: &mut MapAssets,
) {
    let Some((min, _)) = census.bounds else {
        return;
    };
    let floor_y = f32::from(focus_level) * TILE_LEVEL_HEIGHT;
    let compass_centre = Vec3::new(min.x - 16.0, floor_y, min.z - 16.0);
    let camera_rot = Quat::from_euler(EulerRot::YXZ, std::f32::consts::FRAC_PI_4, ISO_PITCH, 0.0);
    let arm_mat = assets.signal(MarkerRole::NextRoom);

    // Central hub
    let hub_mesh = assets.bar(1.6, 0.4, 1.6);
    commands.spawn((
        HexMapVisual,
        HexMapOrientationFrame,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(hub_mesh),
        MeshMaterial3d(arm_mat.clone()),
        RenderLayers::layer(MAP_RENDER_LAYER),
        Transform::from_translation(compass_centre),
        Name::new("Hex map compass hub"),
    ));

    // North: -Z (screen: top-left)
    // East:  +X (screen: top-right)
    // South: +Z (screen: down-right)
    // West:  -X (screen: down-left)
    let arm_defs = [
        (Vec3::new(0.0, 0.0, -1.0), 9.0, "N", true),
        (Vec3::new(1.0, 0.0, 0.0), 6.5, "E", false),
        (Vec3::new(0.0, 0.0, 1.0), 6.5, "S", false),
        (Vec3::new(-1.0, 0.0, 0.0), 6.5, "W", false),
    ];

    for (dir, length, label, primary) in arm_defs {
        let width = if primary { 0.9 } else { 0.6 };
        let arm_mesh = if dir.x.abs() > 0.5 {
            assets.bar(length, 0.35, width)
        } else {
            assets.bar(width, 0.35, length)
        };
        let arm_pos = compass_centre + dir * (length * 0.5);
        commands.spawn((
            HexMapVisual,
            HexMapOrientationFrame,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(arm_mesh),
            MeshMaterial3d(arm_mat.clone()),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(arm_pos),
            Name::new(format!("Hex map compass arm {label}")),
        ));

        let text_color = observed_style::marker(MarkerRole::NextRoom).base_color;
        let label_pos = compass_centre + dir * (length + 2.0) + Vec3::Y * 0.8;
        commands.spawn((
            HexMapVisual,
            HexMapOrientationFrame,
            DespawnOnExit(GameState::HexWfc),
            Text2d::new(label),
            TextFont {
                font_size: FontSize::Px(if primary { 22.0 } else { 18.0 }),
                ..default()
            },
            TextColor(text_color),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(label_pos).with_rotation(camera_rot),
            Name::new(format!("Hex map compass label {label}")),
        ));
    }

    census.see(compass_centre, Vec3::new(14.0, 2.0, 14.0));
}
