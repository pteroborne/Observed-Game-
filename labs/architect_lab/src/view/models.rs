//! Shared board/hand models. The lab owns a port graph, not authored tile IDs:
//! choose an authored one-level representative with exactly those lateral ports.
//! Uncovered masks get an honest low-wall topology shell, never a mismatched tile.
use bevy::prelude::*;
use observed_authoring::{CompiledTileCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_hex::{HexFace, PortClass};
use observed_style::architect::{Role, color};
use observed_traversal::ConvexRenderMesh;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone)]
pub(crate) struct Part {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub transform: Transform,
}
#[derive(Resource)]
pub(crate) struct Models {
    pub cube: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub pyramid: Handle<Mesh>,
    pub slab: Handle<Mesh>,
    signals: BTreeMap<Role, Handle<StandardMaterial>>,
    rooms: BTreeMap<(ArchitectureRegister, u8), Vec<Part>>,
    pub ghost: Handle<StandardMaterial>,
}
fn catalog() -> &'static [TilePrototype] {
    static TILES: OnceLock<Vec<TilePrototype>> = OnceLock::new();
    TILES.get_or_init(|| {
        CompiledTileCatalog::from_ron(include_str!(
            "../../../../assets/tiles/compiled_catalog.ron"
        ))
        .expect("committed tile catalog parses")
        .runtime_catalog(&ArchitectureRegister::ALL.map(|r| r.slug()))
        .expect("committed tile catalog expands")
        .cells
    })
}
pub(super) fn matching_tile(
    register: ArchitectureRegister,
    mask: u8,
) -> Option<&'static TilePrototype> {
    catalog()
        .iter()
        .filter(|tile| {
            tile.levels == 1
                && tile.key.register == register.slug()
                && tile.signature.port(HexFace::Up) == PortClass::Sealed
                && tile.signature.port(HexFace::Down) == PortClass::Sealed
                && HexFace::LATERAL.into_iter().all(|f| {
                    (tile.signature.port(f) != PortClass::Sealed) == (mask & (1 << f.index()) != 0)
                })
        })
        .min_by_key(|tile| tile.key.variant)
}
impl Models {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let cube = meshes.add(Cuboid::default());
        let sphere = meshes.add(Sphere::new(0.9));
        let pyramid_points = [
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(-1.0, 0.0, 1.0),
            Vec3::new(0.0, 2.4, 0.0),
        ];
        let pyramid = hull_mesh(meshes, &pyramid_points);
        let slab = hull_mesh(
            meshes,
            &observed_hex::prism_hull(0.5, 0.98).map(Vec3::from_array),
        );
        let ghost = materials.add(StandardMaterial {
            base_color: color(Role::Selected).with_alpha(0.35),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        Self {
            cube,
            sphere,
            pyramid,
            slab,
            signals: BTreeMap::new(),
            rooms: BTreeMap::new(),
            ghost,
        }
    }
    pub fn signal(
        &mut self,
        role: Role,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.signals
            .entry(role)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: color(role),
                    unlit: true,
                    ..default()
                })
            })
            .clone()
    }
    pub fn room(
        &mut self,
        register: ArchitectureRegister,
        mask: u8,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Vec<Part> {
        if let Some(parts) = self.rooms.get(&(register, mask)) {
            return parts.clone();
        }
        let floor = surface(
            materials,
            register,
            observed_style::ArchitectureSurfaceRole::Floor,
        );
        let wall = surface(
            materials,
            register,
            observed_style::ArchitectureSurfaceRole::Wall,
        );
        let fixture = self.signal(Role::Fixture, materials);
        let mut parts = Vec::new();
        if let Some(tile) = matching_tile(register, mask) {
            for hull in &tile.hulls {
                let (min, max, centroid) = observed_cutaway::measure(hull);
                let region = observed_style::iso::hull_region(min, max, centroid);
                if matches!(region, observed_style::iso::HullRegion::Ceiling) {
                    continue;
                }
                let is_floor = matches!(region, observed_style::iso::HullRegion::Floor);
                // Keep structural shape but cap walls: the exact door openings remain.
                let near = Vec2::new(centroid.x, centroid.z)
                    .dot(observed_style::iso::detent_bearing(0))
                    > 0.0;
                if matches!(region, observed_style::iso::HullRegion::Perimeter) && near {
                    continue;
                }
                let cap = if is_floor {
                    8.0
                } else if near {
                    1.5
                } else {
                    2.6
                };
                let points: Vec<Vec3> = hull
                    .iter()
                    .map(|p| Vec3::new(p.x, p.y.min(cap), p.z))
                    .collect();
                if let Some(render) = ConvexRenderMesh::from_convex_hull(&points)
                    && let Some(mut mesh) = observed_cutaway::mesh_from(&render)
                {
                    // Dark sawn faces keep thick structural masses subordinate
                    // to walkable floor. Vertical authored faces retain their material.
                    let colors: Vec<[f32; 4]> = render
                        .normals
                        .iter()
                        .map(|n| {
                            if !is_floor && n[1] > 0.5 {
                                observed_style::architect::CUT_SURFACE_MULTIPLIER
                            } else {
                                [1.0; 4]
                            }
                        })
                        .collect();
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
                    parts.push(Part {
                        mesh: meshes.add(mesh),
                        material: if is_floor {
                            floor.clone()
                        } else {
                            wall.clone()
                        },
                        transform: Transform::IDENTITY,
                    });
                }
            }
        } else {
            parts.push(Part {
                mesh: self.slab.clone(),
                material: floor.clone(),
                transform: Transform::IDENTITY,
            });
            for face in HexFace::LATERAL {
                let (a, b) = edge(face);
                let a = Vec3::new(a[0], 0.0, a[1]);
                let b = Vec3::new(b[0], 0.0, b[1]);
                let delta = b - a;
                let open = mask & (1 << face.index()) != 0;
                let segments: &[(f32, f32)] = if open {
                    &[(0.0, 0.25), (0.75, 1.0)]
                } else {
                    &[(0.0, 1.0)]
                };
                for &(lo, hi) in segments {
                    let center = a + delta * ((lo + hi) * 0.5) + Vec3::Y * 1.25;
                    parts.push(Part {
                        mesh: self.cube.clone(),
                        material: wall.clone(),
                        transform: Transform::from_translation(center)
                            .with_rotation(Quat::from_rotation_y(-delta.z.atan2(delta.x)))
                            .with_scale(Vec3::new(delta.length() * (hi - lo), 1.5, 0.35)),
                    });
                }
            }
        }
        for face in HexFace::LATERAL
            .into_iter()
            .filter(|f| mask & (1 << f.index()) != 0)
        {
            let (a, b) = edge(face);
            let midpoint = Vec3::new((a[0] + b[0]) * 0.5, 0.6, (a[1] + b[1]) * 0.5);
            let tangent = Vec3::new(b[0] - a[0], 0.0, b[1] - a[1]).normalize();
            parts.push(Part {
                mesh: self.cube.clone(),
                material: fixture.clone(),
                transform: Transform::from_translation(midpoint * Vec3::new(0.92, 1.0, 0.92))
                    .with_rotation(Quat::from_rotation_y(-tangent.z.atan2(tangent.x)))
                    .with_scale(Vec3::new(2.2, 0.06, 0.15)),
            });
        }
        self.rooms.insert((register, mask), parts.clone());
        parts
    }
}
fn surface(
    materials: &mut Assets<StandardMaterial>,
    register: ArchitectureRegister,
    role: observed_style::ArchitectureSurfaceRole,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: observed_style::architect::surface(
            register,
            matches!(role, observed_style::ArchitectureSurfaceRole::Floor),
        ),
        perceptual_roughness: 0.9,
        ..default()
    })
}
fn hull_mesh(meshes: &mut Assets<Mesh>, points: &[Vec3]) -> Handle<Mesh> {
    meshes.add(
        observed_cutaway::mesh_from(
            &ConvexRenderMesh::from_convex_hull(points).expect("nondegenerate model hull"),
        )
        .expect("triangulated model"),
    )
}

fn edge(face: HexFace) -> ([f32; 2], [f32; 2]) {
    let [a, b] = observed_hex::face_edge(face);
    ([a.0 as f32, a.1 as f32], [b.0 as f32, b.1 as f32])
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ModelAssets<'w> {
    pub models: ResMut<'w, Models>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
}
