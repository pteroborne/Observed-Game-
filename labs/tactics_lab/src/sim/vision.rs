//! What the squad observes, and therefore what the solver may not touch.
//!
//! Observation here is a bounded flood through **open ports**, not a radius.
//! That is the readable-constraint rule from `agents.md` applied to sight: a
//! wall between two cells stops observation because it stops movement, so a
//! player can read the freeze from the same lines they read the route from. A
//! radius would be cheaper and would leak through solid rock.

use std::collections::BTreeSet;

use observed_facility::hex_wfc::{HexObservationFrame, HexSpace, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass, ports_compatible};

/// Cells within `hops` open ports of `origin`, including `origin` itself.
///
/// Pure and total: reads the lattice, allocates one set, and never fails. Cells
/// outside the grid and `Void` cells simply do not appear.
#[must_use]
pub fn observed_from(world: &HexWfcWorld, origin: HexCoord, hops: u8) -> BTreeSet<HexCoord> {
    let mut seen = BTreeSet::new();
    if !is_solid_space(world, origin) {
        return seen;
    }
    seen.insert(origin);
    let mut frontier = vec![origin];
    for _ in 0..hops {
        let mut next = Vec::new();
        for cell in frontier.drain(..) {
            for face in HexFace::ALL {
                let Some(neighbor) = step_through(world, cell, face) else {
                    continue;
                };
                if seen.insert(neighbor) {
                    next.push(neighbor);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    seen
}

/// The cell reached by leaving `from` through `face`, or `None` when that face
/// is not passable.
///
/// **This is the movement rule as well as the sight rule.** A lateral face needs
/// an open door; a vertical face needs a ramp or shaft opening, and needs it
/// from *both* sides — a shaft you can fall down is not a shaft you can climb,
/// and the solver states the pairing on both cells.
#[must_use]
pub fn step_through(world: &HexWfcWorld, from: HexCoord, face: HexFace) -> Option<HexCoord> {
    let placement = world.placements.get(&from)?;
    let ports = placement.ports();
    let leaving = ports.port(face);
    if leaving == PortClass::Sealed {
        return None;
    }
    let neighbor = world.config.grid().neighbor(from, face)?;
    let arriving = world.placements.get(&neighbor)?;
    if arriving.space == HexSpace::Void {
        return None;
    }
    // The same relation the solver bonds faces with, rather than a local
    // "not sealed" test: a `Door` facing a `ShaftOpen` is not a passage, and
    // re-deriving that here is how the two would eventually disagree.
    ports_compatible(leaving, arriving.ports().port(face.opposite())).then_some(neighbor)
}

/// Every face `from` can currently be left through.
#[must_use]
pub fn exits(world: &HexWfcWorld, from: HexCoord) -> Vec<(HexFace, HexCoord)> {
    HexFace::ALL
        .into_iter()
        .filter_map(|face| step_through(world, from, face).map(|cell| (face, cell)))
        .collect()
}

/// Whether a cell is real space a unit could stand in.
#[must_use]
pub fn is_solid_space(world: &HexWfcWorld, cell: HexCoord) -> bool {
    world
        .placements
        .get(&cell)
        .is_some_and(|placement| placement.space != HexSpace::Void)
}

/// Seed an observation frame with the landmark cells the solver must always
/// protect: every stamped room, whatever anyone can see.
///
/// Mirrors `HexWfcMatch::build_observation`, which does the same before handing
/// the frame to relayout. Rooms are the fixed points a player navigates by, and
/// a facility whose rooms move is one nobody can hold a map of.
pub fn seed_landmarks(world: &HexWfcWorld, frame: &mut HexObservationFrame) {
    frame.landmark_cells.extend(
        world
            .blueprints
            .iter()
            .flat_map(|blueprint| blueprint.cells.iter().copied()),
    );
}
