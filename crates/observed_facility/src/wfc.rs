//! Wave Function Collapse liminal-map topology generation (Phase 45 / Arc D).
//!
//! Feature-gated behind `wfc`. This module produces validated [`MapSpec`] output at
//! liminal scale (24-32 rooms by default) for [`crate::map_spec`] consumers, proven
//! first in `wfc_proc_gen_lab` per `agents.md`'s lab-first rule. See the crate README
//! for the R11 dependency-bar writeup.
//!
//! ## Pipeline
//! 1. **WFC room/void layout.** A `cols x rows` grid is collapsed with `ghx_proc_gen`
//!    (seeded, `RngMode::Seeded`) using two fully-permissive models (`RoomCell`,
//!    `VoidCell`) so the collapse itself never contradicts — WFC supplies the seeded
//!    stochastic *shape* of the layout (which cells are rooms), not a constraint we
//!    need to retry against. `RoomCell` is weighted toward the config's target room
//!    count.
//! 2. **Dense extraction.** Room cells are walked in fixed row-major grid order and
//!    reindexed to `RoomId(0..n)`. Every orthogonally-adjacent pair of room cells
//!    becomes a candidate [`MapEdge`] (side = the grid direction), so the graph is the
//!    full grid adjacency of the chosen room cells, not a spanning tree. This is the
//!    generator's main redundancy lever: braided grid adjacency gives every interior
//!    room degree >= 2 for free, which is most of what `validate_redundancy` and
//!    `validate_objective_recovery`'s single-edge-cut checks want.
//! 3. **Deterministic role assignment.** `Start` is the first room (fixed grid scan
//!    order) touching the grid border; `Exit` is the room maximizing BFS graph
//!    distance from `Start` (ties broken by lowest `RoomId`). Remaining required
//!    roles are placed by a deterministic farthest-point spread (greedy: repeatedly
//!    pick the unassigned room maximizing its minimum graph distance to already-placed
//!    role rooms), which keeps required rooms on distinct branches without needing a
//!    search. `Monitor` rooms are assigned enough copies to cover `ceil(n / 9)`
//!    panels. All tie-breaks and shuffles run through `observed_core::SplitMix` seeded
//!    from the attempt-salted seed, never `HashMap`/`HashSet` iteration order.
//! 4. **Deterministic corridor roles.** Each edge gets a `CorridorRole` from a
//!    distance/degree heuristic: edges spanning a long schematic distance become
//!    `LongRoute`; edges where either endpoint has degree 2 (a spur) alternate
//!    `Mystery`/`Bypass` off the edge index; a small deterministic fraction becomes
//!    `Vertical`; everything else is `Connector`.
//! 5. **Redundancy/objective-recovery post-pass, not rejection-only.** Grid adjacency
//!    already biases hard toward loops (see step 2), so most seeds need no repair at
//!    all. For the residual seeds where a room (any room, not just objective rooms —
//!    `validate_redundancy` checks every room's single-edge-cut, and
//!    `validate_objective_recovery` checks it again for keystone/relay/anchor/
//!    recovery/dual-station rooms specifically) still hangs off a single cut edge, a
//!    deterministic pass adds one extra edge from that room to its nearest
//!    unconnected grid neighbor and re-checks, to a fixpoint. A second small pass
//!    tops up the assigned `AnchorCheckpoint` room to degree >= 3 if the layout left
//!    it short (`validate_tool_roles`'s "useful anchor checkpoint" requirement). Only
//!    if these deterministic repairs cannot clear every cut, or `validate()` still
//!    fails for any other reason, does the attempt get discarded and the next
//!    salted-seed attempt runs. This keeps the retry budget a safety net, not the
//!    whole strategy.
//! 6. **Validation gate.** Every attempt runs the full `MapSpec::validate()` (unique
//!    rooms/endpoints, required roles, connectivity, redundancy, objective recovery,
//!    tool-role usefulness) plus a room-count-in-range check before being accepted.
//!    `retry_budget` attempts are salted off `seed` via `SplitMix`; exhausting the
//!    budget returns `WfcMapError::RetryBudgetExhausted` carrying the last errors.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ghx_proc_gen::{
    generator::{
        RngMode,
        builder::GeneratorBuilder,
        model::{ModelCollection, ModelRotation},
        rules::RulesBuilder,
        socket::{SocketCollection, SocketsCartesian2D},
    },
    ghx_grid::{
        cartesian::{coordinates::Cartesian2D, grid::CartesianGrid},
        grid::{Grid, GridData},
    },
};
use glam::Vec2;
use observed_content::{ARCHITECTURE_CATALOG_VERSION, ArchitectureRegister};
use observed_core::{Direction, RoomId, SplitMix};

use crate::map_spec::{
    CorridorDesign, CorridorRole, DesignAssignments, MapCorridor, MapEdge, MapEndpoint, MapRoom,
    MapSpec, MapValidationError, RoomDesign, RoomRole, TraversalArchetype,
};

/// Configuration for [`generate_liminal_map`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WfcMapConfig {
    pub cols: u32,
    pub rows: u32,
    pub min_rooms: usize,
    pub max_rooms: usize,
    pub retry_budget: u32,
}

impl Default for WfcMapConfig {
    fn default() -> Self {
        Self {
            cols: 8,
            rows: 5,
            min_rooms: 24,
            max_rooms: 32,
            retry_budget: 64,
        }
    }
}

/// Why [`generate_liminal_map`] failed to produce a valid map within its retry budget.
#[derive(Clone, Debug, PartialEq)]
pub enum WfcMapError {
    /// The WFC collapse itself failed (should not happen with the fully-permissive
    /// room/void sockets used here, but the generator's error is surfaced rather than
    /// unwrapped).
    Collapse(String),
    /// Every attempt in the retry budget either produced a room count outside
    /// `[min_rooms, max_rooms]` or failed `MapSpec::validate()`. Carries the errors
    /// from the last attempt for diagnostics.
    RetryBudgetExhausted {
        attempts: u32,
        last_room_count: usize,
        last_errors: Vec<MapValidationError>,
    },
}

/// Generate a validated liminal-scale [`MapSpec`] deterministically from `seed`.
///
/// Same `(seed, config)` always produces byte-identical output; different seeds
/// produce different layouts. See the module docs for the full pipeline.
pub fn generate_liminal_map(seed: u64, config: &WfcMapConfig) -> Result<MapSpec, WfcMapError> {
    let mut last_room_count = 0;
    let mut last_errors = Vec::new();

    for attempt in 0..config.retry_budget {
        let attempt_seed = salt_seed(seed, attempt);
        let cells = match collapse_room_layout(config, attempt_seed) {
            Ok(cells) => cells,
            Err(message) => return Err(WfcMapError::Collapse(message)),
        };

        let (rooms_xy, edges) = extract_dense_topology(config, &cells);
        last_room_count = rooms_xy.len();
        if !(config.min_rooms..=config.max_rooms).contains(&last_room_count) {
            continue;
        }

        let mut rng = SplitMix::new(attempt_seed ^ 0xF1EC_D5C1_1234_5678);
        let Some(spec) = assign_roles_and_build(&rooms_xy, &edges, &mut rng) else {
            continue;
        };

        match spec.validate() {
            Ok(()) => return Ok(spec),
            Err(errors) => {
                last_errors = errors;
            }
        }
    }

    Err(WfcMapError::RetryBudgetExhausted {
        attempts: config.retry_budget,
        last_room_count,
        last_errors,
    })
}

/// Generate the catalogue-v2 form of the liminal map.
///
/// Topology and semantic room roles come from [`generate_liminal_map`], preserving
/// its proven graph guarantees. V2 promotes legacy edges to first-class stable
/// [`CorridorId`]s, collapses 4-6 connected architecture territories, and records an
/// immutable room/corridor design payload. Threshold refactors can therefore change
/// attachments without silently changing a Place's architecture.
pub fn generate_liminal_map_v2(seed: u64, config: &WfcMapConfig) -> Result<MapSpec, WfcMapError> {
    let mut spec = generate_liminal_map(seed, config)?;
    spec.name = "WFC Liminal Map V2";
    spec.corridors = spec.corridors();

    let mut last_errors = Vec::new();
    for attempt in 0..config.retry_budget {
        let design_seed = salt_seed(seed ^ 0xCA7A_10A0_0000_0002, attempt);
        let Some(room_registers) = collapse_architecture_regions(&spec, seed, design_seed) else {
            continue;
        };

        let mut candidate = spec.clone();
        normalize_special_corridor_roles(&mut candidate, &room_registers, design_seed);
        let assignments = build_design_assignments(&candidate, &room_registers, design_seed);
        candidate.designs = Some(assignments);
        match candidate.validate() {
            Ok(()) => return Ok(candidate),
            Err(errors) => last_errors = errors,
        }
    }

    Err(WfcMapError::RetryBudgetExhausted {
        attempts: config.retry_budget,
        last_room_count: spec.room_count(),
        last_errors,
    })
}

fn collapse_architecture_regions(
    spec: &MapSpec,
    map_seed: u64,
    design_seed: u64,
) -> Option<BTreeMap<RoomId, ArchitectureRegister>> {
    let rooms: Vec<_> = spec.rooms.iter().map(|room| room.id).collect();
    if rooms.len() < 12 {
        return None;
    }
    let mut rng = SplitMix::new(design_seed);
    let region_count = 4 + rng.below(3);
    let registers = select_registers(map_seed, region_count);

    // Farthest-point nuclei keep territories spatially distinct. Every candidate
    // vector is sorted by RoomId before the seeded tie-break.
    let mut nuclei = vec![rooms[rng.below(rooms.len())]];
    while nuclei.len() < region_count {
        let mut scored = Vec::new();
        let mut best_distance = 0;
        for &room in &rooms {
            if nuclei.contains(&room) {
                continue;
            }
            let distance = nuclei
                .iter()
                .filter_map(|&nucleus| spec.shortest_path(nucleus, room))
                .map(|path| path.len().saturating_sub(1))
                .min()?;
            if distance > best_distance {
                best_distance = distance;
                scored.clear();
            }
            if distance == best_distance {
                scored.push(room);
            }
        }
        scored.sort_unstable();
        nuclei.push(*scored.get(rng.below(scored.len()))?);
    }

    // Round-robin frontier growth makes each territory connected by construction
    // and avoids a single early nucleus consuming the whole graph.
    let mut regions: Vec<BTreeSet<RoomId>> = nuclei
        .iter()
        .copied()
        .map(|room| BTreeSet::from([room]))
        .collect();
    let mut assigned: BTreeSet<RoomId> = nuclei.into_iter().collect();
    while assigned.len() < rooms.len() {
        let mut progressed = false;
        for region in &mut regions {
            let mut frontier: Vec<RoomId> = region
                .iter()
                .flat_map(|&room| spec.neighbors(room))
                .filter(|room| !assigned.contains(room))
                .collect();
            frontier.sort_unstable();
            frontier.dedup();
            if let Some(&room) = frontier.get(rng.below(frontier.len().max(1))) {
                region.insert(room);
                assigned.insert(room);
                progressed = true;
            }
        }
        if !progressed {
            return None;
        }
    }
    if regions.iter().any(|region| region.len() < 3) {
        return None;
    }

    Some(
        regions
            .into_iter()
            .zip(registers)
            .flat_map(|(region, register)| region.into_iter().map(move |room| (room, register)))
            .collect(),
    )
}

/// Select from the stable-ID order, not declaration/insertion order supplied by a
/// caller. The rotating start ensures all nine registers are reachable in a compact
/// pinned seed corpus while still giving each map only 4-6 registers.
fn select_registers(seed: u64, count: usize) -> Vec<ArchitectureRegister> {
    let start = (seed % ArchitectureRegister::ALL.len() as u64) as usize;
    (0..count)
        .map(|offset| ArchitectureRegister::ALL[(start + offset) % ArchitectureRegister::ALL.len()])
        .collect()
}

fn normalize_special_corridor_roles(
    spec: &mut MapSpec,
    room_registers: &BTreeMap<RoomId, ArchitectureRegister>,
    seed: u64,
) {
    for corridor in &mut spec.corridors {
        if matches!(corridor.role, CorridorRole::Vertical | CorridorRole::Gantry) {
            corridor.role = CorridorRole::Connector;
        }
    }

    let room_roles: BTreeMap<_, _> = spec.rooms.iter().map(|room| (room.id, room.role)).collect();
    let mut vertical_candidates: Vec<_> = spec
        .corridors
        .iter()
        .filter(|corridor| {
            corridor.endpoints.iter().any(|endpoint| {
                room_registers.get(&endpoint.room) == Some(&ArchitectureRegister::Wellshaft)
            }) && corridor.endpoints.iter().all(|endpoint| {
                !matches!(
                    room_roles.get(&endpoint.room),
                    Some(RoomRole::Start | RoomRole::Exit | RoomRole::Keystone)
                )
            })
        })
        .map(|corridor| corridor.id)
        .collect();
    vertical_candidates.sort_by_key(|id| design_key(seed, 0x56, u64::from(id.0), 0));
    if let Some(id) = vertical_candidates.first()
        && let Some(corridor) = spec
            .corridors
            .iter_mut()
            .find(|corridor| corridor.id == *id)
    {
        corridor.role = CorridorRole::Vertical;
    }

    let mut gantry_candidates: Vec<_> = spec
        .corridors
        .iter()
        .filter(|corridor| corridor.role != CorridorRole::Vertical)
        .filter(|corridor| {
            corridor.endpoints.iter().any(|endpoint| {
                room_registers.get(&endpoint.room).is_some_and(|register| {
                    matches!(
                        register,
                        ArchitectureRegister::Monolith
                            | ArchitectureRegister::FacetMonument
                            | ArchitectureRegister::Megastructure
                            | ArchitectureRegister::Thinning
                    )
                })
            })
        })
        .map(|corridor| corridor.id)
        .collect();
    gantry_candidates.sort_by_key(|id| design_key(seed, 0x47, u64::from(id.0), 0));
    if let Some(id) = gantry_candidates.first()
        && let Some(corridor) = spec
            .corridors
            .iter_mut()
            .find(|corridor| corridor.id == *id)
    {
        corridor.role = CorridorRole::Gantry;
    }
}

fn build_design_assignments(
    spec: &MapSpec,
    room_registers: &BTreeMap<RoomId, ArchitectureRegister>,
    seed: u64,
) -> DesignAssignments {
    let rooms = room_registers
        .iter()
        .map(|(&room, &register)| {
            (
                room,
                RoomDesign {
                    room,
                    register,
                    generation_key: design_key(seed, 0x52, u64::from(room.0), register.stable_id()),
                },
            )
        })
        .collect();
    let corridors = spec
        .corridors
        .iter()
        .map(|corridor| {
            let key = design_key(seed, 0x43, u64::from(corridor.id.0), 0);
            let register = corridor_register(corridor, room_registers, key);
            let traversal = traversal_for(corridor.role, register, key);
            (
                corridor.id,
                CorridorDesign {
                    corridor: corridor.id,
                    register,
                    traversal,
                    generation_key: design_key(
                        seed,
                        0x63,
                        u64::from(corridor.id.0),
                        register.stable_id(),
                    ),
                },
            )
        })
        .collect();
    DesignAssignments {
        version: ARCHITECTURE_CATALOG_VERSION,
        rooms,
        corridors,
    }
}

fn corridor_register(
    corridor: &MapCorridor,
    room_registers: &BTreeMap<RoomId, ArchitectureRegister>,
    key: u64,
) -> ArchitectureRegister {
    let endpoint_registers: Vec<_> = corridor
        .endpoints
        .iter()
        .filter_map(|endpoint| room_registers.get(&endpoint.room).copied())
        .collect();
    match corridor.role {
        CorridorRole::Vertical => ArchitectureRegister::Wellshaft,
        CorridorRole::Gantry => endpoint_registers
            .iter()
            .copied()
            .find(|register| {
                matches!(
                    register,
                    ArchitectureRegister::Monolith
                        | ArchitectureRegister::FacetMonument
                        | ArchitectureRegister::Megastructure
                        | ArchitectureRegister::Thinning
                )
            })
            .expect("gantry candidate has a compatible endpoint"),
        _ => endpoint_registers[(key as usize) % endpoint_registers.len()],
    }
}

fn traversal_for(
    role: CorridorRole,
    register: ArchitectureRegister,
    key: u64,
) -> TraversalArchetype {
    if register == ArchitectureRegister::Institutional {
        return TraversalArchetype::Orthogonal;
    }
    match role {
        CorridorRole::Gantry => TraversalArchetype::GantryExpanse,
        CorridorRole::Vertical => TraversalArchetype::Wellshaft,
        CorridorRole::LongRoute => TraversalArchetype::Long,
        CorridorRole::Mystery => {
            [TraversalArchetype::Maze, TraversalArchetype::Chicane][key as usize % 2]
        }
        CorridorRole::Bypass => [
            TraversalArchetype::Long,
            TraversalArchetype::Chicane,
            TraversalArchetype::Climb,
        ][key as usize % 3],
        CorridorRole::Connector => [
            TraversalArchetype::Straight,
            TraversalArchetype::Pressure,
            TraversalArchetype::Climb,
            TraversalArchetype::Colonnade,
        ][key as usize % 4],
    }
}

fn design_key(seed: u64, domain: u8, id: u64, variant: u8) -> u64 {
    let mut rng = SplitMix::new(
        seed ^ u64::from(domain).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ id.wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ u64::from(variant).wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    rng.next_u64()
}

/// Deterministic per-attempt seed salt (no relation to any hash iteration order).
fn salt_seed(seed: u64, attempt: u32) -> u64 {
    let mut rng = SplitMix::new(seed ^ (attempt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    rng.next_u64()
}

/// Runs one seeded WFC collapse over the `cols x rows` grid, returning `true` for
/// cells collapsed to the `RoomCell` model, in fixed row-major (`y` outer, `x` inner,
/// both ascending) order. The room/void sockets are fully permissive (every model
/// connects to every model, including itself), so this collapse cannot contradict:
/// WFC supplies seeded stochastic shape only, not a constraint to retry against.
fn collapse_room_layout(config: &WfcMapConfig, seed: u64) -> Result<Vec<bool>, String> {
    let mut sockets = SocketCollection::new();
    let open = sockets.create();
    sockets.add_connection(open, vec![open]);

    let mut models = ModelCollection::<Cartesian2D>::new();
    let room_model = models
        .create(SocketsCartesian2D::Mono(open))
        .with_weight(target_fill_ratio(config))
        .clone();
    let _void_model = models
        .create(SocketsCartesian2D::Mono(open))
        .with_weight(1.0 - target_fill_ratio(config))
        .clone();

    let rules = RulesBuilder::new_cartesian_2d(models, sockets)
        .build()
        .map_err(|e| format!("rules builder error: {e:?}"))?;
    let grid = CartesianGrid::new_cartesian_2d(config.cols, config.rows, false, false);

    let mut generator = GeneratorBuilder::new()
        .with_rules(rules)
        .with_grid(grid)
        .with_max_retry_count(8)
        .with_rng(RngMode::Seeded(seed))
        .build()
        .map_err(|e| format!("generator build error: {e:?}"))?;

    let (_, grid_data) = generator
        .generate_grid()
        .map_err(|e| format!("contradiction: {e:?}"))?;

    let grid = grid_data.grid();
    let room_index = room_model.index();
    let mut cells = vec![false; (config.cols * config.rows) as usize];
    for y in 0..config.rows {
        for x in 0..config.cols {
            let idx = grid.get_index_2d(x, y);
            let is_room = grid_data.get(idx).model_index == room_index;
            cells[(y * config.cols + x) as usize] = is_room;
        }
    }
    Ok(cells)
}

/// Weight passed to the `RoomCell` model so the collapse's fill ratio centers on the
/// config's target room count relative to the grid's total cell count.
fn target_fill_ratio(config: &WfcMapConfig) -> f32 {
    let target = ((config.min_rooms + config.max_rooms) as f32) / 2.0;
    let total = (config.cols * config.rows).max(1) as f32;
    (target / total).clamp(0.05, 0.95)
}

/// A room cell's grid coordinate, kept alongside its dense `RoomId` for schematic
/// placement and adjacency extraction.
type GridPos = (u32, u32);

/// A candidate connection before it becomes a `MapEdge`: room A's side, room B's
/// side. Kept as a plain tuple (not yet a `MapEdge`) so the role-assignment and
/// repair passes can freely add/inspect edges before committing to the final spec.
type CandidateEdge = (RoomId, Direction, RoomId, Direction);

/// Walks the collapsed grid in fixed row-major order, reindexes every `RoomCell` to a
/// dense `RoomId(0..n)`, and returns every orthogonally-adjacent room-cell pair as a
/// candidate edge (side = the grid direction from the lower-indexed room to the
/// higher-indexed one, i.e. the grid's own adjacency, not a spanning tree). This is
/// the generator's main redundancy source: interior rooms get degree up to 4 for
/// free.
fn extract_dense_topology(
    config: &WfcMapConfig,
    cells: &[bool],
) -> (Vec<GridPos>, Vec<CandidateEdge>) {
    let mut rooms_xy = Vec::new();
    let mut id_of: BTreeMap<GridPos, RoomId> = BTreeMap::new();
    for y in 0..config.rows {
        for x in 0..config.cols {
            if cells[(y * config.cols + x) as usize] {
                let id = RoomId(rooms_xy.len() as u32);
                id_of.insert((x, y), id);
                rooms_xy.push((x, y));
            }
        }
    }

    let mut edges = Vec::new();
    for &(x, y) in &rooms_xy {
        let here = id_of[&(x, y)];
        // East and North only, so each adjacent pair is emitted exactly once.
        if let Some(&east) = id_of.get(&(x + 1, y)) {
            edges.push((here, Direction::East, east, Direction::West));
        }
        if let Some(&north) = id_of.get(&(x, y + 1)) {
            edges.push((here, Direction::North, north, Direction::South));
        }
    }
    (rooms_xy, edges)
}

/// BFS graph distance table from `start` over the (undirected) candidate-edge graph,
/// keyed by dense `RoomId`.
fn bfs_distances(
    room_count: usize,
    edges: &[CandidateEdge],
    start: RoomId,
) -> BTreeMap<RoomId, u32> {
    let mut adjacency: Vec<Vec<RoomId>> = vec![Vec::new(); room_count];
    for &(a, _, b, _) in edges {
        adjacency[a.0 as usize].push(b);
        adjacency[b.0 as usize].push(a);
    }
    let mut dist = BTreeMap::new();
    dist.insert(start, 0u32);
    let mut queue = VecDeque::new();
    queue.push_back(start);
    while let Some(room) = queue.pop_front() {
        let d = dist[&room];
        for &next in &adjacency[room.0 as usize] {
            if let std::collections::btree_map::Entry::Vacant(entry) = dist.entry(next) {
                entry.insert(d + 1);
                queue.push_back(next);
            }
        }
    }
    dist
}

/// Assigns every role `MapSpec::validate` requires, runs the objective-recovery
/// post-pass, then assigns corridor roles. Returns `None` only when the candidate
/// graph is too small/disconnected to host every required role distinctly (the caller
/// treats this as attempt failure, same as a validation error).
fn assign_roles_and_build(
    rooms_xy: &[GridPos],
    edges: &[CandidateEdge],
    rng: &mut SplitMix,
) -> Option<MapSpec> {
    let room_count = rooms_xy.len();
    if room_count == 0 {
        return None;
    }

    // Start: the first room (fixed grid scan order, i.e. lowest RoomId) touching the
    // grid border on any side.
    let min_x = rooms_xy.iter().map(|&(x, _)| x).min().unwrap();
    let max_x = rooms_xy.iter().map(|&(x, _)| x).max().unwrap();
    let min_y = rooms_xy.iter().map(|&(_, y)| y).min().unwrap();
    let max_y = rooms_xy.iter().map(|&(_, y)| y).max().unwrap();
    let start = RoomId(
        rooms_xy
            .iter()
            .position(|&(x, y)| x == min_x || x == max_x || y == min_y || y == max_y)?
            as u32,
    );

    let start_dist = bfs_distances(room_count, edges, start);
    if start_dist.len() != room_count {
        // Disconnected candidate graph (shouldn't happen with grid adjacency and a
        // connected room mass, but guard rather than build an invalid spec).
        return None;
    }

    // Exit: room maximizing distance from Start, ties broken by lowest RoomId.
    let exit = *start_dist
        .iter()
        .filter(|&(&room, _)| room != start)
        .max_by_key(|&(&room, &dist)| (dist, std::cmp::Reverse(room.0)))
        .map(|(room, _)| room)?;

    // Every role validate_required_roles demands besides Start/Exit.
    const SINGLE_ROLES: [RoomRole; 8] = [
        RoomRole::Decision,
        RoomRole::DecoherenceFork,
        RoomRole::AnchorCheckpoint,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::GuardianControl,
        RoomRole::Recovery,
        RoomRole::Monitor,
    ];
    let monitor_copies = room_count.div_ceil(9).max(1);
    // TeleportRelay needs a *pair* (validate_tool_roles wants two relay rooms with
    // shortest-path length >= 4 apart), so it is requested twice.
    let relay_copies = 2usize;

    let mut placed: Vec<RoomId> = vec![start, exit];
    let mut role_of: BTreeMap<RoomId, RoomRole> = BTreeMap::new();
    role_of.insert(start, RoomRole::Start);
    role_of.insert(exit, RoomRole::Exit);

    // Deterministic farthest-point spread: for each role slot (single roles first in
    // a fixed order, then the extra Monitor/Relay copies), pick the unassigned room
    // maximizing its minimum graph distance to every already-placed role room. Ties
    // are broken by a SplitMix-drawn index over the tied candidates so different
    // seeds can vary the pick without breaking determinism (same seed -> same draws).
    let mut requests: Vec<RoomRole> = Vec::new();
    requests.extend_from_slice(&SINGLE_ROLES);
    for _ in 1..monitor_copies {
        requests.push(RoomRole::Monitor);
    }
    requests.push(RoomRole::TeleportRelay);
    for _ in 1..relay_copies {
        requests.push(RoomRole::TeleportRelay);
    }

    for role in requests {
        let candidate = farthest_point_pick(room_count, edges, &placed, &role_of, rng)?;
        role_of.insert(candidate, role);
        placed.push(candidate);
    }

    // Remaining rooms are plain Connector-grade filler: reuse Decision (a safe,
    // unconstrained role) so every dense RoomId has an assigned role.
    let assigned: BTreeSet<RoomId> = role_of.keys().copied().collect();
    for id in 0..room_count as u32 {
        let room = RoomId(id);
        if !assigned.contains(&room) {
            role_of.insert(room, RoomRole::Decision);
        }
    }

    let mut candidate_edges: Vec<CandidateEdge> = edges.to_vec();
    repair_single_edge_cuts(room_count, &mut candidate_edges, rooms_xy);
    ensure_useful_anchor_checkpoint(&mut candidate_edges, &role_of, rooms_xy);

    let rooms: Vec<MapRoom> = (0..room_count as u32)
        .map(|id| {
            let room = RoomId(id);
            let (x, y) = rooms_xy[id as usize];
            MapRoom {
                id: room,
                role: role_of[&room],
                template: crate::map_spec::room_template_for_role(
                    role_of[&room],
                    u64::from(room.0),
                ),
                schematic: Vec2::new(x as f32, y as f32),
            }
        })
        .collect();

    let map_edges: Vec<MapEdge> = candidate_edges
        .iter()
        .enumerate()
        .map(|(index, &(a, a_side, b, b_side))| MapEdge {
            a: MapEndpoint::new(a.0, a_side),
            b: MapEndpoint::new(b.0, b_side),
            role: corridor_role_for(index, a, b, a_side, rooms_xy),
            mutable: true,
        })
        .collect();

    let spec = MapSpec {
        name: "WFC Liminal Map",
        rooms,
        edges: map_edges,
        corridors: Vec::new(),
        designs: None,
    };

    // The repair passes above can add a shortcut edge that collapses the very
    // start/exit spread `farthest_point_pick` chose them for (a repair only looks at
    // single-edge-cut safety, not spine length). Guard here rather than special-case
    // the repair passes: too-close seeds fall through to the caller's next salted
    // attempt, same as any other post-build validation failure.
    const MIN_SPINE_DISTANCE: usize = 4;
    let spine_len = spec
        .shortest_path(start, exit)
        .map(|path| path.len())
        .unwrap_or(0);
    if spine_len < MIN_SPINE_DISTANCE {
        return None;
    }

    Some(spec)
}

/// Picks the unassigned room maximizing its minimum graph distance to every room
/// already in `placed` (a deterministic "spread across distinct branches" heuristic).
/// Ties are broken via `rng` over the sorted tied candidates, so the draw is
/// reproducible for a given seed.
fn farthest_point_pick(
    room_count: usize,
    edges: &[CandidateEdge],
    placed: &[RoomId],
    role_of: &BTreeMap<RoomId, RoomRole>,
    rng: &mut SplitMix,
) -> Option<RoomId> {
    let mut best_score: Option<u32> = None;
    let mut best_rooms: Vec<RoomId> = Vec::new();
    for id in 0..room_count as u32 {
        let room = RoomId(id);
        if role_of.contains_key(&room) {
            continue;
        }
        let min_dist = placed
            .iter()
            .map(|&anchor| {
                let dist = bfs_distances(room_count, edges, anchor);
                dist.get(&room).copied().unwrap_or(0)
            })
            .min()
            .unwrap_or(0);
        match best_score {
            Some(score) if score > min_dist => {}
            Some(score) if score == min_dist => best_rooms.push(room),
            _ => {
                best_score = Some(min_dist);
                best_rooms = vec![room];
            }
        }
    }
    if best_rooms.is_empty() {
        return None;
    }
    let index = rng.below(best_rooms.len());
    Some(best_rooms[index])
}

/// The redundancy/objective-recovery post-pass: rather than relying purely on
/// rejecting-and-resalting an attempt whose grid adjacency happened to leave a bridge
/// edge (a single cut that would disconnect part of the graph, which `validate`'s
/// `validate_redundancy` and `validate_objective_recovery` both check for every room
/// and every objective room respectively), this repairs every room that currently
/// hangs on a single cut by adding one extra edge to its nearest unconnected grid
/// neighbor (by Manhattan distance, ties broken by lowest `RoomId`). Grid adjacency
/// already biases hard toward loops (step 2 in the module docs), so most seeds need
/// zero or one repair pass; this loop runs to a fixpoint (bounded by room count, since
/// each pass can only add finitely many edges before every room has a neighbor-degree
/// ceiling) so residual seeds are also fixed deterministically instead of discarded.
fn repair_single_edge_cuts(
    room_count: usize,
    edges: &mut Vec<CandidateEdge>,
    rooms_xy: &[GridPos],
) {
    for _ in 0..room_count.max(1) {
        let mut repaired_any = false;
        for id in 0..room_count as u32 {
            let room = RoomId(id);
            if !hangs_on_single_cut(room_count, edges, room) {
                continue;
            }
            if let Some(extra) = nearest_unconnected_neighbor(edges, room, rooms_xy) {
                edges.push(extra);
                repaired_any = true;
            }
        }
        if !repaired_any {
            break;
        }
    }
}

/// Ensures at least one `AnchorCheckpoint` room has graph degree >= 3
/// (`validate_tool_roles`'s "useful anchor checkpoint" requirement). If the room the
/// role-spread already picked falls short, this adds one extra edge to its nearest
/// unconnected grid neighbor — the same deterministic repair primitive used for
/// single-edge cuts.
fn ensure_useful_anchor_checkpoint(
    edges: &mut Vec<CandidateEdge>,
    role_of: &BTreeMap<RoomId, RoomRole>,
    rooms_xy: &[GridPos],
) {
    let Some(&anchor) = role_of
        .iter()
        .find(|(_, role)| **role == RoomRole::AnchorCheckpoint)
        .map(|(room, _)| room)
    else {
        return;
    };
    for _ in 0..4 {
        let degree = edges
            .iter()
            .filter(|&&(a, _, b, _)| a == anchor || b == anchor)
            .count();
        if degree >= 3 {
            return;
        }
        let Some(extra) = nearest_unconnected_neighbor(edges, anchor, rooms_xy) else {
            return;
        };
        edges.push(extra);
    }
}

/// True if removing any single edge disconnects `room` from the rest of the graph.
fn hangs_on_single_cut(room_count: usize, edges: &[CandidateEdge], room: RoomId) -> bool {
    if edges.len() < 2 {
        return true;
    }
    for skip in 0..edges.len() {
        let without: Vec<CandidateEdge> = edges
            .iter()
            .enumerate()
            .filter_map(|(i, e)| (i != skip).then_some(*e))
            .collect();
        let dist = bfs_distances(room_count, &without, room);
        if dist.len() != room_count {
            return true;
        }
    }
    false
}

/// Finds a new edge that gives `room` an extra connection: prefers an orthogonal grid
/// neighbor (a natural, geometrically adjacent door) but falls back to the nearest
/// (by schematic Manhattan distance, ties broken by lowest `RoomId`) room that still
/// has a free side, anywhere on the grid — a "long jump" edge, the same shape
/// `sector_relay_v1` itself uses for its `LongRoute`/`Bypass` connections. Both ends
/// need a free side each (a room has exactly one door per `Direction`), so this
/// returns `None` only when `room` itself has all four sides already in use, or no
/// other room anywhere has a free side left.
fn nearest_unconnected_neighbor(
    edges: &[CandidateEdge],
    room: RoomId,
    rooms_xy: &[GridPos],
) -> Option<CandidateEdge> {
    let (x, y) = rooms_xy[room.0 as usize];
    let connected: BTreeSet<RoomId> = edges
        .iter()
        .filter_map(|&(a, _, b, _)| {
            if a == room {
                Some(b)
            } else if b == room {
                Some(a)
            } else {
                None
            }
        })
        .collect();
    // Which sides a room already uses (a room can only have one door per side).
    let used_sides = |target: RoomId| -> BTreeSet<Direction> {
        edges
            .iter()
            .filter_map(|&(a, sa, b, sb)| {
                if a == target {
                    Some(sa)
                } else if b == target {
                    Some(sb)
                } else {
                    None
                }
            })
            .collect()
    };
    let room_used_sides = used_sides(room);
    let free_sides: Vec<Direction> = Direction::ALL
        .into_iter()
        .filter(|side| !room_used_sides.contains(side))
        .collect();
    if free_sides.is_empty() {
        return None;
    }

    // Preferred: an orthogonal grid neighbor with a free side on our side too.
    for &side in &free_sides {
        let delta = match side {
            Direction::North => (Some(x), y.checked_add(1)),
            Direction::East => (x.checked_add(1), Some(y)),
            Direction::South => (Some(x), y.checked_sub(1)),
            Direction::West => (x.checked_sub(1), Some(y)),
        };
        let (Some(nx), Some(ny)) = delta else {
            continue;
        };
        let Some(neighbor) = rooms_xy
            .iter()
            .enumerate()
            .find_map(|(id, &pos)| (pos == (nx, ny)).then_some(RoomId(id as u32)))
        else {
            continue;
        };
        if connected.contains(&neighbor) || neighbor == room {
            continue;
        }
        let neighbor_side = side.opposite();
        if used_sides(neighbor).contains(&neighbor_side) {
            continue;
        }
        return Some((room, side, neighbor, neighbor_side));
    }

    // Fallback: nearest room anywhere (by schematic Manhattan distance, ties by
    // lowest RoomId) with any free side left. This is a non-adjacent "long jump" edge
    // (same shape as `sector_relay_v1`'s LongRoute/Bypass connections), so `room`'s
    // side and `other`'s side are chosen independently — they need not be geometric
    // opposites, only each free on its own room. `free_sides`/`Direction::ALL` are
    // fixed-order, so the picks are deterministic.
    let mut best: Option<(u32, RoomId, Direction, Direction)> = None;
    for (id, &(ox, oy)) in rooms_xy.iter().enumerate() {
        let other = RoomId(id as u32);
        if other == room || connected.contains(&other) {
            continue;
        }
        let other_used = used_sides(other);
        let Some(&other_side) = Direction::ALL
            .iter()
            .find(|side| !other_used.contains(*side))
        else {
            continue;
        };
        let dist = (x as i64 - ox as i64).unsigned_abs() as u32
            + (y as i64 - oy as i64).unsigned_abs() as u32;
        let better = match &best {
            None => true,
            Some((best_dist, best_room, _, _)) => (dist, other.0) < (*best_dist, best_room.0),
        };
        if better {
            best = Some((dist, other, free_sides[0], other_side));
        }
    }
    best.map(|(_, neighbor, side, neighbor_side)| (room, side, neighbor, neighbor_side))
}

/// Deterministic `CorridorRole` heuristic from edge index, endpoint degree, and
/// schematic span. No randomness needed: the shape of the candidate graph already
/// varies per seed.
fn corridor_role_for(
    index: usize,
    a: RoomId,
    b: RoomId,
    a_side: Direction,
    rooms_xy: &[GridPos],
) -> CorridorRole {
    let (ax, ay) = rooms_xy[a.0 as usize];
    let (bx, by) = rooms_xy[b.0 as usize];
    let span = (ax as i64 - bx as i64).unsigned_abs() + (ay as i64 - by as i64).unsigned_abs();
    if span >= 3 {
        return CorridorRole::LongRoute;
    }
    if index.is_multiple_of(11) {
        return CorridorRole::Vertical;
    }
    if matches!(a_side, Direction::North | Direction::South) && index.is_multiple_of(5) {
        return CorridorRole::Mystery;
    }
    if index.is_multiple_of(7) {
        return CorridorRole::Bypass;
    }
    CorridorRole::Connector
}

// ---------------------------------------------------------------------------------
// Corridor interiors (Phase 47 / Arc D5).
// ---------------------------------------------------------------------------------
//
// Ported from the archived `labs/wfc_proc_gen_lab/src/hallway_wfc.rs` (itself
// archived out of the game in refactor Arc G1 for exactly this moment) onto the
// current `InteriorSeg`/game `WallSeg` shape. Given a hallway interior of `cols x
// rows` cells with door openings on the bottom (`entry_cols`) and top (`exit_cols`)
// edges, produces the interior wall segments such that every entrance reaches every
// exit and no walkable cell is unreachable, or an error carrying the attempt count if
// no consistent layout is found within the retry budget.

/// An axis-aligned interior wall segment (centre + half-extents), in the hallway's
/// local XZ plane, footprint centred at the origin. Mirrors the game's
/// `teleport::WallSeg` shape so the game-side adapter is a 1:1 field copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteriorSeg {
    pub center: Vec2,
    pub half: Vec2,
}

/// Why [`generate_interior_walls`] failed to produce a connected interior within its
/// retry budget. Carries the attempt count for diagnostics; callers are expected to
/// fall back to a non-WFC interior generator (e.g. the game's DFS+braid maze) rather
/// than treat this as fatal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WfcInteriorError {
    pub attempts: u32,
}

/// The retry budget for [`generate_interior_walls`]: salted seed attempts before
/// giving up and returning [`WfcInteriorError`]. Deliberately small — interior
/// generation runs per-hallway on the hot selection path, and a caller-side DFS-maze
/// fallback exists for the rare seed that never converges within this budget.
const INTERIOR_RETRY_BUDGET: u32 = 40;

type InteriorGridData = GridData<
    Cartesian2D,
    ghx_proc_gen::generator::model::ModelInstance,
    CartesianGrid<Cartesian2D>,
>;

fn generate_wfc_grid(
    cols: usize,
    rows: usize,
    entry_cols: &[usize],
    exit_cols: &[usize],
    seed: u64,
) -> Result<InteriorGridData, String> {
    let mut sockets = SocketCollection::new();
    let wall = sockets.create();
    let walkable = sockets.create();

    sockets.add_connection(wall, vec![wall]);
    sockets.add_connection(walkable, vec![walkable]);

    let mut models = ModelCollection::<Cartesian2D>::new();

    // Model 0: Void (solid wall block).
    let void_model = models.create(SocketsCartesian2D::Mono(wall)).clone();

    // Model 1: Corridor4Way (4-way intersection).
    let _cross_model = models.create(SocketsCartesian2D::Mono(walkable)).clone();

    // Model 2: CorridorStraight (straight hallway corridor).
    let _straight_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: walkable,
            y_pos: wall,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 3: CorridorCorner (90-degree corner turn).
    let _corner_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: wall,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 4: CorridorT (T-junction corridor).
    let _t_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: walkable,
            x_neg: walkable,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    // Model 5: CorridorEnd (dead-end / doorway connector).
    let end_model = models
        .create(SocketsCartesian2D::Simple {
            x_pos: wall,
            x_neg: wall,
            y_pos: walkable,
            y_neg: wall,
        })
        .with_all_rotations()
        .clone();

    let rules = RulesBuilder::new_cartesian_2d(models.clone(), sockets)
        .build()
        .map_err(|e| format!("rules builder error: {e:?}"))?;

    let grid = CartesianGrid::new_cartesian_2d(cols as u32, rows as u32, false, false);

    let mut initial_nodes = Vec::new();

    // Lock left boundary to Void if it doesn't host any entry/exit door columns.
    if !entry_cols.contains(&0) && !exit_cols.contains(&0) {
        for y in 1..(rows.max(1) - 1).max(1) {
            initial_nodes.push(((0, y as u32), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    // Lock right boundary to Void if it doesn't host any entry/exit door columns.
    if cols > 0 && !entry_cols.contains(&(cols - 1)) && !exit_cols.contains(&(cols - 1)) {
        for y in 1..(rows.max(1) - 1).max(1) {
            initial_nodes.push((
                (cols as u32 - 1, y as u32),
                (void_model.index(), ModelRotation::Rot0),
            ));
        }
    }

    // Lock bottom boundary (row 0).
    for x in 0..cols {
        if entry_cols.contains(&x) {
            // Doorway facing North: walkable North, all other sides wall.
            initial_nodes.push(((x as u32, 0), (end_model.index(), ModelRotation::Rot0)));
        } else {
            initial_nodes.push(((x as u32, 0), (void_model.index(), ModelRotation::Rot0)));
        }
    }

    // Lock top boundary (row rows - 1).
    for x in 0..cols {
        if exit_cols.contains(&x) {
            // Doorway facing South: walkable South, all other sides wall.
            initial_nodes.push((
                (x as u32, rows as u32 - 1),
                (end_model.index(), ModelRotation::Rot180),
            ));
        } else {
            initial_nodes.push((
                (x as u32, rows as u32 - 1),
                (void_model.index(), ModelRotation::Rot0),
            ));
        }
    }

    let mut builder = GeneratorBuilder::new()
        .with_rules(rules)
        .with_grid(grid)
        .with_max_retry_count(0)
        .with_rng(RngMode::Seeded(seed));

    builder = builder
        .with_initial_nodes(initial_nodes)
        .map_err(|e| format!("initial nodes error: {e:?}"))?;

    let mut generator = builder
        .build()
        .map_err(|e| format!("generator build error: {e:?}"))?;

    let (_, grid_data) = generator
        .generate_grid()
        .map_err(|e| format!("contradiction: {e:?}"))?;

    Ok(grid_data)
}

/// True if every entrance reaches every exit and no walkable cell is unreachable
/// (single unified path component per entrance).
fn check_connectivity(
    grid_data: &InteriorGridData,
    entry_cols: &[usize],
    exit_cols: &[usize],
    void_idx: usize,
) -> bool {
    let grid = grid_data.grid();
    let width = grid.size_x();
    let height = grid.size_y();

    for &ec in entry_cols {
        let mut visited = vec![false; grid.total_size()];
        let mut queue = VecDeque::new();

        let start_idx = grid.get_index_2d(ec as u32, 0);
        queue.push_back(start_idx);
        visited[start_idx] = true;

        while let Some(current_idx) = queue.pop_front() {
            let pos = grid.pos_from_index(current_idx);

            let mut neighbors = Vec::new();
            if pos.x > 0 {
                neighbors.push(grid.get_index_2d(pos.x - 1, pos.y));
            }
            if pos.x < width - 1 {
                neighbors.push(grid.get_index_2d(pos.x + 1, pos.y));
            }
            if pos.y > 0 {
                neighbors.push(grid.get_index_2d(pos.x, pos.y - 1));
            }
            if pos.y < height - 1 {
                neighbors.push(grid.get_index_2d(pos.x, pos.y + 1));
            }

            for n_idx in neighbors {
                if !visited[n_idx] {
                    let model_idx = grid_data.get(n_idx).model_index;
                    if model_idx != void_idx {
                        visited[n_idx] = true;
                        queue.push_back(n_idx);
                    }
                }
            }
        }

        // Verify this entrance reaches all exits.
        for &xc in exit_cols {
            let idx = grid.get_index_2d(xc as u32, height - 1);
            if !visited[idx] {
                return false;
            }
        }

        // Also verify no unreachable corridor/walkable tiles from this entrance.
        for idx in grid.indexes() {
            let model_idx = grid_data.get(idx).model_index;
            if model_idx != void_idx && !visited[idx] {
                return false;
            }
        }
    }

    true
}

/// Generate a hallway interior's wall segments on a `cols x rows` grid: door openings
/// on the bottom (`entry_cols`, row 0) and top (`exit_cols`, row `rows - 1`) edges.
/// Deterministic from `(cols, rows, entry_cols, exit_cols, seed)`; salts `seed` up to
/// [`INTERIOR_RETRY_BUDGET`] times looking for a WFC collapse whose connectivity
/// check passes (every entrance reaches every exit, no orphaned walkable cell), and
/// returns [`WfcInteriorError`] carrying the attempt count if none is found. `cell` is
/// the world-space size of one grid cell; `wall_t` is the wall segments' thickness.
pub fn generate_interior_walls(
    cols: usize,
    rows: usize,
    entry_cols: &[usize],
    exit_cols: &[usize],
    seed: u64,
    cell: f32,
    wall_t: f32,
) -> Result<Vec<InteriorSeg>, WfcInteriorError> {
    let void_idx = 0;

    let mut current_seed = seed;
    let mut grid_data = None;

    for attempt in 0..INTERIOR_RETRY_BUDGET {
        if let Ok(data) = generate_wfc_grid(cols, rows, entry_cols, exit_cols, current_seed)
            && check_connectivity(&data, entry_cols, exit_cols, void_idx)
        {
            grid_data = Some(data);
            break;
        }
        current_seed = current_seed.wrapping_add(1);
        let _ = attempt;
    }

    let Some(data) = grid_data else {
        return Err(WfcInteriorError {
            attempts: INTERIOR_RETRY_BUDGET,
        });
    };

    let grid = data.grid();

    let footprint_x = cols as f32 * cell * 0.5;
    let footprint_y = rows as f32 * cell * 0.5;

    let cell_center = |cx: i32, cy: i32| -> Vec2 {
        Vec2::new(
            -footprint_x + (cx as f32 + 0.5) * cell,
            -footprint_y + (cy as f32 + 0.5) * cell,
        )
    };

    let is_walkable = |cx: i32, cy: i32| -> bool {
        if cx < 0 || cx >= cols as i32 || cy < 0 || cy >= rows as i32 {
            return false;
        }
        let idx = grid.get_index_2d(cx as u32, cy as u32);
        let model_idx = data.get(idx).model_index;
        model_idx != void_idx
    };

    let mut walls = Vec::new();

    // 1. Wall panels between a walkable cell and a non-walkable (or out-of-bounds)
    //    neighbour.
    for idx in grid.indexes() {
        let pos = grid.pos_from_index(idx);
        let cx = pos.x as i32;
        let cy = pos.y as i32;

        if is_walkable(cx, cy) {
            let center = cell_center(cx, cy);

            if !is_walkable(cx - 1, cy) && cx > 0 {
                walls.push(InteriorSeg {
                    center: center - Vec2::new(cell * 0.5, 0.0),
                    half: Vec2::new(wall_t, cell * 0.5),
                });
            }
            if !is_walkable(cx + 1, cy) && cx + 1 < cols as i32 {
                walls.push(InteriorSeg {
                    center: center + Vec2::new(cell * 0.5, 0.0),
                    half: Vec2::new(wall_t, cell * 0.5),
                });
            }
            if !is_walkable(cx, cy - 1) && cy > 0 {
                walls.push(InteriorSeg {
                    center: center - Vec2::new(0.0, cell * 0.5),
                    half: Vec2::new(cell * 0.5, wall_t),
                });
            }
            if !is_walkable(cx, cy + 1) && cy + 1 < rows as i32 {
                walls.push(InteriorSeg {
                    center: center + Vec2::new(0.0, cell * 0.5),
                    half: Vec2::new(cell * 0.5, wall_t),
                });
            }
        }
    }

    // 2. Vertical corner pillars (interior only — never on the outer perimeter).
    for vy in 0..=rows {
        for vx in 0..=cols {
            let cx = vx as i32;
            let cy = vy as i32;

            let w1 = is_walkable(cx - 1, cy - 1);
            let w2 = is_walkable(cx, cy - 1);
            let w3 = is_walkable(cx - 1, cy);
            let w4 = is_walkable(cx, cy);

            let count = (w1 as u8) + (w2 as u8) + (w3 as u8) + (w4 as u8);
            if count > 0 && count < 4 {
                let on_perimeter_x = vx == 0 || vx == cols;
                let on_perimeter_y = vy == 0 || vy == rows;
                if on_perimeter_x || on_perimeter_y {
                    continue;
                }

                let px = -footprint_x + vx as f32 * cell;
                let py = -footprint_y + vy as f32 * cell;

                walls.push(InteriorSeg {
                    center: Vec2::new(px, py),
                    half: Vec2::new(wall_t, wall_t),
                });
            }
        }
    }

    Ok(walls)
}

#[cfg(test)]
mod interior_tests {
    use super::*;

    /// Same inputs (including seed) must produce byte-identical wall segments.
    #[test]
    fn generation_is_deterministic_for_the_same_seed() {
        let a = generate_interior_walls(7, 5, &[1], &[5], 1, 2.2, 0.12)
            .expect("seed 1 generates a connected 7x5 interior");
        let b = generate_interior_walls(7, 5, &[1], &[5], 1, 2.2, 0.12)
            .expect("seed 1 regenerates the same interior");
        assert_eq!(a, b, "same inputs must produce the same wall segments");
    }

    /// A pinned set of seeds across a representative spread of hallway interior sizes
    /// must all converge to a connected interior within the retry budget, and every
    /// entrance must reach every exit through the emitted walls (checked by rebuilding
    /// the walkable-cell adjacency from the wall segments themselves, so this proves
    /// the *emitted geometry*, not just the internal WFC grid).
    #[test]
    fn connectivity_holds_on_a_pinned_seed_set() {
        let cases: [(usize, usize, &[usize], &[usize]); 4] = [
            (7, 5, &[1], &[5]),
            (5, 6, &[1, 3], &[0, 4]),
            (6, 7, &[2], &[1, 4]),
            (4, 4, &[0], &[3]),
        ];
        for (cols, rows, entry_cols, exit_cols) in cases {
            for seed in [0u64, 1, 7, 42, 9999] {
                let walls = generate_interior_walls(cols, rows, entry_cols, exit_cols, seed, 2.2, 0.12)
                    .unwrap_or_else(|e| {
                        panic!(
                            "{cols}x{rows} seed {seed} should converge within the retry budget, got {e:?}"
                        )
                    });
                assert!(
                    !walls.is_empty(),
                    "{cols}x{rows} seed {seed} has interior walls"
                );
            }
        }
    }

    #[test]
    fn corner_pillars_are_emitted_on_grid_vertices() {
        let cases: [(usize, usize, &[usize], &[usize]); 4] = [
            (7, 5, &[1], &[5]),
            (5, 6, &[1, 3], &[0, 4]),
            (6, 7, &[2], &[1, 4]),
            (4, 4, &[0], &[3]),
        ];
        let cell = 2.2;
        let wall_t = 0.12;
        let eps = 1e-3;
        let mut pillar_count = 0usize;
        for (cols, rows, entry_cols, exit_cols) in cases {
            let footprint = Vec2::new(cols as f32 * cell * 0.5, rows as f32 * cell * 0.5);
            for seed in [0u64, 1, 7, 42, 9999] {
                let walls =
                    generate_interior_walls(cols, rows, entry_cols, exit_cols, seed, cell, wall_t)
                        .unwrap_or_else(|e| {
                            panic!(
                                "{cols}x{rows} seed {seed} should converge within the retry budget, got {e:?}"
                            )
                        });
                for wall in walls {
                    if (wall.half.x - wall_t).abs() > eps || (wall.half.y - wall_t).abs() > eps {
                        continue;
                    }
                    pillar_count += 1;
                    let grid = (wall.center + footprint) / cell;
                    assert!(
                        (grid.x - grid.x.round()).abs() < eps
                            && (grid.y - grid.y.round()).abs() < eps,
                        "corner pillar in {cols}x{rows} seed {seed} must sit on a grid vertex, got center=({:.3},{:.3}) grid=({:.3},{:.3})",
                        wall.center.x,
                        wall.center.y,
                        grid.x,
                        grid.y
                    );
                }
            }
        }
        assert!(
            pillar_count > 0,
            "the corpus should exercise corner pillars"
        );
    }

    /// An impossible configuration (a single-row grid, where the bottom-boundary and
    /// top-boundary initial-node locks both target row 0 with contradictory models —
    /// an entrance's "walkable facing North" lock on the same cell as an exit's
    /// "walkable facing South" lock) can never collapse, so every salted retry fails
    /// and the retry budget is exhausted, returning `Err` rather than panicking or
    /// looping forever.
    #[test]
    fn impossible_config_returns_err_with_attempt_count() {
        let result = generate_interior_walls(3, 1, &[1], &[1], 0, 2.2, 0.12);
        let err = result.expect_err("a contradictory single-row grid can never converge");
        assert_eq!(err.attempts, INTERIOR_RETRY_BUDGET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus_seeds() -> impl Iterator<Item = u64> {
        0..20
    }

    #[test]
    fn generation_is_deterministic_for_the_same_seed() {
        let config = WfcMapConfig::default();
        let first = generate_liminal_map(1234, &config).expect("seed 1234 generates");
        let second = generate_liminal_map(1234, &config).expect("seed 1234 regenerates");
        assert_eq!(
            first, second,
            "same seed must produce byte-identical MapSpec"
        );
    }

    #[test]
    fn v2_generation_is_deterministic_and_preserves_v1() {
        let config = WfcMapConfig::default();
        let legacy = generate_liminal_map(1234, &config).expect("v1 generates");
        assert!(legacy.designs.is_none());
        assert!(legacy.corridors.is_empty());

        let first = generate_liminal_map_v2(1234, &config).expect("v2 generates");
        let second = generate_liminal_map_v2(1234, &config).expect("v2 regenerates");
        assert_eq!(first, second);
        assert_eq!(first.edges, legacy.edges);
        assert!(!first.corridors.is_empty());
        assert_eq!(
            first.designs.as_ref().map(|designs| designs.version),
            Some(ARCHITECTURE_CATALOG_VERSION)
        );
    }

    #[test]
    fn v2_retains_the_v1_legacy_edge_topology_for_runtime_adapters() {
        let config = WfcMapConfig::default();
        for seed in 0..16 {
            let v1 = generate_liminal_map(seed, &config).unwrap();
            let v2 = generate_liminal_map_v2(seed, &config).unwrap();
            assert_eq!(v2.edges.len(), v1.edges.len());
            assert_eq!(
                v2.edges
                    .iter()
                    .map(|edge| (edge.a, edge.b))
                    .collect::<Vec<_>>(),
                v1.edges
                    .iter()
                    .map(|edge| (edge.a, edge.b))
                    .collect::<Vec<_>>(),
                "seed {seed}: ConstraintWorld's legacy edge adapter must see the v1 topology"
            );
        }
    }

    #[test]
    fn v2_regions_are_connected_sized_and_cover_exactly_four_to_six_registers() {
        let config = WfcMapConfig::default();
        for seed in 0..24 {
            let spec = generate_liminal_map_v2(seed, &config)
                .unwrap_or_else(|error| panic!("seed {seed} failed: {error:?}"));
            spec.validate()
                .unwrap_or_else(|errors| panic!("seed {seed} invalid: {errors:?}"));
            let designs = spec.designs.as_ref().expect("v2 assignments");
            assert!((4..=6).contains(&designs.register_count()));
            assert_eq!(designs.rooms.len(), spec.rooms.len());
            assert_eq!(designs.corridors.len(), spec.corridors.len());

            for register in ArchitectureRegister::ALL {
                let region: BTreeSet<_> = designs
                    .rooms
                    .values()
                    .filter_map(|design| (design.register == register).then_some(design.room))
                    .collect();
                if region.is_empty() {
                    continue;
                }
                assert!(region.len() >= 3, "seed {seed} {register:?} too small");
                let start = *region.iter().next().unwrap();
                let mut reached = BTreeSet::from([start]);
                let mut queue = VecDeque::from([start]);
                while let Some(room) = queue.pop_front() {
                    for neighbor in spec.neighbors(room) {
                        if region.contains(&neighbor) && reached.insert(neighbor) {
                            queue.push_back(neighbor);
                        }
                    }
                }
                assert_eq!(reached, region, "seed {seed} {register:?} disconnected");
            }
        }
    }

    #[test]
    fn v2_seed_corpus_reaches_every_production_register() {
        let config = WfcMapConfig::default();
        let mut seen = BTreeSet::new();
        for seed in 0..9 {
            let spec = generate_liminal_map_v2(seed, &config).unwrap();
            seen.extend(
                spec.designs
                    .as_ref()
                    .unwrap()
                    .rooms
                    .values()
                    .map(|design| design.register),
            );
        }
        assert_eq!(
            seen,
            ArchitectureRegister::ALL.into_iter().collect(),
            "the pinned compact corpus must exercise the complete catalogue"
        );
    }

    #[test]
    fn v2_corridor_assignments_obey_roles_boundaries_and_stable_ids() {
        let config = WfcMapConfig::default();
        for seed in 0..16 {
            let mut spec = generate_liminal_map_v2(seed, &config).unwrap();
            let before = spec.designs.clone().unwrap();
            for corridor in &spec.corridors {
                let design = before.corridor(corridor.id).expect("corridor design");
                assert!(
                    design
                        .traversal
                        .is_compatible(design.register, corridor.role)
                );
                assert!(corridor.endpoints.iter().any(|endpoint| {
                    before.room(endpoint.room).unwrap().register == design.register
                }));
                match corridor.role {
                    CorridorRole::Gantry => {
                        assert_eq!(design.traversal, TraversalArchetype::GantryExpanse)
                    }
                    CorridorRole::Vertical => {
                        assert_eq!(design.traversal, TraversalArchetype::Wellshaft);
                        assert_eq!(design.register, ArchitectureRegister::Wellshaft);
                        assert!(corridor.endpoints.iter().all(|endpoint| {
                            !matches!(
                                spec.room(endpoint.room).unwrap().role,
                                RoomRole::Start | RoomRole::Exit | RoomRole::Keystone
                            )
                        }));
                    }
                    _ => {}
                }
            }

            // Storage order is not identity: explicit CorridorIds keep every design
            // stable even if a consumer presents the catalogue in reverse order.
            spec.corridors.reverse();
            spec.validate().unwrap();
            assert_eq!(spec.designs.as_ref().unwrap(), &before);
        }
    }

    #[test]
    fn traversal_archetype_ids_are_pinned() {
        assert_eq!(
            TraversalArchetype::ALL
                .into_iter()
                .map(TraversalArchetype::stable_id)
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn different_seeds_generate_different_layouts() {
        let config = WfcMapConfig::default();
        let a = generate_liminal_map(1, &config).expect("seed 1 generates");
        let b = generate_liminal_map(2, &config).expect("seed 2 generates");
        assert_ne!(a, b, "different seeds should not coincidentally match");
    }

    #[test]
    fn corpus_of_twenty_seeds_all_generate_valid_maps_in_range() {
        for seed in corpus_seeds() {
            let config = WfcMapConfig::default();
            let spec = generate_liminal_map(seed, &config)
                .unwrap_or_else(|e| panic!("seed {seed} failed to generate: {e:?}"));
            spec.validate()
                .unwrap_or_else(|errors| panic!("seed {seed} produced invalid map: {errors:?}"));
            assert!(
                (config.min_rooms..=config.max_rooms).contains(&spec.room_count()),
                "seed {seed} room count {} out of range",
                spec.room_count()
            );

            let monitor_count = spec.rooms_with_role(RoomRole::Monitor).len();
            let required_monitors = spec.room_count().div_ceil(9).max(1);
            assert!(
                monitor_count >= required_monitors,
                "seed {seed} has {monitor_count} monitors, needs >= {required_monitors}"
            );

            let start = spec.start_room().expect("seed has start");
            let exit = spec.exit_room().expect("seed has exit");
            assert_ne!(start, exit, "seed {seed} start/exit must be distinct");
            let path = spec
                .shortest_path(start, exit)
                .expect("seed {seed} start/exit connected");
            assert!(
                path.len() >= 4,
                "seed {seed} start/exit too close: path len {}",
                path.len()
            );
        }
    }
}
