//! Presentation-only preparation of catalogue recipes. No card preview is
//! required and no prepared entry can change acceptance or simulation timing.
use super::{HexWfcVisualAssets, MeshGroupKey};
use crate::hex_wfc::{sim::HexWfcRuntime, view::HexPresentationReadiness};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Default)]
pub(super) struct MeshWarmup {
    next_tile: usize,
    blocked_cells: HashMap<observed_hex::HexCoord, (u32, u64)>,
    structural: VecDeque<(super::mesh::MergedMeshKey, Vec<Vec<Vec3>>)>,
    required: VecDeque<(super::mesh::MergedMeshKey, Vec<Vec<Vec3>>)>,
    requested: HashSet<super::mesh::MergedMeshKey>,
    worker: Option<Task<(super::mesh::MergedMeshKey, Option<Mesh>)>>,
    pending: VecDeque<(String, MeshGroupKey, Vec<Vec<Vec3>>)>,
}

pub(in crate::hex_wfc) fn warm_reusable_meshes(
    runtime: Res<HexWfcRuntime>,
    readiness: Res<HexPresentationReadiness>,
    mut assets: ResMut<HexWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Complete one immutable recipe, then start demanded work before optional
    // catalogue warming. Readiness never blocks simulation or play acceptance.
    assets.poll_prepared_meshes(&mut meshes);
    if assets.warmup.worker.is_some() {
        return;
    }
    let request = if let Some(required) = assets
        .warmup
        .structural
        .pop_front()
        .or_else(|| assets.warmup.required.pop_front())
    {
        Some(required)
    } else {
        if !readiness.entry_neighborhood_ready || readiness.spawned_this_frame > 0 {
            return;
        }
        if assets.warmup.pending.is_empty() {
            let Some(tile) = runtime
                .match_state
                .content()
                .cells()
                .get(assets.warmup.next_tile)
            else {
                return;
            };
            assets.warmup.next_tile += 1;
            let role = if tile.key.archetype.starts_with("climb_") {
                HexStructureRole::Climb
            } else {
                HexStructureRole::Hall
            };
            let pieces: Vec<_> = (0..tile.hulls.len())
                .map(|index| HexStructurePiece::authored_template(tile, index, role))
                .collect();
            let borrowed: Vec<_> = pieces.iter().collect();
            for (key, group) in crate::hex_wfc::view::mesh_group::gather(&borrowed) {
                if key != MeshGroupKey::Hidden {
                    assets.warmup.pending.push_back((
                        format!("{:?}", tile.key),
                        key,
                        group.hulls.iter().map(|hull| hull.to_vec()).collect(),
                    ));
                }
            }
        }
        assets
            .warmup
            .pending
            .pop_front()
            .map(|(tile, group, hulls)| {
                let borrowed: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
                (
                    super::mesh::MergedMeshKey::new(&tile, group, &borrowed),
                    hulls,
                )
            })
    };
    if let Some((key, hulls)) = request {
        if assets.merged_hull_cache.get(&key).is_some() {
            assets.warmup.requested.remove(&key);
            return;
        }
        assets.warmup.requested.insert(key.clone());
        let facing = match key.group {
            MeshGroupKey::Climb(facing) => Some(facing),
            _ => None,
        };
        // Only immutable local geometry crosses this boundary. The worker never
        // sees the match or a card, and a cold play still commits on its own tick.
        assets.warmup.worker = Some(AsyncComputeTaskPool::get().spawn(async move {
            let borrowed: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
            let mesh = super::build_merged_mesh_facing(&borrowed, facing);
            (key, mesh)
        }));
    }
}

impl HexWfcVisualAssets {
    pub(in crate::hex_wfc::view) fn poll_prepared_meshes(&mut self, meshes: &mut Assets<Mesh>) {
        let Some(worker) = &mut self.warmup.worker else {
            return;
        };
        if let Some((key, mesh)) = block_on(poll_once(worker)) {
            self.warmup.worker = None;
            self.warmup.requested.remove(&key);
            if self.merged_hull_cache.get(&key).is_none() {
                if let Some(mesh) = mesh {
                    self.merged_hull_cache.insert(key, meshes.add(mesh));
                } else {
                    self.merged_hull_cache.insert_empty(key);
                }
                self.cache_misses += 1;
            }
        }
    }

    pub(in crate::hex_wfc::view) fn cell_can_retry(
        &self,
        coord: observed_hex::HexCoord,
        generation: u32,
    ) -> bool {
        self.warmup.blocked_cells.get(&coord) != Some(&(generation, self.cache_misses))
    }

    pub(in crate::hex_wfc::view) fn finish_cell_attempt(
        &mut self,
        coord: observed_hex::HexCoord,
        generation: u32,
    ) {
        if self.missing_meshes {
            self.warmup
                .blocked_cells
                .insert(coord, (generation, self.cache_misses));
        } else {
            self.warmup.blocked_cells.remove(&coord);
        }
    }

    pub(super) fn request_merged_recipe(
        &mut self,
        key: super::mesh::MergedMeshKey,
        hulls: &[&[Vec3]],
    ) {
        if self.warmup.required.len() < 64 && self.warmup.requested.insert(key.clone()) {
            self.warmup
                .required
                .push_back((key, hulls.iter().map(|hull| hull.to_vec()).collect()));
        }
    }

    /// A demanded cell waits for complete meshes, keeping any existing parent.
    /// Pending work is bounded; overflow retries next frame in residency order.
    pub(in crate::hex_wfc::view) fn request_cell_meshes(
        &mut self,
        tile: Option<&str>,
        groups: &[(
            MeshGroupKey,
            crate::hex_wfc::view::mesh_group::MergedGroup<'_>,
        )],
    ) -> bool {
        let Some(tile) = tile else {
            return true;
        };
        let mut ready = true;
        for (group, data) in groups {
            if *group == MeshGroupKey::Hidden {
                continue;
            }
            let key = super::mesh::MergedMeshKey::new(tile, *group, &data.hulls);
            if self.merged_hull_cache.get(&key).is_some() {
                continue;
            }
            ready = false;
            if self.warmup.structural.len() < 64 && self.warmup.requested.insert(key.clone()) {
                self.warmup
                    .structural
                    .push_back((key, data.hulls.iter().map(|hull| hull.to_vec()).collect()));
            }
        }
        ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::view::mesh_group;
    use observed_match::hex_wfc::HexPiecePart;
    use observed_traversal::{ColliderShape, StableColliderId};

    #[test]
    fn demanded_recipes_defer_deduplicate_and_use_exact_completed_meshes() {
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = HexWfcVisualAssets::for_test(&mut materials);
        let mut meshes = Assets::<Mesh>::default();
        let points: Vec<_> = [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0]
                    .into_iter()
                    .flat_map(move |y| [-1.0, 1.0].into_iter().map(move |z| Vec3::new(x, y, z)))
            })
            .collect();
        let piece = HexStructurePiece {
            id: StableColliderId(1),
            anchor: default(),
            source_cell: default(),
            role: HexStructureRole::Hall,
            part: HexPiecePart::Authored,
            surface: None,
            tile: None,
            center: Vec3::ZERO,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::ConvexHull { points },
        };
        let groups = mesh_group::gather(&[&piece]);
        assert!(!assets.request_cell_meshes(Some("tile"), &groups));
        assert!(!assets.request_cell_meshes(Some("tile"), &groups));
        assert_eq!(assets.warmup.structural.len(), groups.len());
        // The cold entry path and worker populate the identical geometry key.
        for (key, group) in &groups {
            assets
                .merged_mesh_for(&mut meshes, Some("tile"), *key, &group.hulls)
                .unwrap();
        }
        assert!(assets.request_cell_meshes(Some("tile"), &groups));
        for tile in 0..200 {
            assets.request_cell_meshes(Some(&tile.to_string()), &groups);
        }
        assert_eq!(assets.warmup.structural.len(), 64);
        assert!(
            HexWfcVisualAssets::for_test(&mut materials)
                .warmup
                .structural
                .is_empty()
        );
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn pending_dressing_retries_on_cache_progress_or_geometry_change() {
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = HexWfcVisualAssets::for_test(&mut materials);
        let coord = observed_hex::HexCoord::default();
        assets.missing_meshes = true;
        assets.finish_cell_attempt(coord, 5);
        assert!(!assets.cell_can_retry(coord, 5));
        assert!(
            assets.cell_can_retry(coord, 6),
            "a rewrite must invalidate pending recipes"
        );
        let hull: Vec<_> = [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0]
                    .into_iter()
                    .flat_map(move |y| [-1.0, 1.0].into_iter().map(move |z| Vec3::new(x, y, z)))
            })
            .collect();
        let mut meshes = Assets::<Mesh>::default();
        assets
            .merged_mesh_for(
                &mut meshes,
                Some("completed-recipe"),
                MeshGroupKey::Interior,
                &[&hull],
            )
            .unwrap();
        assert!(
            assets.cell_can_retry(coord, 5),
            "completed mesh work must wake a pending cell"
        );
        assets.missing_meshes = false;
        assets.finish_cell_attempt(coord, 5);
        assert!(assets.warmup.blocked_cells.is_empty());
    }
}
