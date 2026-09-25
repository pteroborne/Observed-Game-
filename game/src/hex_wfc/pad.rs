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
//! author has supplied one, a procedural hexagonal plate otherwise — and every
//! material comes from `observed_style`: the signal parts from the same semantics
//! that colour every other marker, the hardware from `observed_style::equipment`,
//! which keeps it dark so only the ring and lens carry the plate's state.
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
use observed_style::equipment::{Hardware, finish, held, link_column};

use super::equipment::{
    Hand, HeldSway, Spin, held_transform, hex_prism, hex_ring, light_tube, sway_for,
};
use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) enum PadVisual {
    Held(PlayerId),
    Deployed(EquipmentId),
}

/// The plate, as meshes: a hexagonal deck lined up with the cell it lies in, a raised
/// rim with a bright machined edge and six studs, and inside it the signal — a state
/// ring, an inner ring that turns while the link is live, and a lens.
#[derive(Resource)]
pub(super) struct PadVisualAssets {
    authored: Option<Handle<WorldAsset>>,
    deck: Handle<Mesh>,
    rim: Handle<Mesh>,
    edge: Handle<Mesh>,
    stud: Handle<Mesh>,
    ring: Handle<Mesh>,
    floor: Handle<Mesh>,
    inner_ring: Handle<Mesh>,
    lens: Handle<Mesh>,
    column: Handle<Mesh>,
    body: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    /// A plate whose partner is down: the link is usable.
    linked: Handle<StandardMaterial>,
    /// A plate on its own: placed, but connecting nothing yet.
    lone: Handle<StandardMaterial>,
    /// Someone else's plate. Visible — it is a physical object — but never
    /// mistakable for one of yours, because walking onto it does nothing.
    rival: Handle<StandardMaterial>,
    /// A plate in your hand, in your team's colour and the first-person budget.
    carried: Handle<StandardMaterial>,
    /// The haze that rises from a linked plate.
    link_column: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct PadProjection {
    signature: u64,
}

/// A plate's state, which is all that differs between one plate and another.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    Linked,
    Lone,
    Rival,
    Carried,
}

/// Plates drawn in a hand, however many are carried: a stack says how many.
const MAX_STACK: u16 = 3;

pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let authored = crate::view::assets::asset_present(observed_assets::PAD.path)
        .then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset(observed_assets::PAD.path)));
    let hardware = |materials: &mut Assets<StandardMaterial>, part: Hardware| {
        let finish = finish(part);
        materials.add(StandardMaterial {
            base_color: finish.base_color,
            metallic: finish.metallic,
            perceptual_roughness: finish.roughness,
            ..default()
        })
    };
    commands.insert_resource(PadVisualAssets {
        authored,
        // A plate is wider than a footfall and low enough to step over without it
        // reading as an obstacle: 1.6 m across its corners, under 10 cm tall.
        deck: meshes.add(hex_prism(0.80, 0.76, 0.0, 0.035)),
        rim: meshes.add(hex_ring(0.76, 0.66, 0.035, 0.075)),
        edge: meshes.add(hex_ring(0.745, 0.705, 0.075, 0.081)),
        stud: meshes.add(hex_prism(0.034, 0.026, 0.075, 0.097)),
        ring: meshes.add(hex_ring(0.62, 0.55, 0.035, 0.052)),
        floor: meshes.add(hex_prism(0.66, 0.66, 0.035, 0.044)),
        inner_ring: meshes.add(hex_ring(0.37, 0.33, 0.044, 0.051)),
        lens: meshes.add(hex_prism(0.20, 0.15, 0.044, 0.072)),
        column: meshes.add(light_tube(0.58, 1.9)),
        body: hardware(&mut materials, Hardware::Body),
        trim: hardware(&mut materials, Hardware::Trim),
        linked: signal_material(&mut materials, MarkerRole::NextRoom, None),
        lone: signal_material(&mut materials, MarkerRole::Control, None),
        rival: signal_material(&mut materials, MarkerRole::Rival, None),
        carried: signal_material(
            &mut materials,
            MarkerRole::Teammate,
            Some(held(MarkerRole::Teammate)),
        ),
        link_column: materials.add(StandardMaterial {
            base_color: Color::LinearRgba(link_column(MarkerRole::NextRoom)),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: true,
            ..default()
        }),
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
    sway: Res<HeldSway>,
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
        let carried = runtime.match_state.pads.inventory(player.id);
        if carried > 0 && !player.escaped {
            spawn_plate(
                &mut commands,
                &assets,
                PadVisual::Held(player.id),
                held_pose(&runtime, &sway, player),
                Face::Carried,
                carried.min(MAX_STACK),
            );
        }
    }

    for pad in runtime.match_state.pads.deployed.values() {
        let face = if local_team != Some(pad.team) {
            Face::Rival
        } else if runtime
            .match_state
            .pads
            .link_target(pad.team, pad.id)
            .is_some()
        {
            Face::Linked
        } else {
            Face::Lone
        };
        spawn_plate(
            &mut commands,
            &assets,
            PadVisual::Deployed(pad.id),
            // Seated just clear of the deck so it never z-fights the floor slab.
            Transform::from_translation(pad.position + Vec3::Y * 0.01),
            face,
            1,
        );
    }
}

/// Only the carried plate moves every frame; a deployed one is static until the
/// projection rebuilds, exactly as the lantern's deployed cages are.
pub(super) fn sync_dynamic(
    runtime: Res<HexWfcRuntime>,
    sway: Res<HeldSway>,
    mut plates: Query<(&PadVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut plates {
        if let PadVisual::Held(player) = visual
            && let Some(state) = runtime.match_state.players.get(player)
        {
            *transform = held_pose(&runtime, &sway, state);
        }
    }
}

fn spawn_plate(
    commands: &mut Commands,
    assets: &PadVisualAssets,
    visual: PadVisual,
    transform: Transform,
    face: Face,
    stack: u16,
) {
    // The stable domain ID rides in the entity name so a deployed plate can be
    // told from its twin in an inspector or an evidence capture, where two
    // identical discs are otherwise indistinguishable.
    let name = match &visual {
        PadVisual::Held(player) => format!("Teleport plate (carried by {})", player.0),
        PadVisual::Deployed(id) => format!("Teleport plate {}", id.0),
    };
    let signal = match face {
        Face::Linked => assets.linked.clone(),
        Face::Lone => assets.lone.clone(),
        Face::Rival => assets.rival.clone(),
        Face::Carried => assets.carried.clone(),
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
            for level in 0..stack {
                // A carried stack: each plate under the last, only the top one lit.
                let at = Transform::from_translation(Vec3::Y * -0.1 * f32::from(level));
                let part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>| {
                    (
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Visibility::Inherited,
                        at,
                    )
                };
                if let Some(ref authored) = assets.authored {
                    root.spawn((WorldAssetRoot(authored.clone()), Visibility::Inherited, at));
                } else {
                    root.spawn(part(&assets.deck, &assets.body));
                    root.spawn(part(&assets.rim, &assets.body));
                    root.spawn(part(&assets.edge, &assets.trim));
                    root.spawn(part(&assets.floor, &assets.body));
                    for corner in 0..6 {
                        #[allow(clippy::cast_precision_loss)]
                        let angle = std::f32::consts::FRAC_PI_6
                            + std::f32::consts::FRAC_PI_3 * corner as f32;
                        root.spawn((
                            Mesh3d(assets.stud.clone()),
                            MeshMaterial3d(assets.trim.clone()),
                            Visibility::Inherited,
                            at * Transform::from_xyz(angle.cos() * 0.71, 0.0, angle.sin() * 0.71),
                        ));
                    }
                }
                if level > 0 {
                    continue;
                }
                // The signal rides on either body, so an authored mesh still reports
                // link state rather than needing to encode it in the art.
                root.spawn(part(&assets.ring, &signal));
                root.spawn(part(&assets.lens, &signal));
                let mut inner = root.spawn(part(&assets.inner_ring, &signal));
                if face == Face::Linked {
                    inner.insert(Spin {
                        axis: Vec3::Y,
                        rate: 0.6,
                        rest: Quat::IDENTITY,
                        // Only a plate on the floor turns: it is the world's.
                        in_hand: false,
                    });
                    root.spawn((
                        Mesh3d(assets.column.clone()),
                        MeshMaterial3d(assets.link_column.clone()),
                        Visibility::Inherited,
                        Transform::from_xyz(0.0, 0.05, 0.0),
                        bevy::light::NotShadowCaster,
                        bevy::light::NotShadowReceiver,
                    ));
                }
            }
        });
}

/// A carried plate rides low in the off hand, face tipped up toward the eye so its
/// ring can be read, mirrored from the lantern so the two tools read as a pair. Close
/// in, inside the body's own radius, so it never pushes into a wall the body is
/// standing against.
pub(in crate::hex_wfc) const HAND: Hand = Hand {
    offset: Vec3::new(-0.13, -0.15, -0.20),
    roll: -0.3,
    tip: 0.8,
    scale: 0.065,
};

/// The corners of a box around a carried stack in its own frame, for
/// `held_devices_stay_inside_the_body`.
#[cfg(test)]
pub(in crate::hex_wfc) const REACH: [Vec3; 2] = [
    Vec3::new(-0.80, -0.1 * (MAX_STACK - 1) as f32, -0.80),
    Vec3::new(0.80, 0.097, 0.80),
];

fn held_pose(
    runtime: &HexWfcRuntime,
    sway: &HeldSway,
    player: &observed_match::hex_wfc::HexPlayerState,
) -> Transform {
    held_transform(player, sway_for(runtime, sway, player), &HAND)
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
    emissive: Option<LinearRgba>,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: emissive.unwrap_or(treatment.emissive),
        perceptual_roughness: 0.55,
        ..default()
    })
}
