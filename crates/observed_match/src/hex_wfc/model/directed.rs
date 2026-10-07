//! The facility changed by an Architect rather than by the director's schedule.
//!
//! Architect Ascent replaces the scheduled relayout with card plays and retraction. The
//! rules decide what changes; this is how a decided change reaches the physical match:
//! the same logical, geometry and collider deltas a relayout commit produces, applied
//! atomically, so presentation and bodies cannot tell the two apart.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexPlacement, HexRelayoutDelta, HexWfcError};
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, HexFace, hex_origin};
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

    /// A point on the floor of `cell`, as near its centre as a body could stand there: the
    /// centre itself if a body fits, otherwise the nearest clear, supported spot on rings
    /// round it. Where a fixture stands, so everyone can walk up to it. `None` when no
    /// body fits anywhere near the centre, as on a ramp or in a cell of pillars.
    ///
    /// Pure queries against the colliders the match was built with, so every peer finds
    /// the same point.
    #[must_use]
    pub fn standing_point(&self, cell: HexCoord) -> Option<Vec3> {
        const RINGS: [f32; 4] = [0.0, 1.5, 3.0, 4.0];
        const STEPS: u8 = 12;
        let floor = Vec3::from_array(hex_origin(cell)) + Vec3::Y * FLOOR_SLAB_TOP;
        RINGS
            .into_iter()
            .flat_map(|radius| {
                let steps = if radius == 0.0 { 1 } else { STEPS };
                (0..steps).map(move |step| {
                    let angle = f32::from(step) * std::f32::consts::TAU / f32::from(STEPS);
                    Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
                })
            })
            .find_map(|offset| self.stands_at(floor + offset))
    }

    /// Whether a body fits standing with its feet at `feet`, on something solid at that
    /// height: not in a wall, over a hole or at a drop. Where exactly its feet would rest.
    pub(super) fn stands_at(&self, feet: Vec3) -> Option<Vec3> {
        const CLEARANCE: f32 = 0.05;
        let config = self.content.traversal_profile().controller();
        let centre_height = config.half_height + CLEARANCE;
        let centre = feet + Vec3::Y * centre_height;
        if !self
            .physics
            .capsule_is_clear(centre, config.radius, config.half_height)
        {
            return None;
        }
        let drop = self
            .physics
            .ray_distance(centre, Vec3::NEG_Y, centre_height + 0.3)?;
        ((drop - centre_height).abs() < 0.2).then_some(centre - Vec3::Y * drop)
    }

    /// Stand `player`'s body at rest in `cell` with its feet at `feet`, looking at `at`,
    /// if a body fits there on solid floor and nothing stands between it and `at`, at its
    /// eye or its waist. Whether it did.
    ///
    /// For evidence captures, as [`Self::jail`] is: play never moves a body this way.
    pub fn stage_body_facing(
        &mut self,
        player: PlayerId,
        cell: HexCoord,
        feet: Vec3,
        at: Vec3,
    ) -> bool {
        let Some(feet) = self.stands_at(feet) else {
            return false;
        };
        let Some(&before) = self.bodies.get(&player) else {
            return false;
        };
        let config = self.content.traversal_profile().controller();
        let centre = feet + Vec3::Y * (config.half_height + 0.02);
        let mut body = observed_traversal::FpsBody::spawned(centre, before.yaw);
        let eye = body.eye(&config);
        let blocked = |from: Vec3| {
            let to = at.with_y(from.y);
            let span = to - from;
            let reach = span.length() - 0.8;
            reach > 0.0
                && self
                    .physics
                    .ray_distance(from, span.normalize(), reach)
                    .is_some()
        };
        if blocked(eye) || blocked(centre) {
            return false;
        }
        let offset = at - eye;
        body.yaw = offset.x.atan2(-offset.z);
        body.pitch = offset.y.atan2(offset.with_y(0.0).length());
        self.bodies.insert(player, body);
        self.players.get_mut(&player).expect("a player").cell = cell;
        self.sync_player_from_body(player);
        true
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
        self.mark_mutation_phase("before_commit");
        let logical = self
            .facility
            .commit_directed_delta(placements)
            .map_err(HexDirectedError::Facility)?;
        self.mark_mutation_phase("logical_commit");
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
        self.mark_mutation_phase("geometry_projection");
        if let Err(error) = self.physics.apply_collider_delta(&geometry.colliders) {
            self.facility
                .revert_relayout_delta(logical)
                .expect("the just-accepted logical delta is revertible");
            return Err(HexDirectedError::Colliders(error));
        }
        self.mark_mutation_phase("collider_apply");
        self.last_geometry_cells
            .extend(geometry.changed_cells.iter().copied());
        self.geometry
            .apply_delta(&geometry)
            .expect("geometry delta was projected from this snapshot");
        self.mark_mutation_phase("geometry_apply");
        self.refresh_spawn_to_exit_cost();
        self.mark_mutation_phase("route_refresh");
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
