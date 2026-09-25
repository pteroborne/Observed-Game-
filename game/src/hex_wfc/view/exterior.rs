//! The facility seen from outside: a light skin of every built cell's exterior, and
//! the keel under every cell that hangs.
//!
//! Presentation streams detailed cells within a few tens of metres of the body, which
//! was plenty while every face onto the outside was a wall and the fog closed in at
//! thirty metres. Open edges changed that: look out of a loggia and the building
//! beyond the streaming radius has to exist. This skin is that building, drawn from
//! the solved facility alone - a storey-high face wherever a cell is walled against
//! the outside, the slab and ceiling bands and a lit lip where it opens, a roof over
//! whatever has sky above it - and a cell's skin is hidden while its detailed
//! geometry is resident, so the two never draw over each other.
//!
//! Keels are the exception: nothing detailed hangs under a cell, so a keel is drawn
//! whether or not the cell above it is resident. Proven first-person in
//! `labs/vista_lab`.
use std::collections::{BTreeMap, BTreeSet};

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::exposure::{Overhang, exposure};
use observed_facility::hex_wfc::{HexCoord, HexFace, HexWfcWorld};
use observed_hex::{CORNERS, FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT, hex_origin};
use observed_match::hex_wfc::{RAILED_BELOW_LEVEL, open_edges};

use super::assets::{HexWfcVisualAssets, MeshGroupKey};
use crate::GameState;

/// How far the skin stands outside the hex edge, so a detailed neighbour's own wall
/// never shares its plane.
const OUTSET: f32 = 1.02;
/// The ceiling band left above an open edge: the underside of the loggia's roof.
const CEILING_BAND: f32 = 0.5;
/// The deepest keel, metres.
const MAX_KEEL: f32 = 22.0;

/// Triangles for one material, in world space.
#[derive(Default)]
pub(super) struct SkinData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// The moon's light, baked into the skin: from where the moon hangs in the sky
/// (`open_air::toward_moon`), so faces rake into light and shadow rather than all
/// reading as one flat grey; baked because the facility carries no directional light.
fn moon_shade(normal: Vec3) -> f32 {
    let toward_moon = Vec3::from_array(observed_style::open_air::toward_moon());
    0.38 + 0.62 * normal.dot(toward_moon).max(0.0)
}

/// Metres per texture repeat, for any material that carries one.
const UV_METRES: f32 = 4.0;

impl SkinData {
    fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// A planar polygon, given in either winding: it is turned to face `outward`.
    fn polygon(&mut self, points: &[Vec3], outward: Vec3) {
        let n = points.len();
        if n < 3 {
            return;
        }
        let facing = (points[1] - points[0]).cross(points[2] - points[0]);
        let flip = facing.dot(outward) < 0.0;
        let base = u32::try_from(self.positions.len()).expect("skin fits u32 indices");
        let normal = outward.normalize_or_zero();
        // Planar world-space coordinates: across the face and up it for a wall, over
        // the ground plan for anything that faces up or down.
        let across = if normal.y.abs() > 0.7 {
            Vec3::X
        } else {
            Vec3::Y.cross(normal).normalize_or_zero()
        };
        let up = normal.cross(across);
        for point in points {
            self.positions.push(point.to_array());
            self.normals.push(normal.to_array());
            self.uvs
                .push([point.dot(across) / UV_METRES, point.dot(up) / UV_METRES]);
            let shade = moon_shade(normal);
            self.colors.push([shade, shade, shade, 1.0]);
        }
        for i in 1..n - 1 {
            #[allow(clippy::cast_possible_truncation)]
            let (b, c) = (base + i as u32, base + i as u32 + 1);
            if flip {
                self.indices.extend([base, c, b]);
            } else {
                self.indices.extend([base, b, c]);
            }
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// One cell's skin, by material.
#[derive(Default)]
pub(super) struct CellSkin {
    pub(super) walls: SkinData,
    pub(super) windows: SkinData,
    pub(super) caps: SkinData,
    pub(super) lips: SkinData,
    pub(super) keel: SkinData,
}

fn corner(index: usize) -> Vec3 {
    let (x, z) = CORNERS[index % 6];
    #[allow(clippy::cast_precision_loss)]
    Vec3::new(x as f32, 0.0, z as f32)
}

fn unbuilt(world: &HexWfcWorld, at: HexCoord, face: HexFace) -> bool {
    world.config.grid().neighbor(at, face).is_none_or(|next| {
        world
            .placements
            .get(&next)
            .is_none_or(|p| p.space.unbuilt())
    })
}

/// Deterministic per-cell variation for keel depth. No random source.
fn keel_hash(at: HexCoord) -> u32 {
    let mut h = u32::from(at.q).wrapping_mul(0x9E37_79B1)
        ^ u32::from(at.r).wrapping_mul(0x85EB_CA77)
        ^ u32::from(at.level).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// The skin of one built cell, or `None` for anything unbuilt.
#[must_use]
pub(super) fn cell_skin(world: &HexWfcWorld, at: HexCoord) -> Option<CellSkin> {
    if !world.placements.get(&at)?.space.built() {
        return None;
    }
    let o = Vec3::from_array(hex_origin(at));
    let open = open_edges(world, at);
    let mut skin = CellSkin::default();
    if let Some(axis) = open.and_then(|open| open.span) {
        span_skin(&mut skin, o, axis);
        return Some(skin);
    }
    for face in HexFace::LATERAL
        .into_iter()
        .filter(|&face| unbuilt(world, at, face))
    {
        let (a, b) = (
            o + corner(face.index()) * OUTSET,
            o + corner(face.index() + 1) * OUTSET,
        );
        let outward = ((a + b) * 0.5 - o).with_y(0.0);
        let band = |skin: &mut SkinData, lo: f32, hi: f32| {
            skin.polygon(
                &[
                    a + Vec3::Y * lo,
                    b + Vec3::Y * lo,
                    b + Vec3::Y * hi,
                    a + Vec3::Y * hi,
                ],
                outward,
            );
        };
        if open.is_some_and(|open| open.opens(face)) {
            band(&mut skin.walls, 0.0, FLOOR_SLAB_TOP);
            band(
                &mut skin.walls,
                TILE_LEVEL_HEIGHT - CEILING_BAND,
                TILE_LEVEL_HEIGHT,
            );
            band(&mut skin.lips, FLOOR_SLAB_TOP, FLOOR_SLAB_TOP + 0.08);
        } else {
            band(&mut skin.walls, 0.0, TILE_LEVEL_HEIGHT);
            // Somebody is home: up to three lit slits, a hand's width proud of the face.
            let slits = keel_hash(at) >> (face.index() * 2) & 3;
            for slot in 0..slits {
                #[allow(clippy::cast_precision_loss)]
                let t = (slot as f32 + 1.0) / (slits as f32 + 1.0);
                let run = b - a;
                let centre = a + run * t + outward.normalize_or_zero() * 0.05;
                let half = run.normalize_or_zero() * 0.28;
                skin.windows.polygon(
                    &[
                        centre - half + Vec3::Y * 2.6,
                        centre + half + Vec3::Y * 2.6,
                        centre + half + Vec3::Y * 5.0,
                        centre - half + Vec3::Y * 5.0,
                    ],
                    outward,
                );
            }
        }
    }
    let ring = |y: f32, inset: f32| -> Vec<Vec3> {
        (0..6)
            .map(|i| o + corner(i) * inset + Vec3::Y * y)
            .collect()
    };
    if unbuilt(world, at, HexFace::Up) {
        skin.caps.polygon(&ring(TILE_LEVEL_HEIGHT, OUTSET), Vec3::Y);
    }
    if unbuilt(world, at, HexFace::Down) {
        keel(&mut skin.keel, world, at, o);
    }
    Some(skin)
}

/// The underside of a hanging cell, and under a cell hanging over true void the
/// stepped keel below it.
fn keel(skin: &mut SkinData, world: &HexWfcWorld, at: HexCoord, o: Vec3) {
    let ring = |y: f32, inset: f32| -> Vec<Vec3> {
        (0..6)
            .map(|i| o + corner(i) * inset + Vec3::Y * y)
            .collect()
    };
    // Only what hangs over true void grows a keel. Over lower structure a keel would
    // hang into the gap between two towers at the height people look through it, so a
    // cell with something below it shows a flat underside instead.
    let over_void = matches!(
        exposure(world, at, RAILED_BELOW_LEVEL).map(|e| e.overhang),
        Some(Overhang::Hanging { onto: None, .. })
    );
    #[allow(clippy::cast_precision_loss)]
    let depth = (8.0 + (keel_hash(at) % 12) as f32).min(MAX_KEEL);
    if !over_void {
        skin.polygon(&ring(0.0, OUTSET), Vec3::NEG_Y);
        return;
    }
    let segments = 3;
    #[allow(clippy::cast_precision_loss)]
    let step = depth / segments as f32;
    let (mut inset, mut y) = (OUTSET, 0.0);
    for segment in 0..segments {
        let bottom = inset * 0.74;
        let (top_ring, bottom_ring) = (ring(y, inset), ring(y - step, bottom));
        for i in 0..6 {
            let j = (i + 1) % 6;
            let outward = (top_ring[i] + top_ring[j] - o * 2.0).with_y(0.0);
            skin.polygon(
                &[top_ring[i], top_ring[j], bottom_ring[j], bottom_ring[i]],
                outward,
            );
        }
        // The ledge each step leaves, and the point the last one ends at.
        let next = bottom * 0.84;
        let lower = if segment + 1 == segments {
            vec![o + Vec3::Y * (y - step)]
        } else {
            ring(y - step, next)
        };
        if lower.len() == 1 {
            skin.polygon(&bottom_ring, Vec3::NEG_Y);
        } else {
            for i in 0..6 {
                let j = (i + 1) % 6;
                skin.polygon(
                    &[bottom_ring[i], bottom_ring[j], lower[j], lower[i]],
                    Vec3::NEG_Y,
                );
            }
        }
        inset = next;
        y -= step;
    }
}

/// A walkway seen from afar: a narrow deck and its two lit lips.
fn span_skin(skin: &mut CellSkin, o: Vec3, axis: HexFace) {
    let (a, b) = (corner(axis.index()), corner(axis.index() + 1));
    let mid = (a + b) * 0.5;
    let along = mid.normalize();
    let across = Vec3::new(-along.z, 0.0, along.x);
    let (reach, w) = (mid.length(), 1.3);
    let top = Vec3::Y * FLOOR_SLAB_TOP;
    let deck = [
        o + top - along * reach - across * w,
        o + top + along * reach - across * w,
        o + top + along * reach + across * w,
        o + top - along * reach + across * w,
    ];
    skin.caps.polygon(&deck, Vec3::Y);
    skin.keel
        .polygon(&deck.map(|p| p - Vec3::Y * 0.35), Vec3::NEG_Y);
    for side in [-1.0, 1.0] {
        let edge = across * (w * side);
        skin.lips.polygon(
            &[
                o + top + edge - along * reach,
                o + top + edge + along * reach,
                o + top + edge + along * reach + Vec3::Y * 0.08,
                o + top + edge - along * reach + Vec3::Y * 0.08,
            ],
            across * side,
        );
    }
}

/// A cell's skin entities: the part hidden while the cell is resident, and its keel.
struct SkinEntities {
    shell: Option<Entity>,
    keel: Option<Entity>,
}

/// Where every cell's skin lives, so a relayout can replace exactly what changed.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct ExteriorSkin {
    cells: BTreeMap<HexCoord, SkinEntities>,
}

#[derive(Component)]
pub(in crate::hex_wfc) struct ExteriorShell(pub(in crate::hex_wfc) HexCoord);

fn spawn_cell(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &HexWfcVisualAssets,
    world: &HexWfcWorld,
    at: HexCoord,
) -> Option<SkinEntities> {
    let skin = cell_skin(world, at)?;
    let register = world
        .architecture
        .get(&at)
        .copied()
        .unwrap_or(ArchitectureRegister::ALL[0]);
    let has_shell = !(skin.walls.is_empty() && skin.caps.is_empty() && skin.lips.is_empty());
    let windows = skin.windows;
    let shell = has_shell.then(|| {
        commands
            .spawn((
                ExteriorShell(at),
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(GameState::HexWfc),
                Name::new(format!("Hex exterior q{} r{} L{}", at.q, at.r, at.level)),
            ))
            .id()
    });
    let mut part = |data: SkinData, group: MeshGroupKey, parent: Option<Entity>| {
        if data.is_empty() {
            return None;
        }
        let mut entity = commands.spawn((
            Mesh3d(meshes.add(data.mesh())),
            MeshMaterial3d(assets.material_for_group(register, group)),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
        match parent {
            Some(parent) => {
                entity.insert(ChildOf(parent));
            }
            None => {
                entity.insert((DespawnOnExit(GameState::HexWfc), Name::new("Hex keel")));
            }
        }
        Some(entity.id())
    };
    let keel = part(skin.keel, MeshGroupKey::Truss, None);
    if let Some(parent) = shell {
        part(skin.walls, MeshGroupKey::Facade, Some(parent));
        part(skin.caps, MeshGroupKey::Roof, Some(parent));
        part(skin.lips, MeshGroupKey::Lip, Some(parent));
        part(windows, MeshGroupKey::Window, Some(parent));
    }
    (shell.is_some() || keel.is_some()).then_some(SkinEntities { shell, keel })
}

/// Build the whole exterior once, on entering the facility.
pub(super) fn spawn_all(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &HexWfcVisualAssets,
    world: &HexWfcWorld,
) -> ExteriorSkin {
    let cells = world
        .placements
        .values()
        .filter(|placement| placement.space.built())
        .filter_map(|placement| {
            spawn_cell(commands, meshes, assets, world, placement.coord)
                .map(|entities| (placement.coord, entities))
        })
        .collect();
    ExteriorSkin { cells }
}

/// A relayout changed some cells; their skins, and their neighbours', follow. Runs
/// before [`super::sync_changed_geometry`] takes the pending set.
pub(in crate::hex_wfc) fn rebuild_changed(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    runtime: Res<crate::hex_wfc::sim::HexWfcRuntime>,
    assets: Option<Res<HexWfcVisualAssets>>,
    skin: Option<ResMut<ExteriorSkin>>,
) {
    let (Some(assets), Some(mut skin)) = (assets, skin) else {
        return;
    };
    if runtime.pending_visual_cells.is_empty() {
        return;
    }
    let world = &runtime.match_state.facility;
    let grid = world.config.grid();
    let touched: BTreeSet<HexCoord> = runtime
        .pending_visual_cells
        .iter()
        .flat_map(|&cell| {
            std::iter::once(cell).chain(
                HexFace::ALL
                    .into_iter()
                    .filter_map(move |face| grid.neighbor(cell, face)),
            )
        })
        .collect();
    for at in touched {
        if let Some(old) = skin.cells.remove(&at) {
            for entity in [old.shell, old.keel].into_iter().flatten() {
                commands.entity(entity).despawn();
            }
        }
        if let Some(entities) = spawn_cell(&mut commands, &mut meshes, &assets, world, at) {
            skin.cells.insert(at, entities);
        }
    }
}

/// A cell's exterior is hidden exactly while its detailed geometry is resident.
pub(in crate::hex_wfc) fn sync_visibility(
    residency: Option<Res<super::HexPresentationResidency>>,
    mut shells: Query<(&ExteriorShell, &mut Visibility)>,
) {
    let Some(residency) = residency else {
        return;
    };
    for (shell, mut visibility) in &mut shells {
        let wanted = if residency.resident.contains_key(&shell.0) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use observed_facility::hex_wfc::profile::SpaceMix;
    use observed_facility::hex_wfc::{
        HexArchetype, HexPlacement, HexSpace, HexWfcConfig, PortClass, lateral_bit,
    };

    use super::*;

    fn world(cells: &[(HexCoord, HexArchetype, u8)]) -> HexWfcWorld {
        let placements = cells
            .iter()
            .map(|&(coord, archetype, doors)| {
                (
                    coord,
                    HexPlacement {
                        coord,
                        space: HexSpace::Hall,
                        archetype,
                        doors,
                        up: PortClass::Sealed,
                        down: PortClass::Sealed,
                    },
                )
            })
            .collect();
        HexWfcWorld {
            seed: 1,
            generation: 0,
            config: HexWfcConfig {
                cols: 8,
                rows: 8,
                levels: 4,
                min_rooms: 0,
                max_rooms: 0,
                retry_budget: 1,
                min_room_distance: 1,
            },
            placements,
            blueprints: Vec::new(),
            architecture: BTreeMap::new(),
            cell_revisions: BTreeMap::new(),
            last_attempts: 1,
            authored_pins: Default::default(),
            space_mix: SpaceMix::baseline(),
            route_corridors: false,
            carve_unrouted: false,
            open_air: false,
            sealed: false,
        }
    }

    fn at(q: u16, r: u16, level: u8) -> HexCoord {
        HexCoord { q, r, level }
    }

    #[test]
    fn an_open_hall_shows_bands_and_a_lip_where_a_walled_one_shows_a_face() {
        let corner = HexArchetype::Corner;
        let doors = lateral_bit(HexFace::East) | lateral_bit(HexFace::SouthWest);
        let lone = world(&[(at(3, 3, 1), corner, doors)]);
        let skin = cell_skin(&lone, at(3, 3, 1)).expect("built");
        // Four open faces: two bands each, and a lip each; the two door faces have
        // nothing beyond them in this world, but a door is never an open edge.
        assert_eq!(skin.walls.indices.len() / 6, 2 * 4 + 2);
        assert_eq!(skin.lips.indices.len() / 6, 4);
        assert!(!skin.caps.is_empty(), "sky above: a roof");
        assert!(!skin.keel.is_empty(), "nothing below: a keel");
    }

    #[test]
    fn a_buried_cell_has_no_skin_to_draw() {
        let hall = HexArchetype::Expanse;
        let mut cells = vec![(at(3, 3, 1), hall, 0)];
        for face in HexFace::ALL {
            let (dq, dr, dl) = face.delta();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            cells.push((
                at((3 + dq) as u16, (3 + dr) as u16, (1 + dl) as u8),
                hall,
                0,
            ));
        }
        let buried = world(&cells);
        let skin = cell_skin(&buried, at(3, 3, 1)).expect("built");
        assert!(skin.walls.is_empty() && skin.caps.is_empty());
        assert!(skin.lips.is_empty() && skin.keel.is_empty());
    }

    #[test]
    fn every_polygon_faces_outward() {
        let lone = world(&[(
            at(3, 3, 1),
            HexArchetype::Corner,
            lateral_bit(HexFace::East),
        )]);
        let skin = cell_skin(&lone, at(3, 3, 1)).expect("built");
        for data in [&skin.walls, &skin.caps, &skin.lips, &skin.keel] {
            for triangle in data.indices.chunks(3) {
                let [a, b, c] =
                    [0, 1, 2].map(|k| Vec3::from_array(data.positions[triangle[k] as usize]));
                let normal = Vec3::from_array(data.normals[triangle[0] as usize]);
                let facing = (b - a).cross(c - a);
                assert!(facing.dot(normal) >= -1e-4, "a back-facing triangle");
            }
        }
    }
}
