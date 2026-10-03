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
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use observed_hex::{HexCoord, HexFace, HexGridSize};

use super::HexWfcConfig;

/// What a climb costs the router: the cells it walks.
const CLIMB_COST: u32 = 5;
/// How much of a route's own recent path a step or a climb is kept clear of.
const RECENT: usize = 24;

/// The run of a climb taken from `from` travelling along `travel`, in walking order:
/// rising, `[foot, mid, high, landing, out]`; falling, `[landing, high, mid, foot,
/// out]`. `out` is where the route goes on. `None` where any of it leaves the lattice.
#[must_use]
pub(super) fn climb_move(
    grid: HexGridSize,
    from: HexCoord,
    travel: HexFace,
    rising: bool,
) -> Option<[HexCoord; 5]> {
    if rising {
        let foot = grid.neighbor(from, travel)?;
        let mid = grid.neighbor(foot, travel)?;
        let high = grid.neighbor(mid, travel)?;
        let landing = grid.neighbor(high, HexFace::Up)?;
        let out = grid.neighbor(landing, travel)?;
        Some([foot, mid, high, landing, out])
    } else {
        let landing = grid.neighbor(from, travel)?;
        let high = grid.neighbor(landing, HexFace::Down)?;
        let mid = grid.neighbor(high, travel)?;
        let foot = grid.neighbor(mid, travel)?;
        let out = grid.neighbor(foot, travel)?;
        Some([landing, high, mid, foot, out])
    }
}

/// The climbs a route `path` takes, each as its cells in climbing order: foot, mid,
/// high, landing. Nothing else may pass through them - one more door on any of them
/// is a mask no climb cell has - but a later route may take the same climb.
#[must_use]
pub(super) fn climbs_in(path: &[HexCoord]) -> Vec<[HexCoord; 4]> {
    let mut climbs = Vec::new();
    for (index, window) in path.windows(2).enumerate() {
        let (here, next) = (window[0], window[1]);
        if here.level == next.level {
            continue;
        }
        if next.level > here.level {
            // ... foot, mid, high = here, landing = next ...
            if index >= 2 {
                climbs.push([path[index - 2], path[index - 1], here, next]);
            }
        } else if let (Some(&mid), Some(&foot)) = (path.get(index + 2), path.get(index + 3)) {
            // ... landing = here, high = next, mid, foot ...
            climbs.push([foot, mid, next, here]);
        }
    }
    climbs
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
    pub climbs: &'a BTreeSet<[HexCoord; 4]>,
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
    let grid = config.grid();
    let walkable = |cell: HexCoord| {
        !ground.reserved.contains(&cell)
            && (!ground.rooms.contains(&cell) || Some(cell) == end_room)
    };
    // Where each cell was reached from, and the climb cells walked to get there.
    let mut came_from: BTreeMap<HexCoord, (HexCoord, Option<[HexCoord; 4]>)> = BTreeMap::new();
    let mut best: BTreeMap<HexCoord, u32> = BTreeMap::from([(start, 0)]);
    let mut frontier = BinaryHeap::from([Reverse((0u32, 0u64, start))]);
    let mut pushed = 0u64;
    while let Some(Reverse((cost, _, cell))) = frontier.pop() {
        if best.get(&cell).is_some_and(|&known| known < cost) {
            continue;
        }
        if goal(cell) {
            let mut path = vec![cell];
            let mut here = cell;
            while let Some(&(previous, via)) = came_from.get(&here) {
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
        let mut recent: BTreeSet<HexCoord> = BTreeSet::new();
        let mut back = cell;
        while recent.len() < RECENT {
            let Some(&(previous, via)) = came_from.get(&back) else {
                break;
            };
            recent.extend(via.into_iter().flatten());
            recent.insert(previous);
            back = previous;
        }
        let mut reach = |next: HexCoord, step: u32, via: Option<[HexCoord; 4]>| {
            let total = cost + step;
            if best.get(&next).is_none_or(|&known| total < known) {
                best.insert(next, total);
                came_from.insert(next, (cell, via));
                pushed += 1;
                frontier.push(Reverse((total, pushed, next)));
            }
        };
        for face in HexFace::LATERAL {
            if let Some(next) = grid.neighbor(cell, face)
                && walkable(next)
                && !recent.contains(&next)
            {
                reach(next, 1, None);
            }
        }
        for travel in HexFace::LATERAL {
            for rising in [true, false] {
                let Some([a, b, c, d, out]) = climb_move(grid, cell, travel, rising) else {
                    continue;
                };
                let through = [a, b, c, d];
                let climbing_order = if rising { through } else { [d, c, b, a] };
                let laid = ground.climbs.contains(&climbing_order);
                let fresh = through.iter().all(|&cell| {
                    !ground.rooms.contains(&cell)
                        && !ground.reserved.contains(&cell)
                        && !ground.keep_clear.contains(&cell)
                        && !(ground.claimed)(cell)
                });
                let crosses = through
                    .iter()
                    .chain([&out])
                    .any(|cell| recent.contains(cell));
                if (laid || fresh) && !crosses && walkable(out) {
                    reach(out, CLIMB_COST, Some(through));
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
}
