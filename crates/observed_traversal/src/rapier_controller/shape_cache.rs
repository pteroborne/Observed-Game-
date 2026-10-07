//! Scene-owned, bounded cache of exact local convex shapes. Warming is optional:
//! every miss uses the same Rapier bake as the cold path, on the same tick.
use crate::ColliderShape;
use glam::Vec3;
use rapier3d::prelude::{SharedShape, Vector};
use std::collections::{HashMap, VecDeque};

const CAPACITY: usize = 8192;

#[derive(Clone, Default)]
pub(super) struct ColliderShapeCache {
    hulls: HashMap<Vec<[u32; 3]>, SharedShape>,
    order: VecDeque<Vec<[u32; 3]>>,
    hits: u64,
    misses: u64,
}

impl ColliderShapeCache {
    pub(super) fn shape(&mut self, shape: &ColliderShape) -> Option<SharedShape> {
        match shape {
            ColliderShape::Cuboid { half } => Some(SharedShape::cuboid(half.x, half.y, half.z)),
            ColliderShape::ConvexHull { points } => self.convex(points),
        }
    }

    pub(super) fn convex(&mut self, points: &[Vec3]) -> Option<SharedShape> {
        let key: Vec<_> = points
            .iter()
            .map(|p| p.to_array().map(f32::to_bits))
            .collect();
        if let Some(shape) = self.hulls.get(&key) {
            self.hits += 1;
            return Some(shape.clone());
        }
        self.misses += 1;
        let points: Vec<_> = points.iter().map(|p| Vector::new(p.x, p.y, p.z)).collect();
        let shape = SharedShape::convex_hull(&points)?;
        if self.hulls.len() == CAPACITY {
            let retired = self
                .order
                .pop_front()
                .expect("full cache has an eviction order");
            self.hulls.remove(&retired);
        }
        self.order.push_back(key.clone());
        self.hulls.insert(key, shape.clone());
        Some(shape)
    }

    pub(super) fn counts(&self) -> [u64; 2] {
        [self.hits, self.misses]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rapier_controller::{RapierTraversalScene, step_character};
    use crate::{
        ArenaSpec, ColliderDelta, ColliderSpec, FpsArena, FpsBody, FpsConfig, StableColliderId,
    };
    use player_input::PlayerIntent;

    #[test]
    fn prepared_and_cold_hulls_preserve_queries_movement_and_stable_ids() {
        let spec = ArenaSpec::from_legacy(&FpsArena::authored());
        let mut cold = RapierTraversalScene::from_arena_spec(&spec);
        let mut warm = cold.clone();
        let hull: Vec<_> = [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0].into_iter().flat_map(move |y| {
                    [-1.0, 1.0]
                        .into_iter()
                        .map(move |z| Vec3::new(x * 0.2, y, z))
                })
            })
            .collect();
        warm.warm_convex_hulls([&hull]);
        for (id, center) in [
            (90, Vec3::new(1.5, 1.0, 0.0)),
            (91, Vec3::new(-1.5, 1.0, 0.0)),
        ] {
            let mut collider = ColliderSpec::cuboid(StableColliderId(id), center, Vec3::ONE);
            collider.shape = ColliderShape::ConvexHull {
                points: hull.clone(),
            };
            let delta = ColliderDelta {
                removed: Default::default(),
                upserted: vec![collider],
            };
            cold.apply_collider_delta(&delta).unwrap();
            warm.apply_collider_delta(&delta).unwrap();
            assert!(warm.contains_collider(StableColliderId(id)));
            assert_eq!(
                cold.ray_distance(Vec3::Y, Vec3::X, 4.0),
                warm.ray_distance(Vec3::Y, Vec3::X, 4.0)
            );
            assert_eq!(
                cold.capsule_is_clear(center, 0.35, 0.9),
                warm.capsule_is_clear(center, 0.35, 0.9)
            );
        }
        assert_eq!(warm.shape_cache_counts(), [2, 1]);
        assert_eq!(cold.shape_cache_counts(), [1, 1]);
        let config = FpsConfig::default();
        let mut a = FpsBody::spawned(Vec3::Y * config.half_height, 0.0);
        let mut b = a;
        for _ in 0..120 {
            let intent = PlayerIntent {
                movement: glam::Vec2::X,
                ..Default::default()
            };
            step_character(&cold, &mut a, intent, &config, 1.0 / 60.0);
            step_character(&warm, &mut b, intent, &config, 1.0 / 60.0);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn failed_bakes_never_enter_the_reusable_cache() {
        let mut cache = ColliderShapeCache::default();
        assert!(cache.convex(&[Vec3::ZERO]).is_none());
        assert!(cache.hulls.is_empty());
    }
}
