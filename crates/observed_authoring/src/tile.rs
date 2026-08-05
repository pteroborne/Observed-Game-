//! Tile schema projection and exact-snap footprint validation.
//!
//! A tile `.map` contains: `worldspawn` brushes (the solid geometry), exactly
//! one `tile_meta` point entity (`archetype`, `register`, `variant`,
//! `levels`), and `tile_port` point entities (`face`, `class`). Projection is
//! importer-only — editor entities describe the tile, they never become the
//! game model.

use std::ffi::CString;

use glam::{Quat, Vec2, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT, face_edge};
use observed_traversal::{ArenaSpec, ColliderShape, ColliderSpec, StableColliderId};
use quake_map::{Entity, QuakeMap};
use serde::{Deserialize, Serialize};

use crate::UNITS_PER_METER;
use crate::brush::brush_vertices;
use crate::manifest::TileKey;

/// On-boundary tolerance for the footprint check, in TrenchBroom units.
/// Authored planes are integer, so anything beyond this is a real violation.
const SNAP_EPSILON: f64 = 1.0e-3;

#[derive(Clone, Debug, PartialEq)]
pub enum TileError {
    Parse(String),
    MissingMeta,
    DuplicateMeta,
    DuplicatePort {
        face: HexFace,
    },
    MissingProperty {
        entity: &'static str,
        key: String,
    },
    UnknownFace(String),
    UnknownClass(String),
    UnknownLightKind(String),
    InvalidPort {
        face: HexFace,
        class: PortClass,
    },
    DegenerateBrush {
        index: usize,
    },
    InvalidLevels,
    /// A vertex escapes the canonical quantized-hexagon prism. Reports the
    /// offending vertex (TrenchBroom units), the violated boundary, and the
    /// exact bound so the author can fix the brush.
    FootprintViolation {
        vertex: [f64; 3],
        boundary: String,
    },
    /// A `tile_stair_node` carries an index another node already claimed, so the
    /// climb order is ambiguous.
    DuplicateStairNode {
        index: u16,
    },
    /// The climb goes down. A spine is ordered bottom to top; a descent in the
    /// middle means the nodes are out of order or the geometry backtracks.
    StairSpineDescends {
        index: u16,
    },
    /// Two parts of the climb pass within a body's width of each other. A
    /// follower picks the segment it is nearest, so overlapping segments are
    /// genuinely ambiguous from a position — which is the failure that made the
    /// old hardcoded follower spin on the spot just past the turn.
    StairSpineSelfCrossing {
        first: usize,
        second: usize,
        separation: f32,
    },
    /// A spine with fewer than two nodes is not a line.
    StairSpineTooShort,
    /// A `tile_deck_node` carries an index another node already claimed.
    DuplicateDeckNode {
        index: u16,
    },
    /// A deck path worth declaring has to route around something; two nodes are
    /// just a straight line, which is the case that needs no path at all.
    DeckPathTooShort,
}

/// Semantic authored light source. Presentation owns its colour and energy;
/// tile sources own only placement and purpose, keeping district treatment in
/// `observed_style` instead of baking ad-hoc RGB values into geometry files.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TileLightKind {
    Practical,
}

/// One tile-local light source in world-space metres (Y-up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLight {
    pub kind: TileLightKind,
    pub position: Vec3,
}

/// How far a spine may sag between consecutive nodes before it counts as a
/// descent rather than authoring noise, in metres.
const STAIR_NODE_LEVEL_TOLERANCE: f32 = 0.05;

/// How far apart two non-adjacent stretches of one spine must stay, in metres.
/// A shade over the 0.76 m player capsule, so a body standing anywhere on the
/// climb is unambiguously on one stretch of it.
pub const STAIR_SPINE_MIN_SEPARATION: f32 = 0.9;

/// How much more a metre of height counts than a metre of floor when deciding
/// which stretch of a climb a body is on. See [`StairSpine::locate`].
const CLIMB_VERTICAL_WEIGHT: f32 = 3.0;

/// How close to a spine's terminal node counts as having got there, in the
/// weighted metric. Shorter than the flat stub at either end, so it can only
/// trigger on deck, and comfortably longer than the residual a walker leaves
/// when it stops steering at its target.
const ARRIVAL_RADIUS: f32 = 0.6;

/// The walkable line through a tile's vertical circulation, in tile-local world
/// metres, ordered from the bottom entry to the top exit.
///
/// This is the contract that lets vertical circulation be more than one shape.
/// Before it existed, the objective bot climbed by hardcoded rise thresholds and
/// named tread points calibrated against the single generated switchback — so
/// authoring a second stair tower would have produced geometry no bot could
/// walk, and even *fixing* the switchback desynchronised the steering from the
/// surface underneath it. A tower now ships the line through itself alongside
/// its brushes, and the two cannot drift because both are derived from the same
/// constants.
///
/// The nodes are a polyline, not a set of targets: a follower walks segment to
/// segment. The first node sits on the lower deck at the foot of the climb and
/// the last on the deck above, so the ends join the flat circulation either
/// side without a special case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StairSpine {
    pub nodes: Vec<Vec3>,
}

impl StairSpine {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() < 2
    }

    /// The spine rotated by `turn` sixths about the tile centre, matching how
    /// hulls and lights rotate.
    #[must_use]
    pub fn rotated(&self, turn: u8) -> Self {
        let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
        Self {
            nodes: self.nodes.iter().map(|&node| rotation * node).collect(),
        }
    }

    /// The index of the segment `point` is nearest, and how far along it the
    /// body has got (0 at the segment's start, 1 at its end).
    ///
    /// Nearest *segment*, deliberately, not nearest node. Selecting a target by
    /// proximity to a waypoint is not monotonic along a path — walking away
    /// from a landing onto the next flight grows the distance back past any
    /// threshold, so the target flips and the body spins on the spot. Distance
    /// to a segment falls and then rises exactly once as you walk it, so the
    /// choice advances with the body and never oscillates. That property is
    /// what [`Self::self_crossing`] exists to protect.
    ///
    /// Nearness is measured with height counting for
    /// [`CLIMB_VERTICAL_WEIGHT`] times as much as lateral distance, because a
    /// body stands *on* a surface: a metre sideways is a step, a metre up is a
    /// wall. Measured plainly, a body on the deck of a switchback comes out
    /// nearest to the flight passing overhead — 2.8 m away through the ceiling
    /// versus 3.8 m along the floor — so it would be steered into the underside
    /// of its own staircase. That is not a hypothetical; it stalled all four
    /// soak bots on the first run of this code.
    #[must_use]
    pub fn locate(&self, point: Vec3) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for index in 0..self.nodes.len().saturating_sub(1) {
            let (distance, t) = segment_distance(
                climb_metric(self.nodes[index]),
                climb_metric(self.nodes[index + 1]),
                climb_metric(point),
            );
            if best.is_none_or(|(_, best_distance, _)| distance < best_distance) {
                best = Some((index, distance, t));
            }
        }
        best.map(|(index, _, t)| (index, t))
    }

    /// The point a body at `point` should walk toward, climbing when `up`.
    ///
    /// Always the far end of the segment the body is on, so the target sits on
    /// the surface underfoot rather than across a void.
    #[must_use]
    pub fn target(&self, point: Vec3, up: bool) -> Option<Vec3> {
        let (index, _) = self.locate(point)?;
        Some(if up {
            self.nodes[index + 1]
        } else {
            self.nodes[index]
        })
    }

    /// Whether a body at `point` has finished climbing: it has reached the last
    /// node, which by contract stands on the deck above.
    ///
    /// A follower needs this as an explicit end, not an inference. The last
    /// stretch of a spine lies flat on the deck it arrives at, so "am I still
    /// on the climb?" cannot be answered by height — a body standing exactly on
    /// the exit node is at deck level and on the line, and without this test it
    /// gets steered toward the point it is already standing on, forever.
    #[must_use]
    pub fn has_arrived(&self, point: Vec3) -> bool {
        self.nodes
            .last()
            .is_some_and(|node| climb_metric(*node).distance(climb_metric(point)) <= ARRIVAL_RADIUS)
    }

    /// The mirror of [`Self::has_arrived`] for a body coming down: it has
    /// reached the first node, on the deck the climb rises from.
    #[must_use]
    pub fn has_descended(&self, point: Vec3) -> bool {
        self.nodes
            .first()
            .is_some_and(|node| climb_metric(*node).distance(climb_metric(point)) <= ARRIVAL_RADIUS)
    }

    /// How far `point` is from the line, or `None` for an empty spine.
    #[must_use]
    pub fn distance(&self, point: Vec3) -> Option<f32> {
        let (index, t) = self.locate(point)?;
        Some(
            self.nodes[index]
                .lerp(self.nodes[index + 1], t)
                .distance(point),
        )
    }

    /// The closest approach between two stretches of the spine that are not
    /// neighbours, if that is nearer than [`STAIR_SPINE_MIN_SEPARATION`].
    #[must_use]
    pub fn self_crossing(&self) -> Option<(usize, usize, f32)> {
        let count = self.nodes.len().saturating_sub(1);
        for first in 0..count {
            for second in first + 2..count {
                let separation = segment_separation(
                    climb_metric(self.nodes[first]),
                    climb_metric(self.nodes[first + 1]),
                    climb_metric(self.nodes[second]),
                    climb_metric(self.nodes[second + 1]),
                );
                if separation < STAIR_SPINE_MIN_SEPARATION {
                    return Some((first, second, separation));
                }
            }
        }
        None
    }
}

/// Distance from `point` to segment `a`..`b`, measured in plan only.
fn plan_segment_distance(a: Vec3, b: Vec3, point: Vec3) -> f32 {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    segment_distance(flat(a), flat(b), flat(point)).0
}

/// Rescale so height dominates lateral distance when judging which stretch of a
/// climb a body is standing on. See [`StairSpine::locate`].
fn climb_metric(point: Vec3) -> Vec3 {
    Vec3::new(point.x, point.y * CLIMB_VERTICAL_WEIGHT, point.z)
}

/// Distance from `point` to segment `a`..`b`, with the parameter along it.
fn segment_distance(a: Vec3, b: Vec3, point: Vec3) -> (f32, f32) {
    let span = b - a;
    let length_squared = span.length_squared();
    let t = if length_squared < 1.0e-6 {
        0.0
    } else {
        ((point - a).dot(span) / length_squared).clamp(0.0, 1.0)
    };
    ((a + span * t).distance(point), t)
}

/// Closest approach between two segments, sampled densely enough that a
/// crossing cannot slip between the samples at the tolerances involved.
fn segment_separation(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> f32 {
    const SAMPLES: usize = 32;
    let mut closest = f32::MAX;
    for step in 0..=SAMPLES {
        let point = a0.lerp(a1, step as f32 / SAMPLES as f32);
        closest = closest.min(segment_distance(b0, b1, point).0);
    }
    closest
}

/// The walkable line around a tile's floor, in tile-local world metres.
///
/// A stair tower's deck is not a disc: the stairwell is a hole in it, and the
/// flights overhang parts of what is left. A body that steers straight at a
/// door on the far side walks into the void or into the underside of a flight.
/// That is bug backlog #19, and it is not fixable by tuning, because there is
/// no line the tower can be assumed to have — every tower shape puts its hole
/// somewhere else.
///
/// So the tower says where its floor goes. The nodes are an ordered open path,
/// not a closed ring: the switchback's deck is a C, with the stairwell cutting
/// the west side, and pretending otherwise would route bodies through a wall.
/// Followers step one node at a time toward the node nearest their goal, so
/// consecutive nodes must be joined by walkable floor — that is the whole
/// contract.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeckPath {
    pub nodes: Vec<Vec3>,
}

impl DeckPath {
    /// How close a follower may come to a waypoint before it commits to the
    /// next leg.
    ///
    /// The follower is deliberately stateless, so a capsule negotiating a
    /// corner can otherwise keep locating itself on the leg it just walked and
    /// target the shared endpoint forever. A quarter metre is smaller than the
    /// production capsule radius and only cuts the inside of a turn by that
    /// amount; authored deck paths must already leave a whole body clear.
    const WAYPOINT_CAPTURE_RADIUS: f32 = 0.25;

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() < 2
    }

    #[must_use]
    pub fn rotated(&self, turn: u8) -> Self {
        let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
        Self {
            nodes: self.nodes.iter().map(|&node| rotation * node).collect(),
        }
    }

    /// The index of the segment nearest `point` in plan — height ignored, since
    /// a deck is one storey and a body on it stands a capsule above the nodes.
    ///
    /// Nearest segment rather than nearest node, for the same reason
    /// [`StairSpine::locate`] does it: a node-based choice flips between two
    /// opposite targets as the body crosses a boundary between them, and the
    /// body orbits that boundary instead of crossing it. Segment distance falls
    /// and rises once as you walk a leg, so the choice advances with the body.
    #[must_use]
    fn locate(&self, point: Vec3) -> Option<usize> {
        (0..self.nodes.len().saturating_sub(1)).min_by(|&a, &b| {
            let to = |index: usize| {
                plan_segment_distance(self.nodes[index], self.nodes[index + 1], point)
            };
            to(a).total_cmp(&to(b))
        })
    }

    /// The next point a body at `from` should walk toward to reach `goal` over
    /// the floor: along the leg it is on, then leg by leg, then `goal` itself.
    ///
    /// One leg at a time, deliberately. Handing over a distant node lets the
    /// body cut the corner, and the corners are where the stairwell is.
    #[must_use]
    pub fn step_toward(&self, from: Vec3, goal: Vec3) -> Option<Vec3> {
        let here = self.locate(from)?;
        let there = self.locate(goal)?;
        Some(match here.cmp(&there) {
            std::cmp::Ordering::Less => {
                let waypoint = here + 1;
                if plan_distance(from, self.nodes[waypoint]) <= Self::WAYPOINT_CAPTURE_RADIUS {
                    if waypoint >= there {
                        goal
                    } else {
                        self.nodes[waypoint + 1]
                    }
                } else {
                    self.nodes[waypoint]
                }
            }
            std::cmp::Ordering::Greater => {
                let waypoint = here;
                if plan_distance(from, self.nodes[waypoint]) <= Self::WAYPOINT_CAPTURE_RADIUS {
                    if waypoint.saturating_sub(1) <= there {
                        goal
                    } else {
                        self.nodes[waypoint - 1]
                    }
                } else {
                    self.nodes[waypoint]
                }
            }
            std::cmp::Ordering::Equal => goal,
        })
    }
}

fn plan_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// A validated, world-space tile ready for placement.
#[derive(Clone, Debug, PartialEq)]
pub struct TilePrototype {
    pub key: TileKey,
    /// Relative deterministic selection weight from authored metadata.
    pub weight: u16,
    /// Vertical levels this prefab spans (1 for flats, 2 for ramp prefabs).
    pub levels: u8,
    pub signature: PortSignature,
    /// Convex hulls in tile-local world meters: origin at the cell center,
    /// level 0 floor at y = 0.
    pub hulls: Vec<Vec<Vec3>>,
    /// Semantic practicals authored against visible fixture geometry.
    pub lights: Vec<TileLight>,
    /// The climbable line through this tile, empty for tiles with no climb.
    pub spine: StairSpine,
    /// The walkable line around this tile's floor, empty where a straight line
    /// across the cell is always fine.
    pub deck: DeckPath,
}

impl TilePrototype {
    /// Collider specs for one instance of this tile, ids offset by `base_id`,
    /// hulls translated by `offset` (typically `hex_origin` of the cell).
    pub fn collider_specs(&self, base_id: u32, offset: Vec3) -> Vec<ColliderSpec> {
        self.collider_specs_with_transform(base_id, offset, Quat::IDENTITY)
    }

    /// Collider specs with translation and rotation transform applied.
    pub fn collider_specs_with_transform(
        &self,
        base_id: u32,
        offset: Vec3,
        rotation: Quat,
    ) -> Vec<ColliderSpec> {
        let rot_arr = [rotation.x, rotation.y, rotation.z, rotation.w];
        self.hulls
            .iter()
            .enumerate()
            .map(|(index, hull)| {
                let rotated_hull: Vec<Vec3> = hull.iter().map(|v| rotation * *v).collect();
                ColliderSpec {
                    id: StableColliderId(base_id + index as u32),
                    center: offset,
                    rotation: rot_arr,
                    shape: ColliderShape::ConvexHull {
                        points: rotated_hull,
                    },
                    friction: 0.8,
                }
            })
            .collect()
    }

    /// A standalone single-tile arena for labs and headless traversal tests.
    pub fn arena_spec(&self) -> ArenaSpec {
        let height = f32::from(self.levels) * TILE_LEVEL_HEIGHT;
        ArenaSpec {
            colliders: self.collider_specs(0, Vec3::ZERO),
            floor_y: 0.0,
            safety_center: Vec3::new(0.0, height * 0.5, 0.0),
            safety_half: Vec3::new(24.0, height + 12.0, 24.0),
        }
    }
}

fn cstr(value: &CString) -> String {
    value.to_string_lossy().into_owned()
}

pub(crate) fn prop(entity: &Entity, key: &str) -> Option<String> {
    entity
        .edict
        .iter()
        .find(|(k, _)| cstr(k) == key)
        .map(|(_, v)| cstr(v))
}

pub(crate) fn required(
    entity: &Entity,
    name: &'static str,
    key: &str,
) -> Result<String, TileError> {
    prop(entity, key).ok_or(TileError::MissingProperty {
        entity: name,
        key: key.to_string(),
    })
}

pub(crate) fn face_from_name(name: &str) -> Result<HexFace, TileError> {
    Ok(match name {
        "east" => HexFace::East,
        "south_east" => HexFace::SouthEast,
        "south_west" => HexFace::SouthWest,
        "west" => HexFace::West,
        "north_west" => HexFace::NorthWest,
        "north_east" => HexFace::NorthEast,
        "up" => HexFace::Up,
        "down" => HexFace::Down,
        other => return Err(TileError::UnknownFace(other.to_string())),
    })
}

pub(crate) fn face_name(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "east",
        HexFace::SouthEast => "south_east",
        HexFace::SouthWest => "south_west",
        HexFace::West => "west",
        HexFace::NorthWest => "north_west",
        HexFace::NorthEast => "north_east",
        HexFace::Up => "up",
        HexFace::Down => "down",
    }
}

pub(crate) fn class_from_name(name: &str) -> Result<PortClass, TileError> {
    Ok(match name {
        "door" => PortClass::Door,
        "ramp_open" => PortClass::RampOpen,
        "shaft_open" => PortClass::ShaftOpen,
        other => return Err(TileError::UnknownClass(other.to_string())),
    })
}

pub(crate) fn class_name(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "sealed",
        PortClass::Door => "door",
        PortClass::RampOpen => "ramp_open",
        PortClass::ShaftOpen => "shaft_open",
    }
}

/// TrenchBroom Z-up units -> world Y-up meters.
fn to_world(point: [f64; 3]) -> Vec3 {
    Vec3::new(
        (point[0] / UNITS_PER_METER) as f32,
        (point[2] / UNITS_PER_METER) as f32,
        (-point[1] / UNITS_PER_METER) as f32,
    )
}

fn parse_origin(entity: &Entity, name: &'static str) -> Result<[f64; 3], TileError> {
    let value = required(entity, name, "origin")?;
    let values = value
        .split_ascii_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TileError::MissingProperty {
            entity: name,
            key: "origin (three numbers)".to_string(),
        })?;
    if values.len() != 3 {
        return Err(TileError::MissingProperty {
            entity: name,
            key: "origin (three numbers)".to_string(),
        });
    }
    Ok([values[0], values[1], values[2]])
}

/// Canonical footprint corner in TrenchBroom units: world plan `(x, z)` maps
/// to editor `(x * S, -z * S)`.
fn tb_corner(corner: (i32, i32)) -> [f64; 2] {
    [
        f64::from(corner.0) * UNITS_PER_METER,
        f64::from(-corner.1) * UNITS_PER_METER,
    ]
}

/// Exact-snap validation: every vertex must lie inside (or on) the canonical
/// hex prism for the tile's level span.
#[derive(Clone, Copy)]
struct FootprintPrism {
    q: i16,
    r: i16,
    level: i8,
    levels: u8,
}

fn footprint_prisms(map: &QuakeMap, levels: u8) -> Result<Vec<FootprintPrism>, TileError> {
    let mut cells = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_cell") {
            continue;
        }
        let parse = |key: &str| -> Result<i16, TileError> {
            required(entity, "tile_cell", key)?
                .parse()
                .map_err(|_| TileError::MissingProperty {
                    entity: "tile_cell",
                    key: format!("{key} (integer)"),
                })
        };
        let level = parse("level")?;
        let span = prop(entity, "levels")
            .unwrap_or_else(|| "1".to_string())
            .parse::<u8>()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_cell",
                key: "levels (u8)".to_string(),
            })?;
        if span == 0 {
            return Err(TileError::InvalidLevels);
        }
        cells.push(FootprintPrism {
            q: parse("q")?,
            r: parse("r")?,
            level: i8::try_from(level).map_err(|_| TileError::MissingProperty {
                entity: "tile_cell",
                key: "level (i8)".to_string(),
            })?,
            levels: span,
        });
    }
    if cells.is_empty() {
        cells.push(FootprintPrism {
            q: 0,
            r: 0,
            level: 0,
            levels,
        });
    }
    Ok(cells)
}

fn inside_plan_footprint(vertex: [f64; 3], cell: FootprintPrism) -> bool {
    let origin_x = f64::from(i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * UNITS_PER_METER;
    let origin_y = f64::from(-i32::from(cell.r) * 12) * UNITS_PER_METER;
    let local = [vertex[0] - origin_x, vertex[1] - origin_y];
    for face in HexFace::LATERAL {
        let [a, b] = face_edge(face).map(tb_corner);
        let edge_side = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if edge_side(local) * edge_side([0.0, 0.0]).signum() < -SNAP_EPSILON * length {
            return false;
        }
    }
    true
}

fn validate_footprint(vertices: &[[f64; 3]], cells: &[FootprintPrism]) -> Result<(), TileError> {
    let level_height = f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER;
    for &vertex in vertices {
        let inside = cells.iter().copied().any(|cell| {
            let floor = f64::from(cell.level) * level_height;
            let ceiling = floor + f64::from(cell.levels) * level_height;
            vertex[2] >= floor - SNAP_EPSILON
                && vertex[2] <= ceiling + SNAP_EPSILON
                && inside_plan_footprint(vertex, cell)
        });
        if !inside {
            let any_vertical_span = cells.iter().copied().any(|cell| {
                let floor = f64::from(cell.level) * level_height;
                let ceiling = floor + f64::from(cell.levels) * level_height;
                vertex[2] >= floor - SNAP_EPSILON && vertex[2] <= ceiling + SNAP_EPSILON
            });
            if !any_vertical_span {
                return Err(TileError::FootprintViolation {
                    vertex,
                    boundary: "vertical bounds of declared tile_cell footprint".to_string(),
                });
            }
            if cells.len() == 1 {
                let cell = cells[0];
                let origin_x =
                    f64::from(i32::from(cell.q) * 14 + i32::from(cell.r) * 7) * UNITS_PER_METER;
                let origin_y = f64::from(-i32::from(cell.r) * 12) * UNITS_PER_METER;
                let local = [vertex[0] - origin_x, vertex[1] - origin_y];
                for face in HexFace::LATERAL {
                    let [a, b] = face_edge(face).map(tb_corner);
                    let side =
                        |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
                    let length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                    if side(local) * side([0.0, 0.0]).signum() < -SNAP_EPSILON * length {
                        return Err(TileError::FootprintViolation {
                            vertex,
                            boundary: format!("{} face plane", face_name(face)),
                        });
                    }
                }
            }
            return Err(TileError::FootprintViolation {
                vertex,
                boundary: "declared tile_cell footprint union".to_string(),
            });
        }
    }
    Ok(())
}

/// Parse and validate one tile `.map` text.
pub fn parse_tile(text: &str) -> Result<TilePrototype, TileError> {
    let map: QuakeMap = quake_map::parse(&mut std::io::Cursor::new(text))
        .map_err(|error| TileError::Parse(error.to_string()))?;

    let mut meta: Option<&Entity> = None;
    let mut ports = [PortClass::Sealed; 8];
    let mut seen_origin_ports = [false; 8];
    let mut worldspawn: Option<&Entity> = None;
    for entity in &map.entities {
        match prop(entity, "classname").as_deref() {
            Some("worldspawn") => worldspawn = Some(entity),
            Some("tile_meta") => {
                if meta.replace(entity).is_some() {
                    return Err(TileError::DuplicateMeta);
                }
            }
            Some("tile_port") => {
                let face = face_from_name(&required(entity, "tile_port", "face")?)?;
                let class = class_from_name(&required(entity, "tile_port", "class")?)?;
                let q = prop(entity, "q")
                    .and_then(|value| value.parse::<i16>().ok())
                    .unwrap_or(0);
                let r = prop(entity, "r")
                    .and_then(|value| value.parse::<i16>().ok())
                    .unwrap_or(0);
                let level = prop(entity, "level")
                    .and_then(|value| value.parse::<i8>().ok())
                    .unwrap_or(0);
                // TilePrototype's compatibility signature describes the
                // origin cell. Whole-room ports are retained by the richer
                // authoring schema and compiled catalog.
                if q == 0 && r == 0 && level == 0 {
                    if seen_origin_ports[face.index()] {
                        return Err(TileError::DuplicatePort { face });
                    }
                    seen_origin_ports[face.index()] = true;
                    ports[face.index()] = class;
                }
            }
            _ => {}
        }
    }
    let meta = meta.ok_or(TileError::MissingMeta)?;
    let key = TileKey {
        archetype: required(meta, "tile_meta", "archetype")?,
        register: required(meta, "tile_meta", "register")?,
        variant: required(meta, "tile_meta", "variant")?
            .parse()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_meta",
                key: "variant (u16)".to_string(),
            })?,
    };
    let weight = prop(meta, "weight")
        .unwrap_or_else(|| "1".to_string())
        .parse::<u16>()
        .ok()
        .filter(|weight| (1..=1000).contains(weight))
        .ok_or_else(|| TileError::MissingProperty {
            entity: "tile_meta",
            key: "weight (1..=1000)".to_string(),
        })?;
    let levels: u8 = required(meta, "tile_meta", "levels")?
        .parse()
        .map_err(|_| TileError::MissingProperty {
            entity: "tile_meta",
            key: "levels (u8)".to_string(),
        })?;
    if levels == 0 {
        return Err(TileError::InvalidLevels);
    }

    let signature =
        PortSignature::try_from_ports(ports).map_err(|invalid| TileError::InvalidPort {
            face: invalid.face,
            class: invalid.class,
        })?;

    let footprint = footprint_prisms(&map, levels)?;
    let mut hulls = Vec::new();
    if let Some(world) = worldspawn {
        for (index, brush) in world.brushes.iter().enumerate() {
            let vertices = brush_vertices(brush).ok_or(TileError::DegenerateBrush { index })?;
            validate_footprint(&vertices, &footprint)?;
            hulls.push(vertices.iter().map(|&v| to_world(v)).collect());
        }
    }

    let mut lights = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_light") {
            continue;
        }
        let origin = parse_origin(entity, "tile_light")?;
        validate_footprint(&[origin], &footprint)?;
        let kind = match prop(entity, "kind").as_deref().unwrap_or("practical") {
            "practical" => TileLightKind::Practical,
            other => return Err(TileError::UnknownLightKind(other.to_string())),
        };
        lights.push(TileLight {
            kind,
            position: to_world(origin),
        });
    }
    lights.sort_by(|a, b| {
        a.position
            .x
            .total_cmp(&b.position.x)
            .then(a.position.y.total_cmp(&b.position.y))
            .then(a.position.z.total_cmp(&b.position.z))
    });

    let mut indexed = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_stair_node") {
            continue;
        }
        let origin = parse_origin(entity, "tile_stair_node")?;
        validate_footprint(&[origin], &footprint)?;
        let index = required(entity, "tile_stair_node", "index")?
            .parse::<u16>()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_stair_node",
                key: "index (u16)".to_string(),
            })?;
        indexed.push((index, to_world(origin)));
    }
    indexed.sort_by_key(|&(index, _)| index);
    if let Some(window) = indexed.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(TileError::DuplicateStairNode { index: window[0].0 });
    }
    if !indexed.is_empty() {
        if indexed.len() < 2 {
            return Err(TileError::StairSpineTooShort);
        }
        if let Some(window) = indexed
            .windows(2)
            .find(|pair| pair[1].1.y < pair[0].1.y - STAIR_NODE_LEVEL_TOLERANCE)
        {
            return Err(TileError::StairSpineDescends { index: window[1].0 });
        }
    }
    let spine = StairSpine {
        nodes: indexed.into_iter().map(|(_, node)| node).collect(),
    };
    let mut deck_indexed = Vec::new();
    for entity in &map.entities {
        if prop(entity, "classname").as_deref() != Some("tile_deck_node") {
            continue;
        }
        let origin = parse_origin(entity, "tile_deck_node")?;
        validate_footprint(&[origin], &footprint)?;
        let index = required(entity, "tile_deck_node", "index")?
            .parse::<u16>()
            .map_err(|_| TileError::MissingProperty {
                entity: "tile_deck_node",
                key: "index (u16)".to_string(),
            })?;
        deck_indexed.push((index, to_world(origin)));
    }
    deck_indexed.sort_by_key(|&(index, _)| index);
    if let Some(window) = deck_indexed.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(TileError::DuplicateDeckNode { index: window[0].0 });
    }
    if !deck_indexed.is_empty() && deck_indexed.len() < 3 {
        return Err(TileError::DeckPathTooShort);
    }
    let deck = DeckPath {
        nodes: deck_indexed.into_iter().map(|(_, node)| node).collect(),
    };
    if let Some((first, second, separation)) = spine.self_crossing() {
        return Err(TileError::StairSpineSelfCrossing {
            first,
            second,
            separation,
        });
    }

    Ok(TilePrototype {
        key,
        weight,
        levels,
        signature,
        hulls,
        lights,
        spine,
        deck,
    })
}

/// Read and parse a tile `.map` from disk.
pub fn load_tile(path: &std::path::Path) -> Result<TilePrototype, TileError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| TileError::Parse(format!("{}: {error}", path.display())))?;
    parse_tile(&text)
}
