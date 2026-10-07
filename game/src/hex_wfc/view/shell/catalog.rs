//! Stable-ID presentation catalogue, updated only for affected projection owners.
use crate::hex_wfc::sim::HexWfcRuntime;
use observed_facility::hex_wfc::{HexCoord, HexWfcWorld};
use observed_match::hex_wfc::{HexLightSource, HexStructureRole, HexWfcGeometrySnapshot};
use std::collections::{BTreeMap, BTreeSet};

/// Lightweight, presentation-only lookup into the authoritative geometry vectors.
///
/// Keeping stable IDs rather than cloning pieces makes the resident renderer cheap to
/// construct even for the production-sized facility. A relayout can reorder the
/// snapshot's packed vectors; stable IDs keep untouched entries valid across changes.
pub(in crate::hex_wfc::view) struct HexGeometryCatalog {
    pub(in crate::hex_wfc::view) generation: u32,
    pub(in crate::hex_wfc::view) cells: BTreeMap<HexCoord, CellGeometryIndex>,
    pub(in crate::hex_wfc::view) boundary_piece_ids: Vec<observed_traversal::StableColliderId>,
}

pub(in crate::hex_wfc::view) struct CellGeometryIndex {
    pub(in crate::hex_wfc::view) footprint: Vec<HexCoord>,
    pub(in crate::hex_wfc::view) piece_ids: Vec<observed_traversal::StableColliderId>,
    pub(in crate::hex_wfc::view) lights: Vec<HexLightSource>,
}

impl HexGeometryCatalog {
    /// Index `geometry`, the projection of `world`: the facility's, or a prison maze's.
    pub(in crate::hex_wfc::view) fn build(
        world: &HexWfcWorld,
        geometry: &HexWfcGeometrySnapshot,
    ) -> Self {
        let mut cells = BTreeMap::<HexCoord, CellGeometryIndex>::new();
        let mut boundary_piece_ids = Vec::new();
        for piece in &geometry.pieces {
            if piece.role == HexStructureRole::Boundary {
                boundary_piece_ids.push(piece.id);
                continue;
            }
            cells
                .entry(piece.source_cell)
                .or_insert_with(|| CellGeometryIndex {
                    footprint: cell_footprint(world, piece.source_cell),
                    piece_ids: Vec::new(),
                    lights: Vec::new(),
                })
                .piece_ids
                .push(piece.id);
        }
        for light in &geometry.lights {
            if let Some(cell) = cells.get_mut(&light.source_cell) {
                cell.lights.push(light.clone());
            }
        }
        Self {
            generation: geometry.generation,
            cells,
            boundary_piece_ids,
        }
    }

    /// Update owners only. Stable piece IDs remain valid even when another
    /// cell's removal moves their packed-vector slots.
    pub(in crate::hex_wfc::view) fn update_changed(
        &mut self,
        runtime: &HexWfcRuntime,
        changed: &BTreeSet<HexCoord>,
    ) {
        let world = &runtime.match_state.facility;
        let geometry = &runtime.match_state.geometry;
        let mut owners = changed.clone();
        for (&owner, cell) in &self.cells {
            if cell.footprint.iter().any(|cell| changed.contains(cell)) {
                owners.insert(owner);
            }
        }
        for room in &world.blueprints {
            if room.cells.iter().any(|cell| changed.contains(cell)) {
                owners.insert(room.anchor);
            }
        }
        for owner in owners {
            let piece_ids: Vec<_> = geometry
                .pieces_in_cell(owner)
                .filter(|piece| piece.role != HexStructureRole::Boundary)
                .map(|piece| piece.id)
                .collect();
            if piece_ids.is_empty() {
                self.cells.remove(&owner);
                continue;
            }
            let lights = geometry
                .lights
                .iter()
                .filter(|light| light.source_cell == owner)
                .cloned()
                .collect();
            self.cells.insert(
                owner,
                CellGeometryIndex {
                    footprint: cell_footprint(world, owner),
                    piece_ids,
                    lights,
                },
            );
        }
        self.generation = geometry.generation;
    }

    pub(in crate::hex_wfc::view) fn contains(&self, coord: HexCoord) -> bool {
        self.cells.contains_key(&coord)
    }
}

/// The grid cells a spawned cell's fixtures actually occupy. An ordinary
/// tile's footprint is just its own coordinate — `push_tile` in
/// `observed_match::hex_wfc::geometry` keys it that way. A whole-room
/// module is different: `push_room` stamps `source_cell = anchor` on every
/// hull of the room, so all of a room's pieces are grouped here under one
/// coordinate, its anchor. Looking that anchor up against the solved
/// world's stamped blueprints recovers the room's true footprint, so
/// streaming and the defensive light fallback can treat every cell the room
/// actually occupies rather than only its anchor. For every coordinate that
/// is *not* a room anchor (all of today's catalog, and every ordinary tile
/// once rooms exist) this returns exactly `[coord]`, matching prior
/// behavior precisely.
pub(in crate::hex_wfc::view) fn cell_footprint(
    world: &HexWfcWorld,
    coord: HexCoord,
) -> Vec<HexCoord> {
    world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor == coord)
        .map(|blueprint| blueprint.cells.clone())
        .unwrap_or_else(|| vec![coord])
}
