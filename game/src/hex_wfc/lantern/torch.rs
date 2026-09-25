//! The observation torch's model: its hardware, how it is put together, where a hand
//! holds it, and how its core glows. The rest of the lantern (projection, the anchor
//! site marker, the Guardian) is in the parent module.
use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::PlayerId;
use observed_style::MarkerRole;
use observed_style::equipment::{Hardware, finish};

use super::super::equipment::{Hand, HeldSway, Spin, held_transform, hex_prism, sway_for};
use super::super::sim::HexWfcRuntime;
use super::{LanternCoreLight, LanternVisual, LanternVisualAssets};
use crate::GameState;

/// Height from the core down to the foot of the pommel, metres.
pub(super) const POMMEL: f32 = 0.234;
/// A lantern set down at a threshold, or waiting in a cache, is drawn this much larger
/// than the one in a hand, so it reads from across a room.
pub(super) const PLACED_SCALE: f32 = 2.4;

/// The corners of a box around the torch in its own frame, for
/// `held_devices_stay_inside_the_body`.
#[cfg(test)]
pub(in crate::hex_wfc) const REACH: [Vec3; 2] = [
    Vec3::new(-0.066, -POMMEL, -0.066),
    Vec3::new(0.066, 0.128, 0.066),
];

/// The torch's hardware, each part with its finish; origin at the core.
pub(super) fn hardware(meshes: &mut Assets<Mesh>) -> Vec<(Handle<Mesh>, Hardware, Transform)> {
    let mut part = |mesh: Mesh, finish: Hardware, at: Transform| (meshes.add(mesh), finish, at);
    let origin = Transform::IDENTITY;
    let mut hardware = vec![
        // Caps, top and bottom, chamfered toward the grip and to a finial.
        part(
            hex_prism(0.060, 0.064, -0.080, -0.065),
            Hardware::Body,
            origin,
        ),
        part(
            hex_prism(0.040, 0.060, -0.098, -0.080),
            Hardware::Body,
            origin,
        ),
        part(
            hex_prism(0.064, 0.060, 0.065, 0.080),
            Hardware::Body,
            origin,
        ),
        part(
            hex_prism(0.060, 0.024, 0.080, 0.106),
            Hardware::Body,
            origin,
        ),
        part(
            hex_prism(0.011, 0.007, 0.106, 0.128),
            Hardware::Trim,
            origin,
        ),
        // The collar the grip seats into, and the grip's pommel.
        part(
            hex_prism(0.022, 0.026, -0.112, -0.098),
            Hardware::Trim,
            origin,
        ),
        part(
            hex_prism(0.021, 0.024, -POMMEL, -0.222),
            Hardware::Trim,
            origin,
        ),
        part(
            Cylinder::new(0.017, 0.11).mesh().resolution(18).build(),
            Hardware::Grip,
            Transform::from_xyz(0.0, -0.167, 0.0),
        ),
    ];
    for y in [-0.132, -0.167, -0.202] {
        hardware.push(part(
            Torus::new(0.0165, 0.0215).mesh().build(),
            Hardware::Trim,
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }
    // Six bars at the cage's corners, outside the glass.
    for corner in 0..6 {
        #[allow(clippy::cast_precision_loss)]
        let angle = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * corner as f32;
        hardware.push(part(
            hex_prism(0.0055, 0.0055, -0.065, 0.065),
            Hardware::Trim,
            Transform::from_xyz(angle.cos() * 0.057, 0.0, angle.sin() * 0.057),
        ));
    }
    hardware
        .into_iter()
        .chain(std::iter::once((
            meshes.add(hex_prism(0.047, 0.047, -0.065, 0.065)),
            Hardware::Glass,
            origin,
        )))
        .collect()
}

/// One material per opaque hardware finish.
pub(super) fn finishes(
    materials: &mut Assets<StandardMaterial>,
) -> BTreeMap<Hardware, Handle<StandardMaterial>> {
    [Hardware::Body, Hardware::Trim, Hardware::Grip]
        .into_iter()
        .map(|part| {
            let finish = finish(part);
            (
                part,
                materials.add(StandardMaterial {
                    base_color: finish.base_color,
                    metallic: finish.metallic,
                    perceptual_roughness: finish.roughness,
                    ..default()
                }),
            )
        })
        .collect()
}

/// One lantern: carried (by whom, and whether that is the local player, whose core
/// follows the light it casts), set down at a threshold, or waiting in a cache.
pub(super) fn spawn_caged_lantern(
    commands: &mut Commands,
    assets: &LanternVisualAssets,
    visual: LanternVisual,
    transform: Transform,
    held_by: Option<(PlayerId, bool)>,
) {
    let (accent, core) = match held_by {
        Some((_, true)) => (assets.held_cage.clone(), assets.held_core.clone()),
        Some((_, false)) => (assets.held_cage.clone(), assets.guide.clone()),
        None => (assets.cage.clone(), assets.guide.clone()),
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
            root.spawn((
                Mesh3d(assets.core.clone()),
                MeshMaterial3d(core),
                Visibility::Inherited,
                Transform::IDENTITY,
                bevy::light::NotShadowCaster,
            ));
            // The gyro: the anchor's purple, turning about a tilted axis around the
            // core, so a lantern reads as an instrument that is watching.
            let rest = Quat::from_rotation_x(1.1);
            root.spawn((
                Mesh3d(assets.gyro.clone()),
                MeshMaterial3d(accent.clone()),
                Visibility::Inherited,
                Transform::from_rotation(rest),
                Spin {
                    axis: Vec3::new(0.35, 1.0, 0.2).normalize(),
                    rate: 0.9,
                    rest,
                },
                bevy::light::NotShadowCaster,
            ));
            if let Some(ref authored) = assets.authored {
                root.spawn((WorldAssetRoot(authored.clone()), Visibility::Inherited));
            } else {
                for (mesh, part, at) in &assets.hardware {
                    let material = match part {
                        Hardware::Glass => assets.glass.clone(),
                        part => assets.finishes[part].clone(),
                    };
                    let mut entity = root.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Visibility::Inherited,
                        *at,
                    ));
                    if *part == Hardware::Glass {
                        entity.insert(bevy::light::NotShadowCaster);
                    }
                }
                root.spawn((
                    Mesh3d(assets.accent.clone()),
                    MeshMaterial3d(accent),
                    Visibility::Inherited,
                    Transform::IDENTITY,
                ));
            }
            if held_by.is_none() {
                root.spawn((
                    Mesh3d(assets.plinth.clone()),
                    MeshMaterial3d(assets.finishes[&Hardware::Body].clone()),
                    Visibility::Inherited,
                    Transform::from_xyz(0.0, -POMMEL, 0.0),
                ));
            }
            if let Some((owner, _)) = held_by {
                root.spawn((
                    LanternCoreLight { owner, glow: 1.0 },
                    PointLight {
                        color: observed_style::marker(MarkerRole::NextRoom).base_color,
                        intensity: 35.0,
                        range: 3.2,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::IDENTITY,
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
                    Transform::IDENTITY,
                ));
            }
        });
}

/// The core the local player looks at glows with the light it casts: brighter toward
/// the exit, stuttering with the Guardian.
pub(in crate::hex_wfc) fn sync_core_glow(
    runtime: Res<HexWfcRuntime>,
    assets: Res<LanternVisualAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    core_lights: Query<&LanternCoreLight>,
) {
    let Some(light) = core_lights
        .iter()
        .find(|light| light.owner == runtime.local_player)
    else {
        return;
    };
    if let Some(mut core) = materials.get_mut(&assets.held_core) {
        core.emissive = observed_style::marker(MarkerRole::NextRoom).emissive * light.glow;
    }
}

/// The torch rides in the main hand, low and to the right, canted in toward the
/// middle of the view. Close in, inside the body's own radius, so it never pushes
/// into a wall the body is standing against.
pub(in crate::hex_wfc) const HAND: Hand = Hand {
    offset: Vec3::new(0.13, -0.095, -0.20),
    roll: 0.16,
    tip: -0.3,
    scale: 0.40,
};

pub(super) fn held_pose(
    runtime: &HexWfcRuntime,
    sway: &HeldSway,
    player: &observed_match::hex_wfc::HexPlayerState,
) -> Transform {
    held_transform(player, sway_for(runtime, sway, player), &HAND)
}

/// How brightly the carried core glows, as a share of its full signal: never below
/// the signal tier's lower half, full near the exit, and dipping with each stutter.
pub(super) fn core_glow(guide: f32, pulse: f32) -> f32 {
    (0.55 + guide.clamp(0.0, 1.0) * 0.45) * pulse
}

#[cfg(test)]
mod tests {
    use super::core_glow;

    /// The core the player reads the exit from never goes dark, is brightest at the
    /// exit, and still stutters with the Guardian.
    #[test]
    fn the_carried_core_glows_toward_the_exit_and_stutters_with_the_guardian() {
        assert!((core_glow(0.0, 1.0) - 0.55).abs() < 1e-6);
        assert!((core_glow(1.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(core_glow(1.0, 0.38) < core_glow(0.0, 1.0));
        assert!(core_glow(2.0, 1.0) <= 1.0, "guide is clamped");
    }
}
