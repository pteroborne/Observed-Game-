//! Draw the ring: the centre stays solid, everything around it is wireframe.
//!
//! # Solid means settled, wireframe means it could be otherwise
//!
//! The whole readability of the mode rests on that one distinction. The module
//! you are authoring is a fact and draws as real geometry, cut away, exactly as
//! it does without the mode. Every ring cell is a *possibility*, so it draws as
//! the outline of whichever candidate is previewed there — the same hulls, from
//! the same triangulator, one filled and one drawn as edges.
//!
//! # Why the ring is not cut away
//!
//! The centre keeps its cutaway; the ring deliberately does not. Two reasons,
//! and the second decides it: a wireframe is already see-through, so the
//! cutaway buys nothing — and `hull_survives` drops whole hulls on the camera
//! side, which is very often *the mating wall*, the one piece of a neighbour
//! this mode exists to show. Cutting it away would remove the answer.
//!
//! # Colours are the ones the tool already uses
//!
//! `Selected` amber marks the cell the keys are on. `Grid` draws every other
//! ring cell, subordinate to the solid centre. `Volatile` is reserved for
//! genuine faults — a cell nothing fits, and the shared edge of two ring cells
//! whose previewed candidates disagree with each other. `Pinned` stays with the
//! centre's clean/failing verdict and is never used here, so one colour never
//! answers two questions.

use bevy::prelude::*;
use observed_authoring::rotation::rotate_hulls;
use observed_hex::{CORNERS, HexFace, TILE_LEVEL_HEIGHT, face_edge, ports_compatible};
use observed_style::{SchematicRole, schematic};

use crate::draw::{LineBatch, unlit};
use crate::module::diagnose::Diagnosis;
use crate::module::neighbor_view::{NeighbourView, RingReport};
use crate::module::neighbors::RingCell;
use crate::module::render::{ModuleVisual, plan, plan_origin};

/// How far above the floor a fault edge is drawn, so it reads as an annotation
/// rather than as part of the tile.
const FAULT_LIFT: f32 = 0.08;

/// Draw the ring around the selected module.
pub fn emit_ring(
    commands: &mut Commands,
    view: &NeighbourView,
    corpus: &[Diagnosis],
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> RingReport {
    let mut report = RingReport {
        cells: view.ring.cells.len(),
        ..RingReport::default()
    };
    if !view.on || view.ring.cells.is_empty() {
        return report;
    }

    let mut focus = LineBatch::default();
    let mut context = LineBatch::default();
    let mut lattice = LineBatch::default();
    let mut fault = LineBatch::default();
    let cursor = view.cursor % view.ring.cells.len();

    for (index, cell) in view.ring.cells.iter().enumerate() {
        let origin = plan_origin(cell.coord.0, cell.coord.1);
        // Every ring cell gets its prism, including the ones that draw a tile.
        // Without it a sealed neighbour is a box floating with no stated
        // boundary, and the seven-cell shape of the view - which is the whole
        // point - never appears. Dimmer than the tiles it contains, because it
        // is the lattice rather than the answer.
        report.edges += prism(&mut lattice, origin, 1);
        let Some(candidate) = view.previewed(cell) else {
            // Nothing in the corpus meets this face. The cell still gets its
            // prism, in the alert colour: an empty space would read as "no
            // neighbour here", and this is the opposite - a hole where one is
            // needed.
            report.empty += 1;
            report.edges += prism(&mut fault, origin, 1);
            continue; // the lattice prism above still stands under it
        };
        let Some(prototype) = corpus
            .get(candidate.source)
            .and_then(|diagnosis| diagnosis.prototype.as_ref())
        else {
            report.empty += 1;
            report.edges += prism(&mut fault, origin, 1);
            continue; // the lattice prism above still stands under it
        };

        let batch = if index == cursor {
            &mut focus
        } else {
            &mut context
        };
        // The same structural-edge rule the isometric observer uses, over the
        // same triangulator the solid pass uses, so a wireframe tile and a
        // solid tile can never be two different shapes.
        let hulls = rotate_hulls(&prototype.hulls, candidate.turn);
        for (from, to) in observed_traversal::structural_edges(&hulls) {
            batch.segment(origin + from, origin + to);
            report.edges += 1;
        }
        report.drawn += 1;
    }

    report.broken_seams = mark_broken_seams(view, &mut fault, &mut report.edges);

    // The lattice runs at a quarter emission, the same reduction
    // `render::spawn_cell_outline` already makes for the centre's own prism and
    // for the same reason: a cell boundary is the frame the answer is drawn in,
    // not the answer. At full strength six of them out-shout the module under
    // inspection, which inverts the whole point of the view.
    let mut dim = schematic(SchematicRole::Grid);
    dim.emissive *= 0.25;

    for (batch, treatment) in [
        (lattice, dim),
        (context, schematic(SchematicRole::Grid)),
        (focus, schematic(SchematicRole::Selected)),
        (fault, schematic(SchematicRole::Volatile)),
    ] {
        if let Some(mesh) = batch.into_mesh() {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(unlit(treatment))),
                ModuleVisual,
            ));
        }
    }
    report
}

/// Where the previewed neighbours contradict **each other**.
///
/// The ring cells are adjacent to one another, not only to the centre — around
/// a single cell they form a closed chain of six. Each is chosen against the
/// centre alone, so a combination that no facility could ever contain is the
/// normal case rather than a rare one. Drawing the disagreeing seam is what
/// turns "six individually legal tiles" into "and here is where they cannot
/// coexist", which is the whole of what a controlled solve would later fix.
fn mark_broken_seams(view: &NeighbourView, fault: &mut LineBatch, edges: &mut usize) -> usize {
    let mut broken = 0;
    for (index, cell) in view.ring.cells.iter().enumerate() {
        let Some(here) = view.previewed(cell) else {
            continue;
        };
        for face in HexFace::LATERAL {
            let (dq, dr, _) = face.delta();
            let across = (cell.coord.0 + dq, cell.coord.1 + dr);
            // Each unordered pair once: only look forward in the ring order.
            let Some((other_index, other)) = view
                .ring
                .cells
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.coord == across)
            else {
                continue;
            };
            if other_index <= index {
                continue;
            }
            let Some(there) = view.previewed(other) else {
                continue;
            };
            if ports_compatible(
                here.signature.port(face),
                there.signature.port(face.opposite()),
            ) {
                continue;
            }
            broken += 1;
            *edges += seam_edge(fault, cell.coord, face);
        }
    }
    broken
}

/// The shared boundary of a cell and its neighbour across `face`, drawn at
/// floor and head height so it reads from the isometric camera.
fn seam_edge(batch: &mut LineBatch, coord: (i32, i32), face: HexFace) -> usize {
    let origin = plan_origin(coord.0, coord.1);
    let [a, b] = face_edge(face);
    let mut drawn = 0;
    for level in 0..2 {
        #[allow(clippy::cast_precision_loss)]
        let y = TILE_LEVEL_HEIGHT * level as f32 + FAULT_LIFT;
        batch.segment(origin + plan(a, y), origin + plan(b, y));
        drawn += 1;
    }
    for corner in [a, b] {
        batch.segment(
            origin + plan(corner, FAULT_LIFT),
            origin + plan(corner, TILE_LEVEL_HEIGHT + FAULT_LIFT),
        );
        drawn += 1;
    }
    drawn
}

/// The canonical hex prism of one cell, from the same corner table the
/// validator enforces.
fn prism(batch: &mut LineBatch, origin: Vec3, levels: u8) -> usize {
    let top = TILE_LEVEL_HEIGHT * f32::from(levels.max(1));
    let mut drawn = 0;
    for level in 0..=u32::from(levels.max(1)) {
        #[allow(clippy::cast_precision_loss)]
        let y = TILE_LEVEL_HEIGHT * level as f32;
        for index in 0..CORNERS.len() {
            let a = CORNERS[index];
            let b = CORNERS[(index + 1) % CORNERS.len()];
            batch.segment(origin + plan(a, y), origin + plan(b, y));
            drawn += 1;
        }
    }
    for corner in CORNERS {
        batch.segment(origin + plan(corner, 0.0), origin + plan(corner, top));
        drawn += 1;
    }
    drawn
}

/// Which ring cells contradict each other under the current preview, for the
/// panel. Same rule as [`mark_broken_seams`], without the drawing.
#[must_use]
pub fn broken_pairs(view: &NeighbourView) -> Vec<(usize, usize, HexFace)> {
    let mut pairs = Vec::new();
    for (index, cell) in view.ring.cells.iter().enumerate() {
        let Some(here) = view.previewed(cell) else {
            continue;
        };
        for face in HexFace::LATERAL {
            let (dq, dr, _) = face.delta();
            let across = (cell.coord.0 + dq, cell.coord.1 + dr);
            let Some((other_index, other)) = view
                .ring
                .cells
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.coord == across)
            else {
                continue;
            };
            if other_index <= index {
                continue;
            }
            let Some(there) = view.previewed(other) else {
                continue;
            };
            if !ports_compatible(
                here.signature.port(face),
                there.signature.port(face.opposite()),
            ) {
                pairs.push((index, other_index, face));
            }
        }
    }
    pairs
}

/// One ring cell's face label and previewed occupant, for the panel.
#[must_use]
pub fn describe(view: &NeighbourView, cell: &RingCell) -> String {
    match view.previewed(cell) {
        Some(candidate) => format!(
            "{} turn {} w{}",
            candidate.name, candidate.turn, candidate.weight
        ),
        None => String::from("NOTHING IN THE CORPUS MEETS THIS FACE"),
    }
}
