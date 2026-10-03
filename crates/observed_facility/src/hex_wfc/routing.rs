//! Corridor search: the shortest way between two cells when changing storey means
//! taking a whole climb composition.
//!
//! A storey is climbed across several cells now (`docs/climb_compositions_plan.md`):
//! a foot, a mid and a high cell along one heading, the landing above the last, and
//! out one cell further on. So the router's vertical move is that whole run, not a
//! step through the floor, and it is a move of five cells rather than one.
//!
//! **Weighted, not breadth-first.** The router this replaces counted a vertical step
//! as one, which was true while a step was a stairwell. A climb counted as one step
//! is a shortcut: a breadth-first search would climb a storey and come straight back
//! down to move eight cells sideways for the price of two. So every move costs the
//! cells it walks, and the search is Dijkstra's, deterministic on ties.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use observed_hex::{HexCoord, HexFace, HexGridSize};

use super::{ClimbTurn, HexWfcConfig};

/// What a climb costs the router: the cells it walks.
const CLIMB_COST: u32 = 5;
/// How much of a route's own recent path a step or a climb is kept clear of.
const RECENT: usize = 24;

/// The run of a climb taken from `from` heading along `heading`, with flight turned by
/// `turn` and landing left by `exit`, in walking order: rising, `[foot, mid, high,
/// landing, out]`; falling, `[landing, high, mid, foot, out]`. `out` is where the route
/// goes on. `None` where any of it leaves the lattice.
#[must_use]
pub(super) fn climb_move(
    grid: HexGridSize,
    from: HexCoord,
    heading: HexFace,
    turn: ClimbTurn,
    exit: ClimbTurn,
    rising: bool,
) -> Option<[HexCoord; 5]> {
    let rising_heading = turn.apply(heading);
    let exit_face = exit.apply(rising_heading);
    if rising {
        let foot = grid.neighbor(from, heading)?;
        let mid = grid.neighbor(foot, heading)?;
        let high = grid.neighbor(mid, rising_heading)?;
        let landing = grid.neighbor(high, HexFace::Up)?;
        let out = grid.neighbor(landing, exit_face)?;
        Some([foot, mid, high, landing, out])
    } else {
        let landing = grid.neighbor(from, exit_face.opposite())?;
        let high = grid.neighbor(landing, HexFace::Down)?;
        let mid = grid.neighbor(high, rising_heading.opposite())?;
        let foot = grid.neighbor(mid, heading.opposite())?;
        let out = grid.neighbor(foot, heading.opposite())?;
        Some([landing, high, mid, foot, out])
    }
}

/// A climb composition a route took, with the 4 climb cells in climbing order
/// `[foot, mid, high, landing]` and the landing's exit cell `exit`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct RoutedClimb {
    pub cells: [HexCoord; 4],
    pub exit: HexCoord,
}

/// The climbs a route `path` takes, each with its 4 climb cells and landing exit cell.
#[must_use]
pub(super) fn routed_climbs_in(path: &[HexCoord]) -> Vec<RoutedClimb> {
    let mut climbs = Vec::new();
    for (index, window) in path.windows(2).enumerate() {
        let (here, next) = (window[0], window[1]);
        if here.level == next.level {
            continue;
        }
        if next.level > here.level {
            // ... foot, mid, high = here, landing = next, out ...
            if index >= 2
                && let Some(&out) = path.get(index + 2)
            {
                climbs.push(RoutedClimb {
                    cells: [path[index - 2], path[index - 1], here, next],
                    exit: out,
                });
            }
        } else if index >= 1
            && let (Some(&mid), Some(&foot)) = (path.get(index + 2), path.get(index + 3))
        {
            // ... in = path[index - 1], landing = here, high = next, mid, foot ...
            climbs.push(RoutedClimb {
                cells: [foot, mid, next, here],
                exit: path[index - 1],
            });
        }
    }
    climbs
}

/// The climbs a route `path` takes, each as its cells in climbing order: foot, mid,
/// high, landing. Nothing else may pass through them - one more door on any of them
/// is a mask no climb cell has - but a later route may take the same climb.
#[cfg(test)]
#[must_use]
pub(super) fn climbs_in(path: &[HexCoord]) -> Vec<[HexCoord; 4]> {
    routed_climbs_in(path)
        .into_iter()
        .map(|rc| rc.cells)
        .collect()
}

/// What the search may walk through.
pub(super) struct Ground<'a> {
    /// Room footprints: never entered.
    pub rooms: &'a BTreeSet<HexCoord>,
    /// Cells earlier routes already spent on climbs: never entered, except by taking
    /// that climb whole.
    pub reserved: &'a BTreeSet<HexCoord>,
    /// The climbs earlier routes laid, in climbing order. A route may take one of
    /// them rather than lay its own: a staircase is shared, as in any building.
    pub climbs: &'a BTreeSet<RoutedClimb>,
    /// Cells earlier routes already claimed. A route may join or cross them, but a
    /// climb may not be laid through one: it already has doors of its own.
    pub claimed: &'a dyn Fn(HexCoord) -> bool,
    /// Cells that must stay corridor: the cell outside every room's door. A climb
    /// laid through one would leave that door opening onto a flight's side.
    pub keep_clear: &'a BTreeSet<HexCoord>,
}

/// The cheapest route from `start` to the first cell `goal` accepts, through lateral
/// doors and whole climbs. Rooms and reserved cells are never entered, except that
/// `end_room` - the far end of a route that finishes at a room's door - may be.
#[must_use]
pub(super) fn corridor_route(
    config: HexWfcConfig,
    ground: &Ground<'_>,
    start: HexCoord,
    goal: impl Fn(HexCoord) -> bool,
    end_room: Option<HexCoord>,
) -> Option<Vec<HexCoord>> {
    corridor_route_inner(config, ground, start, &goal, end_room, false)
        .or_else(|| corridor_route_inner(config, ground, start, &goal, end_room, true))
}

fn corridor_route_inner(
    config: HexWfcConfig,
    ground: &Ground<'_>,
    start: HexCoord,
    goal: &impl Fn(HexCoord) -> bool,
    end_room: Option<HexCoord>,
    allow_turned: bool,
) -> Option<Vec<HexCoord>> {
    let grid = config.grid();
    let walkable = |cell: HexCoord| {
        !ground.reserved.contains(&cell)
            && (!ground.rooms.contains(&cell) || Some(cell) == end_room)
    };
    let cell_count = grid.cell_count();
    let mut came_from: Vec<Option<(HexCoord, Option<[HexCoord; 4]>)>> = vec![None; cell_count];
    let mut best: Vec<u32> = vec![u32::MAX; cell_count];
    best[grid.index(start)] = 0;
    let mut frontier = BinaryHeap::from([Reverse((0u32, 0u64, start))]);
    let mut pushed = 0u64;
    while let Some(Reverse((cost, _, cell))) = frontier.pop() {
        if best[grid.index(cell)] < cost {
            continue;
        }
        if goal(cell) {
            let mut path = vec![cell];
            let mut here = cell;
            while let Some((previous, via)) = came_from[grid.index(here)] {
                if let Some(via) = via {
                    path.extend(via.iter().rev());
                }
                path.push(previous);
                here = previous;
            }
            path.reverse();
            // A route that crossed itself would give one cell two jobs: a climb laid
            // through a cell the same route walks past. The search is over cells, so
            // it cannot see that until the route is whole; it drops this arrival and
            // keeps looking rather than give up on a goal it can still reach.
            let unique: BTreeSet<_> = path.iter().collect();
            if unique.len() == path.len() {
                return Some(path);
            }
            continue;
        }
        // The route's own recent cells, which a step from here may not walk back into
        // and a climb from here may not be laid through. A climb's cells lie within
        // three cells of where it is taken, so the recent stretch is where a route
        // can cross itself; a search over cells keeps one route to each, and one that
        // crossed itself would otherwise be the only route it has to a cell.
        let mut recent = [HexCoord::default(); RECENT];
        let mut recent_len = 0;
        let mut back = cell;
        while recent_len < RECENT {
            let Some((previous, via)) = came_from[grid.index(back)] else {
                break;
            };
            if let Some(via) = via {
                for v in via {
                    if recent_len < RECENT && !recent[..recent_len].contains(&v) {
                        recent[recent_len] = v;
                        recent_len += 1;
                    }
                }
            }
            if recent_len < RECENT && !recent[..recent_len].contains(&previous) {
                recent[recent_len] = previous;
                recent_len += 1;
            }
            back = previous;
        }
        let in_recent = |coord: HexCoord| recent[..recent_len].contains(&coord);
        let mut reach = |next: HexCoord, step: u32, via: Option<[HexCoord; 4]>| {
            let total = cost + step;
            let idx = grid.index(next);
            if total < best[idx] {
                best[idx] = total;
                came_from[idx] = Some((cell, via));
                pushed += 1;
                frontier.push(Reverse((total, pushed, next)));
            }
        };
        for face in HexFace::LATERAL {
            if let Some(next) = grid.neighbor(cell, face)
                && walkable(next)
                && !in_recent(next)
            {
                reach(next, 1, None);
            }
        }
        let clear = |coord: HexCoord| {
            !ground.rooms.contains(&coord)
                && !ground.reserved.contains(&coord)
                && !ground.keep_clear.contains(&coord)
                && !(ground.claimed)(coord)
        };
        // 1. Existing climbs already laid: a later route may take one whole.
        for climb in ground.climbs {
            let [foot, mid, high, landing] = climb.cells;
            let exit_cell = climb.exit;
            let Some(heading) = HexFace::LATERAL
                .into_iter()
                .find(|&f| grid.neighbor(foot, f) == Some(mid))
            else {
                continue;
            };
            // Rising through this laid climb: enter at foot, leave at exit_cell
            if cell.level + 1 < grid.levels
                && grid.neighbor(foot, heading.opposite()) == Some(cell)
                && !in_recent(exit_cell)
                && walkable(exit_cell)
            {
                reach(exit_cell, CLIMB_COST, Some([foot, mid, high, landing]));
            }
            // Falling through this laid climb: enter at exit_cell, leave before foot
            if cell.level > 0
                && cell == exit_cell
                && let Some(out) = grid.neighbor(foot, heading.opposite())
                && !in_recent(out)
                && walkable(out)
            {
                reach(out, CLIMB_COST, Some([landing, high, mid, foot]));
            }
        }
        // 2. Fresh climbs laid through untouched corridor cells.
        if cell.level + 1 < grid.levels {
            for heading in HexFace::LATERAL {
                let Some(foot) = grid.neighbor(cell, heading) else {
                    continue;
                };
                if in_recent(foot) || !clear(foot) {
                    continue;
                };
                let Some(mid) = grid.neighbor(foot, heading) else {
                    continue;
                };
                if in_recent(mid) || !clear(mid) {
                    continue;
                };

                // Try straight (Ahead, Ahead) first
                let straight_high = grid.neighbor(mid, heading);
                let straight_landing = straight_high.and_then(|h| grid.neighbor(h, HexFace::Up));
                let straight_out = straight_landing.and_then(|l| grid.neighbor(l, heading));
                let straight_ok = straight_high.is_some_and(|h| !in_recent(h) && clear(h))
                    && straight_landing.is_some_and(|l| !in_recent(l) && clear(l))
                    && straight_out.is_some_and(|o| !in_recent(o) && walkable(o));
                if straight_ok {
                    let [high, landing, out] = [
                        straight_high.unwrap(),
                        straight_landing.unwrap(),
                        straight_out.unwrap(),
                    ];
                    reach(out, CLIMB_COST, Some([foot, mid, high, landing]));
                    continue;
                }

                if !allow_turned {
                    continue;
                }

                // If straight cannot be laid, evaluate turned shapes
                for turn in ClimbTurn::BENDS {
                    let rising_heading = turn.apply(heading);
                    let Some(high) = grid.neighbor(mid, rising_heading) else {
                        continue;
                    };
                    if in_recent(high) || !clear(high) {
                        continue;
                    };
                    let Some(landing) = grid.neighbor(high, HexFace::Up) else {
                        continue;
                    };
                    if in_recent(landing) || !clear(landing) {
                        continue;
                    };
                    let through = [foot, mid, high, landing];
                    for exit in ClimbTurn::EXITS {
                        let exit_face = exit.apply(rising_heading);
                        let Some(out) = grid.neighbor(landing, exit_face) else {
                            continue;
                        };
                        if !in_recent(out) && walkable(out) {
                            reach(out, CLIMB_COST, Some(through));
                        }
                    }
                }
            }
        }
        if cell.level > 0 {
            for step_face in HexFace::LATERAL {
                let Some(landing) = grid.neighbor(cell, step_face) else {
                    continue;
                };
                if in_recent(landing) || !clear(landing) {
                    continue;
                };
                let Some(high) = grid.neighbor(landing, HexFace::Down) else {
                    continue;
                };
                if in_recent(high) || !clear(high) {
                    continue;
                };
                let exit_face = step_face.opposite();

                // Try straight falling first
                let straight_mid = grid.neighbor(high, step_face);
                let straight_foot = straight_mid.and_then(|m| grid.neighbor(m, step_face));
                let straight_out = straight_foot.and_then(|f| grid.neighbor(f, step_face));
                let straight_ok = straight_mid.is_some_and(|m| !in_recent(m) && clear(m))
                    && straight_foot.is_some_and(|f| !in_recent(f) && clear(f))
                    && straight_out.is_some_and(|o| !in_recent(o) && walkable(o));
                if straight_ok {
                    let [mid, foot, out] = [
                        straight_mid.unwrap(),
                        straight_foot.unwrap(),
                        straight_out.unwrap(),
                    ];
                    reach(out, CLIMB_COST, Some([landing, high, mid, foot]));
                    continue;
                }

                if !allow_turned {
                    continue;
                }

                // If straight cannot be laid, evaluate turned shapes
                for down_mid_face in HexFace::LATERAL {
                    let rising_heading = down_mid_face.opposite();
                    let exit = ClimbTurn::between(rising_heading, exit_face);
                    if !ClimbTurn::EXITS.contains(&exit) {
                        continue;
                    };
                    let Some(mid) = grid.neighbor(high, down_mid_face) else {
                        continue;
                    };
                    if in_recent(mid) || !clear(mid) {
                        continue;
                    };
                    for turn in ClimbTurn::BENDS {
                        let heading =
                            HexFace::LATERAL[(rising_heading.index() + 6 - turn.offset()) % 6];
                        let down_foot_face = heading.opposite();
                        let Some(foot) = grid.neighbor(mid, down_foot_face) else {
                            continue;
                        };
                        if in_recent(foot) || !clear(foot) {
                            continue;
                        };
                        let Some(out) = grid.neighbor(foot, down_foot_face) else {
                            continue;
                        };
                        if !in_recent(out) && walkable(out) {
                            reach(out, CLIMB_COST, Some([landing, high, mid, foot]));
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HexWfcConfig {
        HexWfcConfig {
            cols: 10,
            rows: 10,
            levels: 3,
            ..HexWfcConfig::default()
        }
    }

    fn at(q: u16, r: u16, level: u8) -> HexCoord {
        HexCoord { q, r, level }
    }

    #[test]
    fn a_route_up_a_storey_takes_a_whole_climb() {
        let none = BTreeSet::new();
        let ground = Ground {
            rooms: &none,
            reserved: &none,
            climbs: &BTreeSet::new(),
            claimed: &|_| false,
            keep_clear: &none,
        };
        let end = at(6, 2, 1);
        let path = corridor_route(config(), &ground, at(1, 2, 0), |cell| cell == end, None)
            .expect("a route");
        let climbs = climbs_in(&path).concat();
        assert_eq!(climbs.len(), 4, "one composition: {path:?}");
        // The foot, mid and high stand in a line on the lower storey, the landing over
        // the high cell.
        let [foot, mid, high, landing] = [climbs[0], climbs[1], climbs[2], climbs[3]];
        assert_eq!(
            (foot.level, mid.level, high.level, landing.level),
            (0, 0, 0, 1)
        );
        assert_eq!((landing.q, landing.r), (high.q, high.r));
    }

    #[test]
    fn climbing_is_never_a_lateral_shortcut() {
        let none = BTreeSet::new();
        let ground = Ground {
            rooms: &none,
            reserved: &none,
            climbs: &BTreeSet::new(),
            claimed: &|_| false,
            keep_clear: &none,
        };
        let end = at(9, 2, 0);
        let path = corridor_route(config(), &ground, at(0, 2, 0), |cell| cell == end, None)
            .expect("a route");
        assert!(path.iter().all(|cell| cell.level == 0), "{path:?}");
    }

    #[test]
    fn a_climb_is_never_laid_through_a_claimed_or_reserved_cell() {
        let none = BTreeSet::new();
        let reserved: BTreeSet<_> = (0..10).map(|r| at(3, r, 0)).collect();
        let ground = Ground {
            rooms: &none,
            reserved: &reserved,
            climbs: &BTreeSet::new(),
            claimed: &|cell: HexCoord| cell.q == 4 && cell.level == 0,
            keep_clear: &none,
        };
        let end = at(8, 2, 1);
        if let Some(path) = corridor_route(config(), &ground, at(1, 2, 0), |cell| cell == end, None)
        {
            for cell in climbs_in(&path).concat() {
                assert!(!reserved.contains(&cell) && !(cell.q == 4 && cell.level == 0));
            }
            assert!(path.iter().all(|cell| !reserved.contains(cell)));
        }
    }

    #[test]
    fn a_route_down_takes_a_climb_backwards() {
        let none = BTreeSet::new();
        let ground = Ground {
            rooms: &none,
            reserved: &none,
            climbs: &BTreeSet::new(),
            claimed: &|_| false,
            keep_clear: &none,
        };
        let end = at(1, 2, 0);
        let path = corridor_route(config(), &ground, at(6, 2, 1), |cell| cell == end, None)
            .expect("a route");
        let climbs = climbs_in(&path).concat();
        assert_eq!(climbs.len(), 4, "{path:?}");
        let levels: Vec<_> = climbs.iter().map(|cell| cell.level).collect();
        assert_eq!(levels, vec![0, 0, 0, 1], "in climbing order: {climbs:?}");
    }

    #[test]
    fn a_route_takes_a_turned_climb_when_straight_is_obstructed() {
        let none = BTreeSet::new();
        // Block straight-ahead climb cells along East from row 2
        let mut reserved = BTreeSet::new();
        // Straight East climb from (1, 2, 0) would place high at (4, 2, 0)
        reserved.insert(at(4, 2, 0));
        let ground = Ground {
            rooms: &none,
            reserved: &reserved,
            climbs: &BTreeSet::new(),
            claimed: &|_| false,
            keep_clear: &none,
        };
        let end = at(3, 4, 1);
        let path = corridor_route(config(), &ground, at(1, 2, 0), |cell| cell == end, None)
            .expect("a route using turned climb");
        let climbs = climbs_in(&path).concat();
        assert_eq!(climbs.len(), 4, "{path:?}");
        let [foot, mid, high, landing] = [climbs[0], climbs[1], climbs[2], climbs[3]];
        assert_eq!(
            (foot.level, mid.level, high.level, landing.level),
            (0, 0, 0, 1)
        );
        assert!(!reserved.contains(&foot) && !reserved.contains(&mid) && !reserved.contains(&high));
    }
}
