//! Deterministic Rapier scenes built from compiled hulls and declared
//! assemblies.
//!
//! A certification scene is pure data: the same instance list always produces
//! the same [`ArenaSpec`], with the same stable collider IDs in the same order.
//! Nothing here consults the filesystem, a clock, or a hash map iteration
//! order, so a certificate can name the exact collision world it proved.

use glam::{Quat, Vec3};
use observed_hex::TILE_LEVEL_HEIGHT;
use observed_traversal::{ArenaSpec, ColliderShape, ColliderSpec, StableColliderId};

/// One placed copy of a module's structural hulls inside a certification
/// scene. `turn` is the same clockwise sixth the catalog applies when it
/// expands rotations, so a stacked case tests geometry the catalog can really
/// produce.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlacedModule {
    pub hulls: Vec<Vec<Vec3>>,
    pub offset: Vec3,
    pub turn: u8,
}

impl PlacedModule {
    pub(crate) fn new(hulls: Vec<Vec<Vec3>>, level: i32, turn: u8) -> Self {
        Self {
            hulls,
            offset: Vec3::Y * (level as f32 * TILE_LEVEL_HEIGHT),
            turn,
        }
    }

    pub(crate) fn rotation(&self) -> Quat {
        Quat::from_rotation_y(-f32::from(self.turn) * std::f32::consts::TAU / 6.0)
    }
}

/// Hulls exactly as the catalog stores them, widened to `Vec3`.
pub(crate) fn hulls_from_compiled(hulls: &[Vec<[f32; 3]>]) -> Vec<Vec<Vec3>> {
    hulls
        .iter()
        .map(|hull| hull.iter().copied().map(Vec3::from_array).collect())
        .collect()
}

/// The collision world for one certification case.
///
/// The safety volume is generous but finite on purpose: leaving it is the
/// controller's explicit `recovered` report, and certification treats that as
/// failure rather than progress. An unbounded volume would silently accept a
/// body that fell out of the module.
pub(crate) fn certification_scene(instances: &[PlacedModule], levels: u8) -> ArenaSpec {
    let mut colliders = Vec::new();
    for instance in instances {
        let rotation = instance.rotation();
        let quaternion = [rotation.x, rotation.y, rotation.z, rotation.w];
        for hull in &instance.hulls {
            #[allow(clippy::cast_possible_truncation)]
            let id = StableColliderId(colliders.len() as u32);
            colliders.push(ColliderSpec {
                id,
                center: instance.offset,
                rotation: quaternion,
                shape: ColliderShape::ConvexHull {
                    points: hull.iter().map(|point| rotation * *point).collect(),
                },
                friction: 0.8,
            });
        }
    }
    let height = f32::from(levels.max(1)) * TILE_LEVEL_HEIGHT;
    ArenaSpec {
        colliders,
        floor_y: 0.0,
        safety_center: Vec3::new(0.0, height * 0.5, 0.0),
        safety_half: Vec3::new(24.0, height + 12.0, 24.0),
    }
}

/// A domain-separated digest over the exact hulls a scene was built from.
///
/// Used as the certificate's structural identity when no compiled
/// `structural_hash` is available, so a compatibility certificate still names
/// the geometry it proved rather than only the module that owned it.
pub(crate) fn hull_digest(hulls: &[Vec<Vec3>]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(b"observed2.certification-hulls");
    hash.update((hulls.len() as u64).to_le_bytes());
    for hull in hulls {
        hash.update((hull.len() as u64).to_le_bytes());
        for point in hull {
            for value in [point.x, point.y, point.z] {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
    }
    hash.finalize().into()
}
