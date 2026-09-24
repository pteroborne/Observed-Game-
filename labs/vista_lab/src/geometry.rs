//! World-space pieces for the vista, derived from the exposure survey.
//!
//! Every piece is a convex hull or an oriented block, which is all the traversal
//! controller's colliders and a mesh builder need, and all a test needs to reason
//! about. Pieces carry a semantic [`Look`], never a colour: the view asks
//! `observed_style` how each look is drawn.
//!
//! Proportions are the hex lattice's own — cells 14 m across the flats and 8 m a
//! storey — so a walkway 2.6 m wide on a 14 m cell reads as narrow because it *is*
//! narrow, not because it was drawn thin.
use bevy::math::{Quat, Vec2, Vec3};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexCoord, HexFace};
use observed_hex::{CORNERS, FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::{ColliderShape, ColliderSpec, StableColliderId};

use crate::composition::Vista;
use observed_facility::hex_wfc::exposure::{Exposure, Form, Overhang};

/// Walkway deck width, metres. Wide enough to walk without thinking about it and
/// narrow enough that both lit edges sit in view at once.
pub const WALKWAY_WIDTH: f32 = 2.6;
/// Railing height above the walking surface, metres.
pub const RAIL_HEIGHT: f32 = 1.05;
/// Rise of one stair tread. Under the controller's 0.45 m step-up, with margin.
pub const TREAD_RISE: f32 = 0.4;
/// Height of a pavilion's walls; a storey's walls run the full level.
pub const PAVILION_HEIGHT: f32 = 6.0;
/// The deepest keel, metres below a hanging floor.
pub const MAX_KEEL: f32 = 26.0;

/// What a piece is, for the renderer. None of these is a colour.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Look {
    Floor(ArchitectureRegister),
    Wall(ArchitectureRegister),
    Roof(ArchitectureRegister),
    /// A wall whose outside is open air: part of a cliff.
    SheerFace,
    /// A lit slit in a sheer face: somebody is home.
    Window(ArchitectureRegister),
    /// The storey line across a sheer face, which is what gives a cliff its scale.
    Band(ArchitectureRegister),
    /// The hanging mass under a floating cell.
    Underside,
    /// The lit lip of every drop: the commitment line.
    FallEdge,
    /// A walkway's deck and a stair's treads.
    WalkwayDeck,
    Rail,
    /// The collider behind a railing: solid to a body, invisible to the eye, so the
    /// railing can be drawn as posts and a rail and still stop a walker cleanly.
    Guard,
    /// The slender structure under a walkway or a flight.
    Truss,
    /// The summit beacon.
    Beacon,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// World-space points of a convex hull.
    Hull(Vec<Vec3>),
    /// An oriented box.
    Block {
        center: Vec3,
        rotation: Quat,
        half: Vec3,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub shape: Shape,
    pub look: Look,
    pub collides: bool,
    /// The cell this piece belongs to.
    pub cell: HexCoord,
}

/// A warm practical light inside a pavilion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Practical {
    pub position: Vec3,
    pub register: ArchitectureRegister,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Build {
    pub pieces: Vec<Piece>,
    pub practicals: Vec<Practical>,
    /// Where the summit beacon stands.
    pub beacon: Vec3,
}

impl Build {
    /// The traversal colliders for every piece that collides, with stable ids in
    /// piece order.
    #[must_use]
    pub fn collider_specs(&self) -> Vec<ColliderSpec> {
        self.pieces
            .iter()
            .filter(|piece| piece.collides)
            .enumerate()
            .map(|(index, piece)| {
                let id = StableColliderId(u32::try_from(index).expect("collider count fits"));
                match &piece.shape {
                    Shape::Hull(points) => ColliderSpec {
                        id,
                        center: Vec3::ZERO,
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        shape: ColliderShape::ConvexHull {
                            points: points.clone(),
                        },
                        friction: 0.8,
                    },
                    Shape::Block {
                        center,
                        rotation,
                        half,
                    } => ColliderSpec {
                        id,
                        center: *center,
                        rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
                        shape: ColliderShape::Cuboid { half: *half },
                        friction: 0.8,
                    },
                }
            })
            .collect()
    }
}

/// World origin of a cell: centre of its footprint, at the base of its level.
#[must_use]
pub fn origin(at: HexCoord) -> Vec3 {
    Vec3::from_array(hex_origin(at))
}

/// Top of a cell's floor, where a body stands.
#[must_use]
pub fn floor_point(at: HexCoord) -> Vec3 {
    origin(at) + Vec3::Y * FLOOR_SLAB_TOP
}

fn corner(index: usize) -> Vec3 {
    let (x, z) = CORNERS[index % 6];
    #[allow(clippy::cast_precision_loss)]
    Vec3::new(x as f32, 0.0, z as f32)
}

fn edge(face: HexFace) -> (Vec3, Vec3) {
    (corner(face.index()), corner(face.index() + 1))
}

/// The midpoint of a face's edge, cell-local.
#[must_use]
pub fn face_mid(face: HexFace) -> Vec3 {
    let (a, b) = edge(face);
    (a + b) * 0.5
}

fn yaw_along(direction: Vec3) -> Quat {
    Quat::from_rotation_y(-direction.z.atan2(direction.x))
}

/// Deterministic per-cell variation. No random source anywhere in the lab.
fn hash(at: HexCoord, salt: u32) -> u32 {
    let mut h = u32::from(at.q).wrapping_mul(0x9E37_79B1)
        ^ u32::from(at.r).wrapping_mul(0x85EB_CA77)
        ^ u32::from(at.level).wrapping_mul(0xC2B2_AE3D)
        ^ salt.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn prism(o: Vec3, inset_top: f32, y_top: f32, inset_bottom: f32, y_bottom: f32) -> Vec<Vec3> {
    (0..6)
        .map(|i| o + corner(i) * inset_top + Vec3::Y * y_top)
        .chain((0..6).map(|i| o + corner(i) * inset_bottom + Vec3::Y * y_bottom))
        .collect()
}

struct Builder<'a> {
    vista: &'a Vista,
    out: Build,
}

impl Builder<'_> {
    fn push(&mut self, cell: HexCoord, look: Look, collides: bool, shape: Shape) {
        self.out.pieces.push(Piece {
            shape,
            look,
            collides,
            cell,
        });
    }

    /// A block lying along part of a face's edge.
    ///
    /// `t0..t1` is the stretch of the edge (0 = the face's first corner), `inset`
    /// moves the block's centre line inward from the edge, `depth` is its thickness
    /// across the edge, and `y0..y1` its vertical extent above the cell origin.
    #[allow(clippy::too_many_arguments)]
    fn along_edge(
        &mut self,
        at: HexCoord,
        face: HexFace,
        (t0, t1): (f32, f32),
        inset: f32,
        depth: f32,
        (y0, y1): (f32, f32),
        look: Look,
        collides: bool,
    ) {
        let (a, b) = edge(face);
        let run = b - a;
        let inward = -face_mid(face).normalize();
        let middle = a + run * ((t0 + t1) * 0.5) + inward * inset;
        let center = origin(at) + middle + Vec3::Y * ((y0 + y1) * 0.5);
        self.push(
            at,
            look,
            collides,
            Shape::Block {
                center,
                rotation: yaw_along(run),
                half: Vec3::new(run.length() * (t1 - t0) * 0.5, (y1 - y0) * 0.5, depth * 0.5),
            },
        );
    }

    /// The lit lip, and a railing when the level is safe, over a stretch of edge.
    fn drop_edge(&mut self, e: &Exposure, face: HexFace, stretch: (f32, f32)) {
        let top = FLOOR_SLAB_TOP;
        self.along_edge(
            e.coord,
            face,
            stretch,
            0.08,
            0.1,
            (top, top + 0.04),
            Look::FallEdge,
            false,
        );
        if e.railed {
            self.along_edge(
                e.coord,
                face,
                stretch,
                0.3,
                0.08,
                (top + RAIL_HEIGHT - 0.06, top + RAIL_HEIGHT),
                Look::Rail,
                false,
            );
            // The collider is the whole height of the railing, so nothing slips under.
            self.along_edge(
                e.coord,
                face,
                stretch,
                0.3,
                0.08,
                (top, top + RAIL_HEIGHT),
                Look::Guard,
                true,
            );
            let (a, b) = edge(face);
            let length = (b - a).length() * (stretch.1 - stretch.0);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let posts = (length / 2.2).ceil().max(1.0) as u32;
            for post in 0..=posts {
                #[allow(clippy::cast_precision_loss)]
                let t = stretch.0 + (stretch.1 - stretch.0) * post as f32 / posts as f32;
                self.along_edge(
                    e.coord,
                    face,
                    (t - 0.006, t + 0.006),
                    0.3,
                    0.1,
                    (top, top + RAIL_HEIGHT),
                    Look::Rail,
                    false,
                );
            }
        }
    }

    fn slab(&mut self, at: HexCoord, look: Look) {
        let o = origin(at);
        self.push(
            at,
            look,
            true,
            Shape::Hull(prism(o, 1.0, FLOOR_SLAB_TOP, 1.0, 0.0)),
        );
    }

    fn walkway_face(&self, at: HexCoord, face: HexFace) -> bool {
        self.vista
            .world
            .config
            .grid()
            .neighbor(at, face)
            .and_then(|next| {
                observed_facility::hex_wfc::exposure::exposure(
                    &self.vista.world,
                    next,
                    crate::composition::UNSAFE_FROM_LEVEL,
                )
            })
            .is_some_and(|next| matches!(next.form, Form::Span { .. } | Form::Flight { .. }))
    }

    fn deck(&mut self, e: &Exposure) {
        let register = self.vista.register(e.coord);
        self.slab(e.coord, Look::Floor(register));
        let placement = self.vista.world.placements[&e.coord];
        for face in HexFace::LATERAL {
            if e.is_sheer(face) {
                self.drop_edge(e, face, (0.0, 1.0));
            } else if placement.is_open(face) && self.walkway_face(e.coord, face) {
                // The walkway lands in the middle of this edge; either side of it
                // is still a drop.
                let (a, b) = edge(face);
                let gap = WALKWAY_WIDTH * 0.5 / (b - a).length();
                self.drop_edge(e, face, (0.0, 0.5 - gap));
                self.drop_edge(e, face, (0.5 + gap, 1.0));
            }
        }
    }

    fn enclosed(&mut self, e: &Exposure, height: f32) {
        let register = self.vista.register(e.coord);
        self.slab(e.coord, Look::Floor(register));
        let placement = self.vista.world.placements[&e.coord];
        let wall = |sheer: bool| {
            if sheer {
                Look::SheerFace
            } else {
                Look::Wall(register)
            }
        };
        for face in HexFace::LATERAL {
            let sheer = e.is_sheer(face);
            let (a, b) = edge(face);
            let length = (b - a).length();
            let span = (FLOOR_SLAB_TOP, height);
            if placement.is_open(face) {
                let door = 1.7 / length;
                for stretch in [(0.0, 0.5 - door), (0.5 + door, 1.0)] {
                    self.along_edge(e.coord, face, stretch, 0.25, 0.5, span, wall(sheer), true);
                }
                self.along_edge(
                    e.coord,
                    face,
                    (0.5 - door, 0.5 + door),
                    0.25,
                    0.5,
                    (4.4, height),
                    wall(sheer),
                    true,
                );
            } else {
                self.along_edge(
                    e.coord,
                    face,
                    (0.0, 1.0),
                    0.25,
                    0.5,
                    span,
                    wall(sheer),
                    true,
                );
            }
            if sheer {
                self.sheer_dressing(e.coord, face, height, register);
            }
        }
    }

    /// Windows, the storey band and corner ribs on one sheer face.
    fn sheer_dressing(
        &mut self,
        at: HexCoord,
        face: HexFace,
        height: f32,
        register: ArchitectureRegister,
    ) {
        let windows = hash(at, face.index() as u32) % 4;
        for slot in 0..windows {
            #[allow(clippy::cast_precision_loss)]
            let t = (slot as f32 + 1.0) / (windows as f32 + 1.0);
            self.along_edge(
                at,
                face,
                (t - 0.03, t + 0.03),
                -0.02,
                0.1,
                (2.6, (height - 2.6).min(5.2)),
                Look::Window(register),
                false,
            );
        }
        self.along_edge(
            at,
            face,
            (0.0, 1.0),
            -0.1,
            0.24,
            (height - 0.32, height - 0.1),
            Look::Band(register),
            false,
        );
        for t in [0.0, 1.0] {
            self.along_edge(
                at,
                face,
                (t - 0.035, t + 0.035),
                -0.05,
                0.6,
                (0.0, height),
                Look::SheerFace,
                false,
            );
        }
    }

    fn keel(&mut self, e: &Exposure) {
        let room = match e.overhang {
            Overhang::Supported => return,
            Overhang::Hanging { .. } => e.overhang.metres().map_or(MAX_KEEL + 14.0, |m| m - 4.0),
        };
        #[allow(clippy::cast_precision_loss)]
        let want = 10.0 + (hash(e.coord, 99) % 15) as f32;
        let depth = want.min(MAX_KEEL).min(room);
        if depth < 2.0 {
            return;
        }
        let o = origin(e.coord);
        let segments = 3 + hash(e.coord, 7) % 2;
        #[allow(clippy::cast_precision_loss)]
        let step = depth / segments as f32;
        let mut inset = 1.0;
        let mut y = 0.0;
        for _ in 0..segments {
            let bottom = inset * 0.74;
            self.push(
                e.coord,
                Look::Underside,
                false,
                Shape::Hull(prism(o, inset, y, bottom, y - step)),
            );
            inset = bottom * 0.84;
            y -= step;
        }
    }

    fn span(&mut self, e: &Exposure, axis: HexFace) {
        let o = origin(e.coord);
        let mid = face_mid(axis);
        let along = mid.normalize();
        let across = Vec3::new(-along.z, 0.0, along.x);
        let half_length = mid.length() + 0.15;
        let rotation = yaw_along(along);
        let w = WALKWAY_WIDTH * 0.5;
        let top = FLOOR_SLAB_TOP;
        self.push(
            e.coord,
            Look::WalkwayDeck,
            true,
            Shape::Block {
                center: o + Vec3::Y * (top - 0.175),
                rotation,
                half: Vec3::new(half_length, 0.175, w),
            },
        );
        for side in [-1.0, 1.0] {
            let lateral = across * side;
            self.push(
                e.coord,
                Look::FallEdge,
                false,
                Shape::Block {
                    center: o + lateral * (w - 0.05) + Vec3::Y * (top + 0.02),
                    rotation,
                    half: Vec3::new(half_length, 0.02, 0.05),
                },
            );
            if e.railed {
                let rail = o + lateral * (w - 0.05);
                self.push(
                    e.coord,
                    Look::Rail,
                    false,
                    Shape::Block {
                        center: rail + Vec3::Y * (top + RAIL_HEIGHT - 0.03),
                        rotation,
                        half: Vec3::new(half_length, 0.03, 0.04),
                    },
                );
                self.push(
                    e.coord,
                    Look::Guard,
                    true,
                    Shape::Block {
                        center: rail + Vec3::Y * (top + RAIL_HEIGHT * 0.5),
                        rotation,
                        half: Vec3::new(half_length, RAIL_HEIGHT * 0.5, 0.04),
                    },
                );
                for post in -3..=3 {
                    #[allow(clippy::cast_precision_loss)]
                    let s = post as f32 * half_length / 3.2;
                    self.push(
                        e.coord,
                        Look::Rail,
                        false,
                        Shape::Block {
                            center: rail + along * s + Vec3::Y * (top + RAIL_HEIGHT * 0.5),
                            rotation,
                            half: Vec3::new(0.04, RAIL_HEIGHT * 0.5, 0.04),
                        },
                    );
                }
            }
        }
        // A single inverted-triangle truss: the whole of the walkway's visible support.
        let bottom = o + Vec3::Y * (top - 2.1);
        let mut truss = Vec::new();
        for end in [-1.0, 1.0] {
            let reach = along * (half_length * end);
            for side in [-1.0, 1.0] {
                truss.push(o + reach + across * (w * 0.8 * side) + Vec3::Y * (top - 0.34));
            }
            truss.push(bottom + along * ((half_length - 1.4) * end));
        }
        self.push(e.coord, Look::Truss, false, Shape::Hull(truss));
    }

    fn flight(&mut self, e: &Exposure, entry: HexFace) {
        let o = origin(e.coord);
        let exit = entry.opposite();
        let mid = face_mid(exit);
        let along = mid.normalize();
        let across = Vec3::new(-along.z, 0.0, along.x);
        let reach = mid.length();
        let rotation = yaw_along(along);
        let w = WALKWAY_WIDTH * 0.5;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let treads = (TILE_LEVEL_HEIGHT / TREAD_RISE).round() as u32;
        #[allow(clippy::cast_precision_loss)]
        let run = 2.0 * reach / treads as f32;
        for tread in 0..treads {
            #[allow(clippy::cast_precision_loss)]
            let (s, rise) = (
                -reach + (tread as f32 + 0.5) * run,
                FLOOR_SLAB_TOP + (tread as f32 + 1.0) * TREAD_RISE,
            );
            // An open-riser stair: the body climbs a solid step, the eye sees a thin
            // tread with air under it. Over a drop, the gaps are the point.
            self.push(
                e.coord,
                Look::Guard,
                true,
                Shape::Block {
                    center: o + along * s + Vec3::Y * (rise - TREAD_RISE * 0.5),
                    rotation,
                    half: Vec3::new(run * 0.5 + 0.01, TREAD_RISE * 0.5, w),
                },
            );
            self.push(
                e.coord,
                Look::WalkwayDeck,
                false,
                Shape::Block {
                    center: o + along * s + Vec3::Y * (rise - 0.07),
                    rotation,
                    half: Vec3::new(run * 0.5 - 0.05, 0.07, w - 0.12),
                },
            );
        }
        // Stringers and their lit lips follow the pitch line.
        let pitch = |s: f32| FLOOR_SLAB_TOP + (s + reach) / (2.0 * reach) * TILE_LEVEL_HEIGHT;
        let bar = |side: f32, lift: f32, depth: f32, half_width: f32| -> Vec<Vec3> {
            let mut points = Vec::new();
            for s in [-reach, reach] {
                for lateral in [side * w - half_width, side * w + half_width] {
                    for dy in [lift, lift - depth] {
                        points.push(o + along * s + across * lateral + Vec3::Y * (pitch(s) + dy));
                    }
                }
            }
            points
        };
        for side in [-1.0, 1.0] {
            self.push(
                e.coord,
                Look::Truss,
                false,
                Shape::Hull(bar(side, 0.05, 0.9, 0.08)),
            );
            self.push(
                e.coord,
                Look::FallEdge,
                false,
                Shape::Hull(bar(side, 0.07, 0.04, 0.05)),
            );
            if e.railed {
                self.push(
                    e.coord,
                    Look::Guard,
                    true,
                    Shape::Hull(bar(side, RAIL_HEIGHT, RAIL_HEIGHT, 0.04)),
                );
                self.push(
                    e.coord,
                    Look::Rail,
                    false,
                    Shape::Hull(bar(side, RAIL_HEIGHT, 0.06, 0.04)),
                );
            }
        }
    }

    fn beacon(&mut self, at: HexCoord) {
        let base = floor_point(at);
        self.out.beacon = base;
        self.push(
            at,
            Look::Beacon,
            false,
            Shape::Block {
                center: base + Vec3::Y * 90.0,
                rotation: Quat::IDENTITY,
                half: Vec3::new(0.3, 90.0, 0.3),
            },
        );
        self.push(
            at,
            Look::Beacon,
            true,
            Shape::Block {
                center: base + Vec3::Y * 0.9,
                rotation: Quat::from_rotation_y(0.4),
                half: Vec3::splat(0.9),
            },
        );
    }
}

/// Every piece of the vista, in cell order. Deterministic.
#[must_use]
pub fn build(vista: &Vista, survey: &[Exposure]) -> Build {
    let mut builder = Builder {
        vista,
        out: Build::default(),
    };
    for e in survey {
        match e.form {
            Form::Deck => builder.deck(e),
            Form::Pavilion => {
                builder.enclosed(e, PAVILION_HEIGHT);
                let register = vista.register(e.coord);
                builder.slab_roof(e.coord, register);
                builder.out.practicals.push(Practical {
                    position: origin(e.coord) + Vec3::Y * 4.6,
                    register,
                });
            }
            Form::Storey => builder.enclosed(e, TILE_LEVEL_HEIGHT),
            Form::Span { axis } => builder.span(e, axis),
            Form::Flight { entry } => builder.flight(e, entry),
            Form::Landing => {}
        }
        if matches!(e.form, Form::Deck | Form::Pavilion | Form::Storey) && e.overhang.hangs() {
            builder.keel(e);
        }
    }
    builder.beacon(vista.summit);
    builder.out
}

impl Builder<'_> {
    fn slab_roof(&mut self, at: HexCoord, register: ArchitectureRegister) {
        let o = origin(at);
        self.push(
            at,
            Look::Roof(register),
            true,
            Shape::Hull(prism(o, 1.02, PAVILION_HEIGHT + 0.6, 1.02, PAVILION_HEIGHT)),
        );
    }
}

/// The yaw that faces a body along a plan direction, in the controller's convention.
#[must_use]
pub fn yaw_toward(direction: Vec2) -> f32 {
    direction.x.atan2(-direction.y)
}
