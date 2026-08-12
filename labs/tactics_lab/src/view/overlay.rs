//! Pointer and route overlays. These are deliberately separate from the solid
//! board so moving a mouse never triangulates authored geometry.

use bevy::prelude::*;
use observed_schematic::{LineBatch, floor_ring};
use observed_style::{TacticsRole, Treatment, tactics};

use crate::LabState;
use crate::sim::unit::PLAYER_TEAM;

use super::board::cell_origin;
use super::{BoardOverlay, ViewMode};

const OVERLAY_LIFT: f32 = 0.8;

pub fn rebuild(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    existing: Query<Entity, With<BoardOverlay>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.overlay_dirty {
        return;
    }
    state.overlay_dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let mut route = LineBatch::default();
    let mut limit = LineBatch::default();
    let mut target = LineBatch::default();

    if let Some(preview) = state.preview.as_ref() {
        for step in preview.executable_steps() {
            add_ring(&mut route, state.mode, step.to, 0.78, OVERLAY_LIFT);
        }
        if !preview.reaches_destination || preview.affordable_steps < preview.steps.len() {
            add_ring(
                &mut limit,
                state.mode,
                preview.stopping_cell,
                0.92,
                OVERLAY_LIFT + 0.12,
            );
        }
    }
    if let Some(cell) = state.hovered {
        add_ring(&mut target, state.mode, cell, 0.96, OVERLAY_LIFT + 0.24);
    }

    spawn_batch(
        &mut commands,
        &mut meshes,
        &mut materials,
        route,
        tactics(TacticsRole::ReachableRoute),
        "Affordable route",
    );
    spawn_batch(
        &mut commands,
        &mut meshes,
        &mut materials,
        limit,
        tactics(TacticsRole::RouteLimit),
        "Route AP limit",
    );
    let friendly_hover = state.hovered.is_some_and(|cell| {
        state
            .game
            .units
            .values()
            .any(|unit| !unit.escaped && unit.team == PLAYER_TEAM && unit.cell == cell)
    });
    let role = if friendly_hover
        || state
            .preview
            .as_ref()
            .is_some_and(|preview| preview.can_move())
    {
        TacticsRole::Selectable
    } else if state.hovered.is_some() {
        TacticsRole::Blocked
    } else {
        TacticsRole::Selectable
    };
    spawn_batch(
        &mut commands,
        &mut meshes,
        &mut materials,
        target,
        tactics(role),
        "Pointer target",
    );
}

fn add_ring(
    batch: &mut LineBatch,
    mode: ViewMode,
    cell: observed_hex::HexCoord,
    inset: f32,
    y: f32,
) {
    let origin = cell_origin(mode, cell) + Vec3::Y * y;
    for (from, to) in floor_ring(inset) {
        batch.segment(origin + from, origin + to);
    }
}

fn spawn_batch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    batch: LineBatch,
    treatment: Treatment,
    name: &'static str,
) {
    let Some(mesh) = batch.build() else { return };
    commands.spawn((
        BoardOverlay,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: treatment.base_color,
            emissive: treatment.emissive,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        })),
        Name::new(name),
    ));
}
