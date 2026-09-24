//! Open edges: where a hall meets the outside, its wall comes down.
//!
//! Proven first-person in `labs/vista_lab`. A hall cell whose lateral face borders an
//! unbuilt cell (or the edge of the lattice) loses the authored wall on that face and
//! gains a lit lip, and below [`RAILED_BELOW_LEVEL`] a railing: the hall becomes a
//! loggia under its own ceiling. A straight hall whose four flanks are all open
//! becomes a narrow walkway with nothing either side.
//!
//! **Why unbuilt rather than air.** The sky is air, and air is what the view draws
//! beyond an open edge; but geometry follows *built versus unbuilt*, not air versus
//! rock. A relayout only ever changes built cells inside its region, and the
//! observation halo guarantees nobody is watching a neighbour of that region, so a
//! wall can only come down where nobody is looking. Air and rock can flip anywhere a
//! pocket is sealed or opened; if walls followed them, a watched loggia could close.
//!
//! **How a wall is found.** Authored hulls carry no face, so a hull belongs to the
//! face whose sector (the triangle from the cell centre to that edge) holds its plan
//! centroid; opening a face removes what stands in its sector between floor and
//! ceiling. Most halls are corridors narrower than their cell, so what stands there is
//! often a solid mass running from the corridor out to the hex edge rather than a
//! thin wall, and all of it has to go. Pinned across the whole corpus by
//! `opening_a_face_leaves_it_clear_across_the_corpus`: with every sealed face open,
//! nothing stands in any of them; with one open, fewer than one in a hundred keep a
//! corner mass belonging to the face next door.
use glam::{Quat, Vec2, Vec3};
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexWfcWorld};
use observed_hex::{FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT, face_edge};
use observed_traversal::ColliderShape;

pub use observed_facility::hex_wfc::exposure::can_open;

use super::HexPiecePart;

/// Open edges on this level and above carry no railing. The top three storeys of the
/// production lattice: the design's "higher floors draw increasingly unsafe
/// architecture", told the way a first-person eye cannot miss.
pub const RAILED_BELOW_LEVEL: u8 = 5;
/// A walkway's deck width, metres.
pub const WALKWAY_WIDTH: f32 = 2.6;
/// Railing height above the walking surface, metres.
pub const RAIL_HEIGHT: f32 = 1.05;
/// A hull reaching less than this above the floor slab is floor, not wall.
const ABOVE_FLOOR: f32 = 0.05;
/// A hull starting within this of the level's top is ceiling, not wall.
const OVERHEAD: f32 = 1.0;
/// A hull whose plan centroid is this close to the cell centre stands in the middle of
/// the hall, against no face.
const CENTRAL: f32 = 2.0;

/// Which of a hall cell's faces open onto the outside, and how.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenEdges {
    /// Lateral faces that open, bit `face.index()`.
    pub faces: u8,
    pub railed: bool,
    /// A straight hall open on all four flanks: drawn as a walkway along this axis.
    pub span: Option<HexFace>,
}

impl OpenEdges {
    #[must_use]
    pub const fn opens(&self, face: HexFace) -> bool {
        face.is_lateral() && self.faces & (1 << face.index()) != 0
    }
}

/// One added piece, in world space.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgePiece {
    pub part: HexPiecePart,
    pub center: Vec3,
    pub rotation: Quat,
    pub shape: ColliderShape,
}

impl EdgePiece {
    /// The piece as hull points relative to `origin`, with its rotation applied.
    #[must_use]
    pub fn local_hull(&self, origin: Vec3) -> Vec<Vec3> {
        let offset = self.center - origin;
        match &self.shape {
            ColliderShape::Cuboid { half } => {
                let mut points = Vec::with_capacity(8);
                for x in [-half.x, half.x] {
                    for y in [-half.y, half.y] {
                        for z in [-half.z, half.z] {
                            points.push(offset + self.rotation * Vec3::new(x, y, z));
                        }
                    }
                }
                points
            }
            ColliderShape::ConvexHull { points } => points
                .iter()
                .map(|&point| offset + self.rotation * point)
                .collect(),
        }
    }
}

fn opens_outward(world: &HexWfcWorld, at: HexCoord, face: HexFace) -> bool {
    world.config.grid().neighbor(at, face).is_none_or(|next| {
        world
            .placements
            .get(&next)
            .is_none_or(|p| p.space.unbuilt())
    })
}

/// The open edges of one cell, or `None` if it keeps every wall.
///
/// Only cells that [`can_open`] do.
#[must_use]
pub fn open_edges(world: &HexWfcWorld, at: HexCoord) -> Option<OpenEdges> {
    let placement = world.placements.get(&at)?;
    if !can_open(placement) {
        return None;
    }
    let faces = HexFace::LATERAL
        .into_iter()
        .filter(|&face| !placement.is_open(face) && opens_outward(world, at, face))
        .fold(0u8, |mask, face| mask | (1 << face.index()));
    if faces == 0 {
        return None;
    }
    let doors: Vec<HexFace> = HexFace::LATERAL
        .into_iter()
        .filter(|&face| placement.is_open(face))
        .collect();
    let span = match doors[..] {
        [a, b]
            if placement.archetype == HexArchetype::Straight
                && b == a.opposite()
                && faces.count_ones() == 4 =>
        {
            Some(a)
        }
        _ => None,
    };
    Some(OpenEdges {
        faces,
        railed: at.level < RAILED_BELOW_LEVEL,
        span,
    })
}

/// Railing for the faces of a cell that stays walled but stands on the edge of the
/// lattice.
///
/// The arena shell used to close the lattice's rim, and some room prefabs lean on
/// that: they are authored with a port that the solver seals at the edge, and the
/// shell was what stood in the opening. With the shell gone, every non-opening cell
/// gets a lip and, below [`RAILED_BELOW_LEVEL`], a railing along its rim faces. Where
/// the prefab has a wall there, they sit inside it; where it has an opening, the
/// opening becomes a balcony.
#[must_use]
pub fn rim_pieces(world: &HexWfcWorld, at: HexCoord) -> Vec<EdgePiece> {
    let grid = world.config.grid();
    let origin = Vec3::from_array(observed_hex::hex_origin(at));
    HexFace::LATERAL
        .into_iter()
        .filter(|&face| grid.neighbor(at, face).is_none())
        .flat_map(|face| edge_pieces(origin, face, at.level < RAILED_BELOW_LEVEL))
        .collect()
}

/// The inward unit normal of a face in plan, and the distance from the cell centre
/// to that face.
fn face_frame(face: HexFace) -> (Vec2, f32) {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let mid = Vec2::new((a.0 + b.0) as f32, (a.1 + b.1) as f32) * 0.5;
    (mid.normalize(), mid.length())
}

/// The face a cell-local hull stands against: the one whose sector (the triangle from
/// the cell centre to that edge) holds its plan centroid. `None` for a hull standing
/// in the middle of the cell, which belongs to no face.
fn sector_of(hull: &[Vec3]) -> Option<HexFace> {
    #[allow(clippy::cast_precision_loss)]
    let centroid = hull
        .iter()
        .fold(Vec2::ZERO, |sum, p| sum + Vec2::new(p.x, p.z))
        / hull.len() as f32;
    if centroid.length() < CENTRAL {
        return None;
    }
    HexFace::LATERAL.into_iter().max_by(|&a, &b| {
        centroid
            .dot(face_frame(a).0)
            .total_cmp(&centroid.dot(face_frame(b).0))
    })
}

/// Whether a cell-local authored hull is structure standing against a face that
/// `open` opens: above the floor, below the ceiling, and in that face's sector.
///
/// By sector rather than by distance from the edge, because most halls are corridors
/// narrower than their cell: a corridor's side wall is the inner face of a solid mass
/// that runs out to the hex edge, and all of it has to go for the face to open. What a
/// removed mass uncovers beside a face that stays is the neighbour's own perimeter,
/// never a hole to the outside: every cell authors its own walls. A door's lintel and
/// frame stand in the door's own sector, which never opens.
#[must_use]
pub fn is_opened_wall(hull: &[Vec3], open: &OpenEdges) -> bool {
    let (bottom, top) = hull.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    top > FLOOR_SLAB_TOP + ABOVE_FLOOR
        && bottom < TILE_LEVEL_HEIGHT - OVERHEAD
        && sector_of(hull).is_some_and(|face| open.opens(face))
}

/// Where a removed hull stood, the floor it was also carrying.
///
/// Most halls are corridors narrower than their cell, and the solid masses either
/// side of the corridor run from the floor slab's underside to the ceiling: they are
/// floor as much as wall. Removing one opens the face and holes the floor, which is
/// how the first production runs walked bots straight through it onto the roof
/// below. The filler is the removed hull's own plan footprint as a slab, its top a
/// centimetre under the floor so it never fights the tile's slab where they overlap.
/// `hull` is cell-local; `origin` is the cell's world origin.
#[must_use]
pub fn floor_under(hull: &[Vec3], origin: Vec3) -> Option<EdgePiece> {
    // Masses stand either from the slab's underside or from its top; either way, where
    // they stood is floor once they are gone.
    let bottom = hull.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    if bottom > FLOOR_SLAB_TOP + 0.1 {
        return None;
    }
    let top = FLOOR_SLAB_TOP - 0.01;
    let points = hull
        .iter()
        .flat_map(|p| [Vec3::new(p.x, 0.0, p.z), Vec3::new(p.x, top, p.z)])
        .collect();
    Some(EdgePiece {
        part: HexPiecePart::Authored,
        center: origin,
        rotation: Quat::IDENTITY,
        shape: ColliderShape::ConvexHull { points },
    })
}

fn block(part: HexPiecePart, center: Vec3, along: Vec3, half: Vec3) -> EdgePiece {
    EdgePiece {
        part,
        center,
        rotation: Quat::from_rotation_y(-along.z.atan2(along.x)),
        shape: ColliderShape::Cuboid { half },
    }
}

/// The lip, and when railed the railing, along one open face of a cell whose world
/// origin is `origin`.
#[must_use]
pub fn edge_pieces(origin: Vec3, face: HexFace, railed: bool) -> Vec<EdgePiece> {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let (a, b) = (
        Vec3::new(a.0 as f32, 0.0, a.1 as f32),
        Vec3::new(b.0 as f32, 0.0, b.1 as f32),
    );
    let run = b - a;
    let length = run.length();
    let along = run / length;
    let (normal, _) = face_frame(face);
    let inward = -Vec3::new(normal.x, 0.0, normal.y);
    let at = |inset: f32, t: f32, y: f32| origin + a + run * t + inward * inset + Vec3::Y * y;
    let top = FLOOR_SLAB_TOP;
    let mut pieces = vec![block(
        HexPiecePart::Lip,
        at(0.08, 0.5, top + 0.02),
        along,
        Vec3::new(length * 0.5, 0.02, 0.05),
    )];
    if railed {
        pieces.push(block(
            HexPiecePart::Guard,
            at(0.3, 0.5, top + RAIL_HEIGHT * 0.5),
            along,
            Vec3::new(length * 0.5, RAIL_HEIGHT * 0.5, 0.04),
        ));
        pieces.push(block(
            HexPiecePart::Rail,
            at(0.3, 0.5, top + RAIL_HEIGHT - 0.03),
            along,
            Vec3::new(length * 0.5, 0.03, 0.05),
        ));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let posts = (length / 2.2).ceil() as u32;
        for post in 0..=posts {
            #[allow(clippy::cast_precision_loss)]
            let t = (post as f32 / posts as f32).clamp(0.02, 0.98);
            pieces.push(block(
                HexPiecePart::Rail,
                at(0.3, t, top + RAIL_HEIGHT * 0.5),
                along,
                Vec3::new(0.05, RAIL_HEIGHT * 0.5, 0.05),
            ));
        }
    }
    pieces
}

/// A narrow walkway along `axis` through a cell whose world origin is `origin`: deck,
/// lips on both flanks, a railing when railed, and a single truss beneath.
#[must_use]
pub fn span_pieces(origin: Vec3, axis: HexFace, railed: bool) -> Vec<EdgePiece> {
    let (normal, reach) = face_frame(axis);
    let along = Vec3::new(normal.x, 0.0, normal.y);
    let across = Vec3::new(-along.z, 0.0, along.x);
    let half_length = reach + 0.15;
    let w = WALKWAY_WIDTH * 0.5;
    let top = FLOOR_SLAB_TOP;
    let mut pieces = vec![block(
        HexPiecePart::Walkway,
        origin + Vec3::Y * (top - 0.175),
        along,
        Vec3::new(half_length, 0.175, w),
    )];
    for side in [-1.0, 1.0] {
        let lateral = across * side;
        pieces.push(block(
            HexPiecePart::Lip,
            origin + lateral * (w - 0.05) + Vec3::Y * (top + 0.02),
            along,
            Vec3::new(half_length, 0.02, 0.05),
        ));
        if railed {
            let rail = origin + lateral * (w - 0.05);
            pieces.push(block(
                HexPiecePart::Guard,
                rail + Vec3::Y * (top + RAIL_HEIGHT * 0.5),
                along,
                Vec3::new(half_length, RAIL_HEIGHT * 0.5, 0.04),
            ));
            pieces.push(block(
                HexPiecePart::Rail,
                rail + Vec3::Y * (top + RAIL_HEIGHT - 0.03),
                along,
                Vec3::new(half_length, 0.03, 0.05),
            ));
            for post in -3..=3 {
                #[allow(clippy::cast_precision_loss)]
                let s = post as f32 * half_length / 3.2;
                pieces.push(block(
                    HexPiecePart::Rail,
                    rail + along * s + Vec3::Y * (top + RAIL_HEIGHT * 0.5),
                    along,
                    Vec3::new(0.05, RAIL_HEIGHT * 0.5, 0.05),
                ));
            }
        }
    }
    let mut truss = Vec::new();
    for end in [-1.0, 1.0] {
        let reach = along * (half_length * end);
        for side in [-1.0, 1.0] {
            truss.push(reach + across * (w * 0.8 * side) + Vec3::Y * (top - 0.34));
        }
        truss.push(along * ((half_length - 1.4) * end) + Vec3::Y * (top - 2.1));
    }
    pieces.push(EdgePiece {
        part: HexPiecePart::Truss,
        center: origin,
        rotation: Quat::IDENTITY,
        shape: ColliderShape::ConvexHull { points: truss },
    });
    pieces
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use glam::{Vec2, Vec3};
    use observed_authoring::TilePrototype;
    use observed_content::ArchitectureRegister;
    use observed_facility::hex_wfc::profile::SpaceMix;
    use observed_facility::hex_wfc::{
        HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld,
        PortClass, lateral_bit,
    };
    use observed_hex::{FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT, hex_origin};
    use observed_traversal::rapier_controller::step_character;
    use observed_traversal::{FpsBody, FpsConfig};
    use player_input::PlayerIntent;

    use super::{OpenEdges, RAILED_BELOW_LEVEL, is_opened_wall, open_edges, sector_of};
    use crate::hex_wfc::{HexPiecePart, HexWfcGeometrySnapshot};

    /// Every one-level hall tile a facility can place: the production catalog and the
    /// compatibility cells the tests build from.
    fn hall_tiles() -> Vec<TilePrototype> {
        crate::hex_wfc::test_catalog()
            .cells
            .iter()
            .chain(crate::hex_wfc::test_tiles().iter())
            .filter(|tile| {
                tile.levels == 1
                    && tile.key.archetype.starts_with("hall_")
                    && tile.signature.port(HexFace::Up) == PortClass::Sealed
                    && tile.signature.port(HexFace::Down) == PortClass::Sealed
            })
            .cloned()
            .collect()
    }

    fn opening(faces: impl IntoIterator<Item = HexFace>) -> OpenEdges {
        OpenEdges {
            faces: faces
                .into_iter()
                .fold(0, |mask, face| mask | (1 << face.index())),
            railed: true,
            span: None,
        }
    }

    fn height(hull: &[Vec3]) -> (f32, f32) {
        hull.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        })
    }

    /// Points just inside `face`, clear of its corner pillars, at waist and eye height.
    fn face_samples(face: HexFace) -> Vec<Vec3> {
        let [a, b] = observed_hex::face_edge(face);
        #[allow(clippy::cast_precision_loss)]
        let (a, b) = (
            Vec2::new(a.0 as f32, a.1 as f32),
            Vec2::new(b.0 as f32, b.1 as f32),
        );
        let inward = -((a + b) * 0.5).normalize();
        let mut samples = Vec::new();
        for step in 2..=8 {
            #[allow(clippy::cast_precision_loss)]
            let t = step as f32 / 10.0;
            let plan = a + (b - a) * t + inward * 0.3;
            for y in [FLOOR_SLAB_TOP + 1.0, FLOOR_SLAB_TOP + 1.7] {
                samples.push(Vec3::new(plan.x, y, plan.y));
            }
        }
        samples
    }

    /// Whether a (vertically extruded) hull stands at `point`.
    fn blocks(hull: &[Vec3], point: Vec3) -> bool {
        let (lo, hi) = height(hull);
        (lo..=hi).contains(&point.y)
            && observed_traversal::point_in_convex_plan_hull(hull, Vec2::new(point.x, point.z))
    }

    /// The corpus assumption the rule rests on, pinned: a tile authored some other way
    /// must fail here, not quietly keep a wall standing in an open face.
    #[test]
    fn opening_a_face_leaves_it_clear_across_the_corpus() {
        let tiles = hall_tiles();
        assert!(tiles.len() > 20, "{} hall tiles", tiles.len());
        let (mut faces, mut corners) = (0, 0);
        for tile in &tiles {
            let sealed: Vec<HexFace> = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == PortClass::Sealed)
                .collect();
            let every = opening(sealed.iter().copied());
            for &face in &sealed {
                faces += 1;
                let standing = |open: &OpenEdges| -> Vec<&Vec<Vec3>> {
                    tile.hulls
                        .iter()
                        .filter(|hull| !is_opened_wall(hull, open))
                        .filter(|hull| face_samples(face).iter().any(|&p| blocks(hull, p)))
                        .collect()
                };
                assert!(
                    standing(&every).is_empty(),
                    "{:?}: {face:?} is still walled with every sealed face open",
                    tile.key
                );
                let alone = standing(&opening([face]));
                for hull in &alone {
                    // What stands in a face opened alone is always a corner mass that
                    // belongs to the face next door, which stays.
                    assert!(
                        sector_of(hull)
                            .is_some_and(|other| other != face && sealed.contains(&other)),
                        "{:?}: something of {face:?}'s own still stands in it",
                        tile.key
                    );
                }
                corners += usize::from(!alone.is_empty());
            }
            // Never the floor, the ceiling, or anything standing against a door.
            let doors: Vec<HexFace> = HexFace::LATERAL
                .into_iter()
                .filter(|face| !sealed.contains(face))
                .collect();
            for hull in tile
                .hulls
                .iter()
                .filter(|hull| is_opened_wall(hull, &every))
            {
                assert!(sector_of(hull).is_some_and(|face| !doors.contains(&face)));
                let (lo, hi) = height(hull);
                assert!(hi > FLOOR_SLAB_TOP + 0.05, "{:?} loses floor", tile.key);
                assert!(lo < TILE_LEVEL_HEIGHT - 0.6, "{:?} loses ceiling", tile.key);
            }
        }
        // Measured when the rule was written: 32 of 13,572 faces opened alone keep a
        // corner mass from the face next door, all in one register's straight halls.
        assert!(
            corners * 100 < faces,
            "{corners} corners in {faces} opened faces"
        );
    }

    /// Whether a floor stands under `plan`: something spanning the deck height.
    fn supported(hulls: &[Vec<Vec3>], plan: Vec2) -> bool {
        let deck = FLOOR_SLAB_TOP - 0.05;
        hulls.iter().any(|hull| {
            let (lo, hi) = height(hull);
            lo <= deck && hi >= deck && observed_traversal::point_in_convex_plan_hull(hull, plan)
        })
    }

    /// Whether something stood at `plan` from floor height up: a floor, or the base of
    /// a mass standing on the slab. Either way, a body there after opening needs floor.
    fn grounded(hulls: &[Vec<Vec3>], plan: Vec2) -> bool {
        hulls.iter().any(|hull| {
            let (lo, hi) = height(hull);
            lo <= FLOOR_SLAB_TOP + 0.1
                && hi >= FLOOR_SLAB_TOP - 0.05
                && observed_traversal::point_in_convex_plan_hull(hull, plan)
        })
    }

    /// The solid masses beside a corridor carry its floor too, so opening a face must
    /// leave floor where they stood. The first production run walked bots through the
    /// hole this left and onto the roof below, over and over.
    #[test]
    fn opening_a_face_never_holes_the_floor() {
        let mut checked = 0;
        for tile in hall_tiles() {
            let sealed: Vec<HexFace> = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == PortClass::Sealed)
                .collect();
            let open = opening(sealed.iter().copied());
            let after: Vec<Vec<Vec3>> = tile
                .hulls
                .iter()
                .filter(|hull| !is_opened_wall(hull, &open))
                .cloned()
                .chain(
                    tile.hulls
                        .iter()
                        .filter(|hull| is_opened_wall(hull, &open))
                        .filter_map(|hull| super::floor_under(hull, Vec3::ZERO))
                        .map(|piece| piece.local_hull(Vec3::ZERO)),
                )
                .collect();
            for i in -12..=12 {
                for j in -12..=12 {
                    #[allow(clippy::cast_precision_loss)]
                    let plan = Vec2::new(i as f32 * 0.55, j as f32 * 0.6);
                    if grounded(&tile.hulls, plan) {
                        checked += 1;
                        assert!(
                            supported(&after, plan) || grounded(&after, plan),
                            "{:?}: opening its sealed faces holes the floor at {plan:?}",
                            tile.key
                        );
                    }
                }
            }
        }
        assert!(checked > 10_000, "{checked} floor points checked");
    }

    /// One corner hall on its own at `level`, doors east and south-west.
    fn lone_hall(level: u8) -> HexWfcWorld {
        let at = HexCoord { q: 4, r: 4, level };
        let placement = HexPlacement {
            coord: at,
            space: HexSpace::Hall,
            archetype: HexArchetype::Corner,
            doors: lateral_bit(HexFace::East) | lateral_bit(HexFace::SouthWest),
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        };
        HexWfcWorld {
            seed: 3,
            generation: 0,
            config: HexWfcConfig {
                cols: 9,
                rows: 9,
                levels: 8,
                min_rooms: 0,
                max_rooms: 0,
                retry_budget: 1,
                min_room_distance: 1,
            },
            placements: BTreeMap::from([(at, placement)]),
            blueprints: Vec::new(),
            architecture: BTreeMap::from([(at, ArchitectureRegister::Monolith)]),
            cell_revisions: BTreeMap::from([(at, 1)]),
            last_attempts: 1,
            authored_pins: Default::default(),
            space_mix: SpaceMix::baseline(),
            route_corridors: false,
            carve_unrouted: false,
            open_air: false,
        }
    }

    #[test]
    fn an_opened_hall_keeps_floor_ceiling_and_doors_and_gains_a_lit_edge_per_face() {
        for level in [1, RAILED_BELOW_LEVEL] {
            let world = lone_hall(level);
            let at = *world.placements.keys().next().expect("one cell");
            let open = open_edges(&world, at).expect("a lone hall opens");
            assert_eq!(open.faces.count_ones(), 4, "every face but its two doors");
            assert_eq!(open.railed, level < RAILED_BELOW_LEVEL);
            let snapshot = HexWfcGeometrySnapshot::project(&world, &crate::hex_wfc::test_tiles())
                .expect("projection");
            let count = |part| snapshot.pieces.iter().filter(|p| p.part == part).count();
            assert_eq!(count(HexPiecePart::Lip), 4);
            assert_eq!(count(HexPiecePart::Guard) > 0, open.railed);
            let origin = Vec3::from_array(hex_origin(at));
            for piece in snapshot
                .pieces
                .iter()
                .filter(|p| p.part == HexPiecePart::Authored)
            {
                let observed_traversal::ColliderShape::ConvexHull { points } = &piece.shape else {
                    panic!("authored pieces are hulls");
                };
                let local: Vec<Vec3> = points.iter().map(|&p| p + piece.center - origin).collect();
                assert!(
                    !is_opened_wall(&local, &open),
                    "a wall survived on an open face"
                );
            }
        }
    }

    /// Walk from the middle of a lone hall straight out through an open face.
    fn walk_out(level: u8) -> (f32, f32) {
        let world = lone_hall(level);
        let at = *world.placements.keys().next().expect("one cell");
        let snapshot = HexWfcGeometrySnapshot::project(&world, &crate::hex_wfc::test_tiles())
            .expect("projection");
        let scene = snapshot.rapier_scene();
        let config = FpsConfig::default();
        let floor = Vec3::from_array(hex_origin(at)) + Vec3::Y * FLOOR_SLAB_TOP;
        // North-west is open; walk toward it.
        let [a, b] = observed_hex::face_edge(HexFace::NorthWest);
        #[allow(clippy::cast_precision_loss)]
        let toward = Vec2::new((a.0 + b.0) as f32, (a.1 + b.1) as f32).normalize();
        let mut body = FpsBody::spawned(
            floor + Vec3::Y * config.half_height,
            toward.x.atan2(-toward.y),
        );
        let mut lowest = floor.y;
        for _ in 0..240 {
            step_character(
                &scene,
                &mut body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..PlayerIntent::default()
                },
                &config,
                1.0 / 60.0,
            );
            lowest = lowest.min(body.position.y - config.half_height);
        }
        (floor.y, lowest)
    }

    #[test]
    fn a_railed_open_edge_holds_a_body_and_a_bare_one_lets_it_fall() {
        let (floor, lowest) = walk_out(1);
        assert!(
            lowest > floor - 0.3,
            "railed: fell to {lowest} from {floor}"
        );
        let (floor, lowest) = walk_out(RAILED_BELOW_LEVEL);
        assert!(lowest < floor - 4.0, "bare: still at {lowest} from {floor}");
    }
}
