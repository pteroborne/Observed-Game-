//! Transition mechanics, alignment calculations, and arena building.

use super::{
    DoorGap, ENTRY_INSET, Nav, Place, PlaceGeom, ThresholdLocalSide, corridor_id_for,
    corridor_socket_for,
};
use crate::hallway;
use bevy::math::{Quat, Vec2, Vec3};
use observed_core::{CorridorId, PlaceId, RoomId, ThresholdId};
use observed_facility::junction::{CorridorSpec, JunctionTopology, ThresholdAttachment};
use observed_traversal::rapier_controller::{RapierTraversalScene, StructuralCollider};
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
    /// Entered a corridor through one of its threshold sockets. `corridor` is the stable
    /// identity resolved by the junction topology; `from`/`to` are the thin pair-shaped
    /// accessor deferred consumers still read (the room you left / the corridor's other
    /// endpoint).
    EnteredHallway {
        corridor: CorridorId,
        from: RoomId,
        to: RoomId,
    },
    /// Arrived in a room (from a corridor exit socket, or back through its entry socket).
    ArrivedRoom(RoomId),
}

/// Build the active room/corridor **socket topology** for the current place from the nav
/// projection (the simulation-owned connectivity). This is the single reciprocal
/// structure both the crossing resolver ([`apply_crossing`]) and the geometry passage set
/// consult, so a rendered aperture, a physical aperture, and a graph connection cannot
/// disagree: an unattached or sealed socket has no `partner`, so it is un-crossable *and*
/// renders as a wall by the same fact.
///
/// A room place exposes one attachment per live (non-sealed) connection: the room-side
/// socket ↔ its derived corridor's room-side socket. A hallway place exposes both of its
/// endpoints' attachments, so crossing either the entry or the exit resolves back to a
/// room through the same reciprocal lookup.
pub fn place_junction(place: Place, nav: &Nav) -> JunctionTopology {
    let mut specs: Vec<CorridorSpec> = Vec::new();
    let mut attachments: Vec<ThresholdAttachment> = Vec::new();
    let mut attach = |from_room: RoomId, target: RoomId, room_slot: super::ThresholdSlotId| {
        let cid = corridor_id_for(from_room, target);
        if !specs.iter().any(|spec| spec.id == cid) {
            specs.push(CorridorSpec::with_slot_count(cid, 2));
        }
        let room_tid = ThresholdId::new(PlaceId::Room(from_room), room_slot.0);
        let corridor_slot = corridor_socket_for(from_room, target, from_room);
        let corridor_tid = ThresholdId::new(PlaceId::Corridor(cid), corridor_slot.0);
        if let Ok(attachment) = ThresholdAttachment::new(room_tid, corridor_tid) {
            attachments.push(attachment);
        }
    };
    match place {
        Place::Room(room) => {
            for connection in &nav.connections {
                let slot = nav
                    .slot_for(*connection)
                    .unwrap_or_else(|| default_room_slot(nav, *connection));
                if nav.sealed_slots.contains(&slot) {
                    continue;
                }
                attach(room, *connection, slot);
            }
        }
        Place::Hallway { from, to, .. } => {
            let from_slot = nav
                .hallway_entry_room_slot
                .unwrap_or(super::ThresholdSlotId(0));
            let to_slot = nav
                .hallway_exit_room_slot
                .unwrap_or(super::ThresholdSlotId(0));
            attach(from, to, from_slot);
            attach(to, from, to_slot);
        }
    }
    JunctionTopology::new(specs, attachments).unwrap_or_default()
}

/// Fallback room-side slot for a connection with no explicit `connection_slots` entry:
/// its position in the sorted connection list, mirroring `room_geom`'s fallback so the
/// topology and the rendered doorway pick the same slot.
fn default_room_slot(nav: &Nav, connection: RoomId) -> super::ThresholdSlotId {
    let mut sorted: Vec<RoomId> = nav.connections.clone();
    sorted.sort_unstable_by_key(|r| r.0);
    sorted.dedup();
    let index = sorted.iter().position(|r| *r == connection).unwrap_or(0);
    super::ThresholdSlotId(index as u16)
}

/// Apply a doorway crossing: returns the new place and what happened. Both directions
/// resolve through [`JunctionTopology::partner`] on the active socket set — never by
/// reconstructing a room pair to decide connectivity. From a room, the crossed room
/// socket partners into a corridor socket, so you enter that corridor (rolling its
/// presentation variation); from a corridor, the crossed corridor socket partners back to
/// a room socket, so you arrive in that room. A socket with no partner (sealed/collapsed)
/// is simply un-crossable and this returns the place unchanged.
pub fn apply_crossing(place: Place, gap: &DoorGap, nav: &Nav) -> (Place, Crossing) {
    let topology = place_junction(place, nav);
    match place {
        Place::Room(room) => {
            let room_tid = ThresholdId::new(PlaceId::Room(room), gap.threshold.room.slot.0);
            let corridor = match topology.partner(room_tid).map(|partner| partner.place) {
                Some(PlaceId::Corridor(cid)) => cid,
                // No topology partner (authored/dev nav without slots): fall back to the
                // gap's own derived corridor so single-exit play is unchanged.
                _ => corridor_id_for(room, gap.target),
            };
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
                Crossing::EnteredHallway {
                    corridor,
                    from: room,
                    to,
                },
            )
        }
        Place::Hallway { from, to, .. } => {
            let cid = corridor_id_for(from, to);
            let corridor_slot = corridor_socket_for(from, to, gap.target);
            let corridor_tid = ThresholdId::new(PlaceId::Corridor(cid), corridor_slot.0);
            let arrived = match topology.partner(corridor_tid).map(|partner| partner.place) {
                Some(PlaceId::Room(room)) => room,
                // No topology partner: fall back to the gap's annotated destination room.
                _ => gap.target,
            };
            (Place::Room(arrived), Crossing::ArrivedRoom(arrived))
        }
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
        Place::Room(_) => {
            arrival_gap(geom, place, crossed, from).map(|back| room_alignment(crossed, back))
        }
    }
}

/// The specific gap in the new place `geom` that the body arrives through, having crossed
/// `crossed` from room `from`. This is the same match `crossing_alignment` resolves
/// internally, exposed so callers can read the arrival gap's `floor_y` (a gantry hall's
/// entry now yields both a deck-level arrival and a ground-level return sharing
/// `target == from`, so a plain "first gap targeting `from`" lookup is ambiguous; matching
/// the crossed doorway's threshold identity picks the right one). `None` if the
/// destination room has no doorway back toward `from` (a mid-crossing decohere).
pub fn arrival_gap<'a>(
    geom: &'a PlaceGeom,
    place: Place,
    crossed: &DoorGap,
    from: RoomId,
) -> Option<&'a DoorGap> {
    match place {
        Place::Hallway { .. } => geom.gaps.iter().find(|candidate| {
            candidate.threshold.hall == crossed.threshold.hall
                && candidate.threshold.hall.side == crossed.threshold.room.room
                && candidate.threshold.local_side == ThresholdLocalSide::Hall
        }),
        Place::Room(_) => geom
            .gaps
            .iter()
            .find(|g| g.target == from && g.threshold.room == crossed.threshold.room)
            .or_else(|| geom.gaps.iter().find(|g| g.target == from)),
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
    let mut solids: Vec<Aabb3> = Vec::new();
    if geom.poly.is_some() {
        // Polygon containment owns the angled perimeter. Raised apertures still need a
        // physical lower panel because containment is intentionally 2D and otherwise
        // cannot distinguish a body beneath an elevated doorway.
        for gap in geom
            .gaps
            .iter()
            .filter(|gap| gap.kind.is_passage() && gap.floor_y > 0.01)
        {
            solids.push(Aabb3::from_center_half(
                Vec3::new(gap.center.x, floor_y + gap.floor_y * 0.5, gap.center.y),
                Vec3::new(gap.width * 0.5, gap.floor_y * 0.5, T),
            ));
        }
        for deck in &geom.decks {
            let height = deck.top_y - deck.bottom_y;
            if height > 0.01 {
                solids.push(Aabb3::from_center_half(
                    Vec3::new(
                        deck.center.x,
                        floor_y + deck.bottom_y + height * 0.5,
                        deck.center.y,
                    ),
                    Vec3::new(deck.half.x, height * 0.5, deck.half.y),
                ));
            }
        }
        return FpsArena {
            solids,
            floor_y,
            floor_half: half.x.max(half.y) + 5.0,
        };
    }

    let total_height = structural_height(geom, wall_height);
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

const WALL_HALF_THICKNESS: f32 = 0.4;

/// Append one yawed structural wall cuboid spanning `start..end` on the XZ plane.
/// The local +X axis follows the wall, matching the renderer's `spawn_wall_segment`
/// convention. A small extension seals a whole polygon edge at its corners, but never
/// intrudes into a doorway opening.
fn push_polygon_wall(
    primitives: &mut Vec<StructuralCollider>,
    start: Vec2,
    end: Vec2,
    y_min: f32,
    y_max: f32,
    floor_y: f32,
    seal_corners: bool,
) {
    let delta = end - start;
    let length = delta.length();
    let height = y_max - y_min;
    if length <= 0.01 || height <= 0.01 {
        return;
    }
    let extension = if seal_corners {
        WALL_HALF_THICKNESS * 2.0
    } else {
        0.0
    };
    let center = (start + end) * 0.5;
    primitives.push(StructuralCollider {
        center: Vec3::new(center.x, floor_y + y_min + height * 0.5, center.y),
        half: Vec3::new(
            (length + extension) * 0.5,
            height * 0.5,
            WALL_HALF_THICKNESS,
        ),
        // After a Y rotation, local +X is `(cos(yaw), -sin(yaw))` in XZ.
        yaw: (-delta.y).atan2(delta.x),
    });
}

/// Project a place's authored structural geometry into collision primitives. The
/// projection keeps every physical rule in data: perimeter walls, all doorway cuts,
/// maze walls, and raised decks. It intentionally includes no decorative meshes.
///
/// Box halls retain their tested AABB projection. Convex rooms are promoted to yawed
/// edge panels, so their visible angled walls and their Rapier collision boundary agree
/// without a post-movement 2D containment correction.
pub fn place_structural_primitives(
    geom: &PlaceGeom,
    floor_y: f32,
    wall_height: f32,
) -> Vec<StructuralCollider> {
    let Some(poly) = geom.poly.as_ref() else {
        return place_arena(geom, floor_y, wall_height)
            .solids
            .iter()
            .map(|solid| {
                StructuralCollider::axis_aligned(
                    (solid.min + solid.max) * 0.5,
                    (solid.max - solid.min) * 0.5,
                )
            })
            .collect();
    };

    let total_height = structural_height(geom, wall_height);
    let mut primitives = Vec::new();
    for index in 0..poly.len() {
        let start = poly[index];
        let end = poly[(index + 1) % poly.len()];
        let edge = end - start;
        let edge_length = edge.length();
        if edge_length <= 0.01 {
            continue;
        }
        let tangent = edge / edge_length;
        // A doorway belongs to this edge when its centre projects onto it and lies on
        // the edge line. Sorting makes multiple attachments on one edge deterministic.
        let mut openings: Vec<(f32, f32, &DoorGap)> = geom
            .gaps
            .iter()
            .filter(|gap| gap.kind.is_passage())
            .filter_map(|gap| {
                let relative = gap.center - start;
                let along = relative.dot(tangent);
                let off_edge = (relative - tangent * along).length();
                (off_edge <= 0.05 && along >= -0.05 && along <= edge_length + 0.05).then_some((
                    (along - gap.width * 0.5).clamp(0.0, edge_length),
                    (along + gap.width * 0.5).clamp(0.0, edge_length),
                    gap,
                ))
            })
            .collect();
        openings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let has_openings = !openings.is_empty();

        let mut cursor = 0.0;
        for (lo, hi, gap) in openings {
            if lo > cursor {
                push_polygon_wall(
                    &mut primitives,
                    start + tangent * cursor,
                    start + tangent * lo,
                    0.0,
                    total_height,
                    floor_y,
                    false,
                );
            }
            if hi <= cursor {
                continue;
            }
            let opening_start = start + tangent * lo.max(cursor);
            let opening_end = start + tangent * hi;
            push_polygon_wall(
                &mut primitives,
                opening_start,
                opening_end,
                0.0,
                gap.floor_y,
                floor_y,
                false,
            );
            push_polygon_wall(
                &mut primitives,
                opening_start,
                opening_end,
                gap.floor_y + wall_height,
                total_height,
                floor_y,
                false,
            );
            cursor = hi;
        }
        if cursor < edge_length {
            push_polygon_wall(
                &mut primitives,
                start + tangent * cursor,
                end,
                0.0,
                total_height,
                floor_y,
                !has_openings,
            );
        }
    }

    for segment in &geom.interior {
        primitives.push(StructuralCollider::axis_aligned(
            Vec3::new(
                segment.center.x,
                floor_y + wall_height * 0.5,
                segment.center.y,
            ),
            Vec3::new(segment.half.x, wall_height * 0.5, segment.half.y),
        ));
    }
    for deck in &geom.decks {
        let height = deck.top_y - deck.bottom_y;
        if height > 0.01 {
            primitives.push(StructuralCollider::axis_aligned(
                Vec3::new(
                    deck.center.x,
                    floor_y + deck.bottom_y + height * 0.5,
                    deck.center.y,
                ),
                Vec3::new(deck.half.x, height * 0.5, deck.half.y),
            ));
        }
    }
    primitives
}

/// Build the production Rapier collision scene for one discrete place. Unlike the
/// legacy [`place_arena`] bridge, this scene contains yawed convex-room perimeter walls
/// and therefore owns every player collision constraint.
pub fn place_rapier_scene(
    geom: &PlaceGeom,
    floor_y: f32,
    wall_height: f32,
) -> RapierTraversalScene {
    let primitives = place_structural_primitives(geom, floor_y, wall_height);
    RapierTraversalScene::from_primitives(floor_y, geom.half.x.max(geom.half.y) + 5.0, &primitives)
}
