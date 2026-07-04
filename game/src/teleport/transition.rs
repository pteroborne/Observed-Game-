//! Transition mechanics, alignment calculations, and arena building.

use super::{DoorGap, ENTRY_INSET, Nav, Place, PlaceGeom, ThresholdLocalSide};
use crate::hallway;
use bevy::math::{Quat, Vec2, Vec3};
use observed_core::RoomId;
use observed_traversal::{Aabb3, FpsArena};

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

/// How far a body's feet (`world_feet_y`, in world space) may sit from a gap's local
/// `floor_y` and still be considered "at" that floor for the purposes of using the gap. A
/// generous tolerance: it just needs to be tighter than the gap between two distinct
/// floors the gantry hall uses (0 vs `UPPER_DECK_Y` = 2.1), never zero-crossing rounding.
pub const GAP_FLOOR_TOLERANCE: f32 = 1.2;

/// Are a body's feet at the local floor height `gap.floor_y` requires, within tolerance?
/// `world_feet_y` is the body's feet in world space (`position.y - half_height`);
/// `place_floor_y` is the current place's world floor offset (`place_y_offset`), so the
/// comparison is done in the place's local frame the same way `DoorGap`/`GantryThreshold`
/// author their `floor_y`. Ground-level gaps (`floor_y == 0.0`) keep today's behaviour
/// exactly: any grounded body within the tolerance of the floor still crosses.
pub fn feet_at_gap_floor(world_feet_y: f32, place_floor_y: f32, gap: &DoorGap) -> bool {
    let local_feet_y = world_feet_y - place_floor_y;
    (local_feet_y - gap.floor_y).abs() <= GAP_FLOOR_TOLERANCE
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
/// `offset`. It places a *child* place's local frame into the **current** place's frame â€”
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

fn yaw_mapping(local: Vec2, world: Vec2) -> f32 {
    world.x.atan2(world.y) - local.x.atan2(local.y)
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

/// The alignment placing the **hallway** you cross `gap` into: the selected hallway-side
/// threshold slot tucks just beyond the room opening and the hallway extends away along
/// the doorway's outward normal. This is the slot-aware version of the preview/crossing
/// contract; a multi-entrance maze hall is never aligned by its centreline.
pub fn hallway_alignment(gap: &DoorGap, hallway: &PlaceGeom) -> Option<Align2d> {
    let hall_gap = hallway.gaps.iter().find(|candidate| {
        candidate.threshold.hall == gap.threshold.hall
            && candidate.threshold.hall.side == gap.threshold.room.room
            && candidate.threshold.local_side == ThresholdLocalSide::Hall
    })?;
    Some(hallway_gap_alignment(gap, hall_gap))
}

pub fn hallway_gap_alignment(gap: &DoorGap, hall_gap: &DoorGap) -> Align2d {
    let n = gap.normal;
    let yaw = yaw_mapping(hall_gap.normal, -n);
    Align2d {
        yaw,
        offset: (gap.center + n * PREVIEW_OUTSET) - rot_y(yaw, hall_gap.center),
    }
}

/// The alignment placing the **room** you cross `gap` into so its doorway `back` (the one
/// facing back toward where you stand) sits in the opening and the room extends away.
/// Mirrors `spawn_room_preview`.
pub fn room_alignment(gap: &DoorGap, back: &DoorGap) -> Align2d {
    let n = gap.normal;
    let yaw = yaw_mapping(back.normal, -n);
    let offset = (gap.center + n * PREVIEW_OUTSET) - rot_y(yaw, back.center);
    Align2d { yaw, offset }
}

/// The alignment carrying the body from the current place, across `crossed`, into the new
/// place `geom` it produced (a hallway uses its half-depth; a room uses its doorway back
/// toward `from`). `None` if the destination room has no doorway back toward `from` (a
/// mid-crossing decohere) â€” the caller then falls back to a plain entry snap.
pub fn crossing_alignment(
    geom: &PlaceGeom,
    place: Place,
    crossed: &DoorGap,
    from: RoomId,
) -> Option<Align2d> {
    match place {
        Place::Hallway { .. } => hallway_alignment(crossed, geom),
        Place::Room(_) => geom
            .gaps
            .iter()
            .find(|g| g.target == from && g.threshold.room == crossed.threshold.room)
            .or_else(|| geom.gaps.iter().find(|g| g.target == from))
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
pub fn wall_spans(half_len: f32, mut gaps: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
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

/// Structural height for a box place. Raised thresholds need a full doorway-height
/// opening above their local floor, so a split-level hall raises the shell by the
/// tallest threshold floor.
pub fn structural_height(geom: &PlaceGeom, wall_height: f32) -> f32 {
    wall_height
        + geom
            .gaps
            .iter()
            .map(|gap| gap.floor_y)
            .fold(0.0_f32, f32::max)
}

/// Build the collision world for a place: perimeter walls (as the proven controller's
/// AABB solids) around the footprint, each split into segments around its doorway
/// gaps so the body can walk out through the openings, plus any interior (maze) walls.
/// Polygon rooms have no axis-aligned perimeter â€” their angled walls are enforced by
/// [`contain`] and drawn from the polygon edges â€” so they collide only with the floor.
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

    let total_height = structural_height(geom, wall_height);
    let mut solids = solids;
    let mut segment = |cx: f32, cz: f32, hx: f32, hz: f32, y_min: f32, y_max: f32| {
        let height = y_max - y_min;
        if hx > 0.01 && hz > 0.01 && height > 0.01 {
            solids.push(Aabb3::from_center_half(
                Vec3::new(cx, floor_y + y_min + height * 0.5, cz),
                Vec3::new(hx, height * 0.5, hz),
            ));
        }
    };

    // West (âˆ’X) / East (+X) walls run along Z, split around their Z-centred *passage*
    // gaps (a Side door stays a solid wall, so the body can't walk out into the void).
    for sign in [-1.0_f32, 1.0] {
        let x = sign * half.x;
        let side_gaps: Vec<&DoorGap> = geom
            .gaps
            .iter()
            .filter(|g| {
                g.kind.is_passage() && (g.normal.x - sign).abs() < 0.5 && g.normal.y.abs() < 0.5
            })
            .collect();
        let gaps = side_gaps
            .iter()
            .map(|g| (g.center.y, g.width * 0.5))
            .collect();
        for (cz, hz) in wall_spans(half.y, gaps) {
            segment(x, cz, T, hz, 0.0, total_height);
        }
        for gap in side_gaps {
            let lo = (gap.center.y - gap.width * 0.5).max(-half.y);
            let hi = (gap.center.y + gap.width * 0.5).min(half.y);
            let cz = (lo + hi) * 0.5;
            let hz = (hi - lo) * 0.5;
            if gap.floor_y > 0.0 {
                segment(x, cz, T, hz, 0.0, gap.floor_y);
            }
            let aperture_top = gap.floor_y + wall_height;
            if aperture_top < total_height {
                segment(x, cz, T, hz, aperture_top, total_height);
            }
        }
    }
    // North (âˆ’Z) / South (+Z) walls run along X, split around their X-centred passage gaps.
    for sign in [-1.0_f32, 1.0] {
        let z = sign * half.y;
        let side_gaps: Vec<&DoorGap> = geom
            .gaps
            .iter()
            .filter(|g| {
                g.kind.is_passage() && (g.normal.y - sign).abs() < 0.5 && g.normal.x.abs() < 0.5
            })
            .collect();
        let gaps = side_gaps
            .iter()
            .map(|g| (g.center.x, g.width * 0.5))
            .collect();
        for (cx, hx) in wall_spans(half.x, gaps) {
            segment(cx, z, hx, T, 0.0, total_height);
        }
        for gap in side_gaps {
            let lo = (gap.center.x - gap.width * 0.5).max(-half.x);
            let hi = (gap.center.x + gap.width * 0.5).min(half.x);
            let cx = (lo + hi) * 0.5;
            let hx = (hi - lo) * 0.5;
            if gap.floor_y > 0.0 {
                segment(cx, z, hx, T, 0.0, gap.floor_y);
            }
            let aperture_top = gap.floor_y + wall_height;
            if aperture_top < total_height {
                segment(cx, z, hx, T, aperture_top, total_height);
            }
        }
    }

    // Interior walls (a labyrinth's maze walls), straight from the geometry. The
    // renderer spawns one wall cube per arena solid, so these render for free.
    for seg in &geom.interior {
        segment(
            seg.center.x,
            seg.center.y,
            seg.half.x,
            seg.half.y,
            0.0,
            wall_height,
        );
    }

    // Walkable raised decks. Platforms are thin upper slabs with lower-floor clearance;
    // stair treads remain floor-to-top blocks so the controller can step onto them.
    for deck in &geom.decks {
        let height = deck.top_y - deck.bottom_y;
        if height <= 0.01 {
            continue;
        }
        solids.push(Aabb3::from_center_half(
            Vec3::new(
                deck.center.x,
                floor_y + deck.bottom_y + height * 0.5,
                deck.center.y,
            ),
            Vec3::new(deck.half.x, height * 0.5, deck.half.y),
        ));
    }

    FpsArena {
        solids,
        floor_y,
        floor_half: half.x.max(half.y) + 5.0,
    }
}
