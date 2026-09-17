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
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole, HexTrimKind};
use observed_style::{self as style, ArchitectureSurfaceRole, SurfaceRole};
use observed_traversal::{ColliderShape, ConvexRenderMesh};

use crate::view::assets::ContentScene;
use crate::view::environment::{cuboid_mesh, load_content_scene, load_repeating_texture};

#[derive(Clone)]
pub(in crate::hex_wfc) struct RegisterMaterials {
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    ceiling: Handle<StandardMaterial>,
    fixture: Handle<StandardMaterial>,
    ramp: Handle<StandardMaterial>,
    shaft: Handle<StandardMaterial>,
    boundary: Handle<StandardMaterial>,
}

impl RegisterMaterials {
    fn for_piece(
        &self,
        role: HexStructureRole,
        horizontal_surface: HorizontalSurface,
    ) -> Handle<StandardMaterial> {
        match role {
            HexStructureRole::Room | HexStructureRole::Hall => match horizontal_surface {
                HorizontalSurface::Floor => self.floor.clone(),
                HorizontalSurface::Wall => self.wall.clone(),
                HorizontalSurface::Ceiling => self.ceiling.clone(),
            },
            HexStructureRole::Ramp => self.ramp.clone(),
            HexStructureRole::Shaft => self.shaft.clone(),
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
    hull_cache: HashMap<(String, usize), Handle<Mesh>>,
    cuboid_cache: HashMap<[u32; 3], Handle<Mesh>>,
    /// The doorway model stood in a named threshold. `None` when the asset is
    /// absent, which is a missing frame rather than a missing facility - the
    /// aperture is authored into the room's own geometry either way.
    pub(in crate::hex_wfc) threshold_gate: Option<ContentScene>,
}

impl HexWfcVisualAssets {
    pub(in crate::hex_wfc) fn load(
        asset_server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
        content: &observed_content::ContentManifest,
    ) -> Self {
        let wall_texture = load_repeating_texture(asset_server, observed_assets::WALL.path);
        let floor_texture = load_repeating_texture(asset_server, observed_assets::FLOOR.path);
        let registers = ArchitectureRegister::ALL
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
                let weave = weave_texture(images, style::architecture_weave(register));
                let mut tinted = |look: style::HexSurfaceLook,
                                  texture: Option<Handle<Image>>,
                                  mask_emission: bool| {
                    materials.add(StandardMaterial {
                        base_color: look.base_color,
                        emissive: look.emissive,
                        emissive_texture: if mask_emission { texture.clone() } else { None },
                        unlit: look.unlit,
                        base_color_texture: if look.textured { texture } else { None },
                        perceptual_roughness: palette.surface_roughness,
                        ..default()
                    })
                };
                RegisterMaterials {
                    floor: tinted(
                        style::hex_shell_surface(register, ArchitectureSurfaceRole::Floor),
                        floor_texture.clone(),
                        false,
                    ),
                    wall: tinted(
                        style::hex_shell_surface(register, ArchitectureSurfaceRole::Wall),
                        weave.clone().or_else(|| wall_texture.clone()),
                        register == ArchitectureRegister::ShadowScreen,
                    ),
                    ceiling: tinted(
                        style::hex_shell_surface(register, ArchitectureSurfaceRole::Ceiling),
                        wall_texture.clone(),
                        false,
                    ),
                    fixture: tinted(
                        style::hex_shell_surface(
                            register,
                            ArchitectureSurfaceRole::PracticalFixture,
                        ),
                        None,
                        false,
                    ),
                    ramp: tinted(
                        style::hex_shell_look(&style::surface(SurfaceRole::SafeBypass), register),
                        floor_texture.clone(),
                        false,
                    ),
                    shaft: tinted(
                        style::hex_shell_look(
                            &style::surface(SurfaceRole::WellshaftStone),
                            register,
                        ),
                        wall_texture.clone(),
                        false,
                    ),
                    boundary: tinted(
                        style::hex_shell_look(&style::surface(SurfaceRole::Wall), register),
                        wall_texture.clone(),
                        false,
                    ),
                }
            })
            .collect();
        Self {
            registers,
            hull_cache: HashMap::new(),
            cuboid_cache: HashMap::new(),
            threshold_gate: load_content_scene(asset_server, content, "kenney_gate"),
        }
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
}

/// Convert shared engine-independent render data into Bevy's mesh format.
pub(super) fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let data = ConvexRenderMesh::from_convex_hull(hull)?;
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
        .with_inserted_indices(Indices::U32(data.indices)),
    )
}

/// Draw a register's weave as a repeating tile.
///
/// A white field with darker lines through it, which multiplies the palette
/// tint the same way the shared `wall.png` does - so a weave darkens a surface
/// where its joints are and leaves the district's own colour everywhere else.
/// `SurfaceWeave::None` returns nothing, and the caller falls back to the
/// shared albedo.
fn weave_texture(
    images: &mut Assets<Image>,
    pattern: style::SurfacePattern,
) -> Option<Handle<Image>> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let data = style::surface_weave_rgba(pattern)?;
    let n = style::SURFACE_WEAVE_SIZE;
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    Some(images.add(image))
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
    use observed_traversal::StableColliderId;

    fn piece(points: Vec<Vec3>) -> HexStructurePiece {
        HexStructurePiece {
            id: StableColliderId(1),
            anchor: default(),
            source_cell: default(),
            role: HexStructureRole::Hall,
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
