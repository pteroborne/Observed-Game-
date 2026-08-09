//! Put the annotation where the fault is.
//!
//! Split from `render.rs` so neither file outgrows the 600-line review budget
//! the rest of this tool lives under. The division is by job: `render.rs` draws
//! the module, this draws the one thing that is wrong with it.

use bevy::prelude::*;
use observed_hex::{CORNERS, HexFace, TILE_LEVEL_HEIGHT, face_edge};

use crate::module::diagnose::Highlight;
use crate::module::render::{ModuleVisual, plan, spawn_edge};

/// Radius of the marker dropped on an offending vertex, in metres.
///
/// Small enough to read as a point on a 14 m cell, large enough to find at the
/// default zoom. A vertex error is the one the author cannot locate by eye.
const VERTEX_MARKER_RADIUS: f32 = 0.35;

/// How far outside the hull the highlight sits, so it reads as an annotation
/// rather than as geometry the module actually contains.
const HIGHLIGHT_LIFT: f32 = 0.06;

/// Put the annotation where the fault is.
pub(super) fn spawn_highlight(
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
}
