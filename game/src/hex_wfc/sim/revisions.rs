//! Exact presentation invalidation, including geometry owners whose neighbours changed.
use super::HexWfcRuntime;
use observed_hex::HexCoord;
use std::collections::BTreeMap;

pub(in crate::hex_wfc) fn record_generation_changes(
    runtime: &mut HexWfcRuntime,
    previous_generation: u32,
) {
    if runtime.match_state.facility.generation == previous_generation {
        return;
    }
    runtime
        .pending_visual_cells
        .extend(runtime.match_state.last_geometry_cells.iter().copied());
    let changed = changed_revisions(
        &runtime.match_state.facility.cell_revisions,
        &runtime.presented_revisions,
    );
    for (cell, revision) in changed {
        runtime.pending_visual_cells.insert(cell);
        runtime.presented_revisions.insert(cell, revision);
    }
}

pub(super) fn changed_revisions(
    live: &BTreeMap<HexCoord, u32>,
    presented: &BTreeMap<HexCoord, u32>,
) -> Vec<(HexCoord, u32)> {
    live.iter()
        .filter_map(|(&cell, &revision)| {
            (presented.get(&cell).copied().unwrap_or(0) != revision).then_some((cell, revision))
        })
        .collect()
}
