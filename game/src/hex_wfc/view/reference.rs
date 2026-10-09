//! Capture-only residency control and evidence. Never changes physical geometry.
use crate::hex_wfc::sim::HexWfcRuntime;
use bevy::prelude::*;
use serde_json::{Value, json};

fn is_full(mode: Option<&str>, reference: bool) -> bool {
    reference && mode == Some("full")
}

pub(in crate::hex_wfc) fn full_geometry() -> bool {
    is_full(
        std::env::var("OBSERVED2_REFERENCE_RESIDENCY")
            .ok()
            .as_deref(),
        std::env::var_os("OBSERVED2_COMPOSITION_REFERENCE").is_some()
            || std::env::var_os("OBSERVED2_SPATIAL_REFERENCE").is_some(),
    )
}

pub(in crate::hex_wfc) fn report(world: &mut World) -> Value {
    let runtime = world.resource::<HexWfcRuntime>();
    let residency = world.resource::<super::HexPresentationResidency>();
    let physical = &runtime.match_state.facility;
    let shown = residency
        .resident
        .iter()
        .filter(|(_, cell)| cell.shown)
        .map(|(&owner, _)| owner)
        .collect::<Vec<_>>();
    let rows = physical.placements.values().map(|p| json!({
        "cell": [p.coord.q, p.coord.r, u16::from(p.coord.level)],
        "space": format!("{:?}", p.space),
        "initial_variant": physical.initial_module_variant(p.coord),
        "physical_pieces": runtime.match_state.geometry.pieces_in_cell(p.coord).count(),
        "detail_shown": shown.iter().any(|owner| residency.catalog.cells[owner].footprint.contains(&p.coord)),
    })).collect::<Vec<_>>();
    let replacements = residency
        .replacements
        .iter()
        .map(|cell| [cell.q, cell.r, u16::from(cell.level)])
        .collect::<Vec<_>>();
    let resident = residency.resident.len();
    let total = residency.catalog.cells.len();
    let reach = residency.reach.enter_radius;
    let mut shells = world.query::<(&super::exterior::ExteriorShell, &Visibility)>();
    let proxies = shells
        .iter(world)
        .filter(|(_, visibility)| **visibility != Visibility::Hidden)
        .map(|(shell, _)| [shell.0.q, shell.0.r, u16::from(shell.0.level)])
        .collect::<Vec<_>>();
    json!({
        "residency_mode": if full_geometry() { "full" } else { "normal" },
        "normal_enter_radius_m": reach,
        "physical_geometry_owners": total,
        "resident_owners": resident,
        "pending_replacement_owners": replacements,
        "shown_detail_owners": shown.len(),
        "shown_exterior_cells": proxies,
        "cells": rows,
        "note": "shown means requested entity visibility; camera frustum and occlusion still apply",
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn full_residency_requires_an_explicit_construction_reference() {
        assert!(!super::is_full(Some("full"), false));
        assert!(!super::is_full(Some("normal"), true));
        assert!(!super::is_full(None, true));
        assert!(super::is_full(Some("full"), true));
    }
}
