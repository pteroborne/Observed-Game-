//! Presentation for the caged anchor lantern and physical Guardian. The lantern
//! uses the authored drop-in mesh when present and a procedural cage otherwise.
//! Every material comes from `observed_style`; geometry communicates state in
//! addition to colour (cage, core, deployed threshold lock, tall threat body).

use std::collections::BTreeMap;

use bevy::{gltf::GltfAssetLabel, prelude::*};
use observed_core::{EquipmentId, PlayerId};
use observed_hex::{HexCoord, hex_origin};
use observed_style::MarkerRole;

use super::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::GameState;

#[derive(Component)]
pub(super) enum LanternVisual {
    Held(PlayerId),
    Deployed(EquipmentId),
    Cache(EquipmentId),
}

#[derive(Component)]
pub(super) struct LanternCoreLight(PlayerId);

/// The bead marking where a lantern would hang if the player pressed deploy.
///
/// The anchor was the least discoverable thing in the match: it needs an open
/// threshold, aimed at, from inside a room, and it fails silently everywhere
/// else — so a whole playtest went by without anyone finding it. This is the
/// smallest honest fix. It is a world-space object rather than HUD text, it
/// appears only where the press would actually work, and it is positioned by
/// the simulation's own [`HexAnchorSite`], so it cannot promise a placement the
/// rule would refuse.
#[derive(Component)]
pub(super) struct AnchorGhost;

#[derive(Component)]
pub(super) struct HexGuardianVisual;

#[derive(Clone, Copy)]
pub(super) struct LanternSignalSample {
    seed: u64,
    generation: u32,
    player_cell: HexCoord,
    guardian_cell: HexCoord,
    inventory: u16,
    guide: f32,
    pressure: f32,
}

#[derive(Resource)]
pub(super) struct LanternVisualAssets {
    lantern_mesh: Option<Handle<Mesh>>,
    core: Handle<Mesh>,
    cage_bar: Handle<Mesh>,
    cage_ring: Handle<Mesh>,
    guardian_body: Handle<Mesh>,
    guide: Handle<StandardMaterial>,
    cage: Handle<StandardMaterial>,
    held_cage: Handle<StandardMaterial>,
    threat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct LanternProjection {
    signature: u64,
}

pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The upstream GLB is an Import Replacer sample: its full scene contains
    // Godot-only helper and collision nodes. Loading just the authored torch
    // primitive keeps those helpers out of Bevy and lets the shared semantic
    // material remain the source of colour/emission.
    let lantern_mesh =
        crate::view::assets::asset_present(observed_assets::LANTERN.path).then(|| {
            asset_server.load(
                GltfAssetLabel::Primitive {
                    mesh: 1,
                    primitive: 0,
                }
                .from_asset(observed_assets::LANTERN.path),
            )
        });
    let assets = LanternVisualAssets {
        lantern_mesh,
        core: meshes.add(Sphere::new(0.18)),
        cage_bar: meshes.add(Cylinder::new(0.025, 0.62)),
        cage_ring: meshes.add(Torus::new(0.20, 0.235)),
        guardian_body: meshes.add(Capsule3d::new(0.52, 1.6)),
        guide: signal_material(&mut materials, MarkerRole::NextRoom),
        cage: signal_material(&mut materials, MarkerRole::Control),
        // A carried cage sits inside the first-person exposure budget; its
        // objective core remains signal-tier while the metal does not bloom to
        // white. Deployed/cache cages keep the full control-device treatment.
        held_cage: scaled_signal_material(&mut materials, MarkerRole::Control, 0.35),
        threat: signal_material(&mut materials, MarkerRole::Collapse),
    };
    // One persistent marker, moved and hidden rather than respawned: it changes
    // every time the player turns their head, and spawn/despawn churn at look
    // rate is exactly the cost a Deck should not pay for a hint.
    commands.spawn((
        AnchorGhost,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(assets.core.clone()),
        MeshMaterial3d(assets.held_cage.clone()),
        Transform::from_scale(Vec3::splat(0.55)),
        Visibility::Hidden,
        Name::new("Anchor site marker"),
    ));
    commands.insert_resource(assets);
    commands.insert_resource(LanternProjection::default());
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<LanternVisualAssets>();
    commands.remove_resource::<LanternProjection>();
}

pub(super) fn sync_projection(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Res<LanternVisualAssets>,
    mut projection: ResMut<LanternProjection>,
    existing: Query<Entity, With<LanternVisual>>,
    guardian: Query<Entity, With<HexGuardianVisual>>,
) {
    let signature = equipment_signature(&runtime);
    if projection.signature == signature {
        return;
    }
    projection.signature = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for entity in &guardian {
        commands.entity(entity).despawn();
    }

    for player in runtime.match_state.players.values() {
        if runtime.match_state.lanterns.inventory(player.id) > 0 && !player.escaped {
            spawn_caged_lantern(
                &mut commands,
                &assets,
                LanternVisual::Held(player.id),
                held_pose(player),
                Some(player.id),
            );
        }
    }
    for lantern in runtime.match_state.lanterns.deployed.values() {
        spawn_caged_lantern(
            &mut commands,
            &assets,
            LanternVisual::Deployed(lantern.id),
            Transform::from_translation(lantern.position + Vec3::Y * 0.36),
            None,
        );
    }
    for cache in runtime
        .match_state
        .lanterns
        .caches
        .values()
        .filter(|cache| !cache.collected)
    {
        let origin = Vec3::from_array(hex_origin(cache.cell));
        spawn_caged_lantern(
            &mut commands,
            &assets,
            LanternVisual::Cache(cache.id),
            Transform::from_translation(origin + Vec3::Y * 0.42).with_scale(Vec3::splat(
                1.0 + f32::from(cache.amount.saturating_sub(1)) * 0.12,
            )),
            None,
        );
    }

    let guardian_state = &runtime.match_state.guardian;
    commands
        .spawn((
            HexGuardianVisual,
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(guardian_state.position),
            Visibility::Visible,
            Name::new("Hex Guardian procedural threat"),
        ))
        .with_children(|root| {
            root.spawn((
                Mesh3d(assets.guardian_body.clone()),
                MeshMaterial3d(assets.threat.clone()),
                Transform::from_scale(Vec3::new(0.8, 1.45, 0.8)),
            ));
            root.spawn((
                Mesh3d(assets.cage_ring.clone()),
                MeshMaterial3d(assets.threat.clone()),
                Transform::from_translation(Vec3::Y * 1.25),
            ));
            root.spawn((
                PointLight {
                    color: observed_style::marker(MarkerRole::Collapse).base_color,
                    intensity: 1_000.0,
                    range: 6.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(Vec3::Y * 0.8),
            ));
        });
}

/// Show the bead exactly while the press would work, and nowhere else.
///
/// The breathing scale is the whole invitation: a static bead in a facility
/// full of emissive fittings reads as more scenery, and this has to read as
/// something the player can act on.
pub(super) fn sync_anchor_ghost(
    runtime: Res<HexWfcRuntime>,
    mut ghost: Query<(&mut Transform, &mut Visibility), With<AnchorGhost>>,
) {
    let Ok((mut transform, mut visibility)) = ghost.single_mut() else {
        return;
    };
    let Some(site) = runtime
        .match_state
        .deployable_threshold(runtime.local_player)
    else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;
    transform.translation = site.position;
    let breath = (runtime.match_state.tick as f32 * 0.09)
        .sin()
        .mul_add(0.06, 0.55);
    transform.scale = Vec3::splat(breath);
}

pub(super) fn sync_dynamic(
    runtime: Res<HexWfcRuntime>,
    mut lanterns: Query<(&LanternVisual, &mut Transform)>,
    mut core_lights: Query<(&LanternCoreLight, &mut PointLight)>,
    mut guardian: Query<&mut Transform, (With<HexGuardianVisual>, Without<LanternVisual>)>,
    mut signal_cache: Local<BTreeMap<PlayerId, LanternSignalSample>>,
) {
    for (visual, mut transform) in &mut lanterns {
        match visual {
            LanternVisual::Held(player) => {
                *transform = held_pose(&runtime.match_state.players[player]);
            }
            LanternVisual::Deployed(id) | LanternVisual::Cache(id) => {
                // Keep the stable domain ID present and observed by the projection;
                // these poses change only when the equipment signature rebuilds.
                let _stable_id = id.0;
            }
        }
    }
    for (owner, mut light) in &mut core_lights {
        let player = &runtime.match_state.players[&owner.0];
        let inventory = runtime.match_state.lanterns.inventory(owner.0);
        let seed = runtime.match_state.seed;
        let generation = runtime.match_state.facility.generation;
        let guardian_cell = runtime.match_state.guardian.cell;
        let stale = signal_cache.get(&owner.0).is_none_or(|sample| {
            sample.seed != seed
                || sample.generation != generation
                || sample.player_cell != player.cell
                || sample.guardian_cell != guardian_cell
                || sample.inventory != inventory
        });
        if stale {
            signal_cache.insert(
                owner.0,
                LanternSignalSample {
                    seed,
                    generation,
                    player_cell: player.cell,
                    guardian_cell,
                    inventory,
                    guide: runtime.match_state.lantern_proximity(owner.0),
                    pressure: runtime.match_state.guardian_pressure(owner.0),
                },
            );
        }
        let sample = signal_cache[&owner.0];
        let pressure = if player.cell == guardian_cell {
            // Same-cell pressure includes physical distance, not just topology, so it
            // remains live while the two bodies close on one another. This branch does
            // no graph search.
            runtime.match_state.guardian_pressure(owner.0)
        } else {
            sample.pressure
        };
        let pulse = guardian_flicker(runtime.match_state.tick, pressure);
        let (intensity, range) = carried_light_budget(sample.guide, pulse);
        light.intensity = intensity;
        light.range = range;
    }
    if let Ok(mut transform) = guardian.single_mut() {
        transform.translation = runtime.match_state.guardian.position;
    }
}

fn spawn_caged_lantern(
    commands: &mut Commands,
    assets: &LanternVisualAssets,
    visual: LanternVisual,
    transform: Transform,
    held_owner: Option<PlayerId>,
) {
    let cage_material = if matches!(&visual, LanternVisual::Held(_)) {
        assets.held_cage.clone()
    } else {
        assets.cage.clone()
    };
    commands
        .spawn((
            visual,
            DespawnOnExit(GameState::HexWfc),
            transform,
            Visibility::Visible,
            Name::new("Caged anchor lantern"),
        ))
        .with_children(|root| {
            let authored_mesh = assets.lantern_mesh.is_some();
            let core_transform = if authored_mesh {
                // The source mesh is a slim torch rather than the wide procedural
                // cage. Seat a smaller guide core at its head so the authored
                // handle remains readable instead of disappearing inside a globe.
                Transform::from_translation(Vec3::Y * 0.24).with_scale(Vec3::splat(0.30))
            } else {
                Transform::IDENTITY
            };
            root.spawn((
                Mesh3d(assets.core.clone()),
                MeshMaterial3d(assets.guide.clone()),
                Visibility::Inherited,
                core_transform,
            ));
            if let Some(ref mesh) = assets.lantern_mesh {
                root.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(cage_material.clone()),
                    Visibility::Inherited,
                    Transform::from_translation(Vec3::ZERO).with_scale(Vec3::splat(0.85)),
                ));
            } else {
                for angle in [
                    0.0,
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::PI,
                    4.712_389,
                ] {
                    root.spawn((
                        Mesh3d(assets.cage_bar.clone()),
                        MeshMaterial3d(cage_material.clone()),
                        Visibility::Inherited,
                        Transform::from_translation(Vec3::new(
                            angle.cos() * 0.23,
                            0.0,
                            angle.sin() * 0.23,
                        )),
                    ));
                }
                for y in [-0.31, 0.31] {
                    root.spawn((
                        Mesh3d(assets.cage_ring.clone()),
                        MeshMaterial3d(cage_material.clone()),
                        Visibility::Inherited,
                        Transform::from_translation(Vec3::Y * y),
                    ));
                }
            }
            if let Some(owner) = held_owner {
                root.spawn((
                    LanternCoreLight(owner),
                    PointLight {
                        color: observed_style::marker(MarkerRole::NextRoom).base_color,
                        intensity: 35.0,
                        range: 3.2,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(core_transform.translation),
                ));
            } else {
                root.spawn((
                    PointLight {
                        color: observed_style::marker(MarkerRole::Control).base_color,
                        intensity: 180.0,
                        range: 5.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(core_transform.translation),
                ));
            }
        });
}

fn held_pose(player: &observed_match::hex_wfc::HexPlayerState) -> Transform {
    let rotation = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    Transform::from_translation(
        player.position + Vec3::Y * EYE_OFFSET + rotation * Vec3::new(0.28, -0.24, -0.82),
    )
    .with_rotation(rotation)
    .with_scale(Vec3::splat(0.42))
}

fn carried_light_budget(guide: f32, pulse: f32) -> (f32, f32) {
    (
        (35.0 + guide.clamp(0.0, 1.0) * 215.0) * pulse,
        3.2 + guide.clamp(0.0, 1.0) * 2.8,
    )
}

fn guardian_flicker(tick: u64, pressure: f32) -> f32 {
    if pressure <= 0.0 {
        return 1.0;
    }
    let period = (42.0 - pressure.clamp(0.0, 1.0) * 36.0).round().max(6.0) as u64;
    if tick.is_multiple_of(period) || (tick + 1).is_multiple_of(period) {
        0.38
    } else {
        1.0
    }
}

fn equipment_signature(runtime: &HexWfcRuntime) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x100_0000_01B3);
    };
    for (&player, &count) in &runtime.match_state.lanterns.carried {
        mix(u64::from(player.0));
        mix(u64::from(count));
    }
    for (&id, lantern) in &runtime.match_state.lanterns.deployed {
        mix(u64::from(id.0));
        mix(u64::from(lantern.owner.0));
    }
    for (&id, cache) in &runtime.match_state.lanterns.caches {
        mix(u64::from(id.0));
        mix(u64::from(cache.collected));
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
        metallic: 0.45,
        perceptual_roughness: 0.3,
        ..default()
    })
}

fn scaled_signal_material(
    materials: &mut Assets<StandardMaterial>,
    role: MarkerRole,
    emissive_scale: f32,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive * emissive_scale,
        metallic: 0.55,
        perceptual_roughness: 0.38,
        ..default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardian_pressure_increases_flicker_frequency() {
        let low_period = (0..240)
            .filter(|&tick| guardian_flicker(tick, 0.1) < 1.0)
            .count();
        let high_period = (0..240)
            .filter(|&tick| guardian_flicker(tick, 1.0) < 1.0)
            .count();
        assert!(high_period > low_period);
    }

    #[test]
    fn carried_light_budget_is_bounded_but_tracks_the_exit() {
        let far = carried_light_budget(0.0, 1.0);
        let near = carried_light_budget(1.0, 1.0);
        assert_eq!(far, (35.0, 3.2));
        assert_eq!(near, (250.0, 6.0));
        assert!(near.0 > far.0 && near.1 > far.1);
    }
}
