//! Geometry models and footprint generators for rooms and hallways.

use super::{
    DoorGap, GapKind, HallThreshold, MAZE_CELL, MAZE_WALL_T, Nav, Place, PlaceGeom, ROOM_HALF,
    RoomConnectionSlot, RoomThreshold, THRESHOLD_WIDTH, ThresholdLink, ThresholdLocalSide,
    ThresholdSlotId, WallSeg, corridor_id_for, corridor_socket_for, is_point_on_segment,
};
use crate::hallway;
use crate::layout::{ROOM_SCALE_HUB, ROOM_SCALE_MONITOR, ROOM_SCALE_STANDARD};
use crate::maze;
use bevy::math::Vec2;
use observed_core::RoomId;
use observed_facility::map_spec::{CorridorRole, RoomRole};
use observed_facility::room_def::RoomTemplate;
use observed_match::mutable::EXIT_ROOM;
use observed_traversal::gantry;
use std::f32::consts::PI;

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

const OBSERVATION_ROOM_SIDES: usize = 13;
const OBSERVATION_ROOM_SCALE: f32 = 2.1;

fn uses_observation_room_footprint(role: Option<RoomRole>) -> bool {
    role == Some(RoomRole::Monitor)
}

/// Whether `role` is a hub-style room (`Start`/`Exit`/`Decision`): the rooms teams
/// linger in to read doors, call routes, and make decisions, so they get the most
/// generous footprint (`ROOM_SCALE_HUB`, the liminal-scale dial in `layout.rs`).
fn is_hub_role(role: Option<RoomRole>) -> bool {
    matches!(
        role,
        Some(RoomRole::Start) | Some(RoomRole::Exit) | Some(RoomRole::Decision)
    )
}

/// The liminal-scale footprint multiplier for `role` (see the dials documented in
/// `layout.rs`): hub rooms read as the biggest, monitor rooms scale their many-sided
/// panel-bank shape up, and everything else gets the standard liminal bump.
fn room_scale_for_role(role: Option<RoomRole>) -> f32 {
    if uses_observation_room_footprint(role) {
        ROOM_SCALE_MONITOR
    } else if is_hub_role(role) {
        ROOM_SCALE_HUB
    } else {
        ROOM_SCALE_STANDARD
    }
}

/// A room's footprint polygon (CCW, centred at the origin), seeded so each room keeps a
/// stable shape: a varied **rectangle** (4 sides) or a regular **polygon** of 5â€“8 sides
/// (a pentagon/hexagon/heptagon/octagon, with a random orientation). `min_sides` forces
/// enough edges to host every doorway. Scaled by `room_scale_for_role` (the Phase 46b
/// liminal-scale dials in `layout.rs`) so hub/monitor/standard rooms breathe at
/// role-appropriate volumes on top of the seeded per-room variety.
fn room_polygon(seed: u64, role: Option<RoomRole>, template: Option<RoomTemplate>) -> Vec<Vec2> {
    let observation_room = uses_observation_room_footprint(role);
    let scale = room_scale_for_role(role);
    let profile = template.map(RoomTemplate::shell_profile);
    let n = profile
        .map(|profile| usize::from(profile.sides))
        .unwrap_or_else(|| {
            if observation_room {
                OBSERVATION_ROOM_SIDES
            } else {
                4 + (mix(seed, 1) % 5) as usize
            }
        });
    let x_scale = profile.map(|profile| profile.x_scale).unwrap_or(1.0);
    let z_scale = profile.map(|profile| profile.z_scale).unwrap_or(1.0);
    if n == 4 {
        // A varied rectangle for visual distinction from the polygons.
        let hx = ROOM_HALF * scale * x_scale * (0.92 + unit(seed, 2) * 0.24);
        let hz = ROOM_HALF * scale * z_scale * (0.92 + unit(seed, 3) * 0.24);
        return vec![
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ];
    }
    // A regular n-gon whose apothem (inradius) is a seeded 0.9Ã—â€“1.4Ã— of ROOM_HALF, so
    // rooms vary in size (some tight, some hub-like) the way the rectangles already do;
    // a random rotation keeps orientations varied.
    let apothem = if observation_room {
        ROOM_HALF * OBSERVATION_ROOM_SCALE * scale
    } else {
        ROOM_HALF * scale * (0.9 + unit(seed, 5) * 0.5)
    };
    let circumradius = apothem / (PI / n as f32).cos();
    let rot = unit(seed, 4) * (2.0 * PI / n as f32);
    (0..n)
        .map(|i| {
            let a = rot + i as f32 * 2.0 * PI / n as f32;
            Vec2::new(
                circumradius * a.cos() * x_scale,
                circumradius * a.sin() * z_scale,
            )
        })
        .collect()
}

/// The outward unit normal of the polygon edge `a`â†’`b` (the polygon is centred at 0).
pub(crate) fn outward_normal(a: Vec2, b: Vec2) -> Vec2 {
    let dir = (b - a).normalize_or_zero();
    let mut nrm = Vec2::new(dir.y, -dir.x);
    let mid = (a + b) * 0.5;
    if nrm.dot(mid) < 0.0 {
        nrm = -nrm;
    }
    nrm
}

/// Build a room's footprint: a seeded convex polygon (4â€“8 sides) with one doorway per
/// connection, spread across its edges. The doorway whose target is the spine objective
/// is flagged `Forward`; the rest are `Side` (closed) doors. `seed` keeps each room's
/// shape stable.
pub fn room_geom(
    room: RoomId,
    connections: &[RoomId],
    target: Option<RoomId>,
    seed: u64,
) -> PlaceGeom {
    room_geom_with_slots(room, connections, &[], target, seed)
}

pub fn room_geom_with_slots(
    room: RoomId,
    connections: &[RoomId],
    connection_slots: &[RoomConnectionSlot],
    target: Option<RoomId>,
    seed: u64,
) -> PlaceGeom {
    room_geom_with_slots_and_seals(room, connections, connection_slots, &[], target, seed)
}

pub fn room_geom_with_slots_and_seals(
    room: RoomId,
    connections: &[RoomId],
    connection_slots: &[RoomConnectionSlot],
    sealed_slots: &[ThresholdSlotId],
    target: Option<RoomId>,
    seed: u64,
) -> PlaceGeom {
    room_geom_with_slots_and_seals_for_role(
        room,
        connections,
        connection_slots,
        sealed_slots,
        target,
        None,
        seed,
    )
}

pub fn room_geom_with_slots_and_seals_for_role(
    room: RoomId,
    connections: &[RoomId],
    connection_slots: &[RoomConnectionSlot],
    sealed_slots: &[ThresholdSlotId],
    target: Option<RoomId>,
    role: Option<RoomRole>,
    seed: u64,
) -> PlaceGeom {
    room_geom_with_slots_and_seals_for_role_and_spec(
        room,
        connections,
        connection_slots,
        sealed_slots,
        target,
        role,
        seed,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn room_geom_with_slots_and_seals_for_role_and_spec(
    room: RoomId,
    connections: &[RoomId],
    connection_slots: &[RoomConnectionSlot],
    sealed_slots: &[ThresholdSlotId],
    target: Option<RoomId>,
    role: Option<RoomRole>,
    seed: u64,
    map_spec: Option<&observed_facility::map_spec::MapSpec>,
) -> PlaceGeom {
    let mut conns: Vec<RoomId> = connections.to_vec();
    conns.sort_unstable_by_key(|r| r.0);
    conns.dedup();
    let mut assigned: Vec<(RoomId, ThresholdSlotId)> = conns
        .iter()
        .enumerate()
        .map(|(fallback, &target)| {
            let slot = connection_slots
                .iter()
                .find(|candidate| candidate.target == target)
                .map(|candidate| candidate.slot)
                .unwrap_or(ThresholdSlotId(fallback as u16));
            (target, slot)
        })
        .collect();
    assigned.sort_by_key(|(_, slot)| *slot);
    assigned.dedup_by_key(|(_, slot)| *slot);
    let template = map_spec.and_then(|spec| spec.room(room).map(|room| room.template));
    let verts = room_polygon(seed, role, template);
    let n = verts.len();
    let mut assigned_slots: Vec<ThresholdSlotId> = assigned.iter().map(|(_, slot)| *slot).collect();
    assigned_slots.sort_unstable();
    assigned_slots.dedup();

    let gap_for_slot = |slot: ThresholdSlotId| {
        // Fixed room slots keep a relation on the same wall even when other
        // connections appear/disappear.
        let edge = ((slot.0 as usize % 4) * n) / 4;
        let a = verts[edge];
        let b = verts[(edge + 1) % n];
        let mid = (a + b) * 0.5;
        let len = (b - a).length();
        (
            mid,
            outward_normal(a, b),
            THRESHOLD_WIDTH.min(len - 1.0).max(1.5),
        )
    };

    let mut gaps: Vec<DoorGap> = assigned
        .into_iter()
        .map(|(t, slot)| {
            let (mid, normal, width) = gap_for_slot(slot);
            let mut corridor = corridor_id_for(room, t);
            let mut corridor_slot = corridor_socket_for(room, t, room);
            if let Some(spec) = map_spec {
                let mut found = false;
                for spec_corridor in &spec.corridors {
                    if !spec_corridor
                        .endpoints
                        .iter()
                        .any(|endpoint| endpoint.room == t)
                    {
                        continue;
                    }
                    for (slot_idx, endpoint) in spec_corridor.endpoints.iter().enumerate() {
                        if endpoint.room == room && endpoint.side.index() as u16 == slot.0 {
                            corridor = spec_corridor.id;
                            corridor_slot = ThresholdSlotId(slot_idx as u16);
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
            }
            DoorGap {
                center: mid,
                normal,
                width,
                target: t,
                kind: if Some(t) == target {
                    GapKind::Forward
                } else {
                    GapKind::Side
                },
                threshold: ThresholdLink {
                    room: RoomThreshold { room, slot },
                    hall: HallThreshold {
                        corridor,
                        slot: corridor_slot,
                    },
                    local_side: ThresholdLocalSide::Room,
                },
                floor_y: 0.0,
            }
        })
        .collect();
    for slot in sealed_slots.iter().copied() {
        if assigned_slots.contains(&slot) {
            continue;
        }
        let (center, normal, width) = gap_for_slot(slot);
        let mut corridor = corridor_id_for(room, room);
        let mut corridor_slot = slot;
        if let Some(spec) = map_spec {
            let mut found = false;
            for spec_corridor in &spec.corridors {
                for (slot_idx, endpoint) in spec_corridor.endpoints.iter().enumerate() {
                    if endpoint.room == room && endpoint.side.index() as u16 == slot.0 {
                        corridor = spec_corridor.id;
                        corridor_slot = ThresholdSlotId(slot_idx as u16);
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }
        gaps.push(DoorGap {
            center,
            normal,
            width,
            target: room,
            kind: GapKind::Collapsed,
            threshold: ThresholdLink {
                room: RoomThreshold { room, slot },
                hall: HallThreshold {
                    corridor,
                    slot: corridor_slot,
                },
                local_side: ThresholdLocalSide::Room,
            },
            floor_y: 0.0,
        });
    }
    gaps.sort_by_key(|gap| gap.threshold.room.slot);
    let half = verts.iter().fold(Vec2::ZERO, |acc, v| {
        Vec2::new(acc.x.max(v.x.abs()), acc.y.max(v.y.abs()))
    });
    PlaceGeom {
        half,
        gaps,
        interior: Vec::new(),
        poly: Some(verts),
        decks: Vec::new(),
    }
}

/// Re-open the doorway a room was *entered through* (toward `entry_from`) as an `Entry`
/// passage instead of a sealed `Side` door. The doorway you just walked through then stays
/// a real opening â€” matching the preview you crossed and letting you step back out â€” so
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

/// Re-open the stable room socket selected by a threshold transaction. The room-side
/// socket is the persistent identity; its hallway partner may have refactored or been
/// collapse-sealed after the actor committed to the corridor. In that case the arrival
/// snapshot restores the exact partner that was crossed at that same physical socket.
pub fn open_entry_threshold(
    geom: &mut PlaceGeom,
    crossed: Option<super::ThresholdLink>,
    entry_from: Option<RoomId>,
) {
    let (Some(mut crossed), Some(entry_from)) = (crossed, entry_from) else {
        return;
    };
    if geom.poly.is_none() {
        return;
    }
    crossed.local_side = ThresholdLocalSide::Room;
    if let Some(gap) = geom
        .gaps
        .iter_mut()
        .find(|gap| gap.threshold.room == crossed.room)
    {
        gap.target = entry_from;
        gap.threshold = crossed;
        gap.kind = GapKind::Entry;
    }
}

fn hallway_threshold(
    from: RoomId,
    to: RoomId,
    _side: RoomId,
    slot: ThresholdSlotId,
) -> HallThreshold {
    HallThreshold {
        corridor: corridor_id_for(from, to),
        slot,
    }
}

pub fn resolved_corridor_slot(
    from: RoomId,
    to: RoomId,
    side: RoomId,
    room_slot: ThresholdSlotId,
    map_spec: Option<&observed_facility::map_spec::MapSpec>,
) -> ThresholdSlotId {
    if let Some(spec) = map_spec {
        for corridor in &spec.corridors {
            let has_from = corridor.endpoints.iter().any(|e| e.room == from);
            let has_to = corridor.endpoints.iter().any(|e| e.room == to);
            if has_from && has_to {
                for (idx, endpoint) in corridor.endpoints.iter().enumerate() {
                    if endpoint.room == side && endpoint.side.index() as u16 == room_slot.0 {
                        return ThresholdSlotId(idx as u16);
                    }
                }
            }
        }
    }
    corridor_socket_for(from, to, side)
}

fn hallway_gap_threshold(
    from: RoomId,
    to: RoomId,
    side: RoomId,
    room_slot: ThresholdSlotId,
    hall_slot: ThresholdSlotId,
) -> ThresholdLink {
    ThresholdLink {
        room: RoomThreshold {
            room: side,
            slot: room_slot,
        },
        hall: hallway_threshold(from, to, side, hall_slot),
        local_side: ThresholdLocalSide::Hall,
    }
}

/// Clamp `pos` (XZ) to keep a body of `radius` inside a polygon room, except where it is
/// passing through an open (passage) doorway. A no-op for non-polygon places (hallways,
/// whose walls are real AABB solids). This is the room "collision" â€” applied after the
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
            let through_gap = geom.gaps.iter().any(|g| {
                g.kind.is_passage()
                    && is_point_on_segment(g.center, a, b, 0.05)
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
/// generated labyrinth (interior walls between an entry on the âˆ’Z wall and an exit on
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
    hallway_geom_for_exit(
        from,
        to,
        template,
        layout_seed,
        exit_locked,
        RoomId(EXIT_ROOM),
    )
}

pub fn hallway_geom_for_exit(
    from: RoomId,
    to: RoomId,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
    exit_room: RoomId,
) -> PlaceGeom {
    hallway_geom_with_slots(
        HallwayGeomEndpoints {
            from,
            to,
            from_room_slot: ThresholdSlotId(0),
            to_room_slot: ThresholdSlotId(0),
            exit_room,
        },
        template,
        layout_seed,
        exit_locked,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HallwayGeomEndpoints {
    pub from: RoomId,
    pub to: RoomId,
    pub from_room_slot: ThresholdSlotId,
    pub to_room_slot: ThresholdSlotId,
    pub exit_room: RoomId,
}

/// A grid-driven hallway interior's shared shape, regardless of which generator
/// (WFC or the DFS+braid maze) produced it: the footprint, its interior walls, and
/// the entry/exit door columns to place gaps at. See [`grid_interior`] for the
/// selection between the two generators.
pub(crate) struct GridInterior {
    pub(crate) footprint: Vec2,
    pub(crate) interior: Vec<WallSeg>,
    pub(crate) entry_cols: Vec<usize>,
    pub(crate) exit_cols: Vec<usize>,
}

/// Picks and runs the interior generator for a `grid`-driven hallway template: the
/// WFC labyrinth (`crate::wfc_interior`) for a [`CorridorRole::Mystery`] edge, the
/// shipping DFS+braid maze (`crate::maze`) for every other role (including when the
/// edge's role is unknown — an authored/dev map fallback with no `MapSpec`, or a
/// non-`Mystery` generated edge). If WFC generation fails to converge within its
/// retry budget, this falls back to the DFS maze for that hallway rather than ever
/// failing to emit a hallway; the fallback decision is a pure function of
/// `(cols, rows, layout_seed)`, so the same seed always makes the same choice.
///
/// Silence is deliberate on the WFC fallback path: this runs on the hot per-hallway
/// selection path (every hallway render, not just generation), so it does not log —
/// unlike `observed_facility::wfc::generate_liminal_map`'s retry exhaustion, which is
/// a rare, one-shot, already-logged map-build event.
///
/// `pub(crate)` (rather than private) only so `teleport::test` has a hook to prove
/// the fallback: real hallway grid sizes always converge under WFC (see
/// `wfc_interior`'s pinned-seed test), so exercising the DFS fallback branch needs
/// calling this directly with an out-of-catalog grid size small enough to be
/// unsolvable, which no `HallwayTemplate` ever produces.
pub(crate) fn grid_interior(
    cols: u8,
    rows: u8,
    layout_seed: u64,
    corridor_role: Option<CorridorRole>,
) -> GridInterior {
    let (cols, rows) = (cols as usize, rows as usize);
    if corridor_role == Some(CorridorRole::Mystery)
        && let Ok(wfc) =
            crate::wfc_interior::generate(cols, rows, layout_seed, MAZE_CELL, MAZE_WALL_T)
    {
        let footprint = Vec2::new(cols as f32 * MAZE_CELL * 0.5, rows as f32 * MAZE_CELL * 0.5);
        return GridInterior {
            footprint,
            interior: wfc.walls,
            entry_cols: wfc.entry_cols,
            exit_cols: wfc.exit_cols,
        };
    }
    let m = maze::Maze::generate(cols, rows, layout_seed);
    let footprint = m.footprint_half(MAZE_CELL);
    let interior = m
        .interior_walls(MAZE_CELL, MAZE_WALL_T)
        .into_iter()
        .map(|(center, half)| WallSeg { center, half })
        .collect();
    GridInterior {
        footprint,
        interior,
        entry_cols: m.entry_cols.clone(),
        exit_cols: m.exit_cols.clone(),
    }
}

pub fn hallway_geom_with_slots(
    endpoints: HallwayGeomEndpoints,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
) -> PlaceGeom {
    hallway_geom_with_slots_and_role(endpoints, template, layout_seed, exit_locked, None)
}

/// The role-aware entry point [`geom_for`] and the map-validation audit use: same as
/// [`hallway_geom_with_slots`], but threads the edge's [`CorridorRole`] (when known
/// from the active map spec) through to [`grid_interior`]'s WFC/DFS selection.
/// Every non-`grid` template ignores `corridor_role` entirely, and callers with no
/// map spec (authored/dev fallbacks) pass `None`, which keeps `grid` templates on the
/// DFS maze — byte-identical to before this selection existed.
pub fn hallway_geom_with_slots_and_role(
    endpoints: HallwayGeomEndpoints,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
    corridor_role: Option<CorridorRole>,
) -> PlaceGeom {
    hallway_geom_with_slots_and_role_and_spec(
        endpoints,
        template,
        layout_seed,
        exit_locked,
        corridor_role,
        None,
    )
}

pub(crate) fn hallway_geom_with_slots_and_role_and_spec(
    endpoints: HallwayGeomEndpoints,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
    corridor_role: Option<CorridorRole>,
    map_spec: Option<&observed_facility::map_spec::MapSpec>,
) -> PlaceGeom {
    let HallwayGeomEndpoints {
        from,
        to,
        from_room_slot,
        to_room_slot,
        exit_room,
    } = endpoints;
    let edge_exit_locked = exit_locked && to == exit_room;
    // The abstract WFC role becomes a concrete traversal piece only here, at the
    // simulation-to-geometry projection boundary. A locked objective edge and an edge
    // whose deterministic variation is already the Gantry retain their original piece.
    let template = if corridor_role == Some(CorridorRole::Vertical)
        && !edge_exit_locked
        && template.flavor != hallway::HallwayFlavor::Gantry
    {
        hallway::wellshaft_template()
    } else {
        template
    };
    // A hallway heading into the facility exit shows a solid locked door while the
    // keystone gate is shut; otherwise its onward doorway is a normal passage.
    let exit_kind = if edge_exit_locked {
        GapKind::LockedExit
    } else {
        GapKind::Exit
    };
    if let Some((cols, rows)) = template.grid {
        let GridInterior {
            footprint,
            interior,
            entry_cols,
            exit_cols,
        } = grid_interior(cols, rows, layout_seed, corridor_role);
        let corridor = MAZE_CELL - 2.0 * MAZE_WALL_T;
        let cell_center = |c: usize, r: usize| -> Vec2 {
            Vec2::new(
                -footprint.x + (c as f32 + 0.5) * MAZE_CELL,
                -footprint.y + (r as f32 + 0.5) * MAZE_CELL,
            )
        };
        let rows_usize = rows as usize;
        // The generators expose candidate boundary columns, but a graph socket owns
        // exactly one physical aperture. Reusing one ThresholdId for every candidate
        // made lookup position-dependent and left several ways to walk into the void.
        // Pick one deterministic candidate per endpoint; unused candidates remain
        // ordinary solid perimeter wall.
        let mut gaps = Vec::new();
        if let Some(&ec) = entry_cols.get(layout_seed as usize % entry_cols.len().max(1)) {
            let x = cell_center(ec, 0).x;
            let hall_slot = resolved_corridor_slot(from, to, from, from_room_slot, map_spec);
            gaps.push(DoorGap {
                center: Vec2::new(x, -footprint.y),
                normal: Vec2::new(0.0, -1.0),
                width: corridor,
                target: from,
                kind: GapKind::Entry,
                threshold: hallway_gap_threshold(from, to, from, from_room_slot, hall_slot),
                floor_y: 0.0,
            });
        }
        if let Some(&xc) =
            exit_cols.get(layout_seed.rotate_left(29) as usize % exit_cols.len().max(1))
        {
            let x = cell_center(xc, rows_usize - 1).x;
            let hall_slot = resolved_corridor_slot(from, to, to, to_room_slot, map_spec);
            gaps.push(DoorGap {
                center: Vec2::new(x, footprint.y),
                normal: Vec2::new(0.0, 1.0),
                width: corridor,
                target: to,
                kind: exit_kind,
                threshold: hallway_gap_threshold(from, to, to, to_room_slot, hall_slot),
                floor_y: 0.0,
            });
        }
        return PlaceGeom {
            half: footprint,
            gaps,
            interior,
            poly: None,
            decks: Vec::new(),
        };
    }
    // Straight/chicane/climb pieces vary their length per edge (a deterministic
    // 1.0Ã—â€“2.2Ã— of the template, never below the `MIN_HALL_LENGTH` floor), so repeated
    // connectors read as visibly different runs while always staying a real journey.
    let (base_len, w) = hallway::scaled_dims(template);
    let len = (base_len * hallway::length_scale(layout_seed)).max(hallway::MIN_HALL_LENGTH);
    if template.flavor == hallway::HallwayFlavor::Wellshaft {
        let mut decks = Vec::new();
        let rotated_half = |direction: Vec2, local: Vec2| {
            Vec2::new(
                direction.x.abs() * local.x + direction.y.abs() * local.y,
                direction.y.abs() * local.x + direction.x.abs() * local.y,
            )
        };

        // The pillar is a true hexagonal prism in presentation and a conservative
        // square AABB in the production controller. Its collision core stays inboard
        // of every tread's inner edge, so the walkable band never touches it.
        decks.push(super::DeckSeg {
            center: Vec2::ZERO,
            half: Vec2::splat(hallway::WELL_SHAFT_PILLAR_COLLISION_HALF),
            bottom_y: 0.0,
            top_y: hallway::WELL_SHAFT_HEIGHT + crate::layout::WALL_HEIGHT,
        });

        // Every level owns a regrouping landing and a radial bridge. Only level zero
        // and the top level meet graph gaps; the four middle bridge heads remain solid
        // service-bay walls and are presented as explicitly sealed leaves.
        for level in 0..hallway::WELL_SHAFT_LEVELS {
            let top_y = level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT;
            let direction = hallway::wellshaft_level_direction(level);
            let direction = Vec2::new(direction.0, direction.1);
            let landing = hallway::wellshaft_landing_center(level);
            decks.push(super::DeckSeg {
                center: Vec2::new(landing.0, landing.1),
                half: Vec2::splat(hallway::WELL_SHAFT_LANDING_HALF),
                bottom_y: top_y - hallway::WELL_SHAFT_DECK_THICKNESS,
                top_y,
            });

            let bridge_start =
                hallway::WELL_SHAFT_LANDING_RADIUS + hallway::WELL_SHAFT_LANDING_HALF * 0.65;
            let bridge_length = hallway::WELL_SHAFT_BRIDGE_END_RADIUS - bridge_start;
            let bridge_center = direction * (bridge_start + bridge_length * 0.5);
            decks.push(super::DeckSeg {
                center: bridge_center,
                half: rotated_half(
                    direction,
                    Vec2::new(bridge_length * 0.5, hallway::WELL_SHAFT_BRIDGE_WIDTH * 0.5),
                ),
                bottom_y: top_y - hallway::WELL_SHAFT_DECK_THICKNESS,
                top_y,
            });
        }

        // Treads cantilevered from the pillar: one radial slab per step, spread
        // contiguously across the 60° each flight turns and closing onto the tread
        // below. Collision uses the conservative axis-aligned AABB of each rotated
        // slab. Mid-flight treads carry an outward guard rail; the end treads stay
        // open so the flight connects to its landing and threshold bridge.
        for level in 0..hallway::WELL_SHAFT_LEVELS - 1 {
            let low_y = level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT;
            for step in 0..hallway::WELL_SHAFT_STEPS_PER_FLIGHT {
                let angle = hallway::wellshaft_tread_angle(level, step);
                let u = Vec2::new(angle.cos(), angle.sin());
                let step_top = low_y + step as f32 * hallway::WELL_SHAFT_STEP_RISE;
                decks.push(super::DeckSeg {
                    center: u * hallway::WELL_SHAFT_TREAD_MID_RADIUS,
                    half: rotated_half(
                        u,
                        Vec2::new(
                            hallway::WELL_SHAFT_TREAD_RADIAL_HALF,
                            hallway::WELL_SHAFT_TREAD_TANGENTIAL_HALF,
                        ),
                    ),
                    bottom_y: step_top - hallway::WELL_SHAFT_TREAD_CLOSURE,
                    top_y: step_top,
                });
                if hallway::wellshaft_tread_has_guard(step) {
                    let guard_center = u
                        * (hallway::WELL_SHAFT_TREAD_OUTER_RADIUS
                            + hallway::WELL_SHAFT_GUARD_THICKNESS * 0.5);
                    decks.push(super::DeckSeg {
                        center: guard_center,
                        half: rotated_half(
                            u,
                            Vec2::new(
                                hallway::WELL_SHAFT_GUARD_THICKNESS * 0.5,
                                hallway::WELL_SHAFT_TREAD_TANGENTIAL_HALF,
                            ),
                        ),
                        bottom_y: step_top,
                        top_y: step_top + hallway::WELL_SHAFT_GUARD_HEIGHT,
                    });
                }
            }
        }

        let bottom_direction = hallway::wellshaft_level_direction(0);
        let top_direction = hallway::wellshaft_level_direction(hallway::WELL_SHAFT_LEVELS - 1);
        let bottom_normal = Vec2::new(bottom_direction.0, bottom_direction.1);
        let top_normal = Vec2::new(top_direction.0, top_direction.1);
        let bottom_center = bottom_normal * hallway::WELL_SHAFT_OUTER_APOTHEM;
        let top_center = top_normal * hallway::WELL_SHAFT_OUTER_APOTHEM;
        let poly = (0..6)
            .map(|index| {
                let angle = -PI / 6.0 + index as f32 * PI / 3.0;
                Vec2::new(angle.cos(), angle.sin()) * hallway::WELL_SHAFT_OUTER_RADIUS
            })
            .collect();

        let mut gaps = vec![
            DoorGap {
                center: top_center,
                normal: top_normal,
                width: 3.0,
                target: from,
                kind: GapKind::Entry,
                threshold: hallway_gap_threshold(
                    from,
                    to,
                    from,
                    from_room_slot,
                    resolved_corridor_slot(from, to, from, from_room_slot, map_spec),
                ),
                floor_y: hallway::WELL_SHAFT_HEIGHT,
            },
            DoorGap {
                center: bottom_center,
                normal: bottom_normal,
                width: 3.0,
                target: to,
                kind: exit_kind,
                threshold: hallway_gap_threshold(
                    from,
                    to,
                    to,
                    to_room_slot,
                    resolved_corridor_slot(from, to, to, to_room_slot, map_spec),
                ),
                floor_y: 0.0,
            },
        ];

        if let Some(spec) = map_spec
            && let Some(corridor) = spec.corridors().into_iter().find(|c| {
                c.endpoints.iter().any(|e| e.room == from)
                    && c.endpoints.iter().any(|e| e.room == to)
            })
        {
            for idx in 2..corridor.endpoints.len() {
                let endpoint = corridor.endpoints[idx];
                let level = idx - 1;
                if level < hallway::WELL_SHAFT_LEVELS {
                    let top_y = level as f32 * hallway::WELL_SHAFT_LEVEL_HEIGHT;
                    let direction = hallway::wellshaft_level_direction(level);
                    let normal = Vec2::new(direction.0, direction.1);
                    let center = normal * hallway::WELL_SHAFT_OUTER_APOTHEM;
                    let slot = super::ThresholdSlotId(endpoint.side.index() as u16);
                    let cslot = super::ThresholdSlotId(idx as u16);
                    gaps.push(DoorGap {
                        center,
                        normal,
                        width: 3.0,
                        target: endpoint.room,
                        kind: GapKind::Exit,
                        threshold: hallway_gap_threshold(from, to, endpoint.room, slot, cslot),
                        floor_y: top_y,
                    });
                }
            }
        }

        return PlaceGeom {
            half: Vec2::new(
                hallway::WELL_SHAFT_OUTER_APOTHEM,
                hallway::WELL_SHAFT_OUTER_RADIUS,
            ),
            gaps,
            interior: Vec::new(),
            poly: Some(poly),
            decks,
        };
    }
    if template.flavor == hallway::HallwayFlavor::Chicane {
        // An S-bend: a box with two staggered baffles, each sealing one side and leaving
        // a corridor `c` on the other, so the path slaloms from the +X entry up through
        // the low baffle's gap, across the open middle band, and out the high baffle's
        // âˆ’X gap to the exit. The baffles live in `interior`, so they render + collide
        // through the same path the labyrinths use.
        let hx = w * 0.5;
        let hz = (len * 0.5).max(w);
        let c = (w * 0.42).max(2.4); // walkable corridor (â‰« the 0.4 body radius)
        let baffle_half_x = hx - c * 0.5;
        let interior = vec![
            // Low baffle: seals the âˆ’X side, opening a gap on +X.
            WallSeg {
                center: Vec2::new(-c * 0.5, -hz * 0.33),
                half: Vec2::new(baffle_half_x, MAZE_WALL_T),
            },
            // High baffle: seals the +X side, opening a gap on âˆ’X.
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
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        from,
                        from_room_slot,
                        resolved_corridor_slot(from, to, from, from_room_slot, map_spec),
                    ),
                    floor_y: 0.0,
                },
                DoorGap {
                    center: Vec2::new(-off, hz),
                    normal: Vec2::new(0.0, 1.0),
                    width: c,
                    target: to,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        to,
                        to_room_slot,
                        resolved_corridor_slot(from, to, to, to_room_slot, map_spec),
                    ),
                    floor_y: 0.0,
                },
            ],
            interior,
            poly: None,
            decks: Vec::new(),
        };
    }
    if template.flavor == hallway::HallwayFlavor::Gantry {
        // The real gantry projection: two levels, five gaps across the pure jump-map
        // model's four thresholds (the entry threshold now yields both the deck-level
        // arrival and a ground-level return), matching `observed_traversal::gantry` bolt
        // for bolt (no re-authored numbers). The hall's local frame lines up with the
        // course's own frame (entry at -Z, exit at +Z), so every dimension below is read
        // straight off the course.
        let course = gantry::GantryCourse::authored();
        let hx = gantry::GANTRY_WIDTH * 0.5;
        let hz = gantry::GANTRY_LENGTH * 0.5;

        // Decks: the six jump-map platforms, the upper landing, and the entry landing.
        // Thresholds teleport the body directly (user design ruling: no stairs), so the
        // entry threshold delivers the body straight onto the deck over `entry_landing`
        // (mirroring `upper_landing` on the exit end) instead of climbing a mount stair.
        let mut decks: Vec<super::DeckSeg> = course
            .platforms
            .iter()
            .map(|platform| super::DeckSeg {
                center: platform.center,
                half: platform.half,
                bottom_y: platform.bottom_y,
                top_y: platform.top_y,
            })
            .collect();
        decks.push(super::DeckSeg {
            center: course.upper_landing.center,
            half: course.upper_landing.half,
            bottom_y: course.upper_landing.bottom_y,
            top_y: course.upper_landing.top_y,
        });
        decks.push(super::DeckSeg {
            center: course.entry_landing.center,
            half: course.entry_landing.half,
            bottom_y: course.entry_landing.bottom_y,
            top_y: course.entry_landing.top_y,
        });

        // Gaps: deck entry (delivers the body onto `entry_landing` at UPPER_DECK_Y) + a
        // ground-level return (the understory walk-back, so a fallen body can still leave
        // the way it came) + upper exit (deck-only, floor_y = UPPER_DECK_Y) + the
        // safe-bypass exit (ground, slow lane) + the understory side exit (ground,
        // fall-recovery shortcut back to `from`). The ground return and the side exit both
        // target `from` like the entry, but are typed `Exit` rather than a second `Entry`.
        // `Entry` identifies the exact reciprocal socket used to enter this place; the
        // other return routes are independent passage transactions aimed at `from`, so
        // `Exit` keeps that semantic distinction without crossing-system branches.
        // The ground return sits in the bypass lane (x = SAFE_BYPASS_X) rather than under
        // the deck entry: `place_arena`'s wall-cutting solidifies/apertures a `-Z` wall
        // span per gap by `(center.x, width)`, so a ground gap sharing the deck entry's XZ
        // span would fight it for the same wall segment (two different floor_y apertures
        // can't coexist on one span). The bypass lane is already proven clear of every
        // deck the full length of the hall, so the ground return opens there instead.
        let entry_threshold = course.threshold(gantry::GantryExit::UnderstoryReturn);
        let upper_threshold = course.threshold(gantry::GantryExit::UpperExit);
        let bypass_threshold = course.threshold(gantry::GantryExit::SafeBypassExit);
        let side_threshold = course.threshold(gantry::GantryExit::UnderstorySideExit);
        let find_corridor = |spec: &observed_facility::map_spec::MapSpec| {
            spec.corridors().into_iter().find(|c| {
                c.endpoints.iter().any(|e| e.room == from)
                    && c.endpoints.iter().any(|e| e.room == to)
            })
        };

        let (ep0_room, ep0_slot, ep0_cslot) = if let Some(spec) = map_spec
            && let Some(corridor) = find_corridor(spec)
            && !corridor.endpoints.is_empty()
        {
            (
                corridor.endpoints[0].room,
                super::ThresholdSlotId(corridor.endpoints[0].side.index() as u16),
                super::ThresholdSlotId(0),
            )
        } else {
            (from, from_room_slot, super::ThresholdSlotId(0))
        };

        let (ep1_room, ep1_slot, ep1_cslot) = if let Some(spec) = map_spec
            && let Some(corridor) = find_corridor(spec)
            && corridor.endpoints.len() > 1
        {
            (
                corridor.endpoints[1].room,
                super::ThresholdSlotId(corridor.endpoints[1].side.index() as u16),
                super::ThresholdSlotId(1),
            )
        } else {
            (to, to_room_slot, super::ThresholdSlotId(1))
        };

        let (ep2_room, ep2_slot, ep2_cslot) = if let Some(spec) = map_spec
            && let Some(corridor) = find_corridor(spec)
            && corridor.endpoints.len() > 2
        {
            (
                corridor.endpoints[2].room,
                super::ThresholdSlotId(corridor.endpoints[2].side.index() as u16),
                super::ThresholdSlotId(2),
            )
        } else {
            (from, from_room_slot, super::ThresholdSlotId(2))
        };

        let ground_return_center = Vec2::new(gantry::SAFE_BYPASS_X, entry_threshold.center.y);
        return PlaceGeom {
            half: Vec2::new(hx, hz),
            gaps: vec![
                DoorGap {
                    center: entry_threshold.center,
                    normal: entry_threshold.normal,
                    width: entry_threshold.width,
                    target: ep0_room,
                    kind: GapKind::Entry,
                    threshold: hallway_gap_threshold(from, to, ep0_room, ep0_slot, ep0_cslot),
                    floor_y: gantry::UPPER_DECK_Y,
                },
                DoorGap {
                    center: ground_return_center,
                    normal: entry_threshold.normal,
                    width: entry_threshold.width,
                    target: ep0_room,
                    kind: GapKind::Exit,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        ep0_room,
                        ep0_slot,
                        ThresholdSlotId(4),
                    ),
                    floor_y: entry_threshold.floor_y,
                },
                DoorGap {
                    center: upper_threshold.center,
                    normal: upper_threshold.normal,
                    width: upper_threshold.width,
                    target: ep1_room,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(from, to, ep1_room, ep1_slot, ep1_cslot),
                    floor_y: upper_threshold.floor_y,
                },
                DoorGap {
                    center: bypass_threshold.center,
                    normal: bypass_threshold.normal,
                    width: bypass_threshold.width,
                    target: ep1_room,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        ep1_room,
                        ep1_slot,
                        ThresholdSlotId(3),
                    ),
                    floor_y: bypass_threshold.floor_y,
                },
                DoorGap {
                    center: side_threshold.center,
                    normal: side_threshold.normal,
                    width: side_threshold.width,
                    target: ep2_room,
                    kind: GapKind::Exit,
                    threshold: hallway_gap_threshold(from, to, ep2_room, ep2_slot, ep2_cslot),
                    floor_y: side_threshold.floor_y,
                },
            ],
            interior: Vec::new(),
            poly: None,
            decks,
        };
    }
    if template.flavor == hallway::HallwayFlavor::Colonnade {
        // A wide, long pseudo-room: a regular grid of square pillars straddling the centre
        // axes (so a clear lane always runs straight down the middle, entryâ†’exit, and a
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
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        from,
                        from_room_slot,
                        resolved_corridor_slot(from, to, from, from_room_slot, map_spec),
                    ),
                    floor_y: 0.0,
                },
                DoorGap {
                    center: Vec2::new(0.0, hz),
                    normal: Vec2::new(0.0, 1.0),
                    width: lane,
                    target: to,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        to,
                        to_room_slot,
                        resolved_corridor_slot(from, to, to, to_room_slot, map_spec),
                    ),
                    floor_y: 0.0,
                },
            ],
            interior,
            poly: None,
            decks: Vec::new(),
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
                threshold: hallway_gap_threshold(
                    from,
                    to,
                    from,
                    from_room_slot,
                    resolved_corridor_slot(from, to, from, from_room_slot, map_spec),
                ),
                floor_y: 0.0,
            },
            DoorGap {
                center: Vec2::new(0.0, half.y),
                normal: Vec2::new(0.0, 1.0),
                width: door,
                target: to,
                kind: exit_kind,
                threshold: hallway_gap_threshold(
                    from,
                    to,
                    to,
                    to_room_slot,
                    resolved_corridor_slot(from, to, to, to_room_slot, map_spec),
                ),
                floor_y: 0.0,
            },
        ],
        interior: Vec::new(),
        poly: None,
        decks: Vec::new(),
    }
}

/// Half-size of a colonnade's square structural pillars (world units).
const PILLAR_HALF: f32 = 0.5;
/// Centre-to-centre spacing of colonnade pillars; the clear lane between two columns is
/// `PILLAR_SPACING - 2*PILLAR_HALF`, kept well above the body radius.
const PILLAR_SPACING: f32 = 4.4;
/// Keep pillars this far inside the perimeter so a lane runs around the edges too.
const PILLAR_MARGIN: f32 = 2.6;

/// Pillar-centre offsets along one axis within `Â±limit`, straddling 0 at half-spacing
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
/// current-room one) â€” so a doorway can preview a different room's shape. Seeded exactly
/// like [`geom_for`]'s room branch, so the preview matches the room you'll arrive in.
pub fn room_preview_geom(
    room: RoomId,
    connections: &[RoomId],
    connection_slots: &[RoomConnectionSlot],
    sealed_slots: &[ThresholdSlotId],
    target: Option<RoomId>,
    role: Option<RoomRole>,
    base_seed: u64,
) -> PlaceGeom {
    room_geom_with_slots_and_seals_for_role(
        room,
        connections,
        connection_slots,
        sealed_slots,
        target,
        role,
        mix(base_seed, room.0 as u64),
    )
}

/// The footprint geometry for any place, given the current navigation snapshot.
pub fn geom_for(place: Place, nav: &Nav) -> PlaceGeom {
    let mut geom = match place {
        // The room shape is seeded by the room id + facility seed (not the decohere
        // version), so a room keeps a stable shape while its connections rewire.
        Place::Room(room) => room_geom_with_slots_and_seals_for_role_and_spec(
            room,
            &nav.connections,
            &nav.connection_slots,
            &nav.sealed_slots,
            nav.target_room,
            nav.room_role,
            mix(nav.seed, room.0 as u64),
            nav.map_spec.as_ref(),
        ),
        Place::Hallway {
            from,
            to,
            variation,
            ..
        } => hallway_geom_with_slots_and_role_and_spec(
            HallwayGeomEndpoints {
                from,
                to,
                from_room_slot: nav
                    .hallway_entry_room_slot
                    .or_else(|| nav.slot_for(to))
                    .unwrap_or(ThresholdSlotId(0)),
                to_room_slot: nav.hallway_exit_room_slot.unwrap_or(ThresholdSlotId(0)),
                exit_room: nav.exit_room,
            },
            hallway::template(variation),
            hallway::layout_seed(from, to, variation),
            nav.exit_locked,
            nav.corridor_role_for(to),
            nav.map_spec.as_ref(),
        ),
    };
    // Hallway generators are directional shape functions and historically stamped a
    // pair-derived corridor id into their gaps. The `Place` already carries the socket
    // topology's authoritative corridor identity (including authored/multi-endpoint map
    // ids), so every hallway aperture must use that identity before any reciprocal check.
    if let Place::Hallway { corridor, .. } = place {
        for gap in &mut geom.gaps {
            gap.threshold.hall.corridor = corridor;
        }
    }
    enforce_active_sockets(&mut geom, place, nav);
    geom
}

/// Make the rendered/physical passage set agree with the active junction topology: a
/// room-side socket that is **not** attached in [`super::place_junction`] (sealed, or a
/// connection whose socket collides with a sealed side) has no partner, so its doorway is
/// demoted from a crossable passage to a solid closed door. Because
/// `place_structural_primitives` and `place_rapier_scene` both derive collision from
/// `PlaceGeom.gaps`, render + Rapier apertures then follow the socket set for free — the
/// invariant Phase 74 exists to guarantee ("a socket crossable in the topology never
/// renders/collides as a solid wall, and vice-versa").
///
/// This is a no-op on real navigation data, where connection slots and sealed slots are
/// disjoint and every connection is attached: the demotion only fires for a socket the
/// topology genuinely refuses. A hallway's two endpoint sockets are always attached while
/// it is the current place, so only rooms are adjusted.
fn enforce_active_sockets(geom: &mut PlaceGeom, place: Place, nav: &Nav) {
    let Place::Room(_) = place else {
        return;
    };
    let topology = crate::teleport::place_junction(place, nav);
    if topology.threshold_count() == 0 {
        // Degenerate/fallback nav (no resolvable sockets): leave the authored gaps intact
        // so single-exit authored/dev maps are byte-identical.
        return;
    }
    for gap in geom.gaps.iter_mut() {
        if !gap.kind.is_passage() {
            continue;
        }
        let room = gap.threshold.room.room;
        let socket = observed_core::ThresholdId::new(
            observed_core::PlaceId::Room(room),
            gap.threshold.room.slot.0,
        );
        if topology.partner(socket).is_none() {
            gap.kind = GapKind::Side;
        }
    }
}
