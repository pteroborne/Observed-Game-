//! Style-owned materials and a deterministic mesh cache for the hex facility shell.
//!
//! Every structural surface is a district-tinted `observed_style` treatment, one set
//! per [`ArchitectureRegister`], keyed at render time by each collider piece's role and
//! its source cell's architecture. Authored hulls remain authoritative; shallow
//! Library and Zen detail fits their actual convex supports.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole, HexTrimKind};
use observed_style::{self as style, ArchitectureSurfaceRole, SurfaceRole};
use observed_traversal::{ColliderShape, ConvexRenderMesh};

pub(in crate::hex_wfc) use super::mesh_group::MeshGroupKey;
use super::open_edge_materials::OpenEdgeMaterials;
use crate::view::environment::{cuboid_mesh, load_repeating_texture};

mod concourse;
mod jade;
mod promenade;
mod rain;
mod textures;
mod wonder;
use textures::surface_texture;
use wonder::WonderMaterials;

#[derive(Clone)]
pub(in crate::hex_wfc) struct RegisterMaterials {
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    ceiling: Handle<StandardMaterial>,
    fixture: Handle<StandardMaterial>,
    boundary: Handle<StandardMaterial>,
}

impl RegisterMaterials {
    fn for_piece(
        &self,
        role: HexStructureRole,
        horizontal_surface: HorizontalSurface,
    ) -> Handle<StandardMaterial> {
        match role {
            // A ramp and a stair tower are built of the district's own floor, wall and
            // ceiling, like any hall. They had materials of their own - a route
            // treatment with a cyan glow, and a generic stone - which put every climb
            // in the same teal, in every district.
            HexStructureRole::Room | HexStructureRole::Hall | HexStructureRole::Climb => {
                match horizontal_surface {
                    HorizontalSurface::Floor => self.floor.clone(),
                    HorizontalSurface::Wall => self.wall.clone(),
                    HorizontalSurface::Ceiling => self.ceiling.clone(),
                }
            }
            HexStructureRole::Boundary => self.boundary.clone(),
        }
    }

    pub(in crate::hex_wfc) fn fixture(&self) -> Handle<StandardMaterial> {
        self.fixture.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HorizontalSurface {
    Floor,
    Wall,
    Ceiling,
}

#[derive(Resource)]
pub(in crate::hex_wfc) struct HexWfcVisualAssets {
    registers: Vec<RegisterMaterials>,
    reservoir: WonderMaterials,
    chargeworks: WonderMaterials,
    archive: WonderMaterials,
    rain: rain::RainMaterials,
    concourse: concourse::ConcourseMaterials,
    jade: jade::JadeMaterials,
    promenade: promenade::PromenadeMaterials,
    archive_details: [Handle<StandardMaterial>; 8],
    chargeworks_details: [Handle<StandardMaterial>; 4],
    hull_cache: HashMap<(String, usize), Handle<Mesh>>,
    cuboid_cache: HashMap<[u32; 3], Handle<Mesh>>,
    merged_hull_cache: HashMap<(String, MeshGroupKey), Handle<Mesh>>,
    /// Open-edge pieces are the same in every register: the lip is a signal, and the
    /// railing, walkway and truss belong to the connective structure, not a district.
    open_edge: OpenEdgeMaterials,
}

impl HexWfcVisualAssets {
    pub(in crate::hex_wfc) fn load(
        asset_server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
        _content: &observed_content::ContentManifest,
    ) -> Self {
        let wall_texture = load_repeating_texture(asset_server, observed_assets::WALL.path);
        let registers: Vec<RegisterMaterials> = ArchitectureRegister::ALL
            .into_iter()
            .map(|register| {
                let palette = style::architecture(register);
                // The shell's own look - the albedo pull-down and the emissive
                // trim that keep exact hex hulls in the neon-noir tier - now
                // lives in `observed_style::hex_shell_surface`, so a preview can
                // reproduce it instead of inventing its own greys. The scaling
                // that used to be written out here is that function's body.
                // The register's own weave, drawn rather than loaded. There is
                // one `wall.png` in the repository and there is not going to be
                // a pipeline for ten, so a register says how its surface is
                // divided and this makes the image. A register with no weave
                // keeps the shared albedo, which is a real answer for the
                // Monolith rather than a fallback.
                // What the district is made of: a detail image and a normal map for
                // each of its floor, wall and ceiling (`observed_style::surfaces`),
                // the wall's carrying the district's weave.
                let mut surface =
                    |role: ArchitectureSurfaceRole,
                     mask_emission: bool,
                     materials: &mut Assets<StandardMaterial>| {
                        let look = style::hex_shell_surface(register, role);
                        let drawn = style::surfaces::surface_images(register, role);
                        let albedo = surface_texture(images, drawn.albedo, true);
                        let normal = surface_texture(images, drawn.normal, false);
                        materials.add(StandardMaterial {
                            base_color: look.base_color,
                            emissive: look.emissive,
                            emissive_texture: mask_emission.then(|| albedo.clone()),
                            unlit: look.unlit,
                            base_color_texture: Some(albedo),
                            normal_map_texture: Some(normal),
                            perceptual_roughness: palette.surface_roughness,
                            metallic: if register == ArchitectureRegister::Megastructure {
                                style::reactor::METALLIC
                            } else {
                                0.0
                            },
                            ..default()
                        })
                    };
                let floor = surface(ArchitectureSurfaceRole::Floor, false, materials);
                let wall = surface(ArchitectureSurfaceRole::Wall, false, materials);
                let ceiling = surface(ArchitectureSurfaceRole::Ceiling, false, materials);
                let mut tinted = |look: style::HexSurfaceLook, texture: Option<Handle<Image>>| {
                    materials.add(StandardMaterial {
                        base_color: look.base_color,
                        emissive: look.emissive,
                        unlit: look.unlit,
                        base_color_texture: if look.textured { texture } else { None },
                        perceptual_roughness: palette.surface_roughness,
                        ..default()
                    })
                };
                RegisterMaterials {
                    floor,
                    wall,
                    ceiling,
                    fixture: tinted(
                        style::hex_shell_surface(
                            register,
                            ArchitectureSurfaceRole::PracticalFixture,
                        ),
                        None,
                    ),
                    boundary: tinted(
                        style::hex_shell_look(&style::surface(SurfaceRole::Wall), register),
                        wall_texture.clone(),
                    ),
                }
            })
            .collect();
        let reservoir = WonderMaterials::load_cistern(materials, images);
        // The wonder uses the district's exact cached handles, so the finishes stay together.
        let chargeworks = WonderMaterials::from_register(
            &registers[ArchitectureRegister::Megastructure.stable_id() as usize],
        );
        let archive = WonderMaterials::from_register(
            &registers[ArchitectureRegister::InfiniteGallery.stable_id() as usize],
        );
        Self {
            concourse: concourse::ConcourseMaterials::load(materials, images),
            jade: jade::JadeMaterials::load(materials, images),
            promenade: promenade::PromenadeMaterials::load(materials, images),
            reservoir,
            archive,
            rain: rain::RainMaterials::load(
                materials,
                images,
                &registers[ArchitectureRegister::ShadowScreen.stable_id() as usize],
            ),
            archive_details: wonder::archive_details(materials),
            chargeworks,
            chargeworks_details: wonder::details(materials),
            registers,
            hull_cache: HashMap::new(),
            cuboid_cache: HashMap::new(),
            merged_hull_cache: HashMap::new(),
            open_edge: OpenEdgeMaterials::new(materials),
        }
    }

    #[cfg(test)]
    pub(in crate::hex_wfc) fn for_test(materials: &mut Assets<StandardMaterial>) -> Self {
        let dummy = materials.add(StandardMaterial::default());
        let registers = ArchitectureRegister::ALL
            .into_iter()
            .map(|_| RegisterMaterials {
                floor: dummy.clone(),
                wall: dummy.clone(),
                ceiling: dummy.clone(),
                fixture: dummy.clone(),
                boundary: dummy.clone(),
            })
            .collect();
        Self {
            concourse: concourse::ConcourseMaterials::for_test(&dummy),
            jade: jade::JadeMaterials::for_test(&dummy),
            promenade: promenade::PromenadeMaterials::for_test(&dummy),
            reservoir: WonderMaterials::for_test(&dummy),
            archive: WonderMaterials::for_test(&dummy),
            rain: rain::RainMaterials::for_test(&dummy),
            archive_details: std::array::from_fn(|_| dummy.clone()),
            chargeworks: WonderMaterials::for_test(&dummy),
            chargeworks_details: std::array::from_fn(|_| dummy.clone()),
            registers,
            hull_cache: HashMap::new(),
            cuboid_cache: HashMap::new(),
            merged_hull_cache: HashMap::new(),
            open_edge: OpenEdgeMaterials::new(materials),
        }
    }

    pub(super) fn rain_material(&self, index: usize) -> Handle<StandardMaterial> {
        self.rain.0[index].clone()
    }

    pub(super) fn archive_detail(&self, index: usize) -> Handle<StandardMaterial> {
        self.archive_details[index].clone()
    }
    pub(in crate::hex_wfc) fn archive_material(
        &self,
        group: MeshGroupKey,
    ) -> Handle<StandardMaterial> {
        self.archive.for_group(group)
    }

    pub(super) fn chargeworks_detail(&self, index: usize) -> Handle<StandardMaterial> {
        self.chargeworks_details[index].clone()
    }
    pub(super) fn detail_box(&mut self, meshes: &mut Assets<Mesh>, size: Vec3) -> Handle<Mesh> {
        let key = [size.x.to_bits(), size.y.to_bits(), size.z.to_bits()];
        self.cuboid_cache
            .entry(key)
            .or_insert_with(|| meshes.add(cuboid_mesh(size)))
            .clone()
    }

    pub(in crate::hex_wfc) fn chargeworks_material(
        &self,
        group: MeshGroupKey,
    ) -> Handle<StandardMaterial> {
        self.chargeworks.for_group(group)
    }

    pub(in crate::hex_wfc) fn reservoir_material(
        &self,
        group: MeshGroupKey,
    ) -> Handle<StandardMaterial> {
        self.reservoir.for_group(group)
    }

    pub(in crate::hex_wfc) fn register(
        &self,
        register: ArchitectureRegister,
    ) -> &RegisterMaterials {
        &self.registers[register.stable_id() as usize]
    }

    pub(in crate::hex_wfc) fn material_for_piece(
        &self,
        register: ArchitectureRegister,
        piece: &HexStructurePiece,
    ) -> Handle<StandardMaterial> {
        if register == ArchitectureRegister::OverlitGrid
            && matches!(piece.role, HexStructureRole::Room | HexStructureRole::Hall)
            && let ColliderShape::ConvexHull { points } = &piece.shape
            && observed_traversal::render_mesh::is_overhead_slab(points)
        {
            return self.register(register).ceiling.clone();
        }
        self.register(register)
            .for_piece(piece.role, horizontal_surface(piece))
    }

    /// Cached mesh for a derived seam-trim descriptor. Railings run along a
    /// cell's open lateral edge (a long thin bar at waist height); buttresses
    /// stand at a role/register seam (a slim near-full-level pillar). Both are
    /// cuboids keyed by size in the shared `cuboid_cache`, so every trim piece
    /// of a kind shares one handle. `Lintel` is defined but never emitted by
    /// `derive_trim` yet (the snapshot carries no port class); it maps to a
    /// header bar so the match is exhaustive.
    pub(in crate::hex_wfc) fn trim_mesh(
        &mut self,
        meshes: &mut Assets<Mesh>,
        kind: HexTrimKind,
    ) -> Handle<Mesh> {
        // A hex lateral edge is ~8 m; inset the railing slightly so neighbouring
        // faces' rails do not visibly overlap at the shared corners.
        let size = match kind {
            HexTrimKind::Railing => Vec3::new(7.4, 0.12, 0.12),
            HexTrimKind::Buttress => Vec3::new(0.55, observed_hex::TILE_LEVEL_HEIGHT * 0.9, 0.55),
            HexTrimKind::Lintel => Vec3::new(2.0, 0.22, 0.30),
        };
        let key = [size.x.to_bits(), size.y.to_bits(), size.z.to_bits()];
        self.cuboid_cache
            .entry(key)
            .or_insert_with(|| meshes.add(cuboid_mesh(size)))
            .clone()
    }

    /// Material for derived seam trim: the current register's wall treatment —
    /// already returned to shell albedo and non-signal emission in `load`, so
    /// trim reads as dim structure and never competes with gameplay signals
    /// (Legibility Contract). Reuses the style module rather than inventing a
    /// colour, per the visual-language rule.
    pub(in crate::hex_wfc) fn trim_material(
        &self,
        register: ArchitectureRegister,
    ) -> Handle<StandardMaterial> {
        self.register(register).wall.clone()
    }

    pub(in crate::hex_wfc) fn fixture_mesh(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        let size = Vec3::new(2.25, 0.10, 0.55);
        let key = [size.x.to_bits(), size.y.to_bits(), size.z.to_bits()];
        self.cuboid_cache
            .entry(key)
            .or_insert_with(|| meshes.add(cuboid_mesh(size)))
            .clone()
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

    pub(in crate::hex_wfc) fn material_for_group(
        &self,
        architecture: ArchitectureRegister,
        group: MeshGroupKey,
    ) -> Handle<StandardMaterial> {
        let reg = self.register(architecture);
        match group {
            MeshGroupKey::Floor => reg.floor.clone(),
            MeshGroupKey::Ceiling => reg.ceiling.clone(),
            MeshGroupKey::Interior | MeshGroupKey::Perimeter(_) => reg.wall.clone(),
            MeshGroupKey::Climb(super::mesh_group::Facing::Up) => reg.floor.clone(),
            MeshGroupKey::Climb(super::mesh_group::Facing::Side) => reg.wall.clone(),
            MeshGroupKey::Climb(super::mesh_group::Facing::Down) => reg.ceiling.clone(),
            MeshGroupKey::Boundary => reg.boundary.clone(),
            MeshGroupKey::Lip => self.open_edge.lip.clone(),
            MeshGroupKey::Rail => self.open_edge.rail.clone(),
            MeshGroupKey::Walkway => self.open_edge.walkway.clone(),
            MeshGroupKey::Truss | MeshGroupKey::Hidden => self.open_edge.truss.clone(),
            MeshGroupKey::Facade => self.open_edge.facade.clone(),
            MeshGroupKey::Roof => self.open_edge.roof.clone(),
            MeshGroupKey::Window => reg.fixture.clone(),
        }
    }

    pub(in crate::hex_wfc) fn merged_mesh_for(
        &mut self,
        meshes: &mut Assets<Mesh>,
        tile_key: Option<&str>,
        group: MeshGroupKey,
        hulls: &[&[Vec3]],
    ) -> Option<Handle<Mesh>> {
        let facing = match group {
            MeshGroupKey::Climb(facing) => Some(facing),
            _ => None,
        };
        if let Some(key) = tile_key {
            let cache_key = (key.to_string(), group);
            if let Some(handle) = self.merged_hull_cache.get(&cache_key) {
                return Some(handle.clone());
            }
            let mesh = build_merged_mesh_facing(hulls, facing)?;
            let handle = meshes.add(mesh);
            self.merged_hull_cache.insert(cache_key, handle.clone());
            Some(handle)
        } else {
            let mesh = build_merged_mesh_facing(hulls, facing)?;
            Some(meshes.add(mesh))
        }
    }
}

/// Convert multiple convex hulls into a single merged Bevy mesh, keeping only the
/// triangles that face `facing` when one is given.
pub(super) fn build_merged_mesh_facing(
    hulls: &[&[Vec3]],
    facing: Option<super::mesh_group::Facing>,
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
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, all_uvs)
    .with_inserted_indices(Indices::U32(all_indices))
    .with_generated_tangents()
    .ok()
}

/// Convert shared engine-independent render data into Bevy's mesh format.
pub(super) fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let data = ConvexRenderMesh::from_convex_hull(hull)?;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
    .with_inserted_indices(Indices::U32(data.indices))
    .with_generated_tangents()
    .ok()
}

fn horizontal_surface(piece: &HexStructurePiece) -> HorizontalSurface {
    let ColliderShape::ConvexHull { points } = &piece.shape else {
        return HorizontalSurface::Wall;
    };
    let minimum = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let maximum = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    if maximum <= 0.75 {
        return HorizontalSurface::Floor;
    }
    if minimum >= observed_hex::TILE_LEVEL_HEIGHT - 0.75 {
        return HorizontalSurface::Ceiling;
    }
    // A slab standing off the ground is still something you walk on, and it
    // had been rendering as wall for the same reason a balcony is not at floor
    // level: the test was where the hull sits rather than what shape it is.
    //
    // That is why the facility has no gantries, no mezzanines, no balconies
    // and no dais tops - every one of them would have come out in the wall's
    // material, which in a district like Shadow Screen is the *lit* surface.
    // A deck is thin and wide; a pier is not, and the ratio separates them
    // without needing to know which cell either is in.
    if observed_traversal::render_mesh::is_horizontal_slab(points) {
        return HorizontalSurface::Floor;
    }
    HorizontalSurface::Wall
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_match::hex_wfc::HexPiecePart;
    use observed_traversal::StableColliderId;

    fn piece(points: Vec<Vec3>) -> HexStructurePiece {
        HexStructurePiece {
            id: StableColliderId(1),
            anchor: default(),
            source_cell: default(),
            role: HexStructureRole::Hall,
            part: HexPiecePart::Authored,
            tile: None,
            center: Vec3::ZERO,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::ConvexHull { points },
        }
    }

    #[test]
    fn horizontal_hulls_select_floor_wall_and_ceiling_material_classes() {
        assert_eq!(
            horizontal_surface(&piece(vec![Vec3::ZERO, Vec3::Y * 0.5])),
            HorizontalSurface::Floor
        );
        assert_eq!(
            horizontal_surface(&piece(vec![Vec3::ZERO, Vec3::Y * 4.0])),
            HorizontalSurface::Wall
        );
        assert_eq!(
            horizontal_surface(&piece(vec![
                Vec3::Y * 7.5,
                Vec3::Y * observed_hex::TILE_LEVEL_HEIGHT,
            ])),
            HorizontalSurface::Ceiling
        );
    }

    /// A balcony is a floor. It had been a wall, because it is three metres up
    /// and the classifier only asked how high the hull was.
    #[test]
    fn a_thin_slab_off_the_ground_is_a_deck_rather_than_a_wall() {
        assert_eq!(
            horizontal_surface(&piece(vec![
                Vec3::new(-2.0, 3.0, -1.5),
                Vec3::new(2.0, 3.4, 1.5),
            ])),
            HorizontalSurface::Floor
        );
    }

    /// And a pier is still a wall, at any height. Thickness alone would call a
    /// short post a deck, so the test is the ratio rather than the thickness.
    #[test]
    fn a_stub_at_the_same_height_is_still_a_wall() {
        assert_eq!(
            horizontal_surface(&piece(vec![
                Vec3::new(-0.3, 1.0, -0.3),
                Vec3::new(0.3, 4.0, 0.3),
            ])),
            HorizontalSurface::Wall
        );
    }
}
