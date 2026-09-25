//! The facility changed by an Architect rather than by the director's schedule.
//!
//! Architect Ascent replaces the scheduled relayout with card plays and retraction. The
//! rules decide what changes; this is how a decided change reaches the physical match:
//! the same logical, geometry and collider deltas a relayout commit produces, applied
//! atomically, so presentation and bodies cannot tell the two apart.

use std::collections::BTreeMap;

use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexPlacement, HexRelayoutDelta, HexWfcError};
use observed_hex::{HexCoord, HexFace};
use observed_traversal::ColliderDeltaError;

use super::super::geometry::HexGeometryError;
use super::{HexMatchEvent, HexMatchEventKind, HexWfcMatch};

/// Why a directed change did not reach the physical match. Nothing changed.
#[derive(Clone, Debug, PartialEq)]
pub enum HexDirectedError {
    Facility(HexWfcError),
    Geometry(HexGeometryError),
    Colliders(ColliderDeltaError),
}

impl HexWfcMatch {
    /// Stop the director's scheduled relayout: from here the facility changes only
    /// through [`Self::apply_directed_change`]. Must be called before tick zero, so a
    /// match is directed for its whole life or not at all.
    pub fn hand_mutation_to_architects(&mut self) {
        assert_eq!(self.tick, 0, "a match is directed from tick zero or never");
        self.directed = true;
    }

    #[must_use]
    pub const fn is_directed(&self) -> bool {
        self.directed
    }

    /// Where a player's body is, as a cell, and which lateral face it is turned toward.
    #[must_use]
    pub fn body_cell_and_facing(&self, player: PlayerId) -> Option<(HexCoord, HexFace)> {
        let state = self.players.get(&player)?;
        Some((state.cell, super::movement::look_face(state.yaw, 0.0)))
    }

    /// Commit `placements` to the facility, its geometry and its colliders, together.
    ///
    /// Legality is not checked here: the caller's rules have decided it. What is checked
    /// is that the change can be built, and a change that cannot is refused whole, with
    /// the facility, geometry and colliders exactly as they were.
    pub fn apply_directed_change(
        &mut self,
        placements: BTreeMap<HexCoord, HexPlacement>,
    ) -> Result<&HexRelayoutDelta, HexDirectedError> {
        let logical = self
            .facility
            .commit_directed_delta(placements)
            .map_err(HexDirectedError::Facility)?;
        let geometry = match self.geometry.project_delta_with_rooms(
            &self.facility,
            &logical,
            self.content.cells(),
            self.content.rooms(),
        ) {
            Ok(delta) => delta,
            Err(error) => {
                self.facility
                    .revert_relayout_delta(logical)
                    .expect("the just-accepted logical delta is revertible");
                return Err(HexDirectedError::Geometry(error));
            }
        };
        if let Err(error) = self.physics.apply_collider_delta(&geometry.colliders) {
            self.facility
                .revert_relayout_delta(logical)
                .expect("the just-accepted logical delta is revertible");
            return Err(HexDirectedError::Colliders(error));
        }
        self.geometry
            .apply_delta(&geometry)
            .expect("geometry delta was projected from this snapshot");
        self.refresh_spawn_to_exit_cost();
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: if logical.changed_cells.is_empty() {
                HexMatchEventKind::MutationNoChange
            } else {
                HexMatchEventKind::MutationCommitted
            },
            player: None,
            cell: None,
        });
        Ok(self.last_relayout_delta.insert(logical))
    }
}
