//! Presentation for teleport plates.
//!
//! The plate is the tool's entire user interface. There is no counter, no
//! prompt, and no icon: you know you are carrying one because you can see it in
//! your hand, you know where you dropped one because it is lying on the floor,
//! and you know the link is live because a paired plate is lit and a lone one is
//! not. That reading has to survive with the HUD off, since HUD-off is the
//! shipped default.
//!
//! Geometry follows the drop-in convention — [`observed_assets::PAD`] if an
//! author has supplied one, a procedural disc otherwise — and every material
//! comes from `observed_style`, so a plate is coloured by the same semantics
//! that colour every other marker.
//!
//! # Why no light source
//!
//! The lantern earns a `PointLight` because carrying it is a lighting decision.
//! A plate does not: it is scenery you pass, there can be four of them down at
//! once, and per-pad shadow-casting lights are exactly the kind of cost the
//! Steam Deck cannot absorb. Emissive material carries the read instead.

use bevy::{gltf::GltfAssetLabel, prelude::*};
use observed_core::{EquipmentId, PlayerId};
use observed_style::MarkerRole;

use super::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::GameState;

#[derive(Component)]
pub(super) enum PadVisual {
    Held(PlayerId),
    Deployed(EquipmentId),
}

#[derive(Resource)]
pub(super) struct PadVisualAssets {
    authored: Option<Handle<Scene>>,
    plate: Handle<Mesh>,
    core: Handle<Mesh>,
    /// A plate whose partner is down: the link is usable.
    linked: Handle<StandardMaterial>,
    /// A plate on its own: placed, but connecting nothing yet.
    lone: Handle<StandardMaterial>,
    /// Someone else's plate. Visible — it is a physical object — but never
    /// mistakable for one of yours, because walking onto it does nothing.
    rival: Handle<StandardMaterial>,
    /// The body of the plate, held or deployed.
    shell: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct PadProjection {
    signature: u64,
}

pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let authored = crate::view::assets::asset_present(observed_assets::PAD.path)
        .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset(observed_assets::PAD.path)));
    commands.insert_resource(PadVisualAssets {
        authored,
        // A plate is wider than a footfall and thin enough to step over without
        // it reading as an obstacle.
        plate: meshes.add(Cylinder::new(0.78, 0.05)),
        core: meshes.add(Cylinder::new(0.46, 0.02)),
        linked: signal_material(&mut materials, MarkerRole::NextRoom),
        lone: signal_material(&mut materials, MarkerRole::Control),
        rival: signal_material(&mut materials, MarkerRole::Rival),
        shell: signal_material(&mut materials, MarkerRole::Teammate),
    });
    commands.init_resource::<PadProjection>();
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<PadVisualAssets>();
    commands.remove_resource::<PadProjection>();
}

pub(super) fn sync_projection(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Res<PadVisualAssets>,
    mut projection: ResMut<PadProjection>,
    existing: Query<Entity, With<PadVisual>>,
) {
    let signature = pad_signature(&runtime);
    if projection.signature == signature {
        return;
    }
    projection.signature = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let local_team = runtime
        .match_state
        .players
        .get(&runtime.local_player)
        .map(|player| player.team);

    for player in runtime.match_state.players.values() {
        if runtime.match_state.pads.inventory(player.id) > 0 && !player.escaped {
            spawn_plate(
                &mut commands,
                &assets,
                PadVisual::Held(player.id),
                held_pose(player),
                assets.shell.clone(),
            );
        }
    }

    for pad in runtime.match_state.pads.deployed.values() {
        let face = if local_team != Some(pad.team) {
            assets.rival.clone()
        } else if runtime
            .match_state
            .pads
            .link_target(pad.team, pad.id)
            .is_some()
        {
            assets.linked.clone()
        } else {
            assets.lone.clone()
        };
        spawn_plate(
            &mut commands,
            &assets,
            PadVisual::Deployed(pad.id),
            // Seated just clear of the deck so it never z-fights the floor slab.
            Transform::from_translation(pad.position + Vec3::Y * 0.03),
            face,
        );
    }
}

/// Only the carried plate moves every frame; a deployed one is static until the
/// projection rebuilds, exactly as the lantern's deployed cages are.
pub(super) fn sync_dynamic(
    runtime: Res<HexWfcRuntime>,
    mut plates: Query<(&PadVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut plates {
        if let PadVisual::Held(player) = visual
            && let Some(state) = runtime.match_state.players.get(player)
        {
            *transform = held_pose(state);
        }
    }
}

fn spawn_plate(
    commands: &mut Commands,
    assets: &PadVisualAssets,
    visual: PadVisual,
    transform: Transform,
    face: Handle<StandardMaterial>,
) {
    // The stable domain ID rides in the entity name so a deployed plate can be
    // told from its twin in an inspector or an evidence capture, where two
    // identical discs are otherwise indistinguishable.
    let (held, name) = match &visual {
        PadVisual::Held(player) => (true, format!("Teleport plate (carried by {})", player.0)),
        PadVisual::Deployed(id) => (false, format!("Teleport plate {}", id.0)),
    };
    commands
        .spawn((
            visual,
            DespawnOnExit(GameState::HexWfc),
            transform,
            Visibility::Visible,
            Name::new(name),
        ))
        .with_children(|root| {
            if let Some(ref authored) = assets.authored {
                root.spawn((
                    SceneRoot(authored.clone()),
                    Visibility::Inherited,
                    Transform::IDENTITY,
                ));
            } else {
                root.spawn((
                    Mesh3d(assets.plate.clone()),
                    MeshMaterial3d(assets.shell.clone()),
                    Visibility::Inherited,
                    Transform::IDENTITY,
                ));
            }
            // The lit core rides on top of either body, so an authored mesh still
            // reports link state rather than needing to encode it in the art.
            root.spawn((
                Mesh3d(assets.core.clone()),
                MeshMaterial3d(face),
                Visibility::Inherited,
                Transform::from_translation(Vec3::Y * if held { 0.03 } else { 0.04 }),
            ));
        });
}

/// A stowed plate rides at the player's off hand, mirrored from the lantern so
/// the two tools read as a pair rather than as one tool and one anomaly.
fn held_pose(player: &observed_match::hex_wfc::HexPlayerState) -> Transform {
    let rotation = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    Transform::from_translation(
        player.position + Vec3::Y * EYE_OFFSET + rotation * Vec3::new(-0.30, -0.26, -0.78),
    )
    .with_rotation(rotation * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
    .with_scale(Vec3::splat(0.34))
}

/// Rebuild only when something a viewer could see has changed: who is carrying
/// how many, which plates exist, and — because it changes their colour — which
/// team each belongs to.
fn pad_signature(runtime: &HexWfcRuntime) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x100_0000_01B3);
    };
    for (&player, &count) in &runtime.match_state.pads.carried {
        mix(u64::from(player.0));
        mix(u64::from(count));
    }
    for (&id, pad) in &runtime.match_state.pads.deployed {
        mix(u64::from(id.0));
        mix(u64::from(pad.owner.0));
        mix(u64::from(pad.team.0));
    }
    hash
}

fn signal_material(
    materials: &mut Assets<StandardMaterial>,
    role: MarkerRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.55,
        ..default()
    })
}
