//! What could stand around this tile — the studio's view of it.
//!
//! `observed_facility::hex_wfc::neighborhood` answers the question; this holds
//! the answer, tracks which candidate an author is currently looking at per
//! face, and turns that into geometry.
//!
//! # Solid means settled, wireframe means it could be otherwise
//!
//! The whole readability of this view rests on one distinction. The centre cell
//! is **locked in** — it is what the seed produced, and it draws as real
//! authored geometry, cut away, exactly as `F` already draws it. Every ring cell
//! is **not yet collapsed** as far as this view is concerned: it draws as the
//! wireframe of whichever candidate is being previewed there, so it reads as a
//! possibility rather than as a fact. Nothing else changes between them: the
//! same hulls, from the same projector, one filled and one outlined.
//!
//! Two line colours, and they mean what they already mean everywhere else in
//! this tool:
//!
//! - `SchematicRole::Pinned` (green) — this neighbour is showing what the seed
//!   *actually* produced. Settled.
//! - `SchematicRole::Volatile` (red) — this neighbour is showing an alternative
//!   the solver could equally have chosen. Provisional.
//!
//! So the first thing you see on entering the mode is a green cage matching the
//! solid facility, and every key you press to explore an alternative turns one
//! cage red. Nothing invents a colour; the legend is the one the schematic
//! already uses.
//!
//! # The geometry is projected, not approximated
//!
//! A previewed candidate's hulls come from
//! `observed_match::hex_wfc::project_hypothetical_cell` — the production
//! projector, asked about a world that does not exist. That is the only way this
//! view can promise "what you can expect to see in the real game": a preview
//! that picked its own tile would be showing an author a facility the match
//! would never build. It also means a candidate whose geometry is *missing*
//! reports as missing right here, in the one place somebody is looking at that
//! exact tile, instead of at match load.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_facility::hex_wfc::{
    HexCoord, HexFace, HexPlacement, Neighborhood, NeighborhoodError,
};
use observed_traversal::ColliderShape;

use crate::StudioState;

/// The studio's live neighbourhood readout.
#[derive(Default)]
pub struct NeighborView {
    /// The query result for the current selection, or nothing if the selection
    /// is empty or the query refused.
    pub hood: Option<Neighborhood>,
    /// Why the query refused, verbatim. Shown rather than swallowed: the
    /// interesting refusal is a constraint replay that disagreed with the
    /// world, and quietly showing an empty panel instead would hide it.
    pub refusal: Option<String>,
    /// Which face row the keyboard is on, as an index into `hood.faces`.
    pub cursor: usize,
    /// Which candidate is previewed per ring cell, as an index into that
    /// face's `candidates`. A cell absent here previews its actual placement.
    pub choice: BTreeMap<HexCoord, usize>,
    /// How many whole-ring samples have been rolled. Doubles as the roll, so
    /// the sequence is reproducible from a fresh selection.
    pub rolls: u64,
    /// What the last preview drew, for the status line.
    pub report: NeighborReport,
}

/// What the last neighbourhood pass produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NeighborReport {
    pub ring_cells: usize,
    /// Ring cells currently previewing something other than what was solved.
    pub altered: usize,
    pub edges: usize,
    /// Candidates the corpus could not build. Each one is a real authoring
    /// hole that a match would hit as a load failure.
    pub unbuildable: usize,
}

impl NeighborView {
    /// The face row the keyboard is on.
    #[must_use]
    pub fn selected_face(&self) -> Option<&observed_facility::hex_wfc::FaceDomain> {
        let hood = self.hood.as_ref()?;
        hood.faces.get(self.cursor % hood.faces.len().max(1))
    }

    /// The candidate index currently previewed at `coord`, defaulting to
    /// whatever the seed actually placed there.
    #[must_use]
    pub fn choice_for(&self, coord: HexCoord) -> usize {
        if let Some(&index) = self.choice.get(&coord) {
            return index;
        }
        self.actual_index(coord).unwrap_or(0)
    }

    /// Where a face's actual placement sits in its own weight-ordered list.
    #[must_use]
    pub fn actual_index(&self, coord: HexCoord) -> Option<usize> {
        let hood = self.hood.as_ref()?;
        let domain = hood.faces.iter().find(|face| face.coord == coord)?;
        domain
            .candidates
            .iter()
            .position(|candidate| candidate.is_actual)
    }

    /// The placement previewed at `coord`.
    #[must_use]
    pub fn previewed(&self, coord: HexCoord) -> Option<HexPlacement> {
        let hood = self.hood.as_ref()?;
        let domain = hood.faces.iter().find(|face| face.coord == coord)?;
        let index = self.choice_for(coord) % domain.candidates.len().max(1);
        domain
            .candidates
            .get(index)
            .map(|candidate| candidate.placement)
    }

    /// Move the previewed candidate on the face under the cursor.
    ///
    /// Wraps, because the list is short and an author cycling through six
    /// options should not have to notice which end they are at.
    pub fn step_candidate(&mut self, delta: isize) -> Option<String> {
        let hood = self.hood.as_ref()?;
        if hood.faces.is_empty() {
            return None;
        }
        let face = hood.faces.get(self.cursor % hood.faces.len())?;
        let count = face.candidates.len();
        if count <= 1 {
            return Some(format!(
                "{:?} is forced: the facility leaves exactly one option here",
                face.face
            ));
        }
        let coord = face.coord;
        let current = self.choice_for(coord) % count;
        #[allow(clippy::cast_possible_wrap)]
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;
        self.choice.insert(coord, next);
        let candidate = &face.candidates[next];
        Some(format!(
            "{:?}: candidate {}/{} - {:?}, {:.1}% of this cell's lottery{}",
            face.face,
            next + 1,
            count,
            candidate.placement.archetype,
            candidate.share(face.total_weight) * 100.0,
            if candidate.is_actual {
                " (what this seed placed)"
            } else {
                ""
            }
        ))
    }

    /// Show every neighbour as the seed actually solved it.
    pub fn reset_to_solved(&mut self) {
        self.choice.clear();
    }

    /// Roll one whole consistent ring.
    pub fn roll_ring(
        &mut self,
        world: &observed_facility::hex_wfc::HexWfcWorld,
        profile: &observed_facility::hex_wfc::HexCompositionProfile,
    ) -> String {
        let Some(hood) = self.hood.as_ref() else {
            return String::from("no neighbourhood to roll");
        };
        self.rolls += 1;
        let Some(ring) = hood.sample_ring(world, profile, self.rolls) else {
            return String::from("the ring sampler hit a contradiction; showing the solved ring");
        };
        let mut altered = 0;
        for (coord, placement) in &ring {
            let Some(domain) = hood.faces.iter().find(|face| face.coord == *coord) else {
                continue;
            };
            let Some(index) = domain
                .candidates
                .iter()
                .position(|candidate| candidate.placement == *placement)
            else {
                continue;
            };
            if !domain.candidates[index].is_actual {
                altered += 1;
            }
            self.choice.insert(*coord, index);
        }
        format!(
            "roll {}: {} of {} neighbour(s) differ from what this seed solved",
            self.rolls,
            altered,
            ring.len()
        )
    }
}

/// Re-run the query for the current selection.
///
/// Cheap enough to call on every selection change: the work is one constraint
/// replay plus arc-consistency over at most eight cells, and the replay is the
/// expensive half at roughly one blueprint stamp.
pub fn refresh(state: &mut StudioState) {
    state.neighbors.choice.clear();
    state.neighbors.cursor = 0;
    state.neighbors.rolls = 0;
    state.neighbors.hood = None;
    state.neighbors.refusal = None;

    let Some(centre) = state.selected else {
        state.neighbors.refusal = Some(String::from("select a cell to see what could surround it"));
        return;
    };
    let Some(solved) = state.solved.as_ref() else {
        return;
    };
    match observed_facility::hex_wfc::neighborhood(&solved.world, &state.profile, None, centre) {
        Ok(hood) => state.neighbors.hood = Some(hood),
        Err(error) => state.neighbors.refusal = Some(describe(error)),
    }
}

/// A refusal in the terms an author can act on.
fn describe(error: NeighborhoodError) -> String {
    match error {
        NeighborhoodError::NotPlaced(coord) => {
            format!(
                "({},{},{}) is not a cell of this facility",
                coord.q, coord.r, coord.level
            )
        }
        // Deliberately not softened. This is the tool's own rule showing up:
        // the constraint set could not be recovered, so any domain reported
        // here would be a guess, and a guess is the one thing this view may
        // not be.
        NeighborhoodError::ConstraintReplayFailed => String::from(
            "the solve's blueprints and forced route could not be re-derived, \
             so no honest domain can be shown here",
        ),
        NeighborhoodError::Contradiction(coord) => format!(
            "re-opening ({},{},{}) emptied its domain - a solver rule and this query have drifted",
            coord.q, coord.r, coord.level
        ),
        NeighborhoodError::UnknownPlacement(coord) => format!(
            "({},{},{}) holds a placement that is not in the variant catalogue",
            coord.q, coord.r, coord.level
        ),
    }
}

/// The cell-local hull points of one previewed placement.
///
/// `Err` carries the projector's own complaint, which for the interesting case
/// is "the corpus has no geometry for this" — reported here, at the tile, by
/// the person who can go author it.
pub fn preview_hulls(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    prototypes: &[observed_authoring::TilePrototype],
    coord: HexCoord,
    placement: HexPlacement,
) -> Result<Vec<Vec<Vec3>>, String> {
    let pieces =
        observed_match::hex_wfc::project_hypothetical_cell(world, coord, placement, prototypes)
            .map_err(|error| format!("{error:?}"))?;
    Ok(pieces
        .into_iter()
        .filter_map(|piece| match piece.shape {
            ColliderShape::ConvexHull { points } if !points.is_empty() => Some(points),
            _ => None,
        })
        .collect())
}

/// Which face a lateral index names, for the digit keys.
#[must_use]
pub fn face_at(index: usize) -> Option<HexFace> {
    HexFace::ALL.get(index).copied()
}

/// Draw the ring around the selected cell as the wireframe of whatever is
/// currently previewed there.
///
/// Two batches, and the split carries the meaning: green for a neighbour still
/// showing what the seed actually solved, red for one showing an alternative.
/// Those are the schematic's existing `Pinned`/`Volatile` roles, whose
/// documented meanings are already "the solver will not rewire this" and "it
/// can" -- which is exactly settled versus provisional. No colour is invented
/// here; the legend is the one the rest of the tool already uses.
///
/// A candidate the corpus cannot build draws nothing and is counted instead.
/// Silently drawing an empty cage would read as a very open tile, so the count
/// goes to the panel and the status line, where it reads as the authoring hole
/// it is.
pub fn emit(
    commands: &mut Commands,
    state: &StudioState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> NeighborReport {
    use crate::draw::{LineBatch, StudioVisual, unlit};
    use observed_style::{SchematicRole, schematic};

    let mut report = NeighborReport::default();
    if state.detail_mode != crate::detail::DetailMode::Neighborhood {
        return report;
    }
    let Some(solved) = state.solved.as_ref() else {
        return report;
    };
    let Some(hood) = state.neighbors.hood.as_ref() else {
        return report;
    };
    let Ok((prototypes, _)) = crate::corpus() else {
        return report;
    };

    let mut as_solved = LineBatch::default();
    let mut alternative = LineBatch::default();
    report.ring_cells = hood.faces.len();

    // Deliberately not filtered by `state.layer`. The layer control decides
    // which floors the *schematic* draws, and applying it here would hide the
    // up and down neighbours — the two faces this mode exists to make
    // reachable, and the ones with the most grammar behind them. A neighbourhood
    // is a neighbourhood on every floor it touches.

    for domain in &hood.faces {
        let Some(placement) = state.neighbors.previewed(domain.coord) else {
            continue;
        };
        let is_solved = placement == domain.actual;
        if !is_solved {
            report.altered += 1;
        }
        let hulls = match preview_hulls(&solved.world, prototypes, domain.coord, placement) {
            Ok(hulls) => hulls,
            Err(_) => {
                report.unbuildable += 1;
                continue;
            }
        };
        let origin = Vec3::from_array(observed_hex::hex_origin(domain.coord));
        let batch = if is_solved {
            &mut as_solved
        } else {
            &mut alternative
        };
        // The same structural-edge rule the isometric observer uses, so a
        // wireframe tile and a solid one are never two different shapes.
        for (from, to) in observed_traversal::structural_edges(&hulls) {
            batch.segment(origin + from, origin + to);
            report.edges += 1;
        }
    }

    for (batch, role) in [
        (as_solved, SchematicRole::Pinned),
        (alternative, SchematicRole::Volatile),
    ] {
        if let Some(mesh) = batch.into_mesh() {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(unlit(schematic(role)))),
                StudioVisual,
            ));
        }
    }
    report
}
