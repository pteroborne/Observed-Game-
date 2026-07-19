//! Style-owned materials and a deterministic mesh cache for the hex facility shell.
//!
//! Every structural surface is a district-tinted `observed_style` treatment, one set
//! per [`ArchitectureRegister`], keyed at render time by each collider piece's role and
//! its source cell's architecture. Authored tile hulls carry the geometric detail, so
//! there is no separate procedural "register dressing" pass — the tiles *are* the
//! dressing.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use observed_style::{self as style, SurfaceRole};
use observed_traversal::ColliderShape;
use rapier3d::prelude::{SharedShape, Vector as RapierVector};

use crate::view::assets::palette_tinted_neon_material;
use crate::view::environment::{cuboid_mesh, load_repeating_texture};

#[derive(Clone)]
pub(in crate::hex_wfc) struct RegisterMaterials {
    room: Handle<StandardMaterial>,
    hall: Handle<StandardMaterial>,
    ramp: Handle<StandardMaterial>,
    shaft: Handle<StandardMaterial>,
    boundary: Handle<StandardMaterial>,
}

impl RegisterMaterials {
    pub(in crate::hex_wfc) fn for_role(&self, role: HexStructureRole) -> Handle<StandardMaterial> {
        match role {
            HexStructureRole::Room => self.room.clone(),
            HexStructureRole::Hall => self.hall.clone(),
            HexStructureRole::Ramp => self.ramp.clone(),
            HexStructureRole::Shaft => self.shaft.clone(),
            HexStructureRole::Boundary => self.boundary.clone(),
        }
    }
}

#[derive(Resource)]
pub(in crate::hex_wfc) struct HexWfcVisualAssets {
    registers: Vec<RegisterMaterials>,
    hull_cache: HashMap<(String, usize), Handle<Mesh>>,
    cuboid_cache: HashMap<[u32; 3], Handle<Mesh>>,
}

impl HexWfcVisualAssets {
    pub(in crate::hex_wfc) fn load(
        asset_server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        let wall_texture = load_repeating_texture(asset_server, observed_assets::WALL.path);
        let floor_texture = load_repeating_texture(asset_server, observed_assets::FLOOR.path);
        let registers = ArchitectureRegister::ALL
            .into_iter()
            .map(|register| {
                let palette = style::architecture(register);
                let mut tinted = |role: SurfaceRole, texture: Option<Handle<Image>>| {
                    let mut material =
                        palette_tinted_neon_material(&style::surface(role), &palette, texture);
                    // Exact hex hulls put much more surface area close to the
                    // camera than the teleport rooms this helper was tuned on.
                    // Preserve the style hue while returning shell albedo and
                    // non-signal emission to the neon-noir atmosphere tier.
                    let base = material.base_color.to_srgba();
                    material.base_color = Color::srgba(
                        base.red * 0.38,
                        base.green * 0.38,
                        base.blue * 0.38,
                        base.alpha,
                    );
                    material.emissive *= 0.35;
                    materials.add(material)
                };
                RegisterMaterials {
                    // One authored convex hull can contain floor, wall, and
                    // ceiling faces. Applying the bright Spine route treatment
                    // to the whole room shell turned every vertical surface
                    // into an amber emissive slab. Room decisions are signaled
                    // by thresholds/equipment, so the shell stays structural.
                    room: tinted(SurfaceRole::Plain, floor_texture.clone()),
                    hall: tinted(SurfaceRole::GantryDeck, floor_texture.clone()),
                    ramp: tinted(SurfaceRole::SafeBypass, floor_texture.clone()),
                    shaft: tinted(SurfaceRole::WellshaftStone, wall_texture.clone()),
                    boundary: tinted(SurfaceRole::Wall, wall_texture.clone()),
                }
            })
            .collect();
        Self {
            registers,
            hull_cache: HashMap::new(),
            cuboid_cache: HashMap::new(),
        }
    }

    pub(in crate::hex_wfc) fn register(
        &self,
        register: ArchitectureRegister,
    ) -> &RegisterMaterials {
        &self.registers[register.stable_id() as usize]
    }

    /// A cached render mesh for a collider piece. Cuboids are keyed by size; hull meshes
    /// are keyed by the tile identity plus the hull's index within its cell, so every
    /// cell instancing the same authored tile shares one mesh handle.
    pub(in crate::hex_wfc) fn mesh_for(
        &mut self,
        meshes: &mut Assets<Mesh>,
        piece: &HexStructurePiece,
        hull_index: usize,
    ) -> Option<Handle<Mesh>> {
        match &piece.shape {
            ColliderShape::Cuboid { half } => {
                let size = *half * 2.0;
                let key = [size.x.to_bits(), size.y.to_bits(), size.z.to_bits()];
                Some(
                    self.cuboid_cache
                        .entry(key)
                        .or_insert_with(|| meshes.add(cuboid_mesh(size)))
                        .clone(),
                )
            }
            ColliderShape::ConvexHull { points } => {
                let tile_key = piece.tile.as_ref().map_or_else(
                    || format!("{:?}", piece.source_cell),
                    |tile| format!("{tile:?}"),
                );
                let key = (tile_key, hull_index);
                if let Some(handle) = self.hull_cache.get(&key) {
                    return Some(handle.clone());
                }
                let mesh = hull_mesh(points)?;
                let handle = meshes.add(mesh);
                self.hull_cache.insert(key, handle.clone());
                Some(handle)
            }
        }
    }
}

/// Convert a convex-hull collider's points into a flat-shaded render mesh. Mirrors the
/// hex lab's exact-collider renderer so the visible surface is the collision surface.
fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let points: Vec<_> = hull
        .iter()
        .map(|point| RapierVector::new(point.x, point.y, point.z))
        .collect();
    let shape = SharedShape::convex_hull(&points)?;
    let (vertices, indices) = shape.as_convex_polyhedron()?.to_trimesh();
    let positions: Vec<[f32; 3]> = vertices
        .iter()
        .map(|point| [point.x, point.y, point.z])
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices.into_iter().flatten().collect()))
        .with_duplicated_vertices()
        .with_computed_flat_normals(),
    )
}
