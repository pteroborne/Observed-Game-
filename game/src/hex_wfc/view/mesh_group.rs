//! Which merged mesh a projected piece joins: its part first (open-edge pieces each
//! have their own), then its role, then for halls and rooms which surface of the
//! cell it is.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece, HexStructureRole};
use observed_traversal::ColliderShape;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::hex_wfc) enum MeshGroupKey {
    Floor,
    Ceiling,
    Interior,
    Perimeter(u8),
    /// A ramp's or a stair tower's faces that point one way: drawn in the district's
    /// floor, wall or ceiling by which way that is, so a flight is walked on floor
    /// and a landing's underside is ceiling, whatever the hull it belongs to.
    Climb(Facing),
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

/// Which way a face points, for choosing its surface.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::hex_wfc) enum Facing {
    /// Something to walk on: a floor, a flight, a landing, a balcony.
    Up,
    /// A wall, a balustrade, a pier, the side of a slab.
    Side,
    /// An underside: a ceiling, the soffit of a flight or a balcony.
    Down,
}

impl Facing {
    pub(in crate::hex_wfc) const ALL: [Self; 3] = [Self::Up, Self::Side, Self::Down];

    /// Whether a face with this unit normal points this way. Up to 50 degrees off
    /// level is a surface to walk on - every flight in the corpus is under 36 - and
    /// the same off downward is an underside.
    pub(in crate::hex_wfc) fn holds(self, normal: Vec3) -> bool {
        const LEVEL: f32 = 0.64;
        match self {
            Self::Up => normal.y >= LEVEL,
            Self::Down => normal.y <= -LEVEL,
            Self::Side => normal.y.abs() < LEVEL,
        }
    }
}

impl MeshGroupKey {
    pub(in crate::hex_wfc) fn for_piece(piece: &HexStructurePiece) -> Self {
        match piece.part {
            // A window's frame is the wall it was cut from, drawn as that wall.
            HexPiecePart::Authored | HexPiecePart::Window => {}
            HexPiecePart::Lip => return Self::Lip,
            HexPiecePart::Rail => return Self::Rail,
            HexPiecePart::Walkway => return Self::Walkway,
            HexPiecePart::Truss => return Self::Truss,
            HexPiecePart::Guard | HexPiecePart::Glazing => return Self::Hidden,
        }
        match piece.role {
            // Split by facing when the meshes are built; the key only gathers them.
            HexStructureRole::Climb => Self::Climb(Facing::Side),
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

/// One merged mesh's worth of a cell's hulls, and their extent.
#[derive(Clone)]
pub(in crate::hex_wfc) struct MergedGroup<'a> {
    pub hulls: Vec<&'a [Vec3]>,
    pub min_y: f32,
    pub max_y: f32,
    pub centroid_sum: Vec3,
    pub point_count: usize,
}

/// A cell's pieces gathered into the meshes they are drawn as. A climb's pieces are
/// gathered as one group and drawn as three, by which way each face points.
pub(in crate::hex_wfc) fn gather<'a>(
    pieces: &[&'a HexStructurePiece],
) -> Vec<(MeshGroupKey, MergedGroup<'a>)> {
    let mut groups: BTreeMap<MeshGroupKey, MergedGroup<'a>> = BTreeMap::new();
    for piece in pieces {
        let entry = groups
            .entry(MeshGroupKey::for_piece(piece))
            .or_insert_with(|| MergedGroup {
                hulls: Vec::new(),
                min_y: f32::INFINITY,
                max_y: f32::NEG_INFINITY,
                centroid_sum: Vec3::ZERO,
                point_count: 0,
            });
        if let ColliderShape::ConvexHull { points } = &piece.shape {
            entry.hulls.push(points.as_slice());
            for &pt in points {
                entry.min_y = entry.min_y.min(pt.y);
                entry.max_y = entry.max_y.max(pt.y);
                entry.centroid_sum += pt;
                entry.point_count += 1;
            }
        }
    }
    groups
        .into_iter()
        .flat_map(|(key, group)| {
            let keys = match key {
                MeshGroupKey::Climb(_) => Facing::ALL.map(MeshGroupKey::Climb).to_vec(),
                other => vec![other],
            };
            keys.into_iter().map(move |key| (key, group.clone()))
        })
        .collect()
}
