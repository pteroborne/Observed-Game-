use crate::items::{ItemKind, PlacedItem};
use crate::screens::{
    DespawnOnExit, DroppedItemVisual, GameState, KeystoneItem, MatchAssets, PlaceGeometry,
    TeleportPadGlow,
};
use bevy::prelude::*;
use observed_core::RoomId;
use observed_style::{self as style, MarkerRole};
use std::f32::consts::PI;

/// A glowing gold keystone pickup at the centre of a room, tagged for proximity pickup.
pub(crate) fn spawn_keystone_item(
    commands: &mut Commands,
    assets: &MatchAssets,
    room: RoomId,
    y_offset: f32,
) {
    commands
        .spawn((
            KeystoneItem(room),
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.objective_material.clone()),
            Transform::from_xyz(0.0, y_offset + 1.1, 0.0)
                .with_rotation(Quat::from_rotation_y(PI * 0.25))
                .with_scale(Vec3::splat(0.5)),
            Name::new("Keystone"),
        ))
        .with_children(|item| {
            item.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.3),
                    intensity: 2_400.0,
                    range: 7.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::default(),
            ));
        });
}

pub(crate) fn spawn_dropped_item(
    commands: &mut Commands,
    assets: &MatchAssets,
    item: PlacedItem,
    y_offset: f32,
) {
    match item.kind {
        ItemKind::AnchorTorch => spawn_anchor_torch(commands, assets, item.pos, y_offset),
        ItemKind::TeleportPad => spawn_teleport_pad(commands, assets, item.pos, y_offset),
    }
}

pub(crate) fn spawn_anchor_torch(
    commands: &mut Commands,
    assets: &MatchAssets,
    pos: Vec2,
    y_offset: f32,
) {
    commands
        .spawn((
            DroppedItemVisual,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.anchor_torch_material.clone()),
            Transform::from_xyz(pos.x, y_offset + 0.55, pos.y)
                .with_scale(Vec3::new(0.18, 1.1, 0.18)),
            Name::new("Anchor torch"),
        ))
        .with_children(|torch| {
            torch.spawn((
                Mesh3d(assets.halo_mesh.clone()),
                MeshMaterial3d(assets.anchor_torch_material.clone()),
                Transform::from_xyz(0.0, -0.52, 0.0).with_scale(Vec3::new(1.3, 1.0, 1.3)),
                Name::new("Anchor torch floor halo"),
            ));
            torch.spawn((
                PointLight {
                    color: style::marker(MarkerRole::Control).base_color,
                    intensity: 1_900.0,
                    range: 6.5,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        });
}

pub(crate) fn spawn_teleport_pad(
    commands: &mut Commands,
    assets: &MatchAssets,
    pos: Vec2,
    y_offset: f32,
) {
    commands
        .spawn((
            DroppedItemVisual,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.halo_mesh.clone()),
            MeshMaterial3d(assets.teleport_pad_material.clone()),
            Transform::from_xyz(pos.x, y_offset + 0.05, pos.y)
                .with_scale(Vec3::new(1.75, 1.0, 1.75)),
            Name::new("Teleport pad"),
        ))
        .with_children(|pad| {
            // Solid dark color base that the glow emanates from
            pad.spawn((
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.wall_material.clone()),
                Transform::from_xyz(0.0, -0.01, 0.0).with_scale(Vec3::new(1.0, 0.04, 1.0)),
                Name::new("Teleport pad dark base"),
            ));
            pad.spawn((
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(assets.teleport_pad_material.clone()),
                Transform::from_xyz(0.0, 0.10, 0.0).with_scale(Vec3::new(0.32, 0.08, 0.32)),
                Name::new("Teleport pad core"),
            ));
            pad.spawn((
                PointLight {
                    color: style::marker(MarkerRole::You).base_color,
                    intensity: 1_700.0,
                    range: 6.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.35, 0.0),
            ));
            // Spawn stargate glow cylinder
            pad.spawn((
                TeleportPadGlow,
                Mesh3d(assets.objective_beam_mesh.clone()),
                MeshMaterial3d(assets.teleport_pad_material.clone()),
                Transform::from_xyz(0.0, 0.33 * 0.5 + 0.05, 0.0)
                    .with_scale(Vec3::new(5.0, 0.33, 5.0)),
                Name::new("Teleport pad stargate glow"),
            ));
        });
}
