//! Geometry models and footprint generators for rooms and hallways.

use super::{
    DoorGap, GapKind, HallId, HallThreshold, MAZE_CELL, MAZE_WALL_T, Nav, Place, PlaceGeom,
    ROOM_HALF, RoomConnectionSlot, RoomThreshold, THRESHOLD_WIDTH, ThresholdLink,
    ThresholdLocalSide, ThresholdSlotId, WallSeg,
};
use crate::hallway;
use crate::layout::{ROOM_SCALE_HUB, ROOM_SCALE_MONITOR, ROOM_SCALE_STANDARD};
use crate::maze;
use bevy::math::Vec2;
use observed_core::RoomId;
use observed_facility::map_spec::RoomRole;
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
fn room_polygon(seed: u64, role: Option<RoomRole>) -> Vec<Vec2> {
    let observation_room = uses_observation_room_footprint(role);
    let scale = room_scale_for_role(role);
    let n = if observation_room {
        OBSERVATION_ROOM_SIDES
    } else {
        4 + (mix(seed, 1) % 5) as usize
    };
    if n == 4 {
        // A varied rectangle for visual distinction from the polygons.
        let hx = ROOM_HALF * scale * (0.85 + unit(seed, 2) * 0.7);
        let hz = ROOM_HALF * scale * (0.85 + unit(seed, 3) * 0.7);
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
            Vec2::new(circumradius * a.cos(), circumradius * a.sin())
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
                .unwrap_or(ThresholdSlotId(fallback as u8));
            (target, slot)
        })
        .collect();
    assigned.sort_by_key(|(_, slot)| *slot);
    assigned.dedup_by_key(|(_, slot)| *slot);
    let verts = room_polygon(seed, role);
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
                        hall: HallId::new(room, t),
                        side: room,
                        slot: ThresholdSlotId(0),
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
        gaps.push(DoorGap {
            center,
            normal,
            width,
            target: room,
            kind: GapKind::Collapsed,
            threshold: ThresholdLink {
                room: RoomThreshold { room, slot },
                hall: HallThreshold {
                    hall: HallId::new(room, room),
                    side: room,
                    slot,
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

fn hallway_threshold(
    from: RoomId,
    to: RoomId,
    side: RoomId,
    slot: ThresholdSlotId,
) -> HallThreshold {
    HallThreshold {
        hall: HallId::new(from, to),
        side,
        slot,
    }
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

pub fn hallway_geom_with_slots(
    endpoints: HallwayGeomEndpoints,
    template: &hallway::HallwayTemplate,
    layout_seed: u64,
    exit_locked: bool,
) -> PlaceGeom {
    let HallwayGeomEndpoints {
        from,
        to,
        from_room_slot,
        to_room_slot,
        exit_room,
    } = endpoints;
    // A hallway heading into the facility exit shows a solid locked door while the
    // keystone gate is shut; otherwise its onward doorway is a normal passage.
    let exit_kind = if exit_locked && to == exit_room {
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
        // Multiple entrances (âˆ’Z, back to `from`) and exits (+Z, on to `to`); each at a
        // door column, all reachable from one another through the maze.
        let mut gaps = Vec::new();
        for (slot, &ec) in m.entry_cols.iter().enumerate() {
            let x = m.cell_center(ec, 0, MAZE_CELL).x;
            let hall_slot = ThresholdSlotId(slot as u8);
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
        for (slot, &xc) in m.exit_cols.iter().enumerate() {
            let x = m.cell_center(xc, m.rows - 1, MAZE_CELL).x;
            let hall_slot = ThresholdSlotId(slot as u8);
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
                        ThresholdSlotId(0),
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
                        ThresholdSlotId(0),
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
        // target `from` like the entry, but are typed `Exit` rather than a second `Entry`:
        // `teleport_sim`'s `!crossed_exit` branch in `crossing.rs` treats every
        // `Entry`-kind gap as "walked back out before ever reaching the hallway's exit"
        // and, once one is crossed, never re-checks the other passages that tick — a
        // second `Entry` gap sitting on a different wall would silently race the real
        // entry doorway for that one match. `Exit` has no such special-cased
        // single-purpose branch (the exit-side code keys off
        // `tp.crossed_exit`/`pending_exit`, not "which specific gap"), so a
        // `from`-targeting `Exit` is a normal crossable passage (`is_passage` covers
        // `Exit`) that reaches `cross_into_room(.., from, to, ..)` through the same path
        // as the safe-bypass and upper exits, just aimed back at `from` instead of `to`.
        // (`open_entry`, the other `Entry`-specific consumer, is a no-op for hallways —
        // it only reopens a room's arrival doorway — so it is not a factor either way.)
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
        let ground_return_center = Vec2::new(gantry::SAFE_BYPASS_X, entry_threshold.center.y);
        return PlaceGeom {
            half: Vec2::new(hx, hz),
            gaps: vec![
                DoorGap {
                    center: entry_threshold.center,
                    normal: entry_threshold.normal,
                    width: entry_threshold.width,
                    target: from,
                    kind: GapKind::Entry,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        from,
                        from_room_slot,
                        ThresholdSlotId(0),
                    ),
                    floor_y: gantry::UPPER_DECK_Y,
                },
                DoorGap {
                    center: ground_return_center,
                    normal: entry_threshold.normal,
                    width: entry_threshold.width,
                    target: from,
                    kind: GapKind::Exit,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        from,
                        from_room_slot,
                        ThresholdSlotId(2),
                    ),
                    floor_y: entry_threshold.floor_y,
                },
                DoorGap {
                    center: upper_threshold.center,
                    normal: upper_threshold.normal,
                    width: upper_threshold.width,
                    target: to,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        to,
                        to_room_slot,
                        ThresholdSlotId(0),
                    ),
                    floor_y: upper_threshold.floor_y,
                },
                DoorGap {
                    center: bypass_threshold.center,
                    normal: bypass_threshold.normal,
                    width: bypass_threshold.width,
                    target: to,
                    kind: exit_kind,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        to,
                        to_room_slot,
                        ThresholdSlotId(1),
                    ),
                    floor_y: bypass_threshold.floor_y,
                },
                DoorGap {
                    center: side_threshold.center,
                    normal: side_threshold.normal,
                    width: side_threshold.width,
                    target: from,
                    kind: GapKind::Exit,
                    threshold: hallway_gap_threshold(
                        from,
                        to,
                        from,
                        from_room_slot,
                        ThresholdSlotId(1),
                    ),
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
                        ThresholdSlotId(0),
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
                        ThresholdSlotId(0),
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
                    ThresholdSlotId(0),
                ),
                floor_y: 0.0,
            },
            DoorGap {
                center: Vec2::new(0.0, half.y),
                normal: Vec2::new(0.0, 1.0),
                width: door,
                target: to,
                kind: exit_kind,
                threshold: hallway_gap_threshold(from, to, to, to_room_slot, ThresholdSlotId(0)),
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
    match place {
        // The room shape is seeded by the room id + facility seed (not the decohere
        // version), so a room keeps a stable shape while its connections rewire.
        Place::Room(room) => room_geom_with_slots_and_seals_for_role(
            room,
            &nav.connections,
            &nav.connection_slots,
            &nav.sealed_slots,
            nav.target_room,
            nav.room_role,
            mix(nav.seed, room.0 as u64),
        ),
        Place::Hallway {
            from,
            to,
            variation,
        } => hallway_geom_with_slots(
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
        ),
    }
}
