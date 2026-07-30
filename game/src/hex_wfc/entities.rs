//! Stable-domain presentation of the rival runners and the exit beacon.

use bevy::prelude::*;
use observed_authoring::RoomSocketKind;
use observed_core::PlayerId;
use observed_hex::hex_origin;
use observed_style::{MarkerRole, OutlineRole};

use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct ActorVisual(PlayerId);

#[derive(Component)]
pub(super) struct ObjectiveVisual {
    room_generation_key: u64,
    kind: RoomSocketKind,
}

#[derive(Component)]
pub(super) struct ObjectiveLabel;

#[derive(Resource)]
pub(super) struct EntityVisualAssets {
    runner: Handle<Mesh>,
    beacon: Handle<Mesh>,
    pickup: Handle<Mesh>,
    console: Handle<Mesh>,
    local: Handle<StandardMaterial>,
    teammate: Handle<StandardMaterial>,
    rival: Handle<StandardMaterial>,
    exit: Handle<StandardMaterial>,
    pickup_material: Handle<StandardMaterial>,
    interactable_material: Handle<StandardMaterial>,
}

type ObjectiveVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ObjectiveVisual,
        &'static mut Visibility,
        Option<&'static mut Transform>,
        Option<&'static ObjectiveLabel>,
        Option<&'static mut Text2d>,
    ),
    Without<ActorVisual>,
>;

pub(super) fn setup(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    capture: Option<Res<super::HexWfcCapture>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = EntityVisualAssets {
        // Rival silhouettes are deliberately compact: the physical bodies
        // begin close together, and a full-height opaque capsule at arm's
        // length obscures the architecture players must read.
        runner: meshes.add(Capsule3d::new(0.25, 0.8)),
        beacon: meshes.add(Cuboid::new(1.1, 3.6, 1.1)),
        pickup: meshes.add(Sphere::new(0.55)),
        console: meshes.add(Cuboid::new(0.8, 1.2, 0.55)),
        local: signal_material(&mut materials, MarkerRole::You),
        teammate: signal_material(&mut materials, MarkerRole::Teammate),
        rival: signal_material(&mut materials, MarkerRole::Rival),
        exit: signal_material(&mut materials, MarkerRole::Exit),
        pickup_material: outline_material(&mut materials, OutlineRole::Pickup),
        interactable_material: outline_material(&mut materials, OutlineRole::Interactable),
    };
    let show_local = capture
        .as_deref()
        .is_some_and(|capture| capture.mode == super::HexWfcCaptureMode::Traversal);
    for player in runtime
        .match_state
        .players
        .values()
        .filter(|player| show_local || player.id != runtime.local_player)
    {
        let local_team = runtime.local().team;
        let material = if player.id == runtime.local_player {
            assets.local.clone()
        } else if player.team == local_team {
            assets.teammate.clone()
        } else {
            assets.rival.clone()
        };
        commands.spawn((
            ActorVisual(player.id),
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(assets.runner.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(player.position).with_scale(Vec3::new(0.8, 1.0, 0.8)),
            Name::new(format!("runner {} domain visual", player.id.0)),
        ));
    }
    let exit_origin = Vec3::from_array(hex_origin(runtime.match_state.facility.config.exit()));
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(assets.beacon.clone()),
        MeshMaterial3d(assets.exit.clone()),
        Transform::from_translation(exit_origin + Vec3::Y * 1.8),
        Name::new("hex exit beacon"),
    ));
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        PointLight {
            color: observed_style::marker(MarkerRole::Exit).base_color,
            intensity: 2_200.0,
            range: 18.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(exit_origin + Vec3::Y * 2.5),
        Name::new("hex exit beacon light"),
    ));
    for socket in &runtime.match_state.geometry.sockets {
        let (mesh, material) = match socket.kind {
            RoomSocketKind::Keystone => (assets.pickup.clone(), assets.pickup_material.clone()),
            RoomSocketKind::StationA
            | RoomSocketKind::StationB
            | RoomSocketKind::Monitor
            | RoomSocketKind::GuardianControl
            | RoomSocketKind::Recovery
            | RoomSocketKind::LanternCache => {
                (assets.console.clone(), assets.interactable_material.clone())
            }
            RoomSocketKind::Exit => {
                commands.spawn((
                    ObjectiveVisual {
                        room_generation_key: socket.room_generation_key,
                        kind: socket.kind,
                    },
                    ObjectiveLabel,
                    DespawnOnExit(GameState::HexWfc),
                    Text2d::new(socket_glyph(socket.kind)),
                    TextFont {
                        font_size: 40.0,
                        ..default()
                    },
                    TextColor(observed_style::outline(OutlineRole::ObjectiveBeacon).color),
                    Transform::from_translation(socket.position + Vec3::Y * 4.2),
                    Name::new("EXIT mechanism label"),
                ));
                continue;
            }
        };
        let visual = ObjectiveVisual {
            room_generation_key: socket.room_generation_key,
            kind: socket.kind,
        };
        commands.spawn((
            visual,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(socket.position + Vec3::Y * 0.55)
                .with_rotation(Quat::from_rotation_y(socket.yaw_degrees.to_radians())),
            Name::new(format!("{} mechanism", socket_glyph(socket.kind))),
        ));
        commands.spawn((
            ObjectiveVisual {
                room_generation_key: socket.room_generation_key,
                kind: socket.kind,
            },
            ObjectiveLabel,
            DespawnOnExit(GameState::HexWfc),
            Text2d::new(socket_glyph(socket.kind)),
            TextFont {
                font_size: 34.0,
                ..default()
            },
            TextColor(observed_style::outline(outline_role(socket.kind)).color),
            Transform::from_translation(socket.position + Vec3::Y * 1.75),
            Name::new(format!("{} mechanism label", socket_glyph(socket.kind))),
        ));
    }
    commands.insert_resource(assets);
}

pub(super) fn sync(
    runtime: Res<HexWfcRuntime>,
    mut actors: Query<(&ActorVisual, &mut Transform, &mut Visibility)>,
    camera: Query<&GlobalTransform, With<crate::view::components::GameCam>>,
    mut objectives: ObjectiveVisualQuery,
) {
    for (visual, mut transform, mut visibility) in &mut actors {
        let player = &runtime.match_state.players[&visual.0];
        transform.translation = player.position;
        *visibility = if player.escaped {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
    let camera_rotation = camera.single().ok().map(GlobalTransform::rotation);
    let local_objectives = runtime.match_state.teams[&runtime.local().team].objectives;
    for (visual, mut visibility, transform, label, text) in &mut objectives {
        let visible = visual.kind != RoomSocketKind::Keystone
            || runtime
                .match_state
                .objectives
                .available_keystones
                .contains(&visual.room_generation_key);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let (Some(rotation), Some(mut transform), Some(_)) = (camera_rotation, transform, label)
        {
            transform.rotation = rotation;
        }
        if let (Some(mut text), Some(_)) = (text, label) {
            **text = match visual.kind {
                RoomSocketKind::StationA | RoomSocketKind::StationB
                    if local_objectives.dual_station_complete =>
                {
                    "SYNC READY".to_string()
                }
                RoomSocketKind::StationA | RoomSocketKind::StationB
                    if local_objectives.dual_station_room == Some(visual.room_generation_key)
                        && local_objectives.dual_station_ticks > 0 =>
                {
                    let progress = u32::from(local_objectives.dual_station_ticks) * 100
                        / u32::from(observed_match::hex_wfc::DUAL_STATION_HOLD_TICKS);
                    format!("{} {progress}%", socket_glyph(visual.kind))
                }
                RoomSocketKind::Monitor
                    if runtime
                        .match_state
                        .objectives
                        .surveyed_monitors
                        .contains(&(runtime.local().team, visual.room_generation_key)) =>
                {
                    "SURVEYED".to_string()
                }
                _ => socket_glyph(visual.kind).to_string(),
            };
        }
    }
}

fn socket_glyph(kind: RoomSocketKind) -> &'static str {
    match kind {
        RoomSocketKind::Keystone => "KEY",
        RoomSocketKind::StationA => "SYNC A",
        RoomSocketKind::StationB => "SYNC B",
        RoomSocketKind::Monitor => "SURVEY",
        RoomSocketKind::LanternCache => "ANCHOR",
        RoomSocketKind::GuardianControl => "GUARD",
        RoomSocketKind::Recovery => "RECOVER",
        RoomSocketKind::Exit => "EXIT",
    }
}

fn outline_role(kind: RoomSocketKind) -> OutlineRole {
    match kind {
        RoomSocketKind::Keystone => OutlineRole::Pickup,
        RoomSocketKind::Exit => OutlineRole::ObjectiveBeacon,
        _ => OutlineRole::Interactable,
    }
}

fn outline_material(
    materials: &mut Assets<StandardMaterial>,
    role: OutlineRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::outline(role);
    materials.add(StandardMaterial {
        base_color: treatment.color,
        emissive: treatment.color.to_linear() * 4.0,
        metallic: 0.18,
        perceptual_roughness: 0.3,
        ..default()
    })
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<EntityVisualAssets>();
}

fn signal_material(
    materials: &mut Assets<StandardMaterial>,
    role: MarkerRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        // Close-range rivals remain unmistakably style-owned signals without
        // turning into opaque bloom cards over thresholds and ramps.
        base_color: treatment.base_color.with_alpha(0.46),
        emissive: treatment.emissive * 0.24,
        alpha_mode: AlphaMode::Blend,
        metallic: 0.22,
        perceptual_roughness: 0.38,
        ..Default::default()
    })
}
