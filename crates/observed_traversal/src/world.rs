//! Canonical collision input and the temporary dual-backend traversal seam.
//!
//! `ArenaSpec` is pure, stable data. The game, headless match, replay, and network
//! layers build it without depending on Rapier handles. `TraversalWorld` consumes that
//! data and owns all backend runtime state. The legacy arm exists only for migration;
//! new authored convex geometry is accepted by the Rapier arm only.

use crate::FpsArena;
use glam::Vec3;
use rapier3d::prelude::*;

/// Stable identity of one collider inside a baked/current-place arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableColliderId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    Cuboid { half: Vec3 },
    ConvexHull { points: Vec<Vec3> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColliderSpec {
    pub id: StableColliderId,
    pub center: Vec3,
    /// Rotation stored as a normalized `(x, y, z, w)` quaternion.
    pub rotation: [f32; 4],
    pub shape: ColliderShape,
    pub friction: f32,
}

impl ColliderSpec {
    pub fn cuboid(id: StableColliderId, center: Vec3, half: Vec3) -> Self {
        Self {
            id,
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::Cuboid { half },
            friction: 0.8,
        }
    }
}

/// Pure collision description for one isolated room/hallway Place.
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaSpec {
    pub colliders: Vec<ColliderSpec>,
    pub floor_y: f32,
    /// Conservative kill/recovery volume for this isolated collision world. This is
    /// diagnostic state only: unlike the old `floor_half`, it never creates geometry.
    pub safety_center: Vec3,
    pub safety_half: Vec3,
}

impl ArenaSpec {
    pub fn from_legacy(arena: &FpsArena) -> Self {
        let mut colliders = Vec::with_capacity(arena.solids.len() + 1);
        // The legacy floor is implicit. Rapier needs a real support surface with its
        // top face exactly at `floor_y`.
        colliders.push(ColliderSpec::cuboid(
            StableColliderId(0),
            Vec3::new(0.0, arena.floor_y - 0.5, 0.0),
            Vec3::new(arena.floor_half, 0.5, arena.floor_half),
        ));
        colliders.extend(arena.solids.iter().enumerate().map(|(index, solid)| {
            ColliderSpec::cuboid(
                StableColliderId(index as u32 + 1),
                (solid.min + solid.max) * 0.5,
                (solid.max - solid.min) * 0.5,
            )
        }));
        Self {
            colliders,
            floor_y: arena.floor_y,
            safety_center: Vec3::new(0.0, arena.floor_y, 0.0),
            safety_half: Vec3::new(arena.floor_half, 12.0, arena.floor_half),
        }
    }

    pub fn validate(&self) -> Result<(), ArenaSpecError> {
        let mut ids = std::collections::BTreeSet::new();
        for collider in &self.colliders {
            if !ids.insert(collider.id) {
                return Err(ArenaSpecError::DuplicateId(collider.id));
            }
            if !collider.center.is_finite()
                || !collider.friction.is_finite()
                || collider.friction < 0.0
                || !collider.rotation.iter().all(|value| value.is_finite())
            {
                return Err(ArenaSpecError::InvalidCollider(collider.id));
            }
            match &collider.shape {
                ColliderShape::Cuboid { half } => {
                    if !half.is_finite() || half.min_element() <= 0.0 {
                        return Err(ArenaSpecError::InvalidCollider(collider.id));
                    }
                }
                ColliderShape::ConvexHull { points } => {
                    if points.len() < 4 || points.iter().any(|point| !point.is_finite()) {
                        return Err(ArenaSpecError::InvalidCollider(collider.id));
                    }
                    let rapier_points: Vec<Vector> =
                        points.iter().copied().map(into_rapier).collect();
                    if ColliderBuilder::convex_hull(&rapier_points).is_none() {
                        return Err(ArenaSpecError::DegenerateHull(collider.id));
                    }
                }
            }
        }
        if !self.floor_y.is_finite()
            || !self.safety_center.is_finite()
            || !self.safety_half.is_finite()
            || self.safety_half.min_element() <= 0.0
        {
            return Err(ArenaSpecError::InvalidBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaSpecError {
    DuplicateId(StableColliderId),
    InvalidCollider(StableColliderId),
    DegenerateHull(StableColliderId),
    InvalidBounds,
}

fn into_rapier(value: Vec3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_conversion_is_canonical_and_valid() {
        let arena = FpsArena::authored();
        let a = ArenaSpec::from_legacy(&arena);
        let b = ArenaSpec::from_legacy(&arena);
        assert_eq!(a, b);
        assert_eq!(a.colliders.len(), arena.solids.len() + 1);
        a.validate().unwrap();
    }
}
