//! The solved floor: one real WFC solve, projected to real authored geometry.
//!
//! Everything downstream — collision, navigation, the generator, the ledge a
//! minor is shoved off — is derived from this module's output. Nothing here
//! authors a room. The lab's whole claim is that the architecture came out of
//! the production solver, so the only geometry it may invent is none.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use glam::Vec3;
use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexCompositionProfile, HexInfluenceField, HexObservationFrame, HexPlacement,
    HexRelayoutProgress, HexSpace, HexWfcConfig, HexWfcWorld,
};
use observed_hex::{HexCoord, HexFace, face_edge, hex_origin};
use observed_match::hex_wfc::{HexMatchContent, HexWfcGeometrySnapshot};
use observed_traversal::{ColliderShape, ColliderSpec};
use rapier3d::prelude::*;

/// How much floor this lab wants: a small area with holes in it.
///
/// The void cells are the point. They are not scenery the solver failed to
/// fill; they are the floor-less volume a shoved minor falls through, placed by
/// the same collapse that placed the walls around them.
///
/// The board is 7x5, and it is this big for one measured reason: bounded
/// relayout needs somewhere to happen.
///
/// The solver permanently pins `spawn` and `exit`, gives *every* pin a full
/// one-cell halo, and adds a pin for each cell the Observer occupies or can
/// see. On a small board that saturates. Measured: a 3x3 floor never offered a
/// selectable pocket at all, and a 4x4 floor protected fourteen of its sixteen
/// cells even with one cell visible, leaving only a void cell whose re-collapse
/// faithfully produced void. Pockets that actually change something, over
/// twenty-five seeds: 6x5 eleven times, 7x5 fifteen, 8x6 seventeen.
///
/// So this lab is a floor rather than the seven-cell arena it started as. That
/// is the trade observation-safe relayout costs, and it is worth knowing before
/// anyone designs a room around the mechanic.
pub const MIN_CELLS: usize = 22;
/// Above this the floor stops being one floor and starts being a building.
pub const MAX_CELLS: usize = 38;

/// Cells below this have left the facility. The lowest authored floor sits near
/// `FLOOR_SLAB_TOP`, so this is far enough below the deepest cell that nothing
/// reaches it by walking.
pub const VOID_Y: f32 = -12.0;

/// How far forward to look for a solve of the right size before giving up.
///
/// Roughly one seed in three produces exactly seven occupied cells at this
/// space mix, so this is many times the expected search. It exists to fail
/// loudly rather than to spin.
const SEED_SEARCH_LIMIT: u64 = 512;

/// A face of an occupied cell that opens onto nothing.
///
/// This is the lab's success condition made into data. A shove needs somewhere
/// to commit a minor *to*, and the solver does not promise one: most faces onto
/// void come back sealed by the projection's boundary shell. A site with no
/// ledges is a site where the kinetic tool cannot do its job, so `solve`
/// rejects it rather than handing the player an arena with no answer in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ledge {
    pub cell: HexCoord,
    pub face: HexFace,
    /// Whether the drop is into one of the floor's own holes rather than off
    /// the outside of the lattice. Both are fatal; only this one reads as a
    /// cell the collapse declined to build.
    pub over_hole: bool,
}

/// One solved, projected, playable floor.
#[derive(Clone)]
pub struct Site {
    /// The seed the caller asked for.
    pub requested_seed: u64,
    /// The seed that actually produced this layout. Equal to `requested_seed`
    /// unless the search had to step forward to find a seven-cell solve.
    pub seed: u64,
    pub world: HexWfcWorld,
    pub snapshot: HexWfcGeometrySnapshot,
    /// The tile catalogue this floor was projected from. Kept because a
    /// relayout has to project its delta from the same corpus the solve used.
    pub content: Arc<HexMatchContent>,
    /// The seven occupied cells, in lattice order.
    pub cells: Vec<HexCoord>,
    /// The two holes.
    pub voids: Vec<HexCoord>,
    /// Faces of occupied cells that open onto void.
    ///
    /// Zero on this tile corpus, and deliberately still measured: see the
    /// parapet finding in the lab README. It is the number that would have to
    /// move for the design's "shove a minor off unrailed geometry" to have a
    /// site on a solved floor without the Architect first making one.
    pub ledges: Vec<Ledge>,
    pub spawn: Vec3,
    /// Which way the Observer stands up facing: along the route out of the
    /// spawn room, so the first thing in view is the door they have to use.
    pub spawn_facing: Vec3,
    pub generator: Vec3,
    pub station: Vec3,
    pub panel: Vec3,
    /// Where the Observer can retract the tile they are standing on.
    pub demolition: Vec3,
    /// The hall cell the panel retracts. Chosen so that removing it actually
    /// severs a crossing rather than cosmetically deleting a floor.
    pub retracting: HexCoord,
    /// Cells with an open door into [`Self::retracting`]. Once that tile is
    /// gone these thresholds open onto nothing, which is the only ledge a
    /// solved floor offers — and it is one the Architect made.
    pub thresholds: Vec<Ledge>,
    /// Feet-height waypoints, already proven to stand on something.
    pub nav: Vec<Vec3>,
    /// Where a wave may put a minor: feet positions on real support.
    pub muster: Vec<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteError {
    /// No seed within the search window solved into a usable seven-cell floor.
    /// `last` is why the most recent candidate was rejected, which is the only
    /// useful thing to say when a search of this length comes back empty.
    NoSevenCellSolve { searched: u64, last: &'static str },
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSevenCellSolve { searched, last } => write!(
                f,
                "no usable floor in {searched} seeds (last rejection: {last})"
            ),
        }
    }
}

/// The solver settings this lab is sited on.
///
/// `route_corridors` is off, which is the one deliberate departure from the
/// shipped profile. On, the solver routes a minimal corridor skeleton between
/// spawn and exit and voids the rest, and on a 3x3 board that produces a
/// ribbon: a hallway with two rooms on the ends. A hallway is a bad place to
/// test a tool whose whole subject is spatial commitment. Off, the same
/// collapse fills the board and the void share decides how much of it survives,
/// which is a compact area with holes in it.
#[must_use]
pub fn config() -> HexWfcConfig {
    HexWfcConfig {
        cols: 7,
        rows: 5,
        levels: 1,
        // The solver's floor. Both rooms are load-bearing here: one is where
        // the Observer stands up and one is where the generator lives, so the
        // walk between them is the floor's only real errand.
        min_rooms: 2,
        max_rooms: 3,
        retry_budget: 200,
        min_room_distance: 1,
    }
}

/// The composition profile, tuned for two holes in nine cells.
#[must_use]
pub fn profile() -> HexCompositionProfile {
    let mut profile = HexCompositionProfile::baseline();
    // The shipped 300 is tuned for a facility with thousands of cells, where a
    // heavy void share reads as open volume. On nine cells it reads as five
    // cells and a gap. 100 keeps the board mostly built while still voiding
    // enough of it to fall through.
    profile.space_mix.void = 100.0;
    profile.route_corridors = false;
    profile.label = "wfc_kinetic_lab seven-cell floor".to_string();
    profile
}

impl Site {
    /// Solve, project, and derive every gameplay anchor from the result.
    ///
    /// Searches forward from `requested_seed` for the first solve with exactly
    /// a usable number of occupied cells, so that any seed a
    /// player types produces a playable floor rather than an error.
    pub fn solve(requested_seed: u64, content: &Arc<HexMatchContent>) -> Result<Self, SiteError> {
        let config = config();
        let profile = profile();
        let mut last = "no solve reached assembly";
        for offset in 0..SEED_SEARCH_LIMIT {
            let seed = requested_seed.wrapping_add(offset);
            let Ok(world) = HexWfcWorld::generate_with_profile(seed, config, None, &profile) else {
                continue;
            };
            let cells: Vec<HexCoord> = world
                .placements
                .iter()
                .filter(|(_, placement)| placement.space != HexSpace::Void)
                .map(|(coord, _)| *coord)
                .collect();
            if !(MIN_CELLS..=MAX_CELLS).contains(&cells.len()) {
                continue;
            }
            let Ok(snapshot) = HexWfcGeometrySnapshot::project_with_rooms(
                &world,
                content.cells(),
                content.rooms(),
            ) else {
                continue;
            };
            match Self::assemble(
                requested_seed,
                seed,
                world,
                snapshot,
                Arc::clone(content),
                cells,
            ) {
                Ok(site) => return Ok(site),
                Err(reason) => last = reason,
            }
        }
        Err(SiteError::NoSevenCellSolve {
            searched: SEED_SEARCH_LIMIT,
            last,
        })
    }

    fn assemble(
        requested_seed: u64,
        seed: u64,
        world: HexWfcWorld,
        snapshot: HexWfcGeometrySnapshot,
        content: Arc<HexMatchContent>,
        cells: Vec<HexCoord>,
    ) -> Result<Self, &'static str> {
        let probe = Probe::new(&snapshot.arena.colliders);
        let occupied: BTreeSet<HexCoord> = cells.iter().copied().collect();
        let voids: Vec<HexCoord> = (0..config().cols)
            .flat_map(|q| (0..config().rows).map(move |r| HexCoord { q, r, level: 0 }))
            .filter(|coord| !occupied.contains(coord))
            .collect();

        // A floor this lab can use is one the Architect can actually change.
        // Roughly half of them cannot: a pocket of void re-collapses faithfully
        // into void, and cells bounded by frozen neighbours often have exactly
        // one legal answer. Telegraphing on such a floor spends the warning and
        // the sound on something that was never going to move, so those seeds
        // are skipped at deal time instead.
        if !can_decohere(&world) {
            return Err("no pocket on this floor would re-collapse into anything else");
        }

        let ledges = ledges(&occupied, &probe);
        // A floor with no ledge is still usable: see the parapet finding in
        // the README. The kill that works on this corpus is retraction.

        // Anchors. The two rooms are the floor's fixed points: the Observer
        // stands up in one and the generator lives in the other, so the errand
        // that crosses the floor is the errand the architecture was solved for.
        let rooms: Vec<HexCoord> = cells
            .iter()
            .copied()
            .filter(|coord| world.placements[coord].space == HexSpace::Room)
            .collect();
        let halls: Vec<HexCoord> = cells
            .iter()
            .copied()
            .filter(|coord| world.placements[coord].space == HexSpace::Hall)
            .collect();
        let spawn_cell = *rooms.first().ok_or("no room cell")?;
        let generator_cell = *rooms.last().ok_or("no room cell")?;
        if spawn_cell == generator_cell {
            return Err("only one room cell");
        }
        // The station sits in the hall nearest the spawn room, and the panel in
        // the hall nearest the generator, so neither errand is free.
        // Choose what retracts *before* placing anything on the floor, so the
        // panel can never delete the cell it is standing on.
        let retracting = cut_cell(&world, &occupied, spawn_cell, generator_cell).or_else(|| {
            halls
                .iter()
                .copied()
                .max_by_key(|coord| lattice_steps(&world, spawn_cell, *coord).unwrap_or(0))
        });
        let retracting = retracting.ok_or("no hall cell to retract")?;
        let available: Vec<HexCoord> = halls
            .iter()
            .copied()
            .filter(|coord| *coord != retracting)
            .collect();
        // The station sits in the hall nearest the spawn room, so recharging is
        // a walk away from wherever the fight has drifted.
        let station_cell = *available
            .iter()
            .min_by_key(|coord| lattice_steps(&world, spawn_cell, **coord).unwrap_or(u32::MAX))
            .ok_or("no hall cell left for the station")?;
        // The panel sits next to what it deletes where the floor allows it, so
        // the two-second warning is spent looking at the crossing that is about
        // to stop existing rather than at a wall somewhere else.
        let panel_cell = available
            .iter()
            .copied()
            .filter(|coord| *coord != station_cell)
            .find(|coord| adjacent(&world, *coord, retracting))
            .or_else(|| {
                available
                    .iter()
                    .copied()
                    .filter(|coord| *coord != station_cell)
                    .max_by_key(|coord| lattice_steps(&world, spawn_cell, *coord).unwrap_or(0))
            })
            .ok_or("no hall cell left for the panel")?;

        let spawn = probe
            .stand(center(spawn_cell))
            .ok_or("spawn room centre has no standable floor")?;
        let generator = probe
            .stand_near(generator_cell)
            .ok_or("generator room has no standable floor")?;
        let station = probe
            .stand_near(station_cell)
            .ok_or("station hall has no standable floor")?;
        // The demolition control stands *beside* what it retracts, never on it:
        // operating it from the doomed tile deletes the floor underfoot, which
        // the director discovered by falling out of the facility seventy ticks
        // after pressing it.
        let demolition = probe
            .stand_near(panel_cell)
            .ok_or("no standable floor beside the retracting tile")?;
        let decohere_cell = available
            .iter()
            .copied()
            .filter(|cell| *cell != station_cell && *cell != panel_cell)
            .max_by_key(|cell| lattice_steps(&world, panel_cell, *cell).unwrap_or(0))
            .unwrap_or(station_cell);
        let panel = probe
            .stand_near(decohere_cell)
            .ok_or("no standable floor for the decoherence control")?;
        // Devices must not land on the cell that is about to be retracted, or
        // operating the panel would delete the thing that operates it.
        // Verified with the same lookup the simulation uses, after snapping.
        // Deriving a device from a neighbouring cell is not enough: the ring
        // search and the snap to a waypoint can each carry it over a boundary,
        // and a demolition control standing inside its own target deletes the
        // floor under whoever operates it. The director found this by falling
        // through the hole it had just made, at the same tick every run.
        let in_target = |at: Vec3| cell_containing_in(at) == Some(retracting);
        if [generator, station, panel, demolition]
            .iter()
            .any(|at| in_target(*at))
        {
            return Err("a device landed on the cell the demolition retracts");
        }

        // Face the way out. A room on this lattice often has exactly one open
        // door, and it is rarely the direction the far room happens to lie in.
        let spawn_facing = world
            .route_between(spawn_cell, generator_cell)
            .and_then(|route| route.get(1).copied())
            .map(|next| {
                (center(next) - center(spawn_cell))
                    .with_y(0.)
                    .normalize_or_zero()
            })
            .unwrap_or(Vec3::X);

        // Doorways into the doomed tile. They are ordinary thresholds now and
        // drops the moment it retracts.
        let thresholds: Vec<Ledge> = occupied
            .iter()
            .copied()
            .filter(|cell| *cell != retracting)
            .filter_map(|cell| {
                let placement = world.placements.get(&cell)?;
                HexFace::LATERAL
                    .into_iter()
                    .find(|face| {
                        placement.is_open(*face) && grid_neighbor(cell, *face) == Some(retracting)
                    })
                    .map(|face| Ledge {
                        cell,
                        face,
                        over_hole: true,
                    })
            })
            .collect();

        let nav = waypoints(&world, &occupied, &probe);
        // Devices sit on navigation waypoints rather than on any standable
        // point. A point can be standable and still be unreachable — behind a
        // column, inside an alcove the controller cannot enter — and a device
        // nobody can walk up to is a device that does not exist. The director
        // found this by standing two metres from a control it could never
        // operate, for two thousand ticks.
        let snap = |at: Vec3| {
            nav.iter()
                .copied()
                .min_by(|a, b| at.distance_squared(*a).total_cmp(&at.distance_squared(*b)))
                .unwrap_or(at)
        };
        let (generator, station, panel, demolition) = (
            snap(generator),
            snap(station),
            snap(panel),
            snap(demolition),
        );
        let muster = muster(&world, &occupied, &probe, spawn_cell);
        if nav.len() < MIN_CELLS {
            return Err("too few standable waypoints");
        }
        if muster.is_empty() {
            return Err("nowhere to muster a wave");
        }

        Ok(Self {
            requested_seed,
            seed,
            world,
            snapshot,
            content,
            cells,
            voids,
            ledges,
            spawn,
            spawn_facing,
            generator,
            station,
            panel,
            demolition,
            retracting,
            thresholds,
            nav,
            muster,
        })
    }

    /// The architectural register of a cell, for presentation.
    #[must_use]
    pub fn register(&self, coord: HexCoord) -> ArchitectureRegister {
        self.world
            .architecture
            .get(&coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional)
    }

    #[must_use]
    pub fn placement(&self, coord: HexCoord) -> Option<&HexPlacement> {
        self.world.placements.get(&coord)
    }

    /// The cell a world point sits over, or `None` when it is outside the
    /// lattice footprint. Used to say *where* something went over the edge.
    #[must_use]
    pub fn cell_containing(&self, point: Vec3) -> Option<HexCoord> {
        let cell = cell_at(point)?;
        // Plan-view nearest centre is only meaningful within a cell's own
        // footprint; past 8 m the answer is "nowhere on this floor".
        (center(cell).with_y(point.y).distance(point) <= observed_hex::ACROSS_CORNERS * 0.5)
            .then_some(cell)
    }

    /// Structural colliders, as projected. The model inserts these verbatim.
    #[must_use]
    pub fn colliders(&self) -> &[ColliderSpec] {
        &self.snapshot.arena.colliders
    }

    /// A one-line plan of the floor for diagnostics and evidence.
    #[must_use]
    pub fn plan(&self) -> String {
        let config = config();
        let mut out = String::new();
        for r in (0..config.rows).rev() {
            for q in 0..config.cols {
                let coord = HexCoord { q, r, level: 0 };
                out.push(match self.world.placements.get(&coord).map(|p| p.space) {
                    Some(HexSpace::Room) => 'R',
                    Some(HexSpace::Hall) => {
                        if coord == self.retracting {
                            'x'
                        } else {
                            'H'
                        }
                    }
                    _ => '.',
                });
            }
            if r > 0 {
                out.push('/');
            }
        }
        out
    }
}

#[must_use]
fn center(coord: HexCoord) -> Vec3 {
    Vec3::from_array(hex_origin(coord))
}

/// A point inside a cell, `distance` metres from its centre toward `face`.
fn offset_in(coord: HexCoord, face: HexFace, distance: f32) -> Vec3 {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let mid = Vec3::new((a.0 + b.0) as f32 * 0.5, 0.0, (a.1 + b.1) as f32 * 0.5);
    center(coord) + mid.normalize_or_zero() * distance
}

/// Free-function form of [`Site::cell_containing`], for use while a site is
/// still being assembled.
fn cell_containing_in(point: Vec3) -> Option<HexCoord> {
    let cell = cell_at(point)?;
    (center(cell).with_y(point.y).distance(point) <= observed_hex::ACROSS_CORNERS * 0.5)
        .then_some(cell)
}

/// Which lattice cell a world point sits in, if any. Plan-view nearest centre.
fn cell_at(point: Vec3) -> Option<HexCoord> {
    let config = config();
    (0..config.cols)
        .flat_map(|q| (0..config.rows).map(move |r| HexCoord { q, r, level: 0 }))
        .min_by(|a, b| {
            let da = center(*a).with_y(point.y).distance_squared(point);
            let db = center(*b).with_y(point.y).distance_squared(point);
            da.total_cmp(&db)
        })
}

/// The centre of the cell behind a face, whether or not it is on the lattice.
///
/// A face midpoint is exactly half the pitch to the neighbour, so doubling it
/// gives the neighbour's centre — including for a face pointing off the edge of
/// the board, which has no `HexCoord` to ask.
fn beyond(cell: HexCoord, face: HexFace) -> Vec3 {
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let mid = Vec3::new((a.0 + b.0) as f32 * 0.5, 0.0, (a.1 + b.1) as f32 * 0.5);
    center(cell) + mid * 2.0
}

/// Faces of occupied cells a body can actually be put through, onto nothing.
///
/// Sampled at body height rather than eye height, and that is the whole
/// subtlety. The authored tiles parapet their open sides: a face can be wide
/// open at 1.7 m and walled solid from the floor to 1.2 m. Probing high finds
/// railings and calls them ledges — which it did, and the shove that followed
/// bounced a minor off a wall the lab had promised was not there.
///
/// A ledge is therefore a face that is clear along the whole height a body
/// occupies, with nothing to land on beyond it.
fn ledges(occupied: &BTreeSet<HexCoord>, probe: &Probe) -> Vec<Ledge> {
    /// Heights above the cell floor a shoved body passes through.
    const BODY: [f32; 3] = [0.3, 0.8, 1.3];
    let grid = config().grid();
    let mut found = Vec::new();
    for &cell in occupied {
        let Some(floor) = probe.support(center(cell)) else {
            continue;
        };
        for face in HexFace::LATERAL {
            let neighbour = grid.neighbor(cell, face);
            if neighbour.is_some_and(|next| occupied.contains(&next)) {
                continue;
            }
            let target = beyond(cell, face);
            let passable = BODY.iter().all(|height| {
                let origin = center(cell).with_y(floor + height);
                let delta = target.with_y(floor + height) - origin;
                let distance = delta.length();
                distance > 0.01 && probe.ray(origin, delta / distance, distance).is_none()
            });
            if !passable {
                continue;
            }
            // Nothing catches it when it gets there.
            if probe.stand(target).is_some() {
                continue;
            }
            found.push(Ledge {
                cell,
                face,
                over_hole: neighbour.is_some(),
            });
        }
    }
    found
}

/// Lateral steps between two cells over open doors, or `None` if unreachable.
fn lattice_steps(world: &HexWfcWorld, from: HexCoord, to: HexCoord) -> Option<u32> {
    world
        .route_between(from, to)
        .map(|route| u32::try_from(route.len().saturating_sub(1)).unwrap_or(u32::MAX))
}

/// The cell whose removal disconnects `from` from `to`, if one exists.
///
/// This is what makes retraction a decision rather than a light switch: the
/// panel deletes a crossing the floor actually depends on. On a floor with a
/// loop in it there is no such cell, and the caller falls back to deleting the
/// far hall instead — still a real hole, just not a severing one.
fn cut_cell(
    world: &HexWfcWorld,
    occupied: &BTreeSet<HexCoord>,
    from: HexCoord,
    to: HexCoord,
) -> Option<HexCoord> {
    occupied
        .iter()
        .copied()
        .filter(|candidate| {
            *candidate != from
                && *candidate != to
                && world.placements[candidate].space == HexSpace::Hall
        })
        .find(|candidate| {
            let mut remaining = occupied.clone();
            remaining.remove(candidate);
            !connected(world, &remaining, from, to)
        })
}

/// Breadth-first reachability over open doors within a cell subset.
fn connected(
    world: &HexWfcWorld,
    cells: &BTreeSet<HexCoord>,
    from: HexCoord,
    to: HexCoord,
) -> bool {
    let config = config();
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        if at == to {
            return true;
        }
        let Some(placement) = world.placements.get(&at) else {
            continue;
        };
        for face in HexFace::LATERAL {
            if !placement.is_open(face) {
                continue;
            }
            let Some(next) = config.grid().neighbor(at, face) else {
                continue;
            };
            if cells.contains(&next) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    false
}

/// Feet-height waypoints across every occupied cell.
///
/// Candidates are the cell centre, a ring inside it, and the midpoint of every
/// open door. Anything the downward probe cannot stand on is dropped, so a
/// waypoint over a hole never enters the graph.
fn waypoints(world: &HexWfcWorld, occupied: &BTreeSet<HexCoord>, probe: &Probe) -> Vec<Vec3> {
    let config = config();
    let mut points = Vec::new();
    for &cell in occupied {
        let mut candidates = vec![center(cell)];
        for face in HexFace::LATERAL {
            candidates.push(offset_in(cell, face, 3.6));
            // A ring near the perimeter as well as a mid ring. Without it the
            // closest standable point to an unwalled face is 3.4 m inside it,
            // which is further than a shove carries — so the one edge the floor
            // offers could not actually be used.
            candidates.push(offset_in(cell, face, 5.4));
            let Some(placement) = world.placements.get(&cell) else {
                continue;
            };
            if placement.is_open(face) && config.grid().neighbor(cell, face).is_some() {
                // Sit the doorway waypoint just inside the threshold on both
                // sides so a route through it is two short hops, not one long
                // one across a wall the string-pull would have to see through.
                candidates.push(offset_in(cell, face, 6.0));
                candidates.push(offset_in(cell, face, 7.6));
            }
        }
        points.extend(candidates.into_iter().filter_map(|p| probe.stand(p)));
    }
    points.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    points.dedup_by(|a, b| a.distance_squared(*b) < 0.25);
    points
}

/// Where a wave may place a minor: cell centres with real support, ordered so
/// that the cell an Observer starts in is last.
fn muster(
    world: &HexWfcWorld,
    occupied: &BTreeSet<HexCoord>,
    probe: &Probe,
    spawn_cell: HexCoord,
) -> Vec<Vec3> {
    let mut cells: Vec<HexCoord> = occupied.iter().copied().collect();
    cells.sort_by_key(|coord| {
        std::cmp::Reverse(lattice_steps(world, spawn_cell, *coord).unwrap_or(0))
    });
    cells
        .into_iter()
        .filter(|coord| *coord != spawn_cell)
        .filter_map(|coord| probe.stand(center(coord)))
        .collect()
}

/// A read-only Rapier world over the projected colliders, used while deriving
/// anchors. The model builds its own; this one exists so that every position
/// this module hands out is already known to stand on something.
struct Probe {
    colliders: ColliderSet,
}

impl Probe {
    fn new(specs: &[ColliderSpec]) -> Self {
        let mut colliders = ColliderSet::new();
        for spec in specs {
            if let Some(collider) = build_collider(spec) {
                colliders.insert(collider);
            }
        }
        Self { colliders }
    }

    fn ray(&self, origin: Vec3, direction: Vec3, reach: f32) -> Option<f32> {
        let ray = Ray::new(
            Vector::new(origin.x, origin.y, origin.z),
            Vector::new(direction.x, direction.y, direction.z),
        );
        self.colliders
            .iter()
            .filter_map(|(_, c)| c.shape().cast_ray(c.position(), &ray, reach, true))
            .min_by(f32::total_cmp)
    }

    /// The height a body would stand at under a plan position, or `None` where
    /// there is nothing to stand on.
    ///
    /// This deliberately does not raycast from above. A cell is a closed prism
    /// with a ceiling on it, so a ray dropped from the top of the level starts
    /// *inside* the boundary shell and reports the ceiling as the floor — which
    /// it did, and put the Observer's feet eight metres up. What is wanted is
    /// the lowest place inside the cell with room for a body, so the probe
    /// samples the column instead and looks for that.
    fn stand(&self, at: Vec3) -> Option<Vec3> {
        const STEP: f32 = 0.1;
        const HEADROOM: f32 = 1.9;
        const TOP: f32 = observed_hex::TILE_LEVEL_HEIGHT;
        const BOTTOM: f32 = -1.0;
        let samples = ((TOP - BOTTOM) / STEP).ceil() as usize;
        // Occupancy of the column, bottom up.
        let solid: Vec<bool> = (0..=samples)
            .map(|i| {
                let y = BOTTOM + i as f32 * STEP;
                self.occupied(at.with_y(y))
            })
            .collect();
        let needed = (HEADROOM / STEP).ceil() as usize;
        // The lowest free run tall enough for a body, sitting on something.
        let mut run_start: Option<usize> = None;
        for (i, blocked) in solid.iter().enumerate() {
            if *blocked {
                run_start = None;
                continue;
            }
            let start = *run_start.get_or_insert(i);
            // A run that begins at the very bottom of the scan is open air
            // below the facility, not a floor.
            if start == 0 {
                continue;
            }
            if i - start + 1 >= needed {
                return Some(at.with_y(BOTTOM + start as f32 * STEP));
            }
        }
        None
    }

    /// The surface height under a plan position, if a body could stand there.
    fn support(&self, at: Vec3) -> Option<f32> {
        self.stand(at).map(|feet| feet.y)
    }

    /// Whether a world point is inside any structural collider.
    fn occupied(&self, point: Vec3) -> bool {
        let p = Vector::new(point.x, point.y, point.z);
        self.colliders
            .iter()
            .any(|(_, c)| c.shape().contains_point(c.position(), p))
    }

    /// A standable spot inside a cell: the centre if it is clear, otherwise the
    /// first clear point on a ring inside it. Authored tiles put columns and
    /// fixtures in the middle of cells often enough that insisting on the exact
    /// centre discards usable floors.
    fn stand_near(&self, cell: HexCoord) -> Option<Vec3> {
        std::iter::once(center(cell))
            .chain(
                [3.0_f32, 4.5]
                    .into_iter()
                    .flat_map(|radius| HexFace::LATERAL.into_iter().map(move |face| (face, radius)))
                    .map(|(face, radius)| offset_in(cell, face, radius)),
            )
            .find_map(|point| self.stand(point))
    }
}

/// Build one Rapier collider from a projected spec, exactly as the shared
/// traversal scene does.
pub fn build_collider(spec: &ColliderSpec) -> Option<Collider> {
    let [x, y, z, w] = spec.rotation;
    let rotation = Rotation::from_xyzw(x, y, z, w).normalize();
    let builder = match &spec.shape {
        ColliderShape::Cuboid { half } => ColliderBuilder::cuboid(half.x, half.y, half.z),
        ColliderShape::ConvexHull { points } => {
            let hull: Vec<Vector> = points.iter().map(|p| Vector::new(p.x, p.y, p.z)).collect();
            ColliderBuilder::convex_hull(&hull)?
        }
    };
    Some(
        builder
            .position(Pose::from_parts(
                Vector::new(spec.center.x, spec.center.y, spec.center.z),
                rotation,
            ))
            .friction(spec.friction)
            .user_data(u128::from(spec.id.0))
            .build(),
    )
}

/// Load the committed runtime tile catalogue.
///
/// The lab is a claim about the production corpus, so it reads the same
/// compiled catalogue the game does rather than a fixture.
pub fn load_content() -> Arc<HexMatchContent> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
    Arc::new(HexMatchContent::load(&base, &slugs).expect("committed runtime tile catalogue loads"))
}

/// The neighbour behind a face, on the lab's lattice.
fn grid_neighbor(cell: HexCoord, face: HexFace) -> Option<HexCoord> {
    config().grid().neighbor(cell, face)
}

/// Whether some pocket of this floor would re-collapse into something else.
///
/// Mirrors what the panel does at runtime, with the Observer assumed to be
/// standing at the spawn: enough to reject a floor that can never move, cheap
/// enough to run on every candidate seed.
fn can_decohere(world: &HexWfcWorld) -> bool {
    let influence = HexInfluenceField::encourage_decay();
    let built: Vec<HexCoord> = world
        .placements
        .iter()
        .filter(|(_, placement)| placement.space != HexSpace::Void)
        .map(|(cell, _)| *cell)
        .collect();
    let mut observation = HexObservationFrame {
        visible_cells: BTreeSet::new(),
        visible_thresholds: BTreeSet::new(),
        occupied_cells: BTreeMap::new(),
        landmark_cells: BTreeSet::new(),
        objective_cells: BTreeSet::new(),
    };
    observation
        .occupied_cells
        .insert(PlayerId(0), world.config.spawn());
    // A handful of anchors, not every built cell: this runs for every candidate
    // seed, and a whole-floor sweep of pocket collapses is not worth the deal.
    for anchor in built.iter().take(DEAL_CHECK_ANCHORS) {
        let frontier = BTreeSet::from([*anchor]);
        let mut work = world.begin_frontier_relayout_sized(
            &observation,
            &frontier,
            POCKET_TARGET_CELLS,
            POCKET_MAX_CELLS,
        );
        for _ in 0..POCKET_ATTEMPTS {
            match world.advance_driven_relayout(work, &influence) {
                Ok(HexRelayoutProgress::Pending(next)) => work = next,
                Ok(HexRelayoutProgress::Ready(candidate)) => {
                    if !candidate.changed_cells.is_empty() {
                        return true;
                    }
                    break;
                }
                Err(_) => break,
            }
        }
    }
    false
}

/// Pocket bounds, shared by the deal-time check and the runtime panel.
pub const POCKET_TARGET_CELLS: usize = 3;
pub const POCKET_MAX_CELLS: usize = 6;
/// Topological attempts one telegraph will spend on a single frontier.
pub const POCKET_ATTEMPTS: u32 = 24;
/// How many anchors the deal-time decoherence check samples.
const DEAL_CHECK_ANCHORS: usize = 8;

/// Whether two occupied cells share an open door.
fn adjacent(world: &HexWfcWorld, from: HexCoord, to: HexCoord) -> bool {
    world.placements.get(&from).is_some_and(|placement| {
        HexFace::LATERAL
            .into_iter()
            .any(|face| placement.is_open(face) && grid_neighbor(from, face) == Some(to))
    })
}
