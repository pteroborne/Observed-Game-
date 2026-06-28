//! The **teleport place model**: the player occupies exactly one discrete space at a
//! time — a [`Place::Room`] or a [`Place::Hallway`] — each in its own local frame
//! centred at the origin. Crossing a doorway gap teleports you to the next place
//! (room → its edge's hallway → destination room). Because only the current place
//! exists, everything else is unobserved by construction, and a doorway can point at
//! a freshly-rolled hallway variation / destination whenever you are not inside it.
//!
//! This module is pure geometry + the transition state machine: it builds the current
//! place's footprint + doorway gaps, detects when a moving body crosses a gap, and
//! computes the resulting place. The real fixed-step controller (`observed_traversal`)
//! drives the body; the presentation renders the geometry. It is deliberately
//! controller- and render-agnostic so it can be unit-tested.

use std::f32::consts::PI;

use bevy::math::{Quat, Vec2, Vec3};
use observed_core::RoomId;
use observed_match::mutable::EXIT_ROOM;
use observed_traversal::{Aabb3, FpsArena};

use crate::{hallway, maze};

/// Half-extent of a room's square footprint (world units). Generous so rooms read as
/// breathable volumes, not cells, and so the wider doorway gaps still fit on a polygon edge.
pub const ROOM_HALF: f32 = 8.5;
/// The **standard threshold width** (world units): every crossable doorway is this wide,
/// clamped narrower only where a tight space (a maze corridor, a short polygon edge) forces
/// it. One module so a doorway reads the same everywhere and rooms/halls line up cleanly.
pub const THRESHOLD_WIDTH: f32 = 4.5;
/// How far inside a place the body spawns from the doorway it entered through.
pub const ENTRY_INSET: f32 = 1.2;
/// Side length of one labyrinth cell (world units). The clear corridor is
/// `MAZE_CELL - 2*MAZE_WALL_T`; with the controller's 0.4 body radius that stays roomy.
pub const MAZE_CELL: f32 = 4.2;
/// Half-thickness of a labyrinth interior wall (world units).
pub const MAZE_WALL_T: f32 = 0.3;

/// Where the player currently is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Room(RoomId),
    Hallway {
        from: RoomId,
        to: RoomId,
        variation: usize,
    },
}

/// What a doorway gap does / means in the current place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapKind {
    /// The spine-forward doorway of a room (toward your next objective room).
    Forward,
    /// A non-spine doorway of a room (a side route).
    Side,
    /// A hallway's entry (back toward the room you came from).
    Entry,
    /// A hallway's exit (onward to the destination room).
    Exit,
    /// A hallway's exit toward the facility exit while the **keystone gate is locked** —
    /// a solid, closed door (not a passage) until enough keystones are held.
    LockedExit,
}

impl GapKind {
    /// A passage you can actually cross to another place (a `Side` door, or a
    /// `LockedExit`, is a closed door on a solid wall — you can't pass it).
    pub fn is_passage(self) -> bool {
        matches!(self, GapKind::Forward | GapKind::Entry | GapKind::Exit)
    }
}

/// A crossable gap on the current place's footprint edge, in the place's local frame.
#[derive(Clone, Copy, Debug)]
pub struct DoorGap {
    /// Centre of the gap on the footprint edge (XZ).
    pub center: Vec2,
    /// Outward unit normal — the direction you exit through.
    pub normal: Vec2,
    pub width: f32,
    /// The room this gap ultimately heads toward.
    pub target: RoomId,
    pub kind: GapKind,
}

/// An interior wall segment inside a place's footprint (local frame, centred at 0).
/// Used to carve a labyrinth inside a hallway; rooms and simple halls have none.
#[derive(Clone, Copy, Debug)]
pub struct WallSeg {
    pub center: Vec2,
    pub half: Vec2,
}

/// The current place's footprint + its doorway gaps (local frame, centred at 0).
///
/// `half` is always the bounding half-extent (used for floor/light/bounds sizing). A
/// hallway is an axis-aligned box (`poly: None`) whose walls come from `interior` +
/// `place_arena`'s perimeter. A room is a convex **polygon** (`poly: Some(vertices)`,
/// CCW, centred at 0); its walls are the polygon edges, collision is the convex
/// `contain` clamp, and it has no `interior` or AABB perimeter.
#[derive(Clone, Debug)]
pub struct PlaceGeom {
    pub half: Vec2,
    pub gaps: Vec<DoorGap>,
    pub interior: Vec<WallSeg>,
    pub poly: Option<Vec<Vec2>>,
}

impl PlaceGeom {
    pub fn forward_gap(&self) -> Option<&DoorGap> {
        self.gaps.iter().find(|g| g.kind == GapKind::Forward)
    }
}

/// A small deterministic hash (splitmix64 finalizer) for seeding room shapes.
fn mix(seed: u64, salt: u64) -> u64 {
    let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// A deterministic value in `[0, 1)` for `(seed, salt)`.
fn unit(seed: u64, salt: u64) -> f32 {
    (mix(seed, salt) >> 40) as f32 / (1u64 << 24) as f32
}

/// A room's footprint polygon (CCW, centred at the origin), seeded so each room keeps a
/// stable shape: a varied **rectangle** (4 sides) or a regular **polygon** of 5–8 sides
/// (a pentagon/hexagon/heptagon/octagon, with a random orientation). `min_sides` forces
/// enough edges to host every doorway.
fn room_polygon(seed: u64, min_sides: usize) -> Vec<Vec2> {
    let n = (4 + (mix(seed, 1) % 5) as usize).clamp(min_sides.max(4), 8);
    if n == 4 {
        // A varied rectangle for visual distinction from the polygons.
        let hx = ROOM_HALF * (0.85 + unit(seed, 2) * 0.7);
        let hz = ROOM_HALF * (0.85 + unit(seed, 3) * 0.7);
        return vec![
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ];
    }
    // A regular n-gon whose apothem (inradius) is a seeded 0.9×–1.4× of ROOM_HALF, so
    // rooms vary in size (some tight, some hub-like) the way the rectangles already do;
    // a random rotation keeps orientations varied.
    let apothem = ROOM_HALF * (0.9 + unit(seed, 5) * 0.5);
    let circumradius = apothem / (PI / n as f32).cos();
    let rot = unit(seed, 4) * (2.0 * PI / n as f32);
    (0..n)
        .map(|i| {
            let a = rot + i as f32 * 2.0 * PI / n as f32;
            Vec2::new(circumradius * a.cos(), circumradius * a.sin())
        })
        .collect()
}

/// The outward unit normal of the polygon edge `a`→`b` (the polygon is centred at 0).
fn outward_normal(a: Vec2, b: Vec2) -> Vec2 {
    let dir = (b - a).normalize_or_zero();
    let mut nrm = Vec2::new(dir.y, -dir.x);
    let mid = (a + b) * 0.5;
    if nrm.dot(mid) < 0.0 {
        nrm = -nrm;
    }
    nrm
}

/// Build a room's footprint: a seeded convex polygon (4–8 sides) with one doorway per
/// connection, spread across its edges. The doorway whose target is the spine objective
/// is flagged `Forward`; the rest are `Side` (closed) doors. `seed` keeps each room's
/// shape stable.
pub fn room_geom(connections: &[RoomId], target: Option<RoomId>, seed: u64) -> PlaceGeom {
    let mut conns: Vec<RoomId> = connections.to_vec();
    conns.sort_unstable_by_key(|r| r.0);
    conns.dedup();
    let num = conns.len();
    let verts = room_polygon(seed, num);
    let n = verts.len();
    let gaps = conns
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            // Spread the doorways evenly around the polygon's edges. (The closure only
            // runs when there is at least one connection, so `num >= 1` here.)
            let edge = (i * n) / num;
            let a = verts[edge];
            let b = verts[(edge + 1) % n];
            let mid = (a + b) * 0.5;
            let len = (b - a).length();
            DoorGap {
                center: mid,
                normal: outward_normal(a, b),
                width: THRESHOLD_WIDTH.min(len - 1.0).max(1.5),
                target: t,
                kind: if Some(t) == target {
                    GapKind::Forward
                } else {
                    GapKind::Side
                },
            }
        })
        .collect();
    let half = verts.iter().fold(Vec2::ZERO, |acc, v| {
        Vec2::new(acc.x.max(v.x.abs()), acc.y.max(v.y.abs()))
    });
    PlaceGeom {
        half,
        gaps,
        interior: Vec::new(),
        poly: Some(verts),
    }
}

/// Re-open the doorway a room was *entered through* (toward `entry_from`) as an `Entry`
/// passage instead of a sealed `Side` door. The doorway you just walked through then stays
/// a real opening — matching the preview you crossed and letting you step back out — so
/// entering a room is seamless rather than the opening popping into a wall behind you. A
/// no-op for hallways, the start room (`entry_from == None`), or when that doorway is
/// already the `Forward` passage.
pub fn open_entry(geom: &mut PlaceGeom, entry_from: Option<RoomId>) {
    let Some(from) = entry_from else {
        return;
    };
    if geom.poly.is_none() {
        return;
    }
    if let Some(gap) = geom
        .gaps
        .iter_mut()
        .find(|g| g.target == from && g.kind == GapKind::Side)
    {
        gap.kind = GapKind::Entry;
    }
}

/// Clamp `pos` (XZ) to keep a body of `radius` inside a polygon room, except where it is
/// passing through an open (passage) doorway. A no-op for non-polygon places (hallways,
/// whose walls are real AABB solids). This is the room "collision" — applied after the
/// shared controller moves the body, since that controller only resolves axis-aligned
/// boxes and a polygon's walls are angled.
pub fn contain(geom: &PlaceGeom, pos: Vec2, radius: f32) -> Vec2 {
    let Some(poly) = geom.poly.as_ref() else {
        return pos;
    };
    let n = poly.len();
    if n < 3 {
        return pos;
    }
    let mut p = pos;
    // A few relaxation passes settle the corners where two edges both push.
    for _ in 0..4 {
        for i in 0..n {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            if (b - a).length() < 1e-4 {
                continue;
            }
            let dir = (b - a).normalize_or_zero();
            let inward = -outward_normal(a, b);
            let dist = (p - a).dot(inward); // signed distance inside this edge
            if dist >= radius {
                continue;
            }
            // Let the body slip out through an open doorway on this edge.
            let mid = (a + b) * 0.5;
            let through_gap = geom.gaps.iter().any(|g| {
                g.kind.is_passage()
                    && (g.center - mid).length() < 0.05
                    && (p - g.center).dot(dir).abs() <= g.width * 0.5
            });
            if through_gap {
                continue;
            }
            p += inward * (radius - dist);
        }
    }
    p
}

/// Build a hallway piece's footprint from its template. A `Maze` template is a
/// generated labyrinth (interior walls between an entry on the −Z wall and an exit on
/// the +Z wall, both always connected; see [`crate::maze`]); its concrete layout comes
/// from `layout_seed`. A `Chicane` is an **S-bend**: two staggered interior baffles force
/// a slalom between an offset entry and exit (a gentle weave through a wide space, not a
/// tight corner). Every other flavour is a straight run whose *length* varies by template.
pub fn hallway_geom(
    from: RoomId,
    to: RoomId,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
) -> PlaceGeom {
    // A hallway heading into the facility exit shows a solid locked door while the
    // keystone gate is shut; otherwise its onward doorway is a normal passage.
    let exit_kind = if exit_locked && to.0 == EXIT_ROOM {
        GapKind::LockedExit
    } else {
        GapKind::Exit
    };
    if let Some((cols, rows)) = template.grid {
        let m = maze::Maze::generate(cols as usize, rows as usize, layout_seed);
        let footprint = m.footprint_half(MAZE_CELL);
        let corridor = MAZE_CELL - 2.0 * MAZE_WALL_T;
        let interior = m
            .interior_walls(MAZE_CELL, MAZE_WALL_T)
            .into_iter()
            .map(|(center, half)| WallSeg { center, half })
            .collect();
        // Multiple entrances (−Z, back to `from`) and exits (+Z, on to `to`); each at a
        // door column, all reachable from one another through the maze.
        let mut gaps = Vec::new();
        for &ec in &m.entry_cols {
            let x = m.cell_center(ec, 0, MAZE_CELL).x;
            gaps.push(DoorGap {
                center: Vec2::new(x, -footprint.y),
                normal: Vec2::new(0.0, -1.0),
                width: corridor,
                target: from,
                kind: GapKind::Entry,
            });
        }
        for &xc in &m.exit_cols {
            let x = m.cell_center(xc, m.rows - 1, MAZE_CELL).x;
            gaps.push(DoorGap {
                center: Vec2::new(x, footprint.y),
                normal: Vec2::new(0.0, 1.0),
                width: corridor,
                target: to,
                kind: exit_kind,
            });
        }
        return PlaceGeom {
            half: footprint,
            gaps,
            interior,
            poly: None,
        };
    }
    // Straight/chicane/climb pieces vary their length per edge (a deterministic
    // 1.0×–2.2× of the template, never below the `MIN_HALL_LENGTH` floor), so repeated
    // connectors read as visibly different runs while always staying a real journey.
    let w = template.width;
    let len = (template.length * hallway::length_scale(layout_seed)).max(hallway::MIN_HALL_LENGTH);
    if template.flavor == hallway::HallwayFlavor::Chicane {
        // An S-bend: a box with two staggered baffles, each sealing one side and leaving
        // a corridor `c` on the other, so the path slaloms from the +X entry up through
        // the low baffle's gap, across the open middle band, and out the high baffle's
        // −X gap to the exit. The baffles live in `interior`, so they render + collide
        // through the same path the labyrinths use.
        let hx = w * 0.5;
        let hz = (len * 0.5).max(w);
        let c = (w * 0.42).max(2.4); // walkable corridor (≫ the 0.4 body radius)
        let baffle_half_x = hx - c * 0.5;
        let interior = vec![
            // Low baffle: seals the −X side, opening a gap on +X.
            WallSeg {
                center: Vec2::new(-c * 0.5, -hz * 0.33),
                half: Vec2::new(baffle_half_x, MAZE_WALL_T),
            },
            // High baffle: seals the +X side, opening a gap on −X.
            WallSeg {
                center: Vec2::new(c * 0.5, hz * 0.33),
                half: Vec2::new(baffle_half_x, MAZE_WALL_T),
            },
        ];
        let off = hx - c * 0.5; // align the doorways with the open sides
        return PlaceGeom {
            half: Vec2::new(hx, hz),
            gaps: vec![
                DoorGap {
                    center: Vec2::new(off, -hz),
                    normal: Vec2::new(0.0, -1.0),
                    width: c,
                    target: from,
                    kind: GapKind::Entry,
                },
                DoorGap {
                    center: Vec2::new(-off, hz),
                    normal: Vec2::new(0.0, 1.0),
                    width: c,
                    target: to,
                    kind: exit_kind,
                },
            ],
            interior,
            poly: None,
        };
    }
    if template.flavor == hallway::HallwayFlavor::Colonnade {
        // A wide, long pseudo-room: a regular grid of square pillars straddling the centre
        // axes (so a clear lane always runs straight down the middle, entry→exit, and a
        // cross lane runs side to side), with a margin keeping the columns off the walls.
        let hx = w * 0.5;
        let hz = (len * 0.5).max(w);
        let interior: Vec<WallSeg> = pillar_offsets(hx - PILLAR_MARGIN)
            .into_iter()
            .flat_map(|px| {
                pillar_offsets(hz - PILLAR_MARGIN)
                    .into_iter()
                    .map(move |pz| WallSeg {
                        center: Vec2::new(px, pz),
                        half: Vec2::splat(PILLAR_HALF),
                    })
            })
            .collect();
        // The doorways open onto the clear central lane (no pillar sits at x = 0).
        let lane = (PILLAR_SPACING - 2.0 * PILLAR_HALF).max(3.0);
        return PlaceGeom {
            half: Vec2::new(hx, hz),
            gaps: vec![
                DoorGap {
                    center: Vec2::new(0.0, -hz),
                    normal: Vec2::new(0.0, -1.0),
                    width: lane,
                    target: from,
                    kind: GapKind::Entry,
                },
                DoorGap {
                    center: Vec2::new(0.0, hz),
                    normal: Vec2::new(0.0, 1.0),
                    width: lane,
                    target: to,
                    kind: exit_kind,
                },
            ],
            interior,
            poly: None,
        };
    }
    let half = Vec2::new(w * 0.5, len * 0.5);
    // A standard-width doorway centred on each end wall (the rest of the wider end is wall),
    // so a simple hall's mouths match the room doorways they meet.
    let door = THRESHOLD_WIDTH.min(w);
    PlaceGeom {
        half,
        gaps: vec![
            DoorGap {
                center: Vec2::new(0.0, -half.y),
                normal: Vec2::new(0.0, -1.0),
                width: door,
                target: from,
                kind: GapKind::Entry,
            },
            DoorGap {
                center: Vec2::new(0.0, half.y),
                normal: Vec2::new(0.0, 1.0),
                width: door,
                target: to,
                kind: exit_kind,
            },
        ],
        interior: Vec::new(),
        poly: None,
    }
}

/// Half-size of a colonnade's square structural pillars (world units).
const PILLAR_HALF: f32 = 0.5;
/// Centre-to-centre spacing of colonnade pillars; the clear lane between two columns is
/// `PILLAR_SPACING - 2*PILLAR_HALF`, kept well above the body radius.
const PILLAR_SPACING: f32 = 4.4;
/// Keep pillars this far inside the perimeter so a lane runs around the edges too.
const PILLAR_MARGIN: f32 = 2.6;

/// Pillar-centre offsets along one axis within `±limit`, straddling 0 at half-spacing
/// (so the centre axis at 0 is always a clear lane). Empty if `limit` is too small.
fn pillar_offsets(limit: f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut x = PILLAR_SPACING * 0.5;
    while x <= limit {
        out.push(x);
        out.push(-x);
        x += PILLAR_SPACING;
    }
    out
}

/// A room's footprint geometry given its *own* connection set (not the nav snapshot's
/// current-room one) — so a doorway can preview a different room's shape. Seeded exactly
/// like [`geom_for`]'s room branch, so the preview matches the room you'll arrive in.
pub fn room_preview_geom(
    room: RoomId,
    connections: &[RoomId],
    target: Option<RoomId>,
    base_seed: u64,
) -> PlaceGeom {
    room_geom(connections, target, mix(base_seed, room.0 as u64))
}

/// The footprint geometry for any place, given the current navigation snapshot.
pub fn geom_for(place: Place, nav: &Nav) -> PlaceGeom {
    match place {
        // The room shape is seeded by the room id + facility seed (not the decohere
        // version), so a room keeps a stable shape while its connections rewire.
        Place::Room(room) => room_geom(
            &nav.connections,
            nav.target_room,
            mix(nav.seed, room.0 as u64),
        ),
        Place::Hallway {
            from,
            to,
            variation,
        } => hallway_geom(
            from,
            to,
            hallway::template(variation),
            hallway::layout_seed(from, to, variation),
            nav.exit_locked,
        ),
    }
}

/// An edge `(a, b)` whose hallway variation is frozen at `version` — an **anchor torch**
/// pins the structure so the corridor there stops re-rolling, even as the rest of the
/// maze decoheres. Edge-unordered (`(a, b)` == `(b, a)`).
#[derive(Clone, Copy, Debug)]
pub struct PinnedEdge {
    pub a: RoomId,
    pub b: RoomId,
    pub version: u32,
}

/// A snapshot of the brain's navigation state the place model reads (supplied by the
/// match each frame; constructed directly in tests).
#[derive(Clone, Debug)]
pub struct Nav {
    /// Rooms connected to the current room (its open doorways' partners).
    pub connections: Vec<RoomId>,
    /// The spine-forward objective room, if the local team is still running.
    pub target_room: Option<RoomId>,
    pub seed: u64,
    /// Increments when the graph decoheres, so an edge re-rolls its hallway.
    pub version: u32,
    /// The keystone gate is shut: a hallway toward the facility exit shows a solid
    /// `LockedExit` instead of an open `Exit` until enough keystones are held.
    pub exit_locked: bool,
    /// Edges pinned by a dropped anchor torch (their variation is frozen).
    pub pins: Vec<PinnedEdge>,
}

impl Nav {
    /// The decohere version to use for the edge `(x, y)`: the pinned version if an anchor
    /// torch froze it, otherwise the live `version`.
    pub fn effective_version(&self, x: RoomId, y: RoomId) -> u32 {
        let key = |a: RoomId, b: RoomId| if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        let want = key(x, y);
        self.pins
            .iter()
            .find(|p| key(p.a, p.b) == want)
            .map(|p| p.version)
            .unwrap_or(self.version)
    }

    /// Whether the edge `(x, y)` is **tethered** — frozen by a dropped anchor torch (its
    /// variation pinned). A doorway's frame light reads this so a glance shows which edges
    /// are anchored. Edge-unordered.
    pub fn is_tethered(&self, x: RoomId, y: RoomId) -> bool {
        let key = |a: RoomId, b: RoomId| if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        let want = key(x, y);
        self.pins.iter().any(|p| key(p.a, p.b) == want)
    }
}

/// Did the body's movement segment cross `gap` outward (from inside to outside),
/// within the gap's width? Used by the controller bridge each fixed step.
pub fn crossed(prev: Vec2, next: Vec2, gap: &DoorGap) -> bool {
    let dp = (prev - gap.center).dot(gap.normal);
    let dn = (next - gap.center).dot(gap.normal);
    if dp > 0.0 || dn <= 0.0 {
        return false;
    }
    let t = if (dn - dp).abs() > f32::EPSILON {
        (-dp) / (dn - dp)
    } else {
        1.0
    };
    let point = prev.lerp(next, t.clamp(0.0, 1.0));
    let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
    (point - gap.center).dot(tangent).abs() <= gap.width * 0.5
}

/// The result of crossing a doorway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crossing {
    /// Entered an edge's hallway piece heading toward `to`.
    EnteredHallway { from: RoomId, to: RoomId },
    /// Arrived in a room (from a hallway exit, or back through its entry).
    ArrivedRoom(RoomId),
}

/// Apply a doorway crossing: returns the new place and what happened. From a room you
/// enter the crossed edge's hallway (rolling its variation); from a hallway you arrive
/// in the gap's target room.
pub fn apply_crossing(place: Place, gap: &DoorGap, nav: &Nav) -> (Place, Crossing) {
    match place {
        Place::Room(room) => {
            let to = gap.target;
            // A pinned (anchored) edge keeps its frozen variation; others use the live
            // decohere version, so they re-roll when unobserved.
            let version = nav.effective_version(room, to);
            let variation = hallway::variation_for(room, to, nav.seed, version);
            (
                Place::Hallway {
                    from: room,
                    to,
                    variation,
                },
                Crossing::EnteredHallway { from: room, to },
            )
        }
        Place::Hallway { .. } => (Place::Room(gap.target), Crossing::ArrivedRoom(gap.target)),
    }
}

/// How far the place beyond a doorway is pushed outward so its entry wall tucks behind
/// the current wall (avoids a z-fighting double wall at the threshold). Shared by the
/// passage-preview renderer and the seamless crossing remap so the previewed geometry and
/// the place you teleport into coincide exactly.
pub const PREVIEW_OUTSET: f32 = 0.06;

/// A 2D rigid transform on the XZ plane: rotate by `yaw` about +Y, then translate by
/// `offset`. It places a *child* place's local frame into the **current** place's frame —
/// exactly how the passage preview positions the place beyond a doorway. [`apply`] maps a
/// child-frame point out into the current frame; [`inverse_apply`] maps a current-frame
/// point into the child frame, which is what carries the body continuously through a
/// doorway (no snap) when the child becomes the new current place.
#[derive(Clone, Copy, Debug)]
pub struct Align2d {
    pub yaw: f32,
    pub offset: Vec2,
}

/// Rotate an XZ point by `yaw` about +Y (matching `Quat::from_rotation_y`, so the maths
/// agrees with the renderer's `Transform`).
fn rot_y(yaw: f32, p: Vec2) -> Vec2 {
    let r = Quat::from_rotation_y(yaw) * Vec3::new(p.x, 0.0, p.y);
    Vec2::new(r.x, r.z)
}

impl Align2d {
    /// Map a point in the child place's frame into the current place's frame.
    pub fn apply(self, p: Vec2) -> Vec2 {
        rot_y(self.yaw, p) + self.offset
    }

    /// Map a point in the current place's frame into the child place's frame (the inverse
    /// of [`apply`]). Used to drop the body into the place it just crossed into at the
    /// pose that keeps the camera continuous.
    pub fn inverse_apply(self, p: Vec2) -> Vec2 {
        rot_y(-self.yaw, p - self.offset)
    }
}

/// The alignment placing the **hallway** you cross `gap` into: its entry tucks just beyond
/// the opening and it extends away along the doorway's outward normal. `hallway_half_z` is
/// the hallway footprint's half depth. Mirrors `spawn_hallway_preview`.
pub fn hallway_alignment(gap: &DoorGap, hallway_half_z: f32) -> Align2d {
    let n = gap.normal;
    Align2d {
        yaw: n.x.atan2(n.y),
        offset: gap.center + n * (hallway_half_z + PREVIEW_OUTSET),
    }
}

/// The alignment placing the **room** you cross `gap` into so its doorway `back` (the one
/// facing back toward where you stand) sits in the opening and the room extends away.
/// Mirrors `spawn_room_preview`.
pub fn room_alignment(gap: &DoorGap, back: &DoorGap) -> Align2d {
    let n = gap.normal;
    let world_out = -n;
    let yaw = back.normal.y.atan2(back.normal.x) - world_out.y.atan2(world_out.x);
    let offset = (gap.center + n * PREVIEW_OUTSET) - rot_y(yaw, back.center);
    Align2d { yaw, offset }
}

/// The alignment carrying the body from the current place, across `crossed`, into the new
/// place `geom` it produced (a hallway uses its half-depth; a room uses its doorway back
/// toward `from`). `None` if the destination room has no doorway back toward `from` (a
/// mid-crossing decohere) — the caller then falls back to a plain entry snap.
pub fn crossing_alignment(
    geom: &PlaceGeom,
    place: Place,
    crossed: &DoorGap,
    from: RoomId,
) -> Option<Align2d> {
    match place {
        Place::Hallway { .. } => Some(hallway_alignment(crossed, geom.half.y)),
        Place::Room(_) => geom
            .gaps
            .iter()
            .find(|g| g.target == from)
            .map(|back| room_alignment(crossed, back)),
    }
}

/// Where the body should spawn when it enters `geom` having come from room `from`:
/// just inside the gap it arrived through (or the centre if there is none).
pub fn entry_spawn(geom: &PlaceGeom, from: RoomId) -> Vec2 {
    geom.gaps
        .iter()
        .find(|g| g.target == from)
        .map(|gap| gap.center - gap.normal * ENTRY_INSET)
        .unwrap_or(Vec2::ZERO)
}

/// The solid spans (each as a centre offset + half-length) left on a wall of
/// half-length `half_len` (centred on 0) after removing the `gaps` (centre + half-width
/// along the wall). Generalises the perimeter split to any number of doorways per wall.
fn wall_spans(half_len: f32, mut gaps: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    gaps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let mut cursor = -half_len;
    for (c, hw) in gaps {
        let lo = (c - hw).max(-half_len);
        let hi = (c + hw).min(half_len);
        if lo > cursor {
            out.push(((cursor + lo) * 0.5, (lo - cursor) * 0.5));
        }
        cursor = cursor.max(hi);
    }
    if cursor < half_len {
        out.push(((cursor + half_len) * 0.5, (half_len - cursor) * 0.5));
    }
    out
}

/// Build the collision world for a place: perimeter walls (as the proven controller's
/// AABB solids) around the footprint, each split into segments around its doorway
/// gaps so the body can walk out through the openings, plus any interior (maze) walls.
/// Polygon rooms have no axis-aligned perimeter — their angled walls are enforced by
/// [`contain`] and drawn from the polygon edges — so they collide only with the floor.
pub fn place_arena(geom: &PlaceGeom, floor_y: f32, wall_height: f32) -> FpsArena {
    const T: f32 = 0.4; // wall half-thickness
    let half = geom.half;
    let solids: Vec<Aabb3> = Vec::new();
    if geom.poly.is_some() {
        return FpsArena {
            solids,
            floor_y,
            floor_half: half.x.max(half.y) + 5.0,
        };
    }

    let h = wall_height * 0.5;
    let cy = floor_y + h;
    let mut solids = solids;
    let mut segment = |cx: f32, cz: f32, hx: f32, hz: f32| {
        if hx > 0.01 && hz > 0.01 {
            solids.push(Aabb3::from_center_half(
                Vec3::new(cx, cy, cz),
                Vec3::new(hx, h, hz),
            ));
        }
    };

    // West (−X) / East (+X) walls run along Z, split around their Z-centred *passage*
    // gaps (a Side door stays a solid wall, so the body can't walk out into the void).
    for sign in [-1.0_f32, 1.0] {
        let x = sign * half.x;
        let gaps: Vec<(f32, f32)> = geom
            .gaps
            .iter()
            .filter(|g| {
                g.kind.is_passage() && (g.normal.x - sign).abs() < 0.5 && g.normal.y.abs() < 0.5
            })
            .map(|g| (g.center.y, g.width * 0.5))
            .collect();
        for (cz, hz) in wall_spans(half.y, gaps) {
            segment(x, cz, T, hz);
        }
    }
    // North (−Z) / South (+Z) walls run along X, split around their X-centred passage gaps.
    for sign in [-1.0_f32, 1.0] {
        let z = sign * half.y;
        let gaps: Vec<(f32, f32)> = geom
            .gaps
            .iter()
            .filter(|g| {
                g.kind.is_passage() && (g.normal.y - sign).abs() < 0.5 && g.normal.x.abs() < 0.5
            })
            .map(|g| (g.center.x, g.width * 0.5))
            .collect();
        for (cx, hx) in wall_spans(half.x, gaps) {
            segment(cx, z, hx, T);
        }
    }

    // Interior walls (a labyrinth's maze walls), straight from the geometry. The
    // renderer spawns one wall cube per arena solid, so these render for free.
    for seg in &geom.interior {
        segment(seg.center.x, seg.center.y, seg.half.x, seg.half.y);
    }

    FpsArena {
        solids,
        floor_y,
        floor_half: half.x.max(half.y) + 5.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(connections: &[u32], target: Option<u32>) -> Nav {
        Nav {
            connections: connections.iter().map(|&r| RoomId(r)).collect(),
            target_room: target.map(RoomId),
            seed: 1,
            version: 0,
            exit_locked: false,
            pins: Vec::new(),
        }
    }

    #[test]
    fn room_geom_has_a_gap_per_connection_and_marks_the_forward_one() {
        let geom = room_geom(&[RoomId(1), RoomId(3), RoomId(5)], Some(RoomId(3)), 7);
        assert_eq!(geom.gaps.len(), 3);
        let forward = geom
            .forward_gap()
            .expect("a forward gap toward the objective");
        assert_eq!(forward.target, RoomId(3));
        assert_eq!(
            geom.gaps
                .iter()
                .filter(|g| g.kind == GapKind::Forward)
                .count(),
            1
        );
    }

    #[test]
    fn rooms_are_convex_polygons_with_enough_edges_for_their_doorways() {
        // Across seeds, a room is a 4–8 sided convex polygon with at least one edge per
        // connection, and its gaps sit on distinct edges (their centres differ).
        for seed in 0..40u64 {
            let geom = room_geom(&[RoomId(1), RoomId(3), RoomId(5)], Some(RoomId(3)), seed);
            let poly = geom.poly.as_ref().expect("a room is a polygon");
            assert!(
                (4..=8).contains(&poly.len()) && poly.len() >= geom.gaps.len(),
                "seed {seed}: {} sides for {} doors",
                poly.len(),
                geom.gaps.len()
            );
            // Distinct doorway edges.
            for i in 0..geom.gaps.len() {
                for j in (i + 1)..geom.gaps.len() {
                    assert!(
                        (geom.gaps[i].center - geom.gaps[j].center).length() > 0.5,
                        "seed {seed}: doorways share an edge"
                    );
                }
            }
        }
    }

    #[test]
    fn varied_straight_hallways_have_distinct_lengths() {
        // The straight connector renders at visibly different depths per edge seed.
        let template = hallway::template(0);
        let a = hallway_geom(RoomId(0), RoomId(1), template, 11, false)
            .half
            .y;
        let differ = (0..64u64).any(|s| {
            (hallway_geom(RoomId(0), RoomId(1), template, s, false)
                .half
                .y
                - a)
                .abs()
                > 1.0
        });
        assert!(
            differ,
            "straight hallway length should vary with the edge seed"
        );
    }

    #[test]
    fn hallway_geom_has_an_entry_and_an_exit() {
        let template = hallway::template(0);
        let geom = hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        assert!(
            geom.gaps
                .iter()
                .any(|g| g.kind == GapKind::Entry && g.target == RoomId(0))
        );
        assert!(
            geom.gaps
                .iter()
                .any(|g| g.kind == GapKind::Exit && g.target == RoomId(1))
        );
    }

    #[test]
    fn crossing_detects_an_outward_pass_through_the_gap() {
        let gap = DoorGap {
            center: Vec2::new(0.0, -ROOM_HALF),
            normal: Vec2::new(0.0, -1.0),
            width: THRESHOLD_WIDTH,
            target: RoomId(2),
            kind: GapKind::Forward,
        };
        // Walk from inside (z > -ROOM_HALF) to outside (z < -ROOM_HALF), on-centre.
        assert!(crossed(
            Vec2::new(0.0, -ROOM_HALF + 0.5),
            Vec2::new(0.0, -ROOM_HALF - 0.5),
            &gap
        ));
        // Moving away (inward) does not cross.
        assert!(!crossed(
            Vec2::new(0.0, -ROOM_HALF + 0.5),
            Vec2::new(0.0, 0.0),
            &gap
        ));
        // Crossing the threshold plane but outside the gap width does not count.
        assert!(!crossed(
            Vec2::new(THRESHOLD_WIDTH, -ROOM_HALF + 0.5),
            Vec2::new(THRESHOLD_WIDTH, -ROOM_HALF - 0.5),
            &gap
        ));
    }

    #[test]
    fn the_room_hallway_room_loop_advances_to_the_target() {
        // In room 0, objective is room 1; connections 0↔1 and 0↔3.
        let nav = nav(&[1, 3], Some(1));
        let place = Place::Room(RoomId(0));
        let forward = *geom_for(place, &nav).forward_gap().unwrap();
        assert_eq!(forward.target, RoomId(1));

        // Cross the forward doorway → enter the 0→1 hallway with the edge's variation.
        let (place, crossing) = apply_crossing(place, &forward, &nav);
        assert_eq!(
            crossing,
            Crossing::EnteredHallway {
                from: RoomId(0),
                to: RoomId(1)
            }
        );
        assert_eq!(
            place,
            Place::Hallway {
                from: RoomId(0),
                to: RoomId(1),
                variation: hallway::variation_for(RoomId(0), RoomId(1), nav.seed, nav.version),
            }
        );

        // Walk to the hallway's exit and cross → arrive in room 1.
        let exit = *geom_for(place, &nav)
            .gaps
            .iter()
            .find(|g| g.kind == GapKind::Exit)
            .unwrap();
        let (place, crossing) = apply_crossing(place, &exit, &nav);
        assert_eq!(crossing, Crossing::ArrivedRoom(RoomId(1)));
        assert_eq!(place, Place::Room(RoomId(1)));
    }

    #[test]
    fn an_anchored_edge_keeps_its_hallway_through_decoherence() {
        let mut n = nav(&[1, 3], Some(1));
        n.version = 5; // the live structure has rerolled five times
        // Without a pin, edge (0,1) follows the live decohere version.
        assert_eq!(n.effective_version(RoomId(0), RoomId(1)), 5);
        // Pin edge (0,1) at version 2 (when the torch was dropped).
        n.pins.push(PinnedEdge {
            a: RoomId(0),
            b: RoomId(1),
            version: 2,
        });
        assert_eq!(n.effective_version(RoomId(0), RoomId(1)), 2);
        assert_eq!(
            n.effective_version(RoomId(1), RoomId(0)),
            2,
            "the pin is edge-unordered"
        );
        // A different edge is unaffected — it still re-rolls.
        assert_eq!(n.effective_version(RoomId(0), RoomId(3)), 5);
        // Crossing into the pinned edge yields the frozen variation, not the live one.
        let gap = *room_geom(&n.connections, n.target_room, 1)
            .forward_gap()
            .unwrap();
        let (place, _) = apply_crossing(Place::Room(RoomId(0)), &gap, &n);
        let pinned = match place {
            Place::Hallway { variation, .. } => variation,
            _ => panic!("entered a hallway"),
        };
        assert_eq!(
            pinned,
            hallway::variation_for(RoomId(0), RoomId(1), n.seed, 2)
        );
    }

    #[test]
    fn entry_spawn_places_the_body_just_inside_the_arrival_gap() {
        // Arriving in a room from room 0: spawn just inside the doorway back to 0.
        let geom = room_geom(&[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        let spawn = entry_spawn(&geom, RoomId(0));
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        // Spawn is inset inward from the gap (closer to the room centre).
        assert!(spawn.length() < back.center.length());
    }

    #[test]
    fn align2d_inverse_round_trips() {
        let a = Align2d {
            yaw: 0.9,
            offset: Vec2::new(3.0, -4.0),
        };
        for p in [Vec2::new(1.0, 2.0), Vec2::new(-5.0, 0.3), Vec2::ZERO] {
            let back = a.inverse_apply(a.apply(p));
            assert!((back - p).length() < 1e-4, "round trip {p:?} -> {back:?}");
        }
    }

    #[test]
    fn crossing_a_doorway_carries_the_body_in_continuously() {
        // Room 0 → its 0→1 hallway across the forward gap: the alignment maps the body's
        // pose continuously into the hallway frame — no snap, no view flip.
        let nav = nav(&[1, 3], Some(1));
        let room = Place::Room(RoomId(0));
        let gap = *geom_for(room, &nav).forward_gap().unwrap();
        let (hall, _) = apply_crossing(room, &gap, &nav);
        let hgeom = geom_for(hall, &nav);
        let align = crossing_alignment(&hgeom, hall, &gap, RoomId(0)).expect("hallway alignment");

        // A body just past the room doorway (outward, along the gap normal) maps to just
        // inside the hallway entry (−Z side of its footprint), not snapped elsewhere.
        let threshold = gap.center + gap.normal * 0.3;
        let inside = align.inverse_apply(threshold);
        assert!(
            inside.y < 0.0 && inside.y > -hgeom.half.y,
            "lands just inside the hallway entry: {inside:?}"
        );
        assert!(inside.x.abs() <= hgeom.half.x, "within the hallway width");

        // Heading carries through: walking out through the gap (forward == gap normal)
        // becomes walking +Z into the hallway, regardless of the doorway's facing.
        let old_yaw = gap.normal.x.atan2(-gap.normal.y); // forward(old_yaw) == gap.normal
        let new_yaw = old_yaw + align.yaw;
        let fwd = Vec2::new(new_yaw.sin(), -new_yaw.cos());
        assert!(fwd.y > 0.9, "now facing into the hallway (+Z): {fwd:?}");
    }

    #[test]
    fn entering_a_room_keeps_the_arrival_doorway_an_open_passage() {
        // Room 1 connects back to 0 (where we came from) and on to 2 (the objective).
        let mut geom = room_geom(&[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        // By default the doorway back toward 0 is a sealed Side wall.
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        assert_eq!(back.kind, GapKind::Side);
        // Re-opening the arrival doorway makes it a real passage (so it doesn't pop into a
        // wall behind you) while the forward objective doorway is untouched.
        open_entry(&mut geom, Some(RoomId(0)));
        let back = geom.gaps.iter().find(|g| g.target == RoomId(0)).unwrap();
        assert_eq!(back.kind, GapKind::Entry);
        assert!(back.kind.is_passage());
        assert!(geom.forward_gap().is_some(), "forward doorway is preserved");
        // The start room (no arrival doorway) keeps every non-forward door sealed.
        let mut start = room_geom(&[RoomId(0), RoomId(2)], Some(RoomId(2)), 5);
        open_entry(&mut start, None);
        assert!(start.gaps.iter().all(|g| g.kind != GapKind::Entry));
    }

    #[test]
    fn crossing_a_hallway_exit_carries_the_body_into_the_room_continuously() {
        // Hallway 0→1 exit into room 1 (which connects back to 0 and on to 2).
        let nav1 = nav(&[0, 2], Some(2));
        let hall = Place::Hallway {
            from: RoomId(0),
            to: RoomId(1),
            variation: hallway::variation_for(RoomId(0), RoomId(1), nav1.seed, nav1.version),
        };
        let hgeom = geom_for(hall, &nav1);
        let exit = *hgeom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
        let mut rgeom = geom_for(Place::Room(RoomId(1)), &nav1);
        open_entry(&mut rgeom, Some(RoomId(0)));
        let align = crossing_alignment(&rgeom, Place::Room(RoomId(1)), &exit, RoomId(0))
            .expect("the arrival doorway resolves an alignment");
        // A body just past the hallway exit maps to inside the destination room footprint.
        let threshold = exit.center + exit.normal * 0.3;
        let inside = align.inverse_apply(threshold);
        assert!(
            inside.x.abs() <= rgeom.half.x + 0.6 && inside.y.abs() <= rgeom.half.y + 0.6,
            "lands inside the room footprint: {inside:?} (half {:?})",
            rgeom.half,
        );
    }

    #[test]
    fn an_edge_rolls_its_hallway_by_decohere_version() {
        let nav = nav(&[1], Some(1));
        let gap = *room_geom(&nav.connections, nav.target_room, 1)
            .forward_gap()
            .unwrap();
        let (place, _) = apply_crossing(Place::Room(RoomId(0)), &gap, &nav);
        let v0 = match place {
            Place::Hallway { variation, .. } => variation,
            _ => panic!("entered a hallway"),
        };
        assert_eq!(
            v0,
            hallway::variation_for(RoomId(0), RoomId(1), nav.seed, nav.version)
        );
        // The selection is version-keyed, so an unobserved re-roll can change it.
        assert!((1..32).any(|v| hallway::variation_for(RoomId(0), RoomId(1), nav.seed, v) != v0));
    }

    fn inside_any_solid(arena: &FpsArena, p: Vec3) -> bool {
        arena.solids.iter().any(|s| {
            p.x >= s.min.x
                && p.x <= s.max.x
                && p.y >= s.min.y
                && p.y <= s.max.y
                && p.z >= s.min.z
                && p.z <= s.max.z
        })
    }

    /// The most-violated wall signed distance for `p` (positive = inside), ignoring open
    /// doorway edges. >= radius means the body is safely contained.
    fn deepest_inside(geom: &PlaceGeom, p: Vec2) -> f32 {
        let poly = geom.poly.as_ref().unwrap();
        let n = poly.len();
        let mut worst = f32::INFINITY;
        for i in 0..n {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            let mid = (a + b) * 0.5;
            let is_door = geom
                .gaps
                .iter()
                .any(|g| g.kind.is_passage() && (g.center - mid).length() < 0.05);
            if is_door {
                continue;
            }
            worst = worst.min((p - a).dot(-outward_normal(a, b)));
        }
        worst
    }

    #[test]
    fn a_polygon_room_contains_the_body_but_opens_at_the_doorway() {
        let geom = room_geom(&[RoomId(1)], Some(RoomId(1)), 4);
        let r = 0.4;
        // A polygon room has no AABB walls — its angled walls are the `contain` clamp.
        assert!(
            place_arena(&geom, 0.0, 3.4).solids.is_empty(),
            "a polygon room collides only with the floor"
        );
        let gap = *geom.forward_gap().unwrap();
        // A body driven far outside a wall (away from the door) is pulled back inside.
        let clamped = contain(&geom, -gap.normal * 100.0, r);
        assert!(
            deepest_inside(&geom, clamped) >= r - 0.1,
            "a body outside a wall is contained inside the room"
        );
        // Stepping out through the doorway is allowed (not clamped back).
        let at_door = gap.center + gap.normal * 0.3;
        let after = contain(&geom, at_door, r);
        assert!(
            (after - at_door).length() < 0.01,
            "the doorway stays open so the body can cross"
        );
    }

    #[test]
    fn hallway_arena_opens_both_ends_and_walls_the_sides() {
        let template = hallway::template(0);
        let geom = hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        let arena = place_arena(&geom, 0.0, 3.4);
        let y = 1.0;
        // Entry (−Z) and exit (+Z) are open at the centreline.
        assert!(!inside_any_solid(&arena, Vec3::new(0.0, y, -geom.half.y)));
        assert!(!inside_any_solid(&arena, Vec3::new(0.0, y, geom.half.y)));
        // The long side wall is solid.
        assert!(inside_any_solid(&arena, Vec3::new(geom.half.x, y, 0.0)));
    }

    /// The templates whose flavour is a generated labyrinth.
    fn maze_templates() -> Vec<&'static hallway::HallwayTemplate> {
        hallway::TEMPLATES
            .iter()
            .filter(|t| t.flavor == hallway::HallwayFlavor::Maze)
            .collect()
    }

    #[test]
    fn a_maze_hallway_has_entrances_and_exits_with_interior_walls() {
        for template in maze_templates() {
            for seed in 0..6u64 {
                let geom = hallway_geom(RoomId(2), RoomId(7), template, seed, false);
                let entries: Vec<_> = geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Entry)
                    .collect();
                let exits: Vec<_> = geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Exit)
                    .collect();
                assert!(!entries.is_empty(), "{} has an entrance", template.name);
                assert!(!exits.is_empty(), "{} has an exit", template.name);
                assert!(
                    entries.iter().all(|g| g.target == RoomId(2)),
                    "every entrance leads back to `from`"
                );
                assert!(
                    exits.iter().all(|g| g.target == RoomId(7)),
                    "every exit leads on to `to`"
                );
                assert!(
                    !geom.interior.is_empty(),
                    "{} is a real maze with interior walls",
                    template.name
                );
            }
        }
    }

    /// Can a body of the controller's radius reach the exit from the entry through the
    /// built collision arena? Flood the free space on a fine lattice, confined to the
    /// footprint, and require the exit cell to be reachable from the entry spawn. This
    /// exercises the whole pipeline: maze → interior walls → arena → walkable.
    fn maze_is_walkable(geom: &PlaceGeom) -> bool {
        const STEP: f32 = 0.25;
        const R: f32 = 0.4; // controller body radius
        const HH: f32 = 0.9; // controller half-height
        let arena = place_arena(geom, 0.0, 3.4);
        let half = geom.half;
        let blocked = |px: f32, pz: f32| -> bool {
            let (cy, hy) = (HH, HH); // feet on the floor (floor_y = 0)
            arena.solids.iter().any(|s| {
                px - R < s.max.x
                    && px + R > s.min.x
                    && cy - hy < s.max.y
                    && cy + hy > s.min.y
                    && pz - R < s.max.z
                    && pz + R > s.min.z
            })
        };
        let entry = geom.gaps.iter().find(|g| g.kind == GapKind::Entry).unwrap();
        let exit = geom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
        let start = entry.center - entry.normal * ENTRY_INSET;
        let goal = exit.center - exit.normal * ENTRY_INSET;
        let key = |x: f32, z: f32| -> (i32, i32) {
            ((x / STEP).round() as i32, (z / STEP).round() as i32)
        };
        let goal_key = key(goal.x, goal.y);
        let start_key = key(start.x, start.y);
        if blocked(start_key.0 as f32 * STEP, start_key.1 as f32 * STEP) {
            return false; // spawn itself must be clear
        }
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(start_key);
        queue.push_back(start_key);
        while let Some((ix, iz)) = queue.pop_front() {
            if (ix, iz) == goal_key {
                return true;
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (ix + dx, iz + dz);
                let (wx, wz) = (nx as f32 * STEP, nz as f32 * STEP);
                // Stay strictly inside the footprint so the flood can't leak out a gap.
                if wx.abs() >= half.x || wz.abs() >= half.y {
                    continue;
                }
                if seen.contains(&(nx, nz)) || blocked(wx, wz) {
                    continue;
                }
                seen.insert((nx, nz));
                queue.push_back((nx, nz));
            }
        }
        false
    }

    #[test]
    fn a_maze_hallway_is_walkable_from_entry_to_exit() {
        for template in maze_templates() {
            for seed in 0..12u64 {
                let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
                assert!(
                    maze_is_walkable(&geom),
                    "{} (seed {seed}) must be walkable entry→exit",
                    template.name
                );
            }
        }
    }

    fn chicane_template() -> &'static hallway::HallwayTemplate {
        hallway::TEMPLATES
            .iter()
            .find(|t| t.flavor == hallway::HallwayFlavor::Chicane)
            .expect("a chicane template exists")
    }

    #[test]
    fn a_chicane_hallway_is_a_walkable_s_bend() {
        let template = chicane_template();
        for seed in 0..16u64 {
            let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
            let entry = geom.gaps.iter().find(|g| g.kind == GapKind::Entry).unwrap();
            let exit = geom.gaps.iter().find(|g| g.kind == GapKind::Exit).unwrap();
            assert_eq!(entry.target, RoomId(1));
            assert_eq!(exit.target, RoomId(4));
            assert_eq!(geom.interior.len(), 2, "two staggered baffles form the S");
            // The slalom: entry and exit doorways sit on opposite sides of the corridor.
            assert!(
                entry.center.x * exit.center.x < 0.0,
                "seed {seed}: entry and exit are offset to opposite sides"
            );
            assert!(
                maze_is_walkable(&geom),
                "chicane (seed {seed}) must be walkable entry→exit"
            );
        }
    }

    fn colonnade_templates() -> Vec<&'static hallway::HallwayTemplate> {
        hallway::TEMPLATES
            .iter()
            .filter(|t| t.flavor == hallway::HallwayFlavor::Colonnade)
            .collect()
    }

    #[test]
    fn a_colonnade_is_a_walkable_pillared_pseudo_room() {
        for template in colonnade_templates() {
            for seed in 0..16u64 {
                let geom = hallway_geom(RoomId(1), RoomId(4), template, seed, false);
                // It is a real pillared volume (a grid of interior columns), open at both
                // ends, and reachable entry→exit down the clear central lane.
                assert!(
                    geom.interior.len() >= 4,
                    "{} (seed {seed}) has a grid of pillars",
                    template.name
                );
                assert!(
                    geom.gaps.iter().any(|g| g.kind == GapKind::Entry)
                        && geom.gaps.iter().any(|g| g.kind == GapKind::Exit),
                    "{} is open at both ends",
                    template.name
                );
                // The central lane (x = 0) is clear: no pillar straddles it.
                assert!(
                    geom.interior
                        .iter()
                        .all(|p| p.center.x.abs() - p.half.x > 0.0),
                    "{} keeps a clear central lane",
                    template.name
                );
                assert!(
                    maze_is_walkable(&geom),
                    "{} (seed {seed}) must be walkable entry→exit",
                    template.name
                );
            }
        }
    }

    #[test]
    fn room_footprints_vary_in_size_across_seeds() {
        // Rooms aren't all one size — some read as tight, some as hub-like.
        let areas: Vec<f32> = (0..24u64)
            .map(|seed| {
                let g = room_geom(&[RoomId(1), RoomId(2), RoomId(3)], Some(RoomId(1)), seed);
                g.half.x * g.half.y
            })
            .collect();
        let min = areas.iter().copied().fold(f32::INFINITY, f32::min);
        let max = areas.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            max > min * 1.3,
            "room footprints should vary in size (min {min}, max {max})"
        );
    }

    #[test]
    fn walking_any_hallway_never_climbs_onto_the_roof() {
        use observed_traversal::{FIXED_DT, FpsBody, FpsConfig, step_body};
        use player_input::PlayerIntent;
        let config = FpsConfig::default();
        for (i, template) in hallway::TEMPLATES.iter().enumerate() {
            for seed in 0..8u64 {
                let geom = hallway_geom(RoomId(0), RoomId(1), template, seed, false);
                let arena = place_arena(&geom, 0.0, 3.4);
                let spawn = entry_spawn(&geom, RoomId(0));
                // Face into the hall (+Z, toward the exit), as `place_body` does.
                let mut body =
                    FpsBody::spawned(Vec3::new(spawn.x, config.half_height, spawn.y), PI);
                // Drive forward with a weaving strafe to provoke corner wedging against
                // the perimeter and any interior (maze/baffle) walls.
                for tick in 0..480u32 {
                    let strafe = if (tick / 30) % 2 == 0 { 1.0 } else { -1.0 };
                    step_body(
                        &mut body,
                        PlayerIntent {
                            movement: Vec2::new(strafe, 1.0),
                            ..Default::default()
                        },
                        &arena,
                        &config,
                        FIXED_DT,
                    );
                    let feet = body.position.y - config.half_height;
                    assert!(
                        feet < 0.5,
                        "template {i} ({}) seed {seed} tick {tick}: roofed at feet y={feet}",
                        template.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_hallway_to_the_exit_locks_its_door_when_the_gate_is_shut() {
        let template = hallway::template(0); // a straight connector
        // Heading into the exit room with the gate locked → a solid LockedExit door.
        let locked = hallway_geom(RoomId(7), RoomId(EXIT_ROOM), template, 0, true);
        let exit = locked
            .gaps
            .iter()
            .find(|g| matches!(g.kind, GapKind::LockedExit))
            .expect("a locked exit door");
        assert!(!exit.kind.is_passage(), "a locked exit cannot be crossed");
        // place_arena must wall it off (no void to walk into).
        let arena = place_arena(&locked, 0.0, 3.4);
        assert!(
            inside_any_solid(&arena, Vec3::new(exit.center.x, 1.0, exit.center.y)),
            "the locked exit doorway is solid"
        );

        // Unlocked (gate open) → a normal, crossable Exit at the same place.
        let open = hallway_geom(RoomId(7), RoomId(EXIT_ROOM), template, 0, false);
        assert!(
            open.gaps.iter().any(|g| g.kind == GapKind::Exit),
            "an unlocked exit is a normal passage"
        );
        assert!(!open.gaps.iter().any(|g| g.kind == GapKind::LockedExit));

        // The lock only applies to the exit room — other destinations stay open.
        let elsewhere = hallway_geom(RoomId(1), RoomId(4), template, 0, true);
        assert!(elsewhere.gaps.iter().any(|g| g.kind == GapKind::Exit));
    }
}
