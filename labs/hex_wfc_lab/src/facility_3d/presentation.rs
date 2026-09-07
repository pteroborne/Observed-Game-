//! Exact collider-list rendering and semantic Phase 92 materials.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_facility::hex_wfc::{HexSpace, HexWfcWorld};
use observed_hex::{HexFace, face_edge, hex_origin};
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use observed_style::{ArchitectureSurfaceRole, SurfaceRole};
use observed_traversal::ColliderShape;

use super::{FacilityState, FacilityVisual, landmarks};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::platform::collections::HashMap;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::LabState;

pub(super) fn rebuild_geometry(
    mut commands: Commands,
    world: Res<LabState>,
    mut state: ResMut<FacilityState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    visuals: Query<Entity, (With<FacilityVisual>, Without<DirectionalLight>)>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
    // One image per register for the whole rebuild, not one per hull.
    let mut weaves: HashMap<observed_content::ArchitectureRegister, Option<Handle<Image>>> =
        HashMap::new();
    // The production atlas can contain tens of thousands of authored hulls.
    // Retain the complete projection, but only instantiate the camera-local
    // streaming window as Bevy mesh entities.
    let pieces = state
        .snapshot
        .pieces
        .iter()
        .filter(|piece| landmarks::piece_visible(&state, piece.source_cell))
        .cloned()
        .collect::<Vec<_>>();
    for piece in &pieces {
        let Some(mesh) = piece_mesh(piece) else {
            continue;
        };
        // The district this cell belongs to, and the shell look that follows
        // from it.
        //
        // Without this the facility view paints every register the same, which
        // makes the one question a flythrough exists to answer - *do the
        // districts read as different places?* - unanswerable by construction.
        // `hex_shell_look` already carries the warning, written when the tile
        // lab had the same bug: a preview that reproduces the facility's
        // lighting and then paints it with its own colours is previewing a
        // different building.
        let register = world
            .world
            .architecture
            .get(&piece.source_cell)
            .copied()
            .unwrap_or(observed_content::ArchitectureRegister::Institutional);
        let treatment = observed_style::surface(surface_role(piece.role));
        // `hex_shell_surface` rather than `hex_shell_look` on the generic
        // structural treatment, and the difference is the whole point.
        //
        // `palette_tint_for_surface` blends 28% treatment with 72% palette. Pass
        // the *generic* treatment for a role and that 28% is identical in every
        // district, so the only thing separating two registers is their light
        // and ambient - and two pairs of registers share a key light exactly.
        // Measured that way the ten districts sit within 0.3 to 3.5 dE of each
        // other: a flythrough shows role variation wearing a faint district
        // wash. `architecture_surface(register, role)` is the per-register
        // treatment the palette was designed to be blended with, and it is what
        // makes Liminal Grid ochre rather than a slightly warmer grey.
        let look = observed_style::hex_shell_surface(register, architecture_role(piece.role));
        let semantic_color = look.base_color;
        let material = if state.collider_view {
            StandardMaterial {
                // Both tones are style-owned. Alternating by stable ID makes
                // individual authored hull boundaries explicit without
                // inventing another gameplay colour.
                base_color: if piece.id.0 % 2 == 0 {
                    semantic_color
                } else {
                    treatment.base_color
                },
                // Debug draws the exact collider mesh, but keeps lighting so
                // adjacent coplanar hulls remain visually separable.
                emissive: treatment.emissive * 0.02,
                perceptual_roughness: 1.0,
                ..default()
            }
        } else {
            StandardMaterial {
                base_color: semantic_color,
                // The register's weave, on the same terms the shell uses it.
                // This preview exists to reproduce the shell rather than invent
                // its own greys, and it had drifted: the game gained a drawn
                // per-register surface and this still painted flat colour, so
                // a flythrough could no longer show the one axis the districts
                // had just been given.
                base_color_texture: weave(&mut images, &mut weaves, register),
                // The authored hulls have no baked lightmaps. Keep their
                // semantic treatment legible at first-person scale, in the
                // district's own emissive rather than a neutral one.
                emissive: look.emissive,
                perceptual_roughness: observed_style::architecture(register).surface_roughness,
                ..default()
            }
        };
        commands.spawn((
            FacilityVisual,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(material)),
            Transform::from_translation(piece.center)
                .with_rotation(Quat::from_array(piece.rotation)),
            Name::new(format!("Hex {:?} collider {}", piece.role, piece.id.0)),
        ));
    }
    if landmarks::is_production(&state) {
        spawn_atlas_layer(
            &mut commands,
            &mut meshes,
            &mut materials,
            &world.world,
            HexSpace::Room,
            SurfaceRole::Spine,
            "Production atlas rooms",
        );
        spawn_atlas_layer(
            &mut commands,
            &mut meshes,
            &mut materials,
            &world.world,
            HexSpace::Hall,
            SurfaceRole::GantryDeck,
            "Production atlas halls",
        );
    }
    state.dirty = false;
}

fn spawn_atlas_layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world: &HexWfcWorld,
    space: HexSpace,
    role: SurfaceRole,
    name: &'static str,
) {
    let Some(mesh) = atlas_mesh(world, space) else {
        return;
    };
    let treatment = observed_style::surface(role);
    commands.spawn((
        FacilityVisual,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: treatment.base_color.with_alpha(0.34),
            emissive: treatment.emissive * 0.03,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Name::new(name),
    ));
}

fn atlas_mesh(world: &HexWfcWorld, space: HexSpace) -> Option<Mesh> {
    let cell_count = world
        .placements
        .values()
        .filter(|placement| placement.space == space)
        .count();
    if cell_count == 0 {
        return None;
    }
    let mut positions = Vec::with_capacity(cell_count * 18);
    let mut normals = Vec::with_capacity(cell_count * 18);
    let mut indices = Vec::with_capacity(cell_count * 18);
    for placement in world
        .placements
        .values()
        .filter(|placement| placement.space == space)
    {
        let center = Vec3::from_array(hex_origin(placement.coord)) - Vec3::Y * 0.24;
        for face in HexFace::LATERAL {
            let [a, b] = face_edge(face);
            let edge_a = center + Vec3::new(a.0 as f32, 0.0, a.1 as f32) * 0.88;
            let edge_b = center + Vec3::new(b.0 as f32, 0.0, b.1 as f32) * 0.88;
            let base = positions.len() as u32;
            positions.extend([center.to_array(), edge_a.to_array(), edge_b.to_array()]);
            normals.extend([[0.0, 1.0, 0.0]; 3]);
            indices.extend([base, base + 1, base + 2]);
        }
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// The architectural surface a structural piece presents.
///
/// Halls and rooms are what a body walks past, so they read as wall; a ramp is
/// what it walks on; the boundary shell is the lid over everything.
/// The register's weave as a repeating tile: a white field with darker lines
/// through it, multiplying the palette tint exactly as the shell's own albedo
/// does. `SurfaceWeave::None` draws nothing and the surface stays flat, which
/// is the Monolith's actual answer rather than a missing case.
fn weave(
    images: &mut Assets<Image>,
    cache: &mut HashMap<observed_content::ArchitectureRegister, Option<Handle<Image>>>,
    register: observed_content::ArchitectureRegister,
) -> Option<Handle<Image>> {
    if let Some(existing) = cache.get(&register) {
        return existing.clone();
    }
    let pattern = observed_style::architecture_weave(register);
    let made = if pattern.weave == observed_style::SurfaceWeave::None || pattern.lines == 0 {
        None
    } else {
        const N: usize = 128;
        let pitch = N as f32 / pattern.lines as f32;
        let half = (pitch * pattern.weight * 0.5).max(0.6);
        let on_line = |v: usize| ((v as f32 % pitch) - pitch * 0.5).abs() <= half;
        let struck_value = ((1.0 - pattern.depth) * 255.0).clamp(0.0, 255.0) as u8;
        let mut data = Vec::with_capacity(N * N * 4);
        for y in 0..N {
            for x in 0..N {
                let struck = match pattern.weave {
                    observed_style::SurfaceWeave::Courses => on_line(y),
                    observed_style::SurfaceWeave::Staves => on_line(x),
                    observed_style::SurfaceWeave::Grid => on_line(x) || on_line(y),
                    observed_style::SurfaceWeave::None => false,
                };
                let v = if struck { struck_value } else { 255 };
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: N as u32,
                height: N as u32,
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
    };
    cache.insert(register, made.clone());
    made
}

fn architecture_role(role: HexStructureRole) -> ArchitectureSurfaceRole {
    match role {
        HexStructureRole::Room | HexStructureRole::Hall | HexStructureRole::Shaft => {
            ArchitectureSurfaceRole::Wall
        }
        HexStructureRole::Ramp => ArchitectureSurfaceRole::Floor,
        HexStructureRole::Boundary => ArchitectureSurfaceRole::Ceiling,
    }
}

fn surface_role(role: HexStructureRole) -> SurfaceRole {
    match role {
        HexStructureRole::Room => SurfaceRole::Spine,
        HexStructureRole::Hall => SurfaceRole::GantryDeck,
        HexStructureRole::Ramp => SurfaceRole::SafeBypass,
        HexStructureRole::Shaft => SurfaceRole::WellshaftStone,
        HexStructureRole::Boundary => SurfaceRole::Plain,
    }
}

fn piece_mesh(piece: &HexStructurePiece) -> Option<Mesh> {
    match &piece.shape {
        ColliderShape::Cuboid { half } => Some(Cuboid::from_size(*half * 2.0).mesh().build()),
        ColliderShape::ConvexHull { points } => hull_mesh(points),
    }
}

/// The same convex hull the shell draws, built by the same code.
///
/// This used to hand-roll its own trimesh out of rapier and insert positions
/// and nothing else - no UVs. That was invisible for as long as the shell
/// painted flat colour, and became load-bearing the moment registers gained a
/// drawn surface: a texture on a mesh with no texture coordinates samples one
/// texel and the weave disappears. `ConvexRenderMesh` is what the game already
/// uses and it carries normals and UVs, so the preview now differs from the
/// shell in nothing that matters.
fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let data = observed_traversal::ConvexRenderMesh::from_convex_hull(hull)?;
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
