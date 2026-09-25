//! Presentation for the caged anchor lantern. The Guardian is drawn by `guardian`.
//!
//! The lantern is an observation torch: a knurled grip, a hexagonal cage and glass
//! chamber, and inside it the guide core the player reads the exit from, circled by
//! a slow gyro in the anchor's purple. The core brightens as the exit nears and
//! stutters as the Guardian closes, with the light it casts. A drop-in authored body
//! ([`observed_assets::LANTERN`]) replaces the procedural hardware when an author
//! supplies one; the core and gyro ride inside either. Every material comes from
//! `observed_style`; geometry communicates state in addition to colour (cage, core,
//! deployed threshold lock).

use std::collections::BTreeMap;

use bevy::{gltf::GltfAssetLabel, prelude::*};
use observed_core::{EquipmentId, PlayerId};
use observed_hex::{HexCoord, hex_origin};
use observed_style::MarkerRole;

use observed_style::equipment::{Hardware, finish, held};

use super::equipment::{HeldSway, hex_prism, hex_ring};
use super::sim::HexWfcRuntime;
use crate::GameState;

pub(super) mod torch;

pub(super) use torch::sync_core_glow;
use torch::{PLACED_SCALE, POMMEL, core_glow, held_pose, spawn_caged_lantern};

#[derive(Component)]
pub(super) enum LanternVisual {
    Held(PlayerId),
    Deployed(EquipmentId),
    Cache(EquipmentId),
}

#[derive(Component)]
pub(super) struct LanternCoreLight {
    owner: PlayerId,
    /// How brightly the core glows this frame, as a share of its full signal.
    glow: f32,
}

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
    authored: Option<Handle<WorldAsset>>,
    /// The torch's hardware, each part with its finish; origin at the core.
    hardware: Vec<(Handle<Mesh>, Hardware, Transform)>,
    /// A stand for a lantern set down, under its pommel.
    plinth: Handle<Mesh>,
    core: Handle<Mesh>,
    gyro: Handle<Mesh>,
    accent: Handle<Mesh>,
    ghost: Handle<Mesh>,
    finishes: BTreeMap<Hardware, Handle<StandardMaterial>>,
    glass: Handle<StandardMaterial>,
    guide: Handle<StandardMaterial>,
    /// The local player's own core, whose emission follows the light it casts.
    held_core: Handle<StandardMaterial>,
    cage: Handle<StandardMaterial>,
    held_cage: Handle<StandardMaterial>,
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
    let authored = crate::view::assets::asset_present(observed_assets::LANTERN.path).then(|| {
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(observed_assets::LANTERN.path))
    });
    let hardware = torch::hardware(&mut meshes);
    let finishes = torch::finishes(&mut materials);
    let glass = finish(Hardware::Glass);
    let assets = LanternVisualAssets {
        authored,
        hardware,
        plinth: meshes.add(hex_prism(0.085, 0.060, -0.03, 0.0)),
        core: meshes.add(Sphere::new(0.024).mesh().uv(24, 16)),
        gyro: meshes.add(Torus::new(0.033, 0.039).mesh().build()),
        accent: meshes.add(hex_ring(0.0665, 0.0605, -0.0745, -0.0705)),
        ghost: meshes.add(Sphere::new(0.18)),
        finishes,
        glass: materials.add(StandardMaterial {
            base_color: glass.base_color,
            metallic: glass.metallic,
            perceptual_roughness: glass.roughness,
            // Clear enough that the core, not the glass, is what the eye finds.
            reflectance: 0.3,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        guide: signal_material(&mut materials, MarkerRole::NextRoom),
        held_core: signal_material(&mut materials, MarkerRole::NextRoom),
        cage: signal_material(&mut materials, MarkerRole::Control),
        // A carried lantern's anchor trim sits inside the first-person exposure
        // budget; its guide core stays signal-tier while the purple does not bloom
        // to white. A lantern set down keeps the full control-device treatment.
        held_cage: scaled_signal_material(&mut materials, MarkerRole::Control, held),
    };
    // One persistent marker, moved and hidden rather than respawned: it changes
    // every time the player turns their head, and spawn/despawn churn at look
    // rate is exactly the cost a Deck should not pay for a hint.
    commands.spawn((
        AnchorGhost,
        DespawnOnExit(GameState::HexWfc),
        Mesh3d(assets.ghost.clone()),
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
    sway: Res<HeldSway>,
    assets: Res<LanternVisualAssets>,
    mut projection: ResMut<LanternProjection>,
    existing: Query<Entity, With<LanternVisual>>,
) {
    let signature = equipment_signature(&runtime);
    if projection.signature == signature {
        return;
    }
    projection.signature = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    for player in runtime.match_state.players.values() {
        if runtime.match_state.lanterns.inventory(player.id) > 0 && !player.escaped {
            spawn_caged_lantern(
                &mut commands,
                &assets,
                LanternVisual::Held(player.id),
                held_pose(&runtime, &sway, player),
                Some((player.id, player.id == runtime.local_player)),
            );
        }
    }
    for lantern in runtime.match_state.lanterns.deployed.values() {
        spawn_caged_lantern(
            &mut commands,
            &assets,
            LanternVisual::Deployed(lantern.id),
            // Stood on its plinth on the threshold's floor: the anchor site is the
            // doorway's middle, well above the deck.
            Transform::from_translation(Vec3::new(
                lantern.position.x,
                hex_origin(lantern.cell)[1]
                    + observed_hex::FLOOR_SLAB_TOP
                    + (POMMEL + 0.03) * PLACED_SCALE,
                lantern.position.z,
            ))
            .with_scale(Vec3::splat(PLACED_SCALE)),
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
            {
                let scale = PLACED_SCALE * (1.0 + f32::from(cache.amount.saturating_sub(1)) * 0.12);
                Transform::from_translation(
                    origin + Vec3::Y * (observed_hex::FLOOR_SLAB_TOP + (POMMEL + 0.03) * scale),
                )
                .with_scale(Vec3::splat(scale))
            },
            None,
        );
    }
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
    sway: Res<HeldSway>,
    mut lanterns: Query<(&LanternVisual, &mut Transform)>,
    mut core_lights: Query<(&mut LanternCoreLight, &mut PointLight)>,
    mut signal_cache: Local<BTreeMap<PlayerId, LanternSignalSample>>,
) {
    for (visual, mut transform) in &mut lanterns {
        match visual {
            LanternVisual::Held(player) => {
                *transform = held_pose(&runtime, &sway, &runtime.match_state.players[player]);
            }
            LanternVisual::Deployed(id) | LanternVisual::Cache(id) => {
                // Keep the stable domain ID present and observed by the projection;
                // these poses change only when the equipment signature rebuilds.
                let _stable_id = id.0;
            }
        }
    }
    for (mut owner, mut light) in &mut core_lights {
        let player = &runtime.match_state.players[&owner.owner];
        let inventory = runtime.match_state.lanterns.inventory(owner.owner);
        let seed = runtime.match_state.seed;
        let generation = runtime.match_state.facility.generation;
        let guardian_cell = runtime.match_state.guardian.cell;
        let stale = signal_cache.get(&owner.owner).is_none_or(|sample| {
            sample.seed != seed
                || sample.generation != generation
                || sample.player_cell != player.cell
                || sample.guardian_cell != guardian_cell
                || sample.inventory != inventory
        });
        if stale {
            signal_cache.insert(
                owner.owner,
                LanternSignalSample {
                    seed,
                    generation,
                    player_cell: player.cell,
                    guardian_cell,
                    inventory,
                    guide: runtime.match_state.lantern_proximity(owner.owner),
                    pressure: runtime.match_state.guardian_pressure(owner.owner),
                },
            );
        }
        let sample = signal_cache[&owner.owner];
        let pressure = if player.cell == guardian_cell {
            // Same-cell pressure includes physical distance, not just topology, so it
            // remains live while the two bodies close on one another. This branch does
            // no graph search.
            runtime.match_state.guardian_pressure(owner.owner)
        } else {
            sample.pressure
        };
        let pulse = guardian_flicker(runtime.match_state.tick, pressure);
        let (intensity, range) = carried_light_budget(sample.guide, pulse);
        light.intensity = intensity;
        light.range = range;
        owner.glow = core_glow(sample.guide, pulse);
    }
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
    emissive: impl Fn(MarkerRole) -> LinearRgba,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: emissive(role),
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
