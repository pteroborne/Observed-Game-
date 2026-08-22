//! Pre-collapse constraints: blueprint stamping and the forced spawn→exit
//! route.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, PortSignature, lateral_distance};

use observed_content::ArchitectureRegister;

use crate::map_spec::RoomRole;

use super::blueprint::{self, StampedBlueprint};
use super::relayout::{DistrictSite, district_of};
use super::{HexRoomQuotas, HexWfcConfig, lateral_bit};

/// Every coordinate of the grid in index order.
pub(super) fn all_coords(config: HexWfcConfig) -> impl Iterator<Item = HexCoord> {
    let grid = config.grid();
    (0..grid.cell_count()).map(move |index| grid.coord(index))
}

/// Absolute footprint cells for a blueprint anchored at `anchor`, or `None`
/// when any cell falls outside the grid.
fn absolute_cells(
    anchor: HexCoord,
    cells: &[blueprint::CellOffset],
    grid: HexGridSize,
) -> Option<Vec<HexCoord>> {
    let mut absolute = Vec::with_capacity(cells.len());
    for &(dq, dr, dl) in cells {
        let q = i32::from(anchor.q) + dq;
        let r = i32::from(anchor.r) + dr;
        let level = i32::from(anchor.level) + dl;
        if q < 0 || r < 0 || level < 0 {
            return None;
        }
        let coord = HexCoord {
            q: q as u16,
            r: r as u16,
            level: level as u8,
        };
        if !grid.contains(coord) {
            return None;
        }
        absolute.push(coord);
    }
    Some(absolute)
}

/// The boundary-adjusted signature of every stamped blueprint cell: open
/// sibling seams plus named exterior thresholds, with any face that leaves the
/// grid sealed.
pub(super) fn stamped_signatures(
    config: HexWfcConfig,
    blueprints: &[StampedBlueprint],
) -> BTreeMap<HexCoord, PortSignature> {
    let grid = config.grid();
    let mut signatures = BTreeMap::new();
    for stamped in blueprints {
        let bp = blueprint::blueprint_for_role(stamped.role);
        for (&cell, &offset) in stamped.cells.iter().zip(bp.cells.iter()) {
            let declared = bp.cell_signature(offset);
            let mut ports = [PortClass::Sealed; 8];
            for face in HexFace::ALL {
                if grid.neighbor(cell, face).is_some() {
                    ports[face.index()] = declared.port(face);
                }
            }
            let adjusted = PortSignature::try_from_ports(ports)
                .expect("sealing faces keeps a signature valid");
            signatures.insert(cell, adjusted);
        }
    }
    signatures
}

/// Whether an anchor is a legal stamp: footprint inside the grid, no overlap
/// with or lateral adjacency to occupied cells, every anchor at least
/// `min_room_distance` (lateral hex distance) from every other anchor, and —
/// for freely placed rooms — every named port facing an in-grid neighbor.
fn stamp_at(
    role: RoomRole,
    anchor: HexCoord,
    require_ports_in_grid: bool,
    config: HexWfcConfig,
    occupied: &BTreeSet<HexCoord>,
    anchors: &[HexCoord],
) -> Option<Vec<HexCoord>> {
    let grid = config.grid();
    let bp = blueprint::blueprint_for_role(role);
    let cells = absolute_cells(anchor, &bp.cells, grid)?;
    let blocked = cells.iter().any(|&cell| {
        occupied.contains(&cell)
            || HexFace::LATERAL.iter().any(|&face| {
                grid.neighbor(cell, face)
                    .is_some_and(|neighbor| occupied.contains(&neighbor))
            })
    });
    if blocked {
        return None;
    }
    if anchors
        .iter()
        .any(|&other| lateral_distance(anchor, other) < config.min_room_distance)
    {
        return None;
    }
    if require_ports_in_grid {
        let ports_ok = bp.named_ports.iter().all(|&(_, offset, face)| {
            let index = bp.cells.iter().position(|&cell| cell == offset);
            index.is_some_and(|index| grid.neighbor(cells[index], face).is_some())
        });
        if !ports_ok {
            return None;
        }
    }
    Some(cells)
}

/// Deterministically stamp blueprints against `RoomRole` quotas: `Start` and
/// `Exit` at spawn/exit, then pool roles over a greedy blue-noise sweep of an
/// rng-salted coordinate order. Footprints never overlap or touch laterally,
/// and anchors keep `min_room_distance` structurally.
/// Stamp around room blueprints frozen by observation. Locked stamps retain
/// their exact role, anchor, footprint, and stamp ID; new stamps receive IDs
/// above the previous maximum so identity is never inferred from vector order.
/// Which districts a room role belongs in.
///
/// This is the legibility payoff the arc is for: a player who recognises a
/// district should know what it holds, so heading somewhere is a decision rather
/// than a wander. An empty list means the role goes anywhere.
///
/// The binding is a *preference*, not a constraint, and deliberately so. A seed
/// can put a role's districts in an awkward corner, fill them with other rooms,
/// or — since districts are one anchor per register per level — leave one barely
/// represented at the levels a room could fit. Refusing to stamp in that case
/// would cost the facility a room and could fail the room-count contract
/// outright, which is a much worse outcome than a Monitor turning up somewhere
/// unexpected. [`stamp_blueprints_with_pins`] tries the preferred districts
/// across every candidate coordinate first, and only then falls back.
#[must_use]
fn role_districts(role: RoomRole) -> &'static [ArchitectureRegister] {
    use ArchitectureRegister as R;
    match role {
        // Fixed at the spawn and exit coordinates; the district is whatever is
        // there, and moving them would break the forced route.
        RoomRole::Start | RoomRole::Exit => &[],
        // A hub for reading doors and previews wants to be somewhere open, where
        // the choice is visible before it is taken.
        RoomRole::Decision => &[R::LiminalGrid, R::InfiniteGallery],
        // An unstable junction belongs where the architecture is already
        // uncertain.
        RoomRole::DecoherenceFork => &[R::ShadowScreen, R::Thinning],
        // Freezing thresholds is a structural act; put it against structure.
        RoomRole::AnchorCheckpoint => &[R::Monolith, R::FacetMonument],
        // A side objective should be somewhere you can describe to a teammate.
        RoomRole::Keystone => &[R::FacetMonument, R::Wellshaft],
        // Two operators, industrial scale.
        RoomRole::DualStation => &[R::Megastructure, R::Institutional],
        // Redirecting guardian pressure is administration.
        RoomRole::GuardianControl => &[R::Institutional, R::ShadowScreen],
        // An information room wants light and sightlines.
        RoomRole::Monitor => &[R::OverlitGrid, R::InfiniteGallery],
        // Somewhere to stop. The thinnest, quietest district.
        RoomRole::Recovery => &[R::Thinning, R::LiminalGrid],
        // Not stamped in hex matches at all — see the pool below.
        RoomRole::TeleportRelay => &[],
    }
}

/// Exposed for the district-binding test in `super::tests`.
#[cfg(test)]
#[must_use]
pub(super) fn role_districts_for_probe(role: RoomRole) -> &'static [ArchitectureRegister] {
    role_districts(role)
}

pub(super) fn stamp_blueprints_with_pins(
    config: HexWfcConfig,
    rng: &mut SplitMix,
    locked: &[StampedBlueprint],
    districts: &[DistrictSite],
    room_quotas: Option<HexRoomQuotas>,
) -> Vec<StampedBlueprint> {
    // Inclusive, so `max_rooms` is actually reachable. It was
    // `max_rooms - min_rooms`, which for the production 9..=10 contract is 1, so
    // `rng % span` was always 0 and the target was always 9 — the upper bound
    // had never once been hit. That also meant the last role in the pool below
    // could never be reached, which is half of why `DecoherenceFork` had never
    // appeared in a match.
    let span = (config.max_rooms + 1 - config.min_rooms).max(1) as u64;
    let target = room_quotas.map_or_else(
        || (config.min_rooms + (rng.next_u64() % span) as usize).max(locked.len()),
        HexRoomQuotas::total_with_start_and_exit,
    );

    let mut pool = vec![
        RoomRole::Decision,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::Monitor,
        RoomRole::AnchorCheckpoint,
        RoomRole::Recovery,
        // Four hexes, and the largest authored room in the corpus. It had a
        // blueprint and a `room_decoherence_fork.map` and was absent from this
        // pool, so it had never appeared in a match — bug backlog #16. It goes
        // last because it is the hardest to fit, and a role that cannot be
        // placed simply yields the slot to the next one.
        RoomRole::DecoherenceFork,
    ];
    if config.levels >= 2 {
        // The 2-level atrium leads the pool so tall grids always try one.
        pool.insert(0, RoomRole::GuardianControl);
    }
    // `TeleportRelay` is deliberately absent. Its blueprint exists and the
    // deprecated `full_wfc` path requires a pair of them, but the hex match has
    // no teleport-pad mechanic at all — `sync_teleports_to_bodies` reconciles
    // spawn, setback and escape moves, nothing a player can use. Stamping one
    // would spend a room slot on a room that does nothing, which is worse than
    // leaving it out. The blueprint stays because `full_wfc` still names it.

    let salt = rng.next_u64();
    let mut coords: Vec<HexCoord> = all_coords(config).collect();
    coords.sort_by_key(|&coord| SplitMix::new(coord_key(coord) ^ salt).next_u64());

    let mut stamped: Vec<StampedBlueprint> = locked.to_vec();
    let mut occupied: BTreeSet<HexCoord> = locked
        .iter()
        .flat_map(|blueprint| blueprint.cells.iter().copied())
        .collect();
    let mut anchors: Vec<HexCoord> = locked.iter().map(|blueprint| blueprint.anchor).collect();
    let mut next_id = locked
        .iter()
        .map(|blueprint| blueprint.id)
        .max()
        .map_or(0, |id| id.wrapping_add(1));

    for (role, anchor) in [
        (RoomRole::Start, config.spawn()),
        (RoomRole::Exit, config.exit()),
    ] {
        if stamped.iter().any(|blueprint| blueprint.role == role) {
            continue;
        }
        if let Some(cells) = stamp_at(role, anchor, false, config, &occupied, &anchors) {
            occupied.extend(cells.iter().copied());
            anchors.push(anchor);
            stamped.push(StampedBlueprint {
                id: next_id,
                role,
                anchor,
                cells,
            });
            next_id = next_id.wrapping_add(1);
        }
    }

    let role_targets: Vec<(RoomRole, usize)> = if let Some(quotas) = room_quotas {
        vec![
            (RoomRole::GuardianControl, quotas.guardian_control),
            (RoomRole::DecoherenceFork, quotas.decoherence_fork),
            (RoomRole::Decision, quotas.decision),
            (RoomRole::DualStation, quotas.dual_station),
            (RoomRole::AnchorCheckpoint, quotas.anchor_checkpoint),
            (RoomRole::Keystone, quotas.keystone),
            (RoomRole::Monitor, quotas.monitor),
            (RoomRole::Recovery, quotas.recovery),
        ]
    } else {
        pool.into_iter().map(|role| (role, 1)).collect()
    };
    for (role, role_target) in role_targets {
        if role == RoomRole::GuardianControl && config.levels < 2 {
            continue;
        }
        while stamped.len() < target
            && stamped
                .iter()
                .filter(|blueprint| blueprint.role == role)
                .count()
                < role_target
        {
            // Preferred districts across every candidate first, then anywhere. Two
            // passes rather than a sort, so a role that cannot be placed in its own
            // districts still gets the full field rather than a nearly-empty one.
            let wanted = role_districts(role);
            let placed = [true, false].into_iter().find_map(|preferred| {
                if preferred && wanted.is_empty() {
                    return None;
                }
                coords.iter().copied().find_map(|anchor| {
                    if preferred
                        && !district_of(anchor, districts)
                            .is_some_and(|register| wanted.contains(&register))
                    {
                        return None;
                    }
                    stamp_at(role, anchor, true, config, &occupied, &anchors)
                        .map(|cells| (anchor, cells))
                })
            });
            if let Some((anchor, cells)) = placed {
                occupied.extend(cells.iter().copied());
                anchors.push(anchor);
                stamped.push(StampedBlueprint {
                    id: next_id,
                    role,
                    anchor,
                    cells,
                });
                next_id = next_id.wrapping_add(1);
            } else {
                break;
            }
        }
    }

    stamped
}

/// A corridor skeleton: one narrow path per edge of a spanning tree over the
/// stamped rooms, returned as an *exact* door mask per cell.
///
/// The point of routing rather than weighting. `archetype_bias` can push
/// passageway from 36.6% to 52.5% of hall cells, but only at eight attempts and
/// 7.19 s, because a weight biases without removing and the solver ends up
/// hunting the rare assignments that satisfy a corridor-heavy preference. A
/// route does not have to be searched for - it is decided, and the collapse
/// fills around it. Measured on solved facilities before this existed, the
/// skeleton a spanning tree produces is 81.5% degree-2 and 0.1% degree-4.
///
/// Exact masks, not the `forced_route_edges` floor. That function says "open at
/// least these faces", which is right for a route that only has to exist; a
/// corridor also has to *not* open the other four, and the difference between
/// those two statements is the whole feature.
///
/// Paths avoid room footprints and enter a room only through a named port, the
/// same rule `forced_route_edges` follows. Returns `None` when a tree edge
/// cannot be routed at all, so the caller restamps rather than shipping a
/// facility with an unreachable room.
#[allow(clippy::type_complexity)]
pub(super) fn corridor_skeleton(
    config: HexWfcConfig,
    blueprints: &[StampedBlueprint],
) -> Option<(
    BTreeMap<HexCoord, u8>,
    BTreeMap<HexCoord, PortClass>,
    BTreeMap<HexCoord, PortClass>,
)> {
    let grid = config.grid();
    let room_cells: BTreeSet<HexCoord> = blueprints
        .iter()
        .flat_map(|blueprint| blueprint.cells.iter().copied())
        .collect();

    // Where a corridor may touch each room: the cell just outside a named port,
    // paired with the port cell itself. The pairing matters - a corridor that
    // ended beside a door without opening onto it would be a dead end with a
    // one-door mask, and there is no hall variant with exactly one lateral door
    // and no shaft.
    let attachments: Vec<Vec<(HexCoord, HexCoord)>> = blueprints
        .iter()
        .map(|stamped| {
            let blueprint = super::blueprint::blueprint_for_role(stamped.role);
            blueprint
                .named_ports
                .iter()
                .filter_map(|&(_, offset, face)| {
                    let slot = blueprint.cells.iter().position(|&cell| cell == offset)?;
                    let cell = *stamped.cells.get(slot)?;
                    let outside = grid.neighbor(cell, face)?;
                    (!room_cells.contains(&outside)).then_some((outside, cell))
                })
                .collect()
        })
        .collect();

    // Nearest-first spanning tree, so every room is reachable and no edge is
    // spent twice. Deterministic: ties break on index, and the anchors it
    // measures are already fixed by the stamp.
    let mut visited: BTreeSet<usize> = BTreeSet::from([0]);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    while visited.len() < blueprints.len() {
        let (from, to) = visited
            .iter()
            .flat_map(|&from| {
                (0..blueprints.len())
                    .filter(|to| !visited.contains(to))
                    .map(move |to| (from, to))
            })
            .min_by_key(|&(from, to)| {
                (
                    observed_hex::travel_distance(blueprints[from].anchor, blueprints[to].anchor),
                    from,
                    to,
                )
            })?;
        visited.insert(to);
        edges.push((from, to));
    }

    let mut adjacency: BTreeMap<HexCoord, BTreeSet<HexCoord>> = BTreeMap::new();
    let mut up: BTreeMap<HexCoord, PortClass> = BTreeMap::new();
    let mut down: BTreeMap<HexCoord, PortClass> = BTreeMap::new();
    for (from, to) in edges {
        let (path, start_port, end_port) = attachments[from]
            .iter()
            .flat_map(|start| attachments[to].iter().map(move |end| (*start, *end)))
            .filter_map(|((start, start_port), (end, end_port))| {
                skeleton_path(config, &room_cells, start, end)
                    .map(|path| (path, start_port, end_port))
            })
            .min_by_key(|(path, _, _)| path.len())?;
        // Open each end onto the door it serves, so an endpoint is a passage
        // into a room rather than a corridor stub beside one.
        if let (Some(&first), Some(&last)) = (path.first(), path.last()) {
            adjacency.entry(first).or_default().insert(start_port);
            adjacency.entry(last).or_default().insert(end_port);
        }
        for window in path.windows(2) {
            let (here, next) = (window[0], window[1]);
            if here.level == next.level {
                adjacency.entry(here).or_default().insert(next);
                adjacency.entry(next).or_default().insert(here);
                continue;
            }
            // A climb is a port class, not a door bit. Both cells still enter
            // `adjacency` so a purely vertical stop is not mistaken for an
            // unvisited cell and left without a mask.
            let (lower, upper) = if here.level < next.level {
                (here, next)
            } else {
                (next, here)
            };
            up.insert(lower, PortClass::ShaftOpen);
            down.insert(upper, PortClass::ShaftOpen);
            adjacency.entry(here).or_default();
            adjacency.entry(next).or_default();
        }
    }

    // A cell's mask is exactly the faces it uses, so the corridor is as wide as
    // it needs to be and no wider. Room cells are dropped: their ports are a
    // frozen contract and the stamp already owns them.
    let doors = adjacency
        .into_iter()
        .filter(|(cell, _)| !room_cells.contains(cell))
        .map(|(cell, neighbours)| {
            let mask = HexFace::LATERAL
                .into_iter()
                .filter(|&face| {
                    grid.neighbor(cell, face)
                        .is_some_and(|next| neighbours.contains(&next))
                })
                .fold(0u8, |mask, face| mask | super::lateral_bit(face));
            (cell, mask)
        })
        .collect();
    up.retain(|cell, _| !room_cells.contains(cell));
    down.retain(|cell, _| !room_cells.contains(cell));
    Some((doors, up, down))
}

/// Shortest path between two cells that never enters a room footprint.
///
/// Vertical steps are included and they are the reason this is not simply a
/// lateral walk. Rooms are spread over ten levels, so a spanning tree over them
/// is full of edges between floors, and a lateral-only router cannot route a
/// single one - it returns `None` for the whole skeleton and every seed fails.
/// (That is not hypothetical: this function was lateral-only first, and the
/// probe that sized the idea missed it by routing over a *solved* lattice,
/// where the shafts already existed.)
///
/// A vertical step is claimed the way [`forced_route_edges`] claims one: as a
/// `ShaftOpen` pair on the two cells' facing vertical ports, never as a door
/// bit. Doors and shafts are different port classes and a mask cannot express
/// one of them.
fn skeleton_path(
    config: HexWfcConfig,
    room_cells: &BTreeSet<HexCoord>,
    start: HexCoord,
    end: HexCoord,
) -> Option<Vec<HexCoord>> {
    let grid = config.grid();
    let mut came_from: BTreeMap<HexCoord, HexCoord> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::from([start]);
    let mut seen: BTreeSet<HexCoord> = BTreeSet::from([start]);
    while let Some(cell) = queue.pop_front() {
        if cell == end {
            let mut path = vec![cell];
            let mut here = cell;
            while let Some(&previous) = came_from.get(&here) {
                path.push(previous);
                here = previous;
            }
            path.reverse();
            return Some(path);
        }
        for face in HexFace::ALL {
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            if room_cells.contains(&next) && next != end {
                continue;
            }
            if seen.insert(next) {
                came_from.insert(next, cell);
                queue.push_back(next);
            }
        }
    }
    None
}

fn coord_key(coord: HexCoord) -> u64 {
    u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32)
}

/// The forced spawn→exit constraint: a randomized monotone staircase where
/// each step goes `East` (q+1), `SouthEast` (r+1), or `Up` (level+1). Lateral
/// steps honor blueprint signatures (a route only crosses a room face that is
/// a `Door` port, and only enters the exit room among stamped rooms); vertical
/// steps avoid blueprint cells entirely, claiming `ShaftOpen` pairs instead of
/// door bits. Returns `None` when the stamped blueprints block every sampled
/// staircase — the caller restamps on the next attempt.
#[allow(clippy::type_complexity)]
pub(super) fn forced_route_edges(
    config: HexWfcConfig,
    blueprints: &[StampedBlueprint],
    signatures: &BTreeMap<HexCoord, PortSignature>,
    rng: &mut SplitMix,
) -> Option<(
    BTreeMap<HexCoord, u8>,
    BTreeMap<HexCoord, PortClass>,
    BTreeMap<HexCoord, PortClass>,
)> {
    let grid = config.grid();
    let blueprint_cells: BTreeSet<HexCoord> =
        blueprints.iter().flat_map(|b| b.cells.clone()).collect();
    let exit = config.exit();

    let lateral_ok = |from: HexCoord, to: HexCoord, face: HexFace| {
        if let Some(signature) = signatures.get(&from)
            && signature.port(face) != PortClass::Door
        {
            return false;
        }
        if blueprint_cells.contains(&to) && to != exit {
            return false;
        }
        if let Some(signature) = signatures.get(&to)
            && signature.port(face.opposite()) != PortClass::Door
        {
            return false;
        }
        true
    };

    const ROUTE_RETRIES: u32 = 64;
    for _ in 0..ROUTE_RETRIES {
        let mut doors: BTreeMap<HexCoord, u8> = BTreeMap::new();
        let mut up: BTreeMap<HexCoord, PortClass> = BTreeMap::new();
        let mut down: BTreeMap<HexCoord, PortClass> = BTreeMap::new();
        let mut current = config.spawn();

        while current != exit {
            let mut candidates: Vec<HexFace> = Vec::new();
            for face in [HexFace::East, HexFace::SouthEast] {
                let advances = match face {
                    HexFace::East => current.q < exit.q,
                    _ => current.r < exit.r,
                };
                if advances
                    && let Some(next) = grid.neighbor(current, face)
                    && lateral_ok(current, next, face)
                {
                    candidates.push(face);
                }
            }
            if current.level < exit.level
                && !blueprint_cells.contains(&current)
                && let Some(next) = grid.neighbor(current, HexFace::Up)
                && !blueprint_cells.contains(&next)
            {
                // Weight vertical steps so the forced route tends to climb a
                // column in one run — this is what grows tall wellshafts.
                candidates.push(HexFace::Up);
                candidates.push(HexFace::Up);
                candidates.push(HexFace::Up);
            }
            if candidates.is_empty() {
                break;
            }
            let face = candidates[(rng.next_u64() % candidates.len() as u64) as usize];
            let next = grid
                .neighbor(current, face)
                .expect("candidate faces stay inside the grid");
            if face.is_lateral() {
                *doors.entry(current).or_default() |= lateral_bit(face);
                *doors.entry(next).or_default() |= lateral_bit(face.opposite());
            } else {
                up.insert(current, PortClass::ShaftOpen);
                down.insert(next, PortClass::ShaftOpen);
            }
            current = next;
        }
        if current == exit {
            return Some((doors, up, down));
        }
    }
    None
}
