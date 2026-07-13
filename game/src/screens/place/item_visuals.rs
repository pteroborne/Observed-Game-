use crate::GameState;
use crate::items::{ItemKind, PlacedItem};
use crate::view::assets::MatchAssets;
use crate::view::components::{DroppedItemVisual, KeystoneItem, PlaceGeometry, TeleportPadGlow};
use bevy::prelude::*;
use observed_core::RoomId;
use observed_style::{self as style, MarkerRole};
use std::f32::consts::PI;

use crate::items::ANCHOR_RADIUS;

/// A glowing gold keystone pickup at the centre of a room, tagged for proximity pickup.
#[allow(clippy::manual_is_multiple_of)]
pub(crate) fn spawn_keystone_item(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    room: RoomId,
    y_offset: f32,
) {
    let sprite_image = if room.0 % 2 == 0 {
        assets.keystone_card_sprite(images)
    } else {
        assets.keystone_core_sprite(images)
    };

    if let Some(image) = sprite_image {
        commands
            .spawn((
                KeystoneItem(room),
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                crate::view::sprites::sprite3d_components_with_pivot(
                    image,
                    &style::marker(MarkerRole::NextRoom),
                    crate::view::sprites::DEVICE_PIXELS_PER_METRE,
                    Vec2::new(0.5, 0.5),
                ),
                Transform::from_xyz(0.0, y_offset + 1.1, 0.0),
                Name::new("Keystone sprite"),
            ))
            .with_children(|item| {
                item.spawn((
                    Mesh3d(assets.halo_mesh.clone()),
                    MeshMaterial3d(assets.objective_material.clone()),
                    Transform::from_xyz(0.0, -1.1, 0.0).with_scale(Vec3::new(1.3, 1.0, 1.3)),
                    Name::new("Keystone floor halo"),
                ));
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
        return;
    }

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
    images: &Assets<Image>,
    item: PlacedItem,
    y_offset: f32,
) {
    match item.kind {
        ItemKind::AnchorTorch => spawn_anchor_torch(commands, assets, images, item.pos, y_offset),
        ItemKind::TeleportPad => spawn_teleport_pad(commands, assets, images, item.pos, y_offset),
    }
}

pub(crate) fn spawn_anchor_torch(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    pos: Vec2,
    y_offset: f32,
) {
    let sprite_image = assets
        .anchor_torch_sprite(images)
        .or_else(|| assets.control_device_sprite(images));

    if let Some(image) = sprite_image {
        commands
            .spawn((
                DroppedItemVisual,
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                crate::view::sprites::sprite3d_components(
                    image,
                    &style::marker(MarkerRole::Control),
                    crate::view::sprites::DEVICE_PIXELS_PER_METRE,
                ),
                Transform::from_xyz(pos.x, y_offset + 0.03, pos.y),
                Name::new("Anchor torch sprite"),
            ))
            .with_children(|torch| {
                torch.spawn((
                    Mesh3d(assets.radius_ring_mesh.clone()),
                    MeshMaterial3d(assets.anchor_torch_material.clone()),
                    Transform::from_xyz(0.0, -0.02, 0.0).with_scale(Vec3::splat(ANCHOR_RADIUS)),
                    Name::new("Anchor torch influence radius"),
                ));
                torch.spawn((
                    PointLight {
                        color: style::marker(MarkerRole::Control).base_color,
                        intensity: 1_900.0,
                        range: ANCHOR_RADIUS,
                        shadows_enabled: false,
                        ..default()
                    },
                    Name::new("Anchor torch light"),
                    Transform::from_xyz(0.0, 0.45, 0.0),
                ));
            });
        return;
    }

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
                Mesh3d(assets.radius_ring_mesh.clone()),
                MeshMaterial3d(assets.anchor_torch_material.clone()),
                Transform::from_xyz(0.0, -0.52, 0.0).with_scale(Vec3::splat(ANCHOR_RADIUS)),
                Name::new("Anchor torch influence radius"),
            ));
            torch.spawn((
                PointLight {
                    color: style::marker(MarkerRole::Control).base_color,
                    intensity: 1_900.0,
                    range: ANCHOR_RADIUS,
                    shadows_enabled: false,
                    ..default()
                },
                Name::new("Anchor torch light"),
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        });
}

pub(crate) fn spawn_teleport_pad(
    commands: &mut Commands,
    assets: &MatchAssets,
    images: &Assets<Image>,
    pos: Vec2,
    y_offset: f32,
) {
    let sprite_image = assets
        .relay_device_sprite(images)
        .or_else(|| assets.route_cell_sprite(images))
        .or_else(|| assets.control_device_sprite(images));

    if let Some(image) = sprite_image {
        commands
            .spawn((
                DroppedItemVisual,
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                crate::view::sprites::sprite3d_components_with_pivot(
                    image,
                    &style::marker(MarkerRole::You),
                    crate::view::sprites::DEVICE_PIXELS_PER_METRE,
                    Vec2::new(0.5, 0.5),
                ),
                Transform::from_xyz(pos.x, y_offset + 0.45, pos.y),
                Name::new("Teleport pad sprite"),
            ))
            .with_children(|pad| {
                pad.spawn((
                    Mesh3d(assets.halo_mesh.clone()),
                    MeshMaterial3d(assets.teleport_pad_material.clone()),
                    Transform::from_xyz(0.0, -0.4, 0.0).with_scale(Vec3::new(1.75, 1.0, 1.75)),
                    Name::new("Teleport pad halo"),
                ));
                pad.spawn((
                    Mesh3d(assets.placeholder_mesh.clone()),
                    MeshMaterial3d(assets.wall_material.clone()),
                    Transform::from_xyz(0.0, -0.44, 0.0).with_scale(Vec3::new(1.0, 0.04, 1.0)),
                    Name::new("Teleport pad dark base"),
                ));
                pad.spawn((
                    PointLight {
                        color: style::marker(MarkerRole::You).base_color,
                        intensity: 1_700.0,
                        range: 6.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, -0.1, 0.0),
                ));
                pad.spawn((
                    TeleportPadGlow,
                    Mesh3d(assets.objective_beam_mesh.clone()),
                    MeshMaterial3d(assets.teleport_pad_material.clone()),
                    Transform::from_xyz(0.0, 0.33 * 0.5 - 0.4, 0.0)
                        .with_scale(Vec3::new(5.0, 0.33, 5.0)),
                    Name::new("Teleport pad stargate glow"),
                ));
            });
        return;
    }

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
