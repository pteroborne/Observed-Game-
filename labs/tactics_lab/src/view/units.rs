//! Persistent tactical projections of the production runner silhouette.
//!
//! Logical positions remain discrete hex cells. This presentation layer eases
//! between those authoritative cells and adds a small idle hover; neither
//! interpolation nor bobbing can affect navigation, observation, or replay.

use bevy::prelude::*;
use observed_core::PlayerId;

use crate::LabState;
use crate::sim::unit::PLAYER_TEAM;

use super::board::{cell_origin, draws_level, line_material};
use super::{ViewMode, unit_marker};

const HOVER_HEIGHT: f32 = 5.0;
const HOVER_AMPLITUDE: f32 = 0.42;
const MOVE_SECONDS: f32 = 0.30;

#[derive(Component)]
pub struct UnitVisual {
    id: PlayerId,
    from: Vec3,
    target: Vec3,
    progress: f32,
    mode: ViewMode,
    level: u8,
    selected: bool,
}

fn marker_offset(id: PlayerId, rival: bool) -> Vec3 {
    if rival {
        Vec3::ZERO
    } else {
        let angle = f32::from(id.0) * std::f32::consts::TAU / 6.0;
        Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.8
    }
}

fn target(mode: ViewMode, cell: observed_hex::HexCoord, id: PlayerId, rival: bool) -> Vec3 {
    cell_origin(mode, cell) + marker_offset(id, rival) + Vec3::Y * HOVER_HEIGHT
}

/// Spawn missing projections, follow bot/manual nav one cell at a time, and
/// keep the runners hovering while idle.
pub fn sync(
    time: Res<Time>,
    state: Res<LabState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Query<(
        Entity,
        &mut UnitVisual,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let existing = visuals
        .iter()
        .map(|(_, visual, _, _, _)| visual.id)
        .collect::<std::collections::BTreeSet<_>>();

    for unit in state.game.units.values() {
        if existing.contains(&unit.id) {
            continue;
        }
        let rival = unit.team != PLAYER_TEAM;
        let selected = state.selected == Some(unit.id);
        let at = target(state.mode, unit.cell, unit.id, rival);
        let treatment = unit_marker(selected, rival);
        commands.spawn((
            UnitVisual {
                id: unit.id,
                from: at,
                target: at,
                progress: 1.0,
                mode: state.mode,
                level: state.level,
                selected,
            },
            // The assembled hex game uses this same compact capsule for remote
            // runners. The lab scales it up because its camera sits much higher.
            Mesh3d(meshes.add(Capsule3d::new(0.25, 0.8))),
            MeshMaterial3d(materials.add(line_material(treatment, false))),
            Transform::from_translation(at),
            Visibility::Hidden,
            Name::new(format!("Floating runner {}", unit.id.0)),
        ));
    }

    let elapsed = time.elapsed_secs();
    for (entity, mut visual, mut transform, mut visibility, mut material) in &mut visuals {
        let Some(unit) = state.game.units.get(&visual.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        let rival = unit.team != PLAYER_TEAM;
        let shown = !unit.escaped
            && draws_level(state.mode, unit.cell, state.level)
            && (!rival || state.game.observation.visible_cells.contains(&unit.cell));
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        let next = target(state.mode, unit.cell, unit.id, rival);
        if visual.mode != state.mode || visual.level != state.level {
            visual.from = next;
            visual.target = next;
            visual.progress = 1.0;
            visual.mode = state.mode;
            visual.level = state.level;
        } else if visual.target.distance_squared(next) > 0.001 {
            let t = visual.progress.clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            visual.from = visual.from.lerp(visual.target, eased);
            visual.target = next;
            visual.progress = 0.0;
        }
        visual.progress = (visual.progress + time.delta_secs() / MOVE_SECONDS).min(1.0);
        let t = visual.progress;
        let eased = t * t * (3.0 - 2.0 * t);
        let bob = (elapsed * 2.1 + f32::from(unit.id.0) * 1.7).sin() * HOVER_AMPLITUDE;
        transform.translation = visual.from.lerp(visual.target, eased) + Vec3::Y * bob;

        let selected = state.selected == Some(unit.id);
        if visual.selected != selected {
            visual.selected = selected;
            material.0 = materials.add(line_material(unit_marker(selected, rival), false));
        }
        transform.scale = if selected {
            Vec3::splat(2.65)
        } else {
            Vec3::splat(2.3)
        };
    }
}
