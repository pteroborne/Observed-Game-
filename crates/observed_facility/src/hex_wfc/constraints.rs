//! Pre-collapse constraints: blueprint stamping and the forced spawn→exit
//! route.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, PortSignature, lateral_distance};

use crate::map_spec::RoomRole;

use super::blueprint::{self, StampedBlueprint};
use super::{HexWfcConfig, lateral_bit};

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

/// The boundary-adjusted exterior signature of every stamped blueprint cell:
/// the blueprint's declared ports with any face that leaves the grid sealed.
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
pub(super) fn stamp_blueprints(config: HexWfcConfig, rng: &mut SplitMix) -> Vec<StampedBlueprint> {
    let span = (config.max_rooms - config.min_rooms).max(1) as u64;
    let target = config.min_rooms + (rng.next_u64() % span) as usize;

    let mut pool = vec![
        RoomRole::Decision,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::Monitor,
        RoomRole::AnchorCheckpoint,
        RoomRole::Recovery,
    ];
    if config.levels >= 2 {
        // The 2-level atrium leads the pool so tall grids always try one.
        pool.insert(0, RoomRole::GuardianControl);
    }

    let salt = rng.next_u64();
    let mut coords: Vec<HexCoord> = all_coords(config).collect();
    coords.sort_by_key(|&coord| SplitMix::new(coord_key(coord) ^ salt).next_u64());

    let mut stamped: Vec<StampedBlueprint> = Vec::new();
    let mut occupied: BTreeSet<HexCoord> = BTreeSet::new();
    let mut anchors: Vec<HexCoord> = Vec::new();

    for (role, anchor) in [
        (RoomRole::Start, config.spawn()),
        (RoomRole::Exit, config.exit()),
    ] {
        if let Some(cells) = stamp_at(role, anchor, false, config, &occupied, &anchors) {
            occupied.extend(cells.iter().copied());
            anchors.push(anchor);
            stamped.push(StampedBlueprint {
                id: stamped.len() as u32,
                role,
                anchor,
                cells,
            });
        }
    }

    let mut pool_index = 0;
    while stamped.len() < target && pool_index < pool.len() {
        let role = pool[pool_index];
        pool_index += 1;
        for &anchor in &coords {
            if let Some(cells) = stamp_at(role, anchor, true, config, &occupied, &anchors) {
                occupied.extend(cells.iter().copied());
                anchors.push(anchor);
                stamped.push(StampedBlueprint {
                    id: stamped.len() as u32,
                    role,
                    anchor,
                    cells,
                });
                break;
            }
        }
    }

    stamped
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
