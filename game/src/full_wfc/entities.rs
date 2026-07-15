//! Stable-domain presentation of actors, objectives, and deployable equipment.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_core::{EquipmentId, PlayerId, RoomId};
use observed_facility::map_spec::RoomRole;
use observed_match::full_wfc::{DeployableKind, cell_origin};
use observed_style::MarkerRole;

use super::sim::{FullWfcRuntime, LOCAL_PLAYER};
use crate::GameState;

#[derive(Component)]
pub(super) struct ActorVisual(PlayerId);

#[derive(Component)]
pub(super) struct GuardianVisual;

type GuardianVisualFilter = (With<GuardianVisual>, Without<ActorVisual>);
type KeystoneVisualFilter = (Without<ActorVisual>, Without<GuardianVisual>);

#[derive(Component)]
pub(super) struct KeystoneVisual(RoomId);

#[derive(Component)]
pub(super) struct DeployableVisual(EquipmentId);

#[derive(Resource)]
pub(super) struct EntityVisualAssets {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    teammate: Handle<StandardMaterial>,
    rival: Handle<StandardMaterial>,
    guardian: Handle<StandardMaterial>,
    objective: Handle<StandardMaterial>,
    control: Handle<StandardMaterial>,
}

pub(super) fn setup(
    mut commands: Commands,
    runtime: Res<FullWfcRuntime>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = EntityVisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sphere: meshes.add(Sphere::new(0.5)),
        cylinder: meshes.add(Cylinder::new(0.75, 0.16)),
        teammate: signal_material(&mut materials, MarkerRole::Teammate),
        rival: signal_material(&mut materials, MarkerRole::Rival),
        guardian: signal_material(&mut materials, MarkerRole::Collapse),
        objective: signal_material(&mut materials, MarkerRole::NextRoom),
        control: signal_material(&mut materials, MarkerRole::Control),
    };
    for player in runtime
        .match_state
        .players
        .values()
        .filter(|player| player.id != LOCAL_PLAYER)
    {
        let material = if player.team == runtime.local().team {
            assets.teammate.clone()
        } else {
            assets.rival.clone()
        };
        commands.spawn((
            ActorVisual(player.id),
            DespawnOnExit(GameState::FullWfc),
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(player.position).with_scale(Vec3::new(0.72, 1.8, 0.72)),
            Name::new(format!("player {} domain visual", player.id.0)),
        ));
    }
    commands.spawn((
        GuardianVisual,
        DespawnOnExit(GameState::FullWfc),
        Mesh3d(assets.sphere.clone()),
        MeshMaterial3d(assets.guardian.clone()),
        Transform::from_translation(runtime.match_state.guardian.position)
            .with_scale(Vec3::new(1.5, 2.8, 1.5)),
        Name::new("Guardian threat visual"),
    ));
    for room in runtime.match_state.facility.rooms.values() {
        let origin = cell_origin(room.coord);
        if room.role == RoomRole::Keystone {
            commands.spawn((
                KeystoneVisual(room.id),
                DespawnOnExit(GameState::FullWfc),
                Mesh3d(assets.cube.clone()),
                MeshMaterial3d(assets.objective.clone()),
                Transform::from_translation(origin + Vec3::new(0.0, 1.15, 0.0))
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                    .with_scale(Vec3::splat(0.72)),
                Name::new(format!("physical keystone {}", room.id.0)),
            ));
        }
        spawn_role_fixture(&mut commands, &assets, room.role, origin);
    }
    commands.insert_resource(assets);
}

pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<FullWfcRuntime>,
    assets: Res<EntityVisualAssets>,
    mut actors: Query<(&ActorVisual, &mut Transform, &mut Visibility)>,
    mut guardian: Query<(&mut Transform, &mut Visibility), GuardianVisualFilter>,
    mut keystones: Query<(&KeystoneVisual, &mut Visibility), KeystoneVisualFilter>,
    deployed: Query<(Entity, &DeployableVisual)>,
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
    if let Ok((mut transform, mut visibility)) = guardian.single_mut() {
        transform.translation = runtime.match_state.guardian.position;
        *visibility = if runtime.match_state.guardian.status
            == observed_match::full_wfc::GuardianStatus::Active
        {
            Visibility::Visible
        } else {
            Visibility::Inherited
        };
    }
    for (visual, mut visibility) in &mut keystones {
        *visibility = if runtime.match_state.available_keystones.contains(&visual.0) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let existing = deployed
        .iter()
        .map(|(entity, visual)| (visual.0, entity))
        .collect::<BTreeMap<_, _>>();
    let live = runtime
        .match_state
        .equipment
        .deployed
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    for (id, entity) in &existing {
        if !live.contains(id) {
            commands.entity(*entity).despawn();
        }
    }
    for item in runtime.match_state.equipment.deployed.values() {
        if existing.contains_key(&item.id) {
            continue;
        }
        let (mesh, material, scale, label) = match item.kind {
            DeployableKind::Anchor => (
                assets.cube.clone(),
                assets.control.clone(),
                Vec3::new(0.55, 1.4, 0.55),
                "threshold anchor",
            ),
            DeployableKind::TeleportPad => (
                assets.cylinder.clone(),
                assets.teammate.clone(),
                Vec3::ONE,
                "team teleport pad",
            ),
        };
        commands.spawn((
            DeployableVisual(item.id),
            DespawnOnExit(GameState::FullWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(item.position + Vec3::Y * 0.1).with_scale(scale),
            Name::new(format!("{label} {}", item.id.0)),
        ));
    }
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
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        metallic: 0.22,
        perceptual_roughness: 0.38,
        ..Default::default()
    })
}

fn spawn_role_fixture(
    commands: &mut Commands,
    assets: &EntityVisualAssets,
    role: RoomRole,
    origin: Vec3,
) {
    let (count, material, scale, y) = match role {
        RoomRole::DualStation => (2, assets.control.clone(), Vec3::new(0.55, 1.55, 0.55), 0.78),
        RoomRole::GuardianControl => (1, assets.guardian.clone(), Vec3::new(1.3, 0.65, 1.3), 0.34),
        RoomRole::AnchorCheckpoint => (1, assets.control.clone(), Vec3::new(1.0, 0.22, 1.0), 0.12),
        RoomRole::TeleportRelay => (
            2,
            assets.teammate.clone(),
            Vec3::new(0.65, 0.12, 0.65),
            0.08,
        ),
        RoomRole::Monitor => (3, assets.teammate.clone(), Vec3::new(0.7, 0.7, 0.12), 1.6),
        _ => return,
    };
    for index in 0..count {
        let x = (index as f32 - (count - 1) as f32 * 0.5) * 2.0;
        commands.spawn((
            DespawnOnExit(GameState::FullWfc),
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(origin + Vec3::new(x, y, 0.0)).with_scale(scale),
            Name::new(format!("{} room mechanism", role.label())),
        ));
    }
}
