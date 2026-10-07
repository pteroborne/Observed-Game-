//! Merged structural meshes.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use observed_traversal::ConvexRenderMesh;
/// Convert multiple convex hulls into a single merged Bevy mesh, keeping only the
/// triangles that face `facing` when one is given.
pub(in crate::hex_wfc::view) fn build_merged_mesh_facing(
    hulls: &[&[Vec3]],
    facing: Option<crate::hex_wfc::view::mesh_group::Facing>,
) -> Option<Mesh> {
    let mut all_positions = Vec::new();
    let mut all_normals = Vec::new();
    let mut all_uvs = Vec::new();
    let mut all_indices = Vec::new();

    for hull in hulls {
        let Some(data) = ConvexRenderMesh::from_convex_hull(hull) else {
            continue;
        };
        // Every triangle's corners are its own (`ConvexRenderMesh` duplicates them),
        // so a triangle is three consecutive vertices and can be kept or dropped whole.
        for corner in data.indices.chunks_exact(3) {
            let point = |index: u32| Vec3::from_array(data.positions[index as usize]);
            let (a, b, c) = (point(corner[0]), point(corner[1]), point(corner[2]));
            let normal = (b - a).cross(c - a).normalize_or_zero();
            if facing.is_some_and(|facing| !facing.holds(normal)) {
                continue;
            }
            for &index in corner {
                let next = u32::try_from(all_positions.len()).ok()?;
                all_positions.push(data.positions[index as usize]);
                all_normals.push(data.normals[index as usize]);
                all_uvs.push(data.uvs[index as usize]);
                all_indices.push(next);
            }
        }
    }

    if all_positions.is_empty() {
        return None;
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, all_positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, all_normals)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_1,
        all_uvs
            .iter()
            .map(|uv| [uv[0] * 8.0, uv[1] * 8.0])
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, all_uvs)
    .with_inserted_indices(Indices::U32(all_indices))
    .with_generated_tangents()
    .ok()
}

/// Exact local geometry is part of mesh identity. A tile name and cell alone
/// cannot distinguish a wall opened or closed by a neighbouring rewrite.
#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) struct MergedMeshKey {
    pub(super) tile: String,
    pub(super) group: super::MeshGroupKey,
    pub(super) hulls: Vec<Vec<[u32; 3]>>,
}

impl MergedMeshKey {
    pub(super) fn new(tile: &str, group: super::MeshGroupKey, hulls: &[&[Vec3]]) -> Self {
        let mut keys: Vec<Vec<_>> = hulls
            .iter()
            .map(|hull| {
                hull.iter()
                    .map(|point| point.to_array().map(f32::to_bits))
                    .collect()
            })
            .collect();
        // Packed-vector swaps can reorder hulls in an unchanged owner. The
        // mesh has the same surfaces regardless of their order in the vector.
        keys.sort_unstable();
        Self {
            tile: tile.to_owned(),
            group,
            hulls: keys,
        }
    }
}

const MERGED_CACHE_LIMIT: usize = 4096;

/// Match-local FIFO bound; live parents retain their own mesh handles when a
/// cold recipe is evicted. Reset drops the entire cache with visual assets.
#[derive(Default)]
pub(super) struct MergedMeshCache {
    entries: std::collections::HashMap<MergedMeshKey, Option<Handle<Mesh>>>,
    order: std::collections::VecDeque<MergedMeshKey>,
}

impl MergedMeshCache {
    pub(super) fn get(&self, key: &MergedMeshKey) -> Option<&Option<Handle<Mesh>>> {
        self.entries.get(key)
    }

    pub(super) fn insert(&mut self, key: MergedMeshKey, handle: Handle<Mesh>) {
        self.insert_value(key, Some(handle));
    }

    pub(super) fn insert_empty(&mut self, key: MergedMeshKey) {
        self.insert_value(key, None);
    }

    fn insert_value(&mut self, key: MergedMeshKey, handle: Option<Handle<Mesh>>) {
        if self.entries.contains_key(&key) {
            return;
        }
        if self.entries.len() == MERGED_CACHE_LIMIT {
            let retired = self
                .order
                .pop_front()
                .expect("full cache has an eviction order");
            self.entries.remove(&retired);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_mesh_recipes_are_bounded_and_resettable() {
        let mut cache = MergedMeshCache::default();
        for tile in 0..=MERGED_CACHE_LIMIT {
            cache.insert(
                MergedMeshKey {
                    tile: tile.to_string(),
                    group: super::super::MeshGroupKey::Floor,
                    hulls: Vec::new(),
                },
                Handle::default(),
            );
        }
        assert_eq!(cache.entries.len(), MERGED_CACHE_LIMIT);
        assert!(!cache.entries.keys().any(|key| key.tile == "0"));
        assert!(MergedMeshCache::default().entries.is_empty());
    }
}
