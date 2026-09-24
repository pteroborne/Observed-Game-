//! Which merged mesh a projected piece joins: its part first (open-edge pieces each
//! have their own), then its role, then for halls and rooms which surface of the
//! cell it is.

use bevy::prelude::*;
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece, HexStructureRole};
use observed_traversal::ColliderShape;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::hex_wfc) enum MeshGroupKey {
    Floor,
    Ceiling,
    Interior,
    Perimeter(u8),
    Ramp,
    Shaft,
    Boundary,
    /// The lit lip of an open edge.
    Lip,
    /// An open edge's railing.
    Rail,
    /// A walkway deck.
    Walkway,
    /// The truss under a walkway.
    Truss,
    /// A railing's guard: collided with, never drawn.
    Hidden,
    /// The far-field skin's storey faces (`exterior`).
    Facade,
    /// The far-field skin's roofs.
    Roof,
    /// Lit window slits in the far-field skin: a district's practical light.
    Window,
}

impl MeshGroupKey {
    pub(in crate::hex_wfc) fn for_piece(piece: &HexStructurePiece) -> Self {
        match piece.part {
            HexPiecePart::Authored => {}
            HexPiecePart::Lip => return Self::Lip,
            HexPiecePart::Rail => return Self::Rail,
            HexPiecePart::Walkway => return Self::Walkway,
            HexPiecePart::Truss => return Self::Truss,
            HexPiecePart::Guard => return Self::Hidden,
        }
        match piece.role {
            HexStructureRole::Ramp => Self::Ramp,
            HexStructureRole::Shaft => Self::Shaft,
            HexStructureRole::Boundary => Self::Boundary,
            HexStructureRole::Room | HexStructureRole::Hall => {
                let points = match &piece.shape {
                    ColliderShape::ConvexHull { points } => points.as_slice(),
                    ColliderShape::Cuboid { .. } => return Self::Interior,
                };
                if observed_traversal::render_mesh::is_overhead_slab(points) {
                    Self::Ceiling
                } else if observed_traversal::render_mesh::is_horizontal_slab(points) {
                    Self::Floor
                } else {
                    let centroid = if points.is_empty() {
                        Vec3::ZERO
                    } else {
                        #[allow(clippy::cast_precision_loss)]
                        let sum: Vec3 = points.iter().copied().sum();
                        #[allow(clippy::cast_precision_loss)]
                        let c = sum / points.len() as f32;
                        c
                    };
                    let plan = Vec2::new(centroid.x, centroid.z);
                    if plan.length() < 0.5 {
                        Self::Interior
                    } else {
                        let mut best_face = 0u8;
                        let mut best_dot = f32::NEG_INFINITY;
                        for face in observed_hex::HexFace::LATERAL {
                            let [(ax, az), (bx, bz)] = observed_hex::metrics::face_edge(face);
                            let mid = Vec2::new((ax + bx) as f32 * 0.5, (az + bz) as f32 * 0.5);
                            let dot = plan.dot(mid);
                            if dot > best_dot {
                                best_dot = dot;
                                best_face = face as u8;
                            }
                        }
                        Self::Perimeter(best_face)
                    }
                }
            }
        }
    }
}
