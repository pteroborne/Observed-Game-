//! Where the facility's regions meet, and how much of that boundary you can
//! walk straight through.
//!
//! [`observed_facility::hex_wfc::region_plan`] answers two questions about a
//! facility - what the regions are, and where two of them touch - and until now
//! nothing drew the answer. The number it produces is stark enough to be worth
//! looking at rather than reading: across six production seeds, **49.9% of every
//! region frontier is walkable**, and the busiest single gateway is crossed in
//! twenty places. A boundary open in half its possible places is not a boundary
//! between two areas; it is a seam inside one.
//!
//! # The legend (Legibility Contract — every state means something)
//!
//! This is a **mode**, for the reason [`crate::draw`] states about its own two:
//! one colour cannot answer two questions at once. The status bar says when it
//! is on, because that is what keeps the answer to "which question is this red
//! answering" available without reading the source.
//!
//! While it is on, the schematic's own walls step back to one neutral dim green,
//! the same courtesy [`crate::draw`] already extends to the authored deck and to
//! the neighbourhood explorer. That frees both signal colours for this legend:
//!
//! - **bright green** ([`SchematicRole::Pinned`]) — a region frontier that is
//!   sealed. The boundary holds here; "settled" is what that role already means.
//! - **red** ([`SchematicRole::Volatile`]) — a frontier you can walk straight
//!   across. The openings *are* the finding, so they take the alert colour.
//!
//! So the lattice is dim, the border is bright, and the holes in the border are
//! red. Read it by the ratio: regions that meant something would draw a mostly
//! green border broken by a few deliberate red crossings. What it draws instead
//! is closer to half and half, which is what "uniform mush" looks like.
//!
//! Only the focus floor's frontiers draw, so the context floors above and below
//! do not treble the line work.
//!
//! A region is a whole-height volume, so some frontiers are *vertical* - a cell
//! whose neighbour one floor up belongs to another region. Those are counted and
//! reported but not drawn, because a floor plan has no edge to draw them on;
//! a ring inside the cell would collide with the selection ring, which already
//! owns that shape. The count is what matters: they were invisible in the region
//! model until a survey found the facility leaking through them.

use bevy::prelude::*;
use observed_facility::hex_wfc::region_plan;
use observed_hex::{HexFace, hex_origin};
use observed_style::{SchematicRole, schematic};

use crate::StudioState;

/// Frontier lines ride above the floor plan so they are not buried in it, and
/// below the selection ring so selecting a cell still reads.
const LIFT: f32 = 0.30;

/// What the last region pass measured. Surfaced in the DISTRICTS tab, so the
/// picture always has the number that goes with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionReport {
    pub regions: usize,
    pub gateways: usize,
    /// Frontier cell pairs on the drawn floor.
    pub frontier: usize,
    /// How many of those you can actually walk across.
    pub open: usize,
    /// The most-crossed single gateway, which is what tells a threshold from a
    /// seam: a boundary crossed twenty times is not a threshold.
    pub widest: usize,
    /// Frontier pairs that leave the region through a floor or ceiling rather
    /// than a wall. Counted, not drawn - see the module legend.
    pub vertical: usize,
    /// How many of those you can climb through.
    pub vertical_open: usize,
}

impl RegionReport {
    /// Share of the drawn frontier that is walkable, as a percentage.
    #[must_use]
    pub fn permeability(self) -> f64 {
        if self.frontier == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.open as f64 * 100.0 / self.frontier as f64
        }
    }
}

/// The DISTRICTS tab's region summary.
///
/// Lives here rather than in `chrome.rs` so the tab body stays one line: that
/// file sits a few lines under the six-hundred-line ratchet, and a panel that
/// grows there is a panel that trips it.
#[must_use]
pub fn summary(state: &StudioState) -> String {
    let report = state.region_report;
    if !state.show_regions {
        return String::from("REGION FRONTIERS: off (press D)\n\n");
    }
    format!(
        "REGION FRONTIERS: on\n\
         regions {}  gateways {}  frontier {}  open {} ({:.1}%)  widest {}\n\
         vertical {} ({} open, not drawn)\n\
         a boundary crossed in dozens of places is a seam, not a threshold.\n\n",
        report.regions,
        report.gateways,
        report.frontier,
        report.open,
        report.permeability(),
        report.widest,
        report.vertical,
        report.vertical_open,
    )
}

/// The status-bar legend while region mode is on.
///
/// The bar always says which question the signal colours are answering; this is
/// region mode's answer, and without it the bar would still be claiming green
/// means a held pin.
#[must_use]
pub fn status(report: RegionReport) -> String {
    format!(
        " | REGIONS (green = border holds, red = walk straight through): \
         {} of {} frontier open ({:.0}%), widest gateway {}",
        report.open,
        report.frontier,
        report.permeability(),
        report.widest
    )
}

/// Draw the region frontiers of the floor under inspection.
///
/// Follows [`crate::neighbors::emit`]: batch into one mesh per colour and tag
/// with `StudioVisual`, so the geometry pass owns the lifetime and a redraw
/// clears it with everything else.
pub fn emit(
    commands: &mut Commands,
    state: &StudioState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> RegionReport {
    use crate::draw::{LineBatch, StudioVisual, face_edge, unlit};

    let mut report = RegionReport::default();
    if !state.show_regions {
        return report;
    }
    let Some(solved) = state.solved.as_ref() else {
        return report;
    };

    // Pure in `(seed, config)`, so this agrees with whatever the solver did
    // without being told - which is the property that makes a region plan
    // usable as a contract rather than as a report.
    let plan = region_plan(state.seed, state.config);
    let grid = state.config.grid();
    report.regions = plan.regions.len();
    report.gateways = plan.gateways.len();

    let mut open_lines = LineBatch::default();
    let mut sealed_lines = LineBatch::default();

    for gateway in &plan.gateways {
        let mut crossings = 0usize;
        for &(a, b) in &gateway.frontier {
            if !state.layer.is_focus(a.level) {
                continue;
            }
            let Some(face) = HexFace::LATERAL
                .into_iter()
                .find(|face| grid.neighbor(a, *face) == Some(b))
            else {
                // Vertical: no wall to draw it on, so count it and move on.
                // Silently dropping it would make the border look tidier than
                // it is, which is the one thing this overlay must not do.
                report.vertical += 1;
                let climbable = matches!(
                    (
                        solved.world.placements.get(&a),
                        solved.world.placements.get(&b)
                    ),
                    (Some(here), Some(there))
                        if here.space != observed_facility::hex_wfc::HexSpace::Void
                            && there.space != observed_facility::hex_wfc::HexSpace::Void
                );
                report.vertical_open += usize::from(climbable);
                continue;
            };
            report.frontier += 1;

            // Walkable means both sides open the shared face. A cell missing
            // from the lattice was pruned, which is a wall as far as anyone
            // walking is concerned.
            let walkable = match (
                solved.world.placements.get(&a),
                solved.world.placements.get(&b),
            ) {
                (Some(here), Some(there)) => here.is_open(face) && there.is_open(face.opposite()),
                _ => false,
            };

            let origin = Vec3::from_array(hex_origin(a)) + Vec3::Y * LIFT;
            let (from, to) = face_edge(face);
            if walkable {
                report.open += 1;
                crossings += 1;
                open_lines.segment(origin + from, origin + to);
            } else {
                sealed_lines.segment(origin + from, origin + to);
            }
        }
        report.widest = report.widest.max(crossings);
    }

    for (batch, role) in [
        (sealed_lines, SchematicRole::Pinned),
        (open_lines, SchematicRole::Volatile),
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
