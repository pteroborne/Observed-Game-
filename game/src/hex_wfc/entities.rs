//! Stable-domain presentation of the rival runners and the exit beacon.

use bevy::prelude::*;
use observed_core::PlayerId;
use observed_hex::hex_origin;
use observed_style::MarkerRole;

use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct ActorVisual(PlayerId);

#[derive(Resource)]
pub(super) struct EntityVisualAssets {
    runner: Handle<Mesh>,
    beacon: Handle<Mesh>,
    local: Handle<StandardMaterial>,
    rival: Handle<StandardMaterial>,
    exit: Handle<StandardMaterial>,
}

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
        local: signal_material(&mut materials, MarkerRole::You),
        rival: signal_material(&mut materials, MarkerRole::Rival),
        exit: signal_material(&mut materials, MarkerRole::Exit),
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
        let material = if player.id == runtime.local_player {
            assets.local.clone()
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
    commands.insert_resource(assets);
}

pub(super) fn sync(
    runtime: Res<HexWfcRuntime>,
    mut actors: Query<(&ActorVisual, &mut Transform, &mut Visibility)>,
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
