//! Persistent portal plates and presentation-only transit effects.
//!
//! Plate placement and link ownership remain in `HexPadState`. This module only
//! mirrors that state into procedural geometry and turns a `PadTraversed` event
//! into a short, reviewable animation. The logical runner is already at the
//! destination while these rings play; presentation can never delay or alter a
//! simulation result.

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_core::{EquipmentId, PlayerId};
use observed_hex::HexCoord;
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};

use crate::LabState;
use crate::sim::unit::PLAYER_TEAM;

use super::board::{cell_origin, draws_level, line_material};
use super::{CellPaint, paint};

pub const TELEPORT_SECONDS: f32 = 1.15;
const PLATE_LIFT: f32 = 0.32;
const RING_COUNT: usize = 6;

/// The one transit presentation currently holding player input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportPresentation {
    pub unit: PlayerId,
    pub from: HexCoord,
    pub to: HexCoord,
    pub elapsed: f32,
    spawned: bool,
    switched_level: bool,
}

impl TeleportPresentation {
    #[must_use]
    pub const fn new(unit: PlayerId, from: HexCoord, to: HexCoord) -> Self {
        Self {
            unit,
            from,
            to,
            elapsed: 0.0,
            spawned: false,
            switched_level: false,
        }
    }

    #[must_use]
    pub fn progress(self) -> f32 {
        (self.elapsed / TELEPORT_SECONDS).clamp(0.0, 1.0)
    }
}

#[derive(Component)]
pub struct PadVisual {
    id: EquipmentId,
    linked: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitDirection {
    Depart,
    Arrive,
}

#[derive(Clone, Copy, Debug)]
enum EffectPart {
    Ring {
        direction: TransitDirection,
        delay: f32,
    },
    Beam {
        direction: TransitDirection,
    },
}

#[derive(Component)]
pub struct TeleportEffect {
    cell: HexCoord,
    age: f32,
    part: EffectPart,
    treatment: Treatment,
}

/// Mirror every deployed simulation plate into a low, unmistakable floor
/// assembly. One purple ring means the link is incomplete; a cyan double ring
/// and pulsing practical mean another same-team plate can receive a traversal.
pub fn sync_plates(
    time: Res<Time>,
    state: Res<LabState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Query<(Entity, &mut PadVisual, &mut Transform, &mut Visibility)>,
) {
    let mut retained = BTreeSet::new();
    for (entity, mut visual, mut transform, mut visibility) in &mut visuals {
        let Some(pad) = state.game.pads.deployed.get(&visual.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        let linked = state.game.pads.link_target(pad.team, pad.id).is_some();
        if linked != visual.linked {
            commands.entity(entity).despawn();
            continue;
        }
        visual.linked = linked;
        retained.insert(pad.id);
        transform.translation = cell_origin(state.mode, pad.cell) + Vec3::Y * PLATE_LIFT;
        let pulse =
            (time.elapsed_secs() * if linked { 3.2 } else { 1.4 } + pad.id.0 as f32 * 0.7).sin();
        transform.scale = Vec3::splat(1.0 + pulse * if linked { 0.055 } else { 0.018 });
        transform.rotation =
            Quat::from_rotation_y(time.elapsed_secs() * if linked { 0.22 } else { 0.08 });
        let known = pad.team == PLAYER_TEAM || paint(&state.game, pad.cell) != CellPaint::Unknown;
        *visibility = if known && draws_level(state.mode, pad.cell, state.level) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for pad in state.game.pads.deployed.values() {
        if retained.contains(&pad.id) {
            continue;
        }
        let linked = state.game.pads.link_target(pad.team, pad.id).is_some();
        spawn_plate(
            &mut commands,
            &mut meshes,
            &mut materials,
            pad.id,
            linked,
            cell_origin(state.mode, pad.cell) + Vec3::Y * PLATE_LIFT,
        );
    }
}

fn spawn_plate(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: EquipmentId,
    linked: bool,
    at: Vec3,
) {
    let glow = marker(if linked {
        MarkerRole::You
    } else {
        MarkerRole::Control
    });
    let base_mesh = meshes.add(Cylinder::new(2.35, 0.28));
    let outer_mesh = meshes.add(Torus::new(1.72, 2.16).mesh().build());
    let inner_mesh = meshes.add(Torus::new(0.86, 1.14).mesh().build());
    let base_material = materials.add(line_material(surface(SurfaceRole::Plain), false));
    let outer_material = materials.add(line_material(glow, false));
    let inner_material = materials.add(line_material(
        if linked {
            glow
        } else {
            surface(SurfaceRole::Wall)
        },
        false,
    ));
    commands
        .spawn((
            PadVisual { id, linked },
            Transform::from_translation(at),
            Visibility::Visible,
            Name::new(format!(
                "Portal plate {} ({})",
                id.0,
                if linked { "linked" } else { "unlinked" }
            )),
        ))
        .with_children(|plate| {
            plate.spawn((
                Mesh3d(base_mesh),
                MeshMaterial3d(base_material),
                Transform::IDENTITY,
                Name::new("Portal plate pedestal"),
            ));
            plate.spawn((
                Mesh3d(outer_mesh),
                MeshMaterial3d(outer_material),
                Transform::from_xyz(0.0, 0.19, 0.0),
                Name::new("Portal plate outer glyph"),
            ));
            plate.spawn((
                Mesh3d(inner_mesh),
                MeshMaterial3d(inner_material),
                Transform::from_xyz(0.0, 0.24, 0.0),
                Name::new(if linked {
                    "Portal plate linked core"
                } else {
                    "Portal plate incomplete core"
                }),
            ));
            if linked {
                plate.spawn((
                    PointLight {
                        color: glow.base_color,
                        intensity: 7_500.0,
                        range: 11.0,
                        radius: 1.1,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 2.4, 0.0),
                    Name::new("Linked portal practical"),
                ));
            }
        });
}

/// Start effects once, advance the presentation clock, and change decks at the
/// hidden midpoint for cross-floor links.
pub fn advance(
    time: Res<Time>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut transit) = state.teleport else {
        return;
    };
    if !transit.spawned {
        spawn_transit_effect(
            &mut commands,
            &mut meshes,
            &mut materials,
            transit.from,
            TransitDirection::Depart,
        );
        spawn_transit_effect(
            &mut commands,
            &mut meshes,
            &mut materials,
            transit.to,
            TransitDirection::Arrive,
        );
        transit.spawned = true;
    }
    transit.elapsed += time.delta_secs();
    if !transit.switched_level && transit.progress() >= 0.5 {
        transit.switched_level = true;
        if state.level != transit.to.level {
            state.level = transit.to.level;
            state.dirty = true;
            state.overlay_dirty = true;
        }
    }
    state.teleport = (transit.elapsed < TELEPORT_SECONDS).then_some(transit);
}

fn spawn_transit_effect(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cell: HexCoord,
    direction: TransitDirection,
) {
    let treatment = marker(MarkerRole::You);
    let ring_mesh = meshes.add(Torus::new(1.72, 2.04).mesh().build());
    for index in 0..RING_COUNT {
        let delay = index as f32 * 0.026;
        commands.spawn((
            TeleportEffect {
                cell,
                age: 0.0,
                part: EffectPart::Ring { direction, delay },
                treatment,
            },
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(materials.add(effect_material(treatment, 0.0))),
            Transform::from_scale(Vec3::splat(0.01)),
            Visibility::Visible,
            Name::new(format!("Portal transit {direction:?} ring {index}")),
        ));
    }
    commands.spawn((
        TeleportEffect {
            cell,
            age: 0.0,
            part: EffectPart::Beam { direction },
            treatment,
        },
        Mesh3d(meshes.add(Cylinder::new(1.8, 8.5))),
        MeshMaterial3d(materials.add(effect_material(treatment, 0.0))),
        Transform::from_scale(Vec3::splat(0.01)),
        Visibility::Visible,
        Name::new(format!("Portal transit {direction:?} light column")),
    ));
}

fn effect_material(treatment: Treatment, alpha: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color.with_alpha(alpha),
        emissive: treatment.emissive * alpha,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}

/// Animate the rising source rings and descending destination rings. The
/// staggered stack is deliberately architectural rather than explosive: it
/// reads as machinery lifting the runner through a temporary light column.
pub fn animate_effects(
    time: Res<Time>,
    state: Res<LabState>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut effects: Query<(
        Entity,
        &mut TeleportEffect,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut effect, mut transform, mut visibility, material) in &mut effects {
        effect.age += time.delta_secs();
        if effect.age >= TELEPORT_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = (effect.age / TELEPORT_SECONDS).clamp(0.0, 1.0);
        let origin = cell_origin(state.mode, effect.cell);
        let (height, scale, alpha) = match effect.part {
            EffectPart::Ring { direction, delay } => ring_sample(direction, progress, delay),
            EffectPart::Beam { direction } => beam_sample(direction, progress),
        };
        transform.translation = origin + Vec3::Y * height;
        transform.scale = Vec3::new(scale, scale.max(0.01), scale);
        *visibility = if draws_level(state.mode, effect.cell, state.level) && alpha > 0.001 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(material) = materials.get_mut(&material.0).as_deref_mut() {
            material.base_color = effect.treatment.base_color.with_alpha(alpha * 0.58);
            material.emissive = effect.treatment.emissive * alpha;
        }
    }
}

fn phase(progress: f32, start: f32, duration: f32) -> f32 {
    ((progress - start) / duration).clamp(0.0, 1.0)
}

fn ring_sample(direction: TransitDirection, progress: f32, delay: f32) -> (f32, f32, f32) {
    let (start, rising) = match direction {
        TransitDirection::Depart => (delay, true),
        TransitDirection::Arrive => (0.54 + delay, false),
    };
    let local = phase(progress, start, 0.42);
    let active = progress >= start && progress <= start + 0.42;
    let eased = local * local * (3.0 - 2.0 * local);
    let height = if rising {
        0.5 + eased * 7.4
    } else {
        7.9 - eased * 7.4
    };
    let scale = if rising {
        1.16 - eased * 0.60
    } else {
        0.56 + eased * 0.60
    };
    let alpha = if active {
        (std::f32::consts::PI * local).sin().max(0.0)
    } else {
        0.0
    };
    (height, scale, alpha)
}

fn beam_sample(direction: TransitDirection, progress: f32) -> (f32, f32, f32) {
    let local = match direction {
        TransitDirection::Depart => phase(progress, 0.02, 0.42),
        TransitDirection::Arrive => phase(progress, 0.56, 0.42),
    };
    let active = match direction {
        TransitDirection::Depart => (0.02..=0.44).contains(&progress),
        TransitDirection::Arrive => (0.56..=0.98).contains(&progress),
    };
    let alpha = if active {
        (std::f32::consts::PI * local).sin().max(0.0) * 0.36
    } else {
        0.0
    };
    (4.45, 1.0, alpha)
}

/// Runner placement during transit: lift and contract at the source, remain
/// hidden across the cut, then descend and recover scale at the destination.
#[must_use]
pub fn runner_sample(transit: TeleportPresentation) -> (HexCoord, f32, f32) {
    let progress = transit.progress();
    if progress < 0.42 {
        let local = phase(progress, 0.0, 0.42);
        let eased = local * local * (3.0 - 2.0 * local);
        (transit.from, eased * 3.8, 1.0 - eased * 0.86)
    } else if progress < 0.58 {
        (transit.to, 3.8, 0.02)
    } else {
        let local = phase(progress, 0.58, 0.42);
        let eased = local * local * (3.0 - 2.0 * local);
        (transit.to, (1.0 - eased) * 3.8, 0.14 + eased * 0.86)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transit(progress: f32) -> TeleportPresentation {
        let mut transit = TeleportPresentation::new(
            PlayerId(2),
            HexCoord {
                q: 1,
                r: 2,
                level: 0,
            },
            HexCoord {
                q: 7,
                r: 3,
                level: 2,
            },
        );
        transit.elapsed = TELEPORT_SECONDS * progress;
        transit
    }

    #[test]
    fn runner_disappears_between_source_and_destination() {
        let (cell, _, scale) = runner_sample(transit(0.0));
        assert_eq!(cell, transit(0.0).from);
        assert_eq!(scale, 1.0);
        let (cell, _, scale) = runner_sample(transit(0.5));
        assert_eq!(cell, transit(0.5).to);
        assert!(scale < 0.05);
        let (cell, lift, scale) = runner_sample(transit(1.0));
        assert_eq!(cell, transit(1.0).to);
        assert!(lift <= f32::EPSILON);
        assert!((scale - 1.0).abs() <= f32::EPSILON);
    }

    #[test]
    fn source_rings_rise_and_destination_rings_descend() {
        let source_low = ring_sample(TransitDirection::Depart, 0.05, 0.0).0;
        let source_high = ring_sample(TransitDirection::Depart, 0.35, 0.0).0;
        assert!(source_high > source_low);
        let destination_high = ring_sample(TransitDirection::Arrive, 0.58, 0.0).0;
        let destination_low = ring_sample(TransitDirection::Arrive, 0.90, 0.0).0;
        assert!(destination_low < destination_high);
    }
}
