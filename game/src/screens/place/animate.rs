//! Per-frame place animation systems: sliding door leaves open near the player, walking
//! the rival avatars along their pace segment, pulsing the teleport pad glow, and
//! attaching/detaching the carried-torch point light to the camera.

use bevy::prelude::*;
use observed_style::{self as style, MarkerRole};

use crate::GameState;
use crate::items::ItemsState;
use crate::rivals;
use crate::sim::director::MatchDirector;
use crate::sim::state::{MatchPaused, TeleportState};
use crate::teleport::Place;
use crate::view::assets::MatchAssets;
use crate::view::components::{DoorLeaf, GameCam, RivalAvatar, TeleportPadGlow};

const DOOR_OPEN_RADIUS: f32 = 4.6;
const DOOR_SLIDE_SPEED: f32 = 6.0;

/// Slide any future openable sealed leaf between shut and tucked-into-the-lintel by the
/// player's proximity.
pub(crate) fn animate_doors(
    time: Res<Time>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
) {
    if paused.0 {
        return;
    }
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    let max_step = DOOR_SLIDE_SPEED * time.delta_secs();
    for (leaf, mut transform) in &mut leaves {
        let target = if leaf.openable && body.distance(leaf.center) <= DOOR_OPEN_RADIUS {
            leaf.open_y
        } else {
            leaf.closed_y
        };
        let y = transform.translation.y;
        transform.translation.y = y + (target - y).clamp(-max_step, max_step);
    }
}

pub(crate) fn sync_rival_avatars(
    time: Res<Time>,
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    assets: Res<MatchAssets>,
    paused: Res<MatchPaused>,
    mut avatars: Query<(Entity, &RivalAvatar, &mut Transform)>,
    mut commands: Commands,
) {
    let game = runtime.live.host_match();
    let present: Vec<usize> = match tp.place {
        Place::Room(room) => rivals::rivals_in_room(&game.competitive, room),
        Place::Hallway { .. } => Vec::new(),
    };

    let (a, b) = rivals::pace_segment(&tp.geom);
    let along = b - a;
    let tangent = Vec2::new(-along.y, along.x).normalize_or_zero();
    let n = present.len();

    let mut have: Vec<usize> = Vec::new();
    for (entity, avatar, mut transform) in &mut avatars {
        let Some(slot) = present.iter().position(|&t| t == avatar.team) else {
            commands.entity(entity).despawn();
            continue;
        };
        have.push(avatar.team);
        if paused.0 {
            continue;
        }
        let phase = avatar.team as f32 * 0.7;
        let theta = time.elapsed_secs() * rivals::RIVAL_PACE_SPEED + phase;
        let u = rivals::triangle_wave(theta);
        let lane = (slot as f32 - (n as f32 - 1.0) * 0.5) * 1.3;
        let foot = a + along * u + tangent * lane;
        let bob = (theta * 6.0).sin() * 0.06;
        transform.translation = Vec3::new(foot.x, 0.82 + bob, foot.y);
    }

    for &team in &present {
        if !have.contains(&team) {
            commands.spawn((
                RivalAvatar { team },
                DespawnOnExit(GameState::Match),
                Mesh3d(assets.rival_body_mesh.clone()),
                MeshMaterial3d(assets.rival_material.clone()),
                Transform::from_xyz(a.x, 0.82, a.y),
                Name::new(format!("Rival team {team}")),
            ));
        }
    }
}

pub(crate) fn animate_teleport_pad_glow(
    time: Res<Time>,
    mut glow_q: Query<
        (
            &mut Transform,
            &GlobalTransform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<TeleportPadGlow>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: Option<Res<TeleportState>>,
) {
    let elapsed = time.elapsed_secs();
    let player_pos = tp.map(|t| t.body.position).unwrap_or(Vec3::ZERO);

    for (mut transform, global_transform, material_handle) in &mut glow_q {
        let pad_pos = global_transform.translation();
        let dist = Vec2::new(pad_pos.x, pad_pos.z).distance(Vec2::new(player_pos.x, player_pos.z));
        let stepped_on = dist < 1.2;

        let scale_y = if stepped_on {
            0.33 + (elapsed * 3.5).sin() * 0.03
        } else {
            0.01
        };
        let scale_xz = if stepped_on {
            5.0 + (elapsed * 2.0).cos() * 0.4
        } else {
            5.0
        };

        transform.scale = Vec3::new(scale_xz, scale_y, scale_xz);
        transform.rotate_local_y(time.delta_secs() * 0.5);
        transform.translation.y = scale_y * 0.5 + 0.05;

        if let Some(mat) = materials.get_mut(material_handle.as_ref()) {
            let pad = style::marker(MarkerRole::You);
            let intensity = if stepped_on {
                0.8 + (elapsed * 3.0).sin() * 0.2
            } else {
                0.05
            };
            let mut col = LinearRgba::from(pad.base_color);
            col.alpha *= intensity;
            mat.base_color = Color::from(col);
        }
    }
}

#[derive(Component)]
pub(crate) struct CarriedTorchLight;

pub(crate) fn update_carried_torch_light(
    items: Option<Res<ItemsState>>,
    camera: Query<Entity, With<GameCam>>,
    mut commands: Commands,
    lights: Query<Entity, With<CarriedTorchLight>>,
) {
    let carrying_torch = items
        .map(|it| it.carried(crate::items::ItemKind::AnchorTorch) > 0)
        .unwrap_or(false);
    let has_light = !lights.is_empty();

    if carrying_torch && !has_light {
        if let Some(cam_ent) = camera.iter().next() {
            commands.entity(cam_ent).with_children(|parent| {
                parent.spawn((
                    CarriedTorchLight,
                    PointLight {
                        color: style::marker(MarkerRole::Control).base_color,
                        intensity: 2_200.0,
                        range: 8.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    Name::new("Carried torch light"),
                ));
            });
        }
    } else if !carrying_torch && has_light {
        for entity in &lights {
            commands.entity(entity).despawn();
        }
    }
}
