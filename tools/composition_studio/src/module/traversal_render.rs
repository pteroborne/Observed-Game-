//! Presentation of declared traversal guides and optional physical audit traces.

use bevy::prelude::*;
use observed_style::{SchematicRole, schematic};

use super::rapier_audit::RapierAuditState;
use super::render::{ModuleVisual, spawn_edge};

pub(super) fn spawn_traversal_evidence(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    guide: Option<&observed_match::hex_wfc::ProjectedTraversalGuide>,
    audit: &RapierAuditState,
) {
    if let Some(guide) = guide {
        spawn_guide(commands, meshes, materials, guide);
    }
    if let RapierAuditState::Complete(report) = audit {
        spawn_rapier_audit(commands, meshes, materials, report);
    }
}

/// Draw the declared, identity-projected compatibility annotation.
fn spawn_guide(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    guide: &observed_match::hex_wfc::ProjectedTraversalGuide,
) {
    const LIFT: f32 = 0.18;
    let material = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Selected).base_color,
        emissive: schematic(SchematicRole::Selected).emissive,
        unlit: true,
        ..default()
    });
    for nodes in [
        guide.climb.as_ref().map(|spine| spine.nodes.as_slice()),
        guide.deck.as_ref().map(|deck| deck.nodes.as_slice()),
    ]
    .into_iter()
    .flatten()
    {
        for pair in nodes.windows(2) {
            spawn_edge(
                commands,
                meshes,
                &material,
                pair[0] + Vec3::Y * LIFT,
                pair[1] + Vec3::Y * LIFT,
            );
        }
    }
}

/// Draw actual body traces plus the final follower target. A failed target edge
/// is volatile; no graph cursor or binding is implied by this stateless state.
fn spawn_rapier_audit(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    report: &super::rapier_audit::RapierAuditReport,
) {
    const LIFT: f32 = 0.24;
    let trace_material = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Pinned).base_color,
        emissive: schematic(SchematicRole::Pinned).emissive,
        unlit: true,
        ..default()
    });
    let target_material = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Selected).base_color,
        emissive: schematic(SchematicRole::Selected).emissive,
        unlit: true,
        ..default()
    });
    let failed_material = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Volatile).base_color,
        emissive: schematic(SchematicRole::Volatile).emissive,
        unlit: true,
        ..default()
    });

    for leg in &report.legs {
        for pair in leg.trace.windows(2) {
            spawn_edge(
                commands,
                meshes,
                &trace_material,
                pair[0] + Vec3::Y * LIFT,
                pair[1] + Vec3::Y * LIFT,
            );
        }
        if let Some(target) = leg.last_target {
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.18).mesh().ico(2).unwrap())),
                MeshMaterial3d(target_material.clone()),
                Transform::from_translation(target + Vec3::Y * LIFT),
                ModuleVisual,
            ));
            if leg.failure.is_some() {
                spawn_edge(
                    commands,
                    meshes,
                    &failed_material,
                    leg.final_feet + Vec3::Y * LIFT,
                    target + Vec3::Y * LIFT,
                );
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.30).mesh().ico(2).unwrap())),
                    MeshMaterial3d(failed_material.clone()),
                    Transform::from_translation(leg.final_feet + Vec3::Y * LIFT),
                    ModuleVisual,
                ));
            }
        }
    }
}
