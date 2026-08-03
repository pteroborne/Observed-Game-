//! Bounded presentation residency for the canonical hex facility.
//!
//! Arc Q's first-frame contract: presentation admits cells near the runner under a
//! per-frame budget and retires distant ones with hysteresis, so a production-sized
//! facility is never spawned in one blocking frame. Residency state itself lives in
//! `view`, which this module is a child of.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use bevy::prelude::*;
use observed_facility::hex_wfc::HexCoord;
use observed_hex::hex_origin;

/// How far residency reaches for cells, and how many levels either side.
///
/// A field rather than the module constants because the spectator overview
/// needs a wider one: it frames tiles around the body and the authored geometry
/// has to exist out to the edge of that frame, or the detailed half of the view
/// is a 30 m island in a 200 m picture. Play keeps [`Reach::play`] exactly.
#[derive(Clone, Copy, Debug)]
pub(in crate::hex_wfc) struct Reach {
    pub(in crate::hex_wfc) enter_radius: f32,
    pub(in crate::hex_wfc) enter_levels: u8,
    pub(in crate::hex_wfc) exit_radius: f32,
    pub(in crate::hex_wfc) exit_levels: u8,
}

impl Default for Reach {
    fn default() -> Self {
        Self::play()
    }
}

impl Reach {
    /// The shipped play budget, unchanged.
    pub(in crate::hex_wfc) fn play() -> Self {
        Self {
            enter_radius: STREAM_ENTER_RADIUS,
            enter_levels: STREAM_ENTER_LEVELS,
            exit_radius: STREAM_EXIT_RADIUS,
            exit_levels: STREAM_EXIT_LEVELS,
        }
    }

    /// Out to `radius` metres, with the hysteresis margin play uses.
    pub(in crate::hex_wfc) fn out_to(radius: f32) -> Self {
        let play = Self::play();
        Self {
            enter_radius: radius.max(play.enter_radius),
            exit_radius: radius.max(play.enter_radius) * (play.exit_radius / play.enter_radius),
            ..play
        }
    }
}

use super::assets::HexWfcVisualAssets;
use super::{
    CELL_DESPAWN_BUDGET, CELL_SPAWN_BUDGET, ENTRY_SAFE_RADIUS, HexPresentationReadiness,
    HexPresentationResidency, ResidentCell, STREAM_ENTER_LEVELS, STREAM_ENTER_RADIUS,
    STREAM_EXIT_LEVELS, STREAM_EXIT_RADIUS,
};
use crate::hex_wfc::sim::HexWfcRuntime;

pub(crate) fn sync_changed_geometry(
    mut commands: Commands,
    mut runtime: ResMut<HexWfcRuntime>,
    mut residency: ResMut<HexPresentationResidency>,
    mut perf: Option<ResMut<crate::hex_wfc::perf::HexPerfMetrics>>,
) {
    if runtime.pending_visual_cells.is_empty() {
        return;
    }
    let started = Instant::now();
    let changed = std::mem::take(&mut runtime.pending_visual_cells);
    // Invalidate only resident projections touched by the logical delta. A room parent
    // is touched when any cell in its full footprint changes; off-screen cells have no
    // presentation entity to rebuild.
    let invalidated = residency
        .resident
        .keys()
        .copied()
        .filter(|coord| {
            residency.catalog.cells.get(coord).is_some_and(|cell| {
                changed.contains(coord)
                    || cell.footprint.iter().any(|coord| changed.contains(coord))
            })
        })
        .collect::<Vec<_>>();
    for coord in invalidated {
        if let Some(cell) = residency.resident.remove(&coord) {
            commands.entity(cell.entity).despawn();
        }
    }
    residency.catalog.rebuild(&runtime);
    debug_assert_eq!(
        residency.catalog.generation,
        runtime.match_state.geometry.generation
    );
    // A full authoritative reconstruction may replace a projection owner even if the
    // supplied revision set was conservative. Never retain an entity with no new index.
    let retired = residency
        .resident
        .keys()
        .copied()
        .filter(|&coord| !residency.catalog.contains(coord))
        .collect::<Vec<_>>();
    for coord in retired {
        if let Some(cell) = residency.resident.remove(&coord) {
            commands.entity(cell.entity).despawn();
        }
    }
    crate::hex_wfc::perf::record_view(
        &mut perf,
        crate::hex_wfc::perf::ViewTimingKind::MutationRebuild,
        &runtime,
        started.elapsed(),
    );
}

/// Whether `coord` alone is within a given residency window. This is the pure
/// arithmetic core of the stream planner and its footprint tests.
pub(super) fn cell_in_stream_range(
    coord: HexCoord,
    focus_position: Vec3,
    focus_level: u8,
    radius: f32,
    levels: u8,
) -> bool {
    let origin = Vec3::from_array(hex_origin(coord));
    let plan = Vec2::new(origin.x - focus_position.x, origin.z - focus_position.z).length();
    let level_gap = coord.level.abs_diff(focus_level);
    plan <= radius && level_gap <= levels
}

/// A cell's fixture group is streamed in when ANY cell of its footprint is
/// in range — for an ordinary tile the footprint is just its own coordinate
/// (identical to the old single-point check); for a whole-room module it is
/// the room's complete stamped footprint, so a large room stays visible
/// while the player is anywhere inside it rather than popping in/out keyed
/// to a single anchor cell that might be far from the player.
pub(super) fn footprint_in_range(
    footprint: &[HexCoord],
    focus_position: Vec3,
    focus_level: u8,
    radius: f32,
    levels: u8,
) -> bool {
    footprint
        .iter()
        .any(|&coord| cell_in_stream_range(coord, focus_position, focus_level, radius, levels))
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ResidencyPlan {
    pub(super) spawn: Vec<HexCoord>,
    pub(super) despawn: Vec<HexCoord>,
    pub(super) desired_cells: usize,
    pub(super) pending_cells: usize,
}

fn footprint_distance_squared(footprint: &[HexCoord], focus_position: Vec3) -> f32 {
    footprint
        .iter()
        .map(|&coord| {
            let origin = Vec3::from_array(hex_origin(coord));
            Vec2::new(origin.x - focus_position.x, origin.z - focus_position.z).length_squared()
        })
        .fold(f32::INFINITY, f32::min)
}

fn sort_nearest_first(
    cells: &mut [HexCoord],
    catalog: &super::shell::HexGeometryCatalog,
    focus_position: Vec3,
    focus_cell: HexCoord,
) {
    cells.sort_by(|a, b| {
        let a_cell = &catalog.cells[a];
        let b_cell = &catalog.cells[b];
        let a_current = !a_cell.footprint.contains(&focus_cell);
        let b_current = !b_cell.footprint.contains(&focus_cell);
        a_current
            .cmp(&b_current)
            .then_with(|| {
                footprint_distance_squared(&a_cell.footprint, focus_position).total_cmp(
                    &footprint_distance_squared(&b_cell.footprint, focus_position),
                )
            })
            .then_with(|| a.cmp(b))
    });
}

pub(super) fn plan_residency(
    catalog: &super::shell::HexGeometryCatalog,
    resident: &BTreeMap<HexCoord, ResidentCell>,
    focus_position: Vec3,
    focus_cell: HexCoord,
    spawn_budget: usize,
    despawn_budget: usize,
    reach: Reach,
) -> ResidencyPlan {
    let mut spawn_candidates = Vec::new();
    let mut despawn_candidates = Vec::new();
    let mut desired_cells = 0;
    for (&coord, cell) in &catalog.cells {
        let is_resident = resident.contains_key(&coord);
        // This explicit containment guard is stronger than distance: a large authored
        // room can never disappear while the runner occupies any cell of its footprint.
        let contains_runner = cell.footprint.contains(&focus_cell);
        let wanted = contains_runner
            || if is_resident {
                footprint_in_range(
                    &cell.footprint,
                    focus_position,
                    focus_cell.level,
                    reach.exit_radius,
                    reach.exit_levels,
                )
            } else {
                footprint_in_range(
                    &cell.footprint,
                    focus_position,
                    focus_cell.level,
                    reach.enter_radius,
                    reach.enter_levels,
                )
            };
        if wanted {
            desired_cells += 1;
            if !is_resident {
                spawn_candidates.push(coord);
            }
        } else if is_resident {
            despawn_candidates.push(coord);
        }
    }
    sort_nearest_first(&mut spawn_candidates, catalog, focus_position, focus_cell);
    // Retire the farthest cells first. This makes a bounded removal backlog preserve the
    // most useful geometry without weakening the exit-window hysteresis rule.
    sort_nearest_first(&mut despawn_candidates, catalog, focus_position, focus_cell);
    despawn_candidates.reverse();
    let pending_cells = spawn_candidates.len();
    spawn_candidates.truncate(spawn_budget);
    despawn_candidates.truncate(despawn_budget);
    ResidencyPlan {
        spawn: spawn_candidates,
        despawn: despawn_candidates,
        desired_cells,
        pending_cells,
    }
}

pub(super) fn initial_spawn_batch(
    catalog: &super::shell::HexGeometryCatalog,
    focus: &observed_match::hex_wfc::HexPlayerState,
    budget: usize,
) -> BTreeSet<HexCoord> {
    let mut safe = catalog
        .cells
        .iter()
        .filter_map(|(&coord, cell)| {
            (cell.footprint.contains(&focus.cell)
                || footprint_in_range(
                    &cell.footprint,
                    focus.position,
                    focus.cell.level,
                    ENTRY_SAFE_RADIUS,
                    0,
                ))
            .then_some(coord)
        })
        .collect::<Vec<_>>();
    sort_nearest_first(&mut safe, catalog, focus.position, focus.cell);
    assert!(
        safe.len() <= budget,
        "validated entry neighborhood exceeded its presentation spawn budget"
    );
    let mut remaining = catalog
        .cells
        .iter()
        .filter_map(|(&coord, cell)| {
            (!safe.contains(&coord)
                && footprint_in_range(
                    &cell.footprint,
                    focus.position,
                    focus.cell.level,
                    STREAM_ENTER_RADIUS,
                    STREAM_ENTER_LEVELS,
                ))
            .then_some(coord)
        })
        .collect::<Vec<_>>();
    sort_nearest_first(&mut remaining, catalog, focus.position, focus.cell);
    safe.extend(
        remaining
            .into_iter()
            .take(budget.saturating_sub(safe.len())),
    );
    safe.into_iter().collect()
}

pub(super) fn presentation_readiness(
    catalog: &super::shell::HexGeometryCatalog,
    resident: &BTreeMap<HexCoord, ResidentCell>,
    focus: &observed_match::hex_wfc::HexPlayerState,
    spawned_this_frame: usize,
    despawned_this_frame: usize,
) -> HexPresentationReadiness {
    let plan = plan_residency(
        catalog,
        resident,
        focus.position,
        focus.cell,
        0,
        0,
        Reach::play(),
    );
    let entry_neighborhood_ready = catalog.cells.iter().all(|(&coord, cell)| {
        let safe = cell.footprint.contains(&focus.cell)
            || footprint_in_range(
                &cell.footprint,
                focus.position,
                focus.cell.level,
                ENTRY_SAFE_RADIUS,
                0,
            );
        !safe || resident.contains_key(&coord)
    });
    HexPresentationReadiness {
        catalog_cells: catalog.cells.len(),
        desired_cells: plan.desired_cells,
        resident_cells: resident.len(),
        pending_cells: plan.pending_cells,
        resident_child_pieces: resident.values().map(|cell| cell.child_pieces).sum(),
        spawned_this_frame,
        despawned_this_frame,
        entry_neighborhood_ready,
        stream_window_ready: plan.pending_cells == 0,
    }
}

pub(crate) fn sync_streamed_cells(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    mut residency: ResMut<HexPresentationResidency>,
    mut readiness: ResMut<HexPresentationReadiness>,
    mut assets: ResMut<HexWfcVisualAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut perf: Option<ResMut<crate::hex_wfc::perf::HexPerfMetrics>>,
) {
    let focus = runtime.local();
    if residency.defer_incremental_once {
        residency.defer_incremental_once = false;
        *readiness = presentation_readiness(&residency.catalog, &residency.resident, focus, 0, 0);
        crate::hex_wfc::perf::record_streaming(
            &mut perf,
            readiness.resident_cells,
            0,
            readiness.resident_child_pieces,
        );
        return;
    }
    let spawn_budget = if residency.capture_unbounded {
        usize::MAX
    } else {
        CELL_SPAWN_BUDGET
    };
    let reach = residency.reach;
    let plan = plan_residency(
        &residency.catalog,
        &residency.resident,
        focus.position,
        focus.cell,
        spawn_budget,
        CELL_DESPAWN_BUDGET,
        reach,
    );
    let mut despawned = 0;
    for coord in plan.despawn {
        if let Some(cell) = residency.resident.remove(&coord) {
            commands.entity(cell.entity).despawn();
            despawned += 1;
        }
    }
    let requested = plan.spawn.into_iter().collect::<BTreeSet<_>>();
    let spawned = super::shell::spawn_cells(
        &mut commands,
        &mut assets,
        &mut meshes,
        &runtime,
        &residency.catalog,
        &requested,
    );
    let spawned_count = spawned.len();
    for spawned in spawned {
        residency.resident.insert(
            spawned.coord,
            ResidentCell {
                entity: spawned.entity,
                child_pieces: spawned.child_pieces,
            },
        );
    }
    *readiness = presentation_readiness(
        &residency.catalog,
        &residency.resident,
        focus,
        spawned_count,
        despawned,
    );
    crate::hex_wfc::perf::record_streaming(
        &mut perf,
        readiness.resident_cells,
        spawned_count + despawned,
        readiness.resident_child_pieces,
    );
}
