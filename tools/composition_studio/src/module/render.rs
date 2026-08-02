//! Draw the selected module, and light up whatever is wrong with it.

use bevy::prelude::*;
use observed_hex::{CORNERS, HexFace, TILE_LEVEL_HEIGHT, face_edge};
use observed_style::{SchematicRole, schematic};
use observed_traversal::ConvexRenderMesh;

use crate::module::app::ModuleState;
use crate::module::diagnose::Highlight;

/// `CORNERS` and `face_edge` are integer plan-view metres - the quantized
/// hexagon's whole point is that its corners are exact - so every use here goes
/// through one conversion rather than scattering `as f32` casts that would each
/// be a place to get the axis mapping wrong.
#[must_use]
fn plan(point: (i32, i32), y: f32) -> Vec3 {
    #[allow(clippy::cast_precision_loss)]
    Vec3::new(point.0 as f32, y, point.1 as f32)
}

/// Everything this system owns, despawned wholesale on rebuild.
#[derive(Component)]
pub struct ModuleVisual;

/// Radius of the marker dropped on an offending vertex, in metres.
///
/// Small enough to read as a point on a 14 m cell, large enough to find at the
/// default zoom. A vertex error is the one the author cannot locate by eye.
const VERTEX_MARKER_RADIUS: f32 = 0.35;

/// How far outside the hull the highlight sits, so it reads as an annotation
/// rather than as geometry the module actually contains.
const HIGHLIGHT_LIFT: f32 = 0.06;

/// Rebuild when the selection or the corpus moved.
pub fn rebuild_module_view(
    mut commands: Commands,
    mut state: ResMut<ModuleState>,
    existing: Query<Entity, With<ModuleVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let detent = state.detent;
    let Some(diagnosis) = state.current() else {
        return;
    };

    // Clean modules draw in the settled colour, failing ones in the alert
    // colour, so the verdict is legible before a word is read.
    let role = if diagnosis.is_clean() {
        SchematicRole::Pinned
    } else {
        SchematicRole::Volatile
    };
    let body = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Grid).base_color.with_alpha(0.85),
        perceptual_roughness: 0.85,
        ..default()
    });
    let marker = materials.add(StandardMaterial {
        base_color: schematic(role).base_color,
        emissive: schematic(role).emissive,
        unlit: true,
        ..default()
    });

    // A key light, off the view axis, so the hull faces take different values.
    // Ambient alone renders a module as one flat silhouette - which is exactly
    // what the first capture of this tool showed, and useless for judging
    // geometry.
    let bearing = crate::viewport::detent_bearing(detent);
    let key_from = Quat::from_rotation_y(crate::draw::KEY_LIGHT_OFFSET)
        * Vec3::new(bearing.x, 0.0, bearing.y)
        + Vec3::Y * 1.15;
    commands.spawn((
        DirectionalLight {
            illuminance: crate::draw::KEY_ILLUMINANCE,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(key_from * 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        ModuleVisual,
    ));

    if let Some(prototype) = diagnosis.prototype.as_ref() {
        for (index, hull) in prototype.hulls.iter().enumerate() {
            let Some(render) = ConvexRenderMesh::from_convex_hull(hull) else {
                continue;
            };
            let Some(mesh) = crate::detail::mesh_from(&render) else {
                continue;
            };
            // The one hull a `DegenerateBrush` names is drawn in the alert
            // colour; everything else stays neutral so the marked one is the
            // only thing competing for attention.
            let material = if diagnosis.highlight == Highlight::Hull(index) {
                marker.clone()
            } else {
                body.clone()
            };
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                ModuleVisual,
            ));
        }
    }

    // The cell outline is always drawn: it is the contract the module is being
    // judged against, and a footprint violation is only legible against it.
    let levels = diagnosis
        .prototype
        .as_ref()
        .map_or(1, |prototype| prototype.levels.max(1));
    spawn_cell_outline(&mut commands, &mut meshes, &mut materials, levels);

    spawn_highlight(&mut commands, &mut meshes, &marker, diagnosis.highlight);
}

/// The canonical hex prism, as edges. Drawn from `observed_hex::CORNERS` so it
/// is the same boundary the validator tests against, not an approximation of
/// it.
fn spawn_cell_outline(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    levels: u8,
) {
    let outline = materials.add(StandardMaterial {
        base_color: schematic(SchematicRole::Grid).base_color.with_alpha(0.5),
        emissive: schematic(SchematicRole::Grid).emissive * 0.25,
        unlit: true,
        ..default()
    });
    let top = TILE_LEVEL_HEIGHT * f32::from(levels);
    for level in 0..=u32::from(levels) {
        let y = TILE_LEVEL_HEIGHT * level as f32;
        for index in 0..CORNERS.len() {
            let a = CORNERS[index];
            let b = CORNERS[(index + 1) % CORNERS.len()];
            spawn_edge(commands, meshes, &outline, plan(a, y), plan(b, y));
        }
    }
    for corner in CORNERS {
        spawn_edge(
            commands,
            meshes,
            &outline,
            plan(corner, 0.0),
            plan(corner, top),
        );
    }
}

/// Put the annotation where the fault is.
fn spawn_highlight(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    highlight: Highlight,
) {
    match highlight {
        Highlight::Vertex(point) => {
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(VERTEX_MARKER_RADIUS).mesh().ico(2).unwrap())),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(point),
                ModuleVisual,
            ));
        }
        Highlight::Face { face, .. } => {
            // Lateral faces get their edge drawn full height; up/down get the
            // whole cap ring, because "the top face" has no single edge.
            if let Some([a, b]) = lateral_edge(face) {
                for level in 0..2 {
                    let y = TILE_LEVEL_HEIGHT * level as f32 + HIGHLIGHT_LIFT;
                    spawn_edge(commands, meshes, material, plan(a, y), plan(b, y));
                }
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(a, 0.0),
                    plan(a, TILE_LEVEL_HEIGHT),
                );
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(b, 0.0),
                    plan(b, TILE_LEVEL_HEIGHT),
                );
            } else {
                let y = if face == HexFace::Up {
                    TILE_LEVEL_HEIGHT
                } else {
                    0.0
                } + HIGHLIGHT_LIFT;
                for index in 0..CORNERS.len() {
                    let a = CORNERS[index];
                    let b = CORNERS[(index + 1) % CORNERS.len()];
                    spawn_edge(commands, meshes, material, plan(a, y), plan(b, y));
                }
            }
        }
        Highlight::Cell(_) => {
            // Single-cell view, so the cell reference is always the one on
            // screen: ring its floor rather than pretending to offset it.
            for index in 0..CORNERS.len() {
                let a = CORNERS[index];
                let b = CORNERS[(index + 1) % CORNERS.len()];
                spawn_edge(
                    commands,
                    meshes,
                    material,
                    plan(a, HIGHLIGHT_LIFT),
                    plan(b, HIGHLIGHT_LIFT),
                );
            }
        }
        // Already handled by recolouring the hull itself.
        Highlight::Hull(_) => {}
        // Nothing in the geometry to point at; the panel carries the message.
        Highlight::Whole => {}
    }
}

/// The two plan-space corners of a lateral face, or `None` for up/down.
#[must_use]
pub fn lateral_edge(face: HexFace) -> Option<[(i32, i32); 2]> {
    face.is_lateral().then(|| face_edge(face))
}

/// A thin box standing in for a line. Bevy has no thick-line primitive, and a
/// gizmo would not survive a screenshot capture.
fn spawn_edge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    from: Vec3,
    to: Vec3,
) {
    let delta = to - from;
    let length = delta.length();
    if length < f32::EPSILON {
        return;
    }
    let mesh = meshes.add(Cuboid::new(0.08, 0.08, length).mesh());
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(from + delta * 0.5).looking_to(delta.normalize(), Vec3::Y),
        ModuleVisual,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every lateral face must resolve to a real edge, and the caps must not
    /// pretend to have one. A silent `None` on a lateral face would drop the
    /// highlight for exactly the ports authors get wrong most.
    #[test]
    fn every_lateral_face_has_an_edge_and_the_caps_do_not() {
        for face in HexFace::ALL {
            let edge = lateral_edge(face);
            assert_eq!(
                edge.is_some(),
                face.is_lateral(),
                "{face:?} disagrees with its own laterality"
            );
            if let Some([a, b]) = edge {
                assert_ne!(a, b, "{face:?} has a degenerate edge");
            }
        }
    }

    /// The outline is drawn from the same corner table the validator enforces.
    ///
    /// The quantized hexagon is deliberately **not** regular - east and west sit
    /// 7 m out while north and south reach 8 - so this asserts those two spans
    /// rather than a single radius. A regular-hexagon assumption here is exactly
    /// the sub-metre gap the quantization exists to avoid.
    #[test]
    fn the_outline_uses_the_canonical_quantized_hexagon() {
        assert_eq!(CORNERS.len(), 6);
        let widest = CORNERS.iter().map(|corner| corner.0.abs()).max();
        let tallest = CORNERS.iter().map(|corner| corner.1.abs()).max();
        assert_eq!(widest, Some(7), "across flats must stay 14 m");
        assert_eq!(tallest, Some(8), "across corners must stay 16 m");
    }

    /// The plan conversion maps editor axes to world axes, not to themselves.
    /// Swapping y and z here would draw every outline rotated flat.
    #[test]
    fn plan_puts_the_hexagon_in_the_ground_plane() {
        let point = plan((7, -4), 2.5);
        assert!((point.x - 7.0).abs() < 1e-6);
        assert!((point.y - 2.5).abs() < 1e-6, "height must be Y");
        assert!((point.z + 4.0).abs() < 1e-6, "plan depth must be Z");
    }
}
