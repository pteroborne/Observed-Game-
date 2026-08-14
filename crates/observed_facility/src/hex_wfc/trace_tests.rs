//! Gates for the trace fold.
//!
//! The fold is what any step-by-step viewer draws from, so the load-bearing
//! claim is that replaying a whole trace lands on the world the solver actually
//! returned. Everything else here guards a case a real trace reaches rarely
//! enough that a viewer would ship broken on it: a retry, and a contradiction.

use super::*;

fn cell(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

fn placed(coord: HexCoord) -> SolveStep {
    SolveStep::Collapsed {
        coord,
        space: HexSpace::Hall,
        archetype: HexArchetype::Straight,
        doors: 0,
        up: PortClass::Sealed,
        down: PortClass::Sealed,
    }
}

#[test]
fn a_full_fold_reproduces_the_solved_world() {
    for n in 0..8u64 {
        let seed = 0xA11C_E3D0_0000_0000 | n;
        let (world, steps) = HexWfcWorld::generate_traced(seed, HexWfcConfig::default())
            .expect("compact fixture solves");
        let folded = fold_trace(&steps);

        let replayed: BTreeMap<HexCoord, HexPlacement> = folded
            .iter()
            .filter_map(|(&coord, cell)| cell.resolved.map(|placement| (coord, placement)))
            .collect();
        assert_eq!(
            replayed, world.placements,
            "replaying {seed:#018x} in full must land on the world the solver returned"
        );
    }
}

#[test]
fn the_summary_counts_what_the_fold_holds() {
    let (world, steps) =
        HexWfcWorld::generate_traced(0xA11C_E3D0_0000_0008, HexWfcConfig::default())
            .expect("compact fixture solves");

    // Partway through, and at the end. A summary that only agreed at the end
    // would be useless to a readout that counts up as a replay plays.
    for cursor in [steps.len() / 3, steps.len() / 2, steps.len()] {
        let prefix = &steps[..cursor];
        let folded = fold_trace(prefix);
        let summary = summarise_trace(prefix);
        let resolved = folded.values().filter(|c| c.resolved.is_some()).count();
        assert_eq!(summary.collapsed as usize, resolved, "at cursor {cursor}");
        let open = folded
            .values()
            .filter(|c| c.resolved.is_none() && c.pruned_to.is_some())
            .count();
        assert_eq!(summary.open as usize, open, "at cursor {cursor}");
    }

    let whole = summarise_trace(&steps);
    assert_eq!(whole.collapsed as usize, world.placements.len());
    assert_eq!(whole.open, 0, "a finished solve leaves nothing undecided");
    assert_eq!(whole.open_domain, 0);
    assert!(whole.completed.is_some());
}

#[test]
fn collapsed_never_goes_backwards_within_an_attempt() {
    let (_, steps) = HexWfcWorld::generate_traced(0xA11C_E3D0_0000_0011, HexWfcConfig::default())
        .expect("compact fixture solves");
    let mut previous = 0;
    for cursor in 0..=steps.len() {
        let summary = summarise_trace(&steps[..cursor]);
        assert!(
            summary.collapsed >= previous,
            "collapsed fell from {previous} to {} at cursor {cursor}",
            summary.collapsed
        );
        previous = summary.collapsed;
    }
}

#[test]
fn an_abandoned_attempt_leaves_nothing_behind() {
    // A retry starts from an empty lattice. Carrying the abandoned attempt's
    // cells forward would draw a facility that never existed.
    let steps = vec![
        SolveStep::AttemptStart { attempt: 0 },
        placed(cell(1, 1)),
        placed(cell(2, 1)),
        SolveStep::Contradiction { coord: cell(3, 1) },
        SolveStep::AttemptStart { attempt: 1 },
        placed(cell(4, 1)),
    ];

    let folded = fold_trace(&steps);
    assert_eq!(folded.len(), 1, "only the live attempt's cell survives");
    assert!(folded.contains_key(&cell(4, 1)));

    let summary = summarise_trace(&steps);
    assert_eq!(summary.collapsed, 1);
    assert_eq!(
        summary.attempt, 2,
        "attempts are reported counting from one"
    );
    assert_eq!(
        summary.contradictions, 0,
        "the failed attempt's contradiction does not follow it into the retry"
    );
}

#[test]
fn a_contradiction_unresolves_the_cell_and_says_so() {
    let steps = vec![
        SolveStep::AttemptStart { attempt: 0 },
        SolveStep::DomainPruned {
            coord: cell(1, 1),
            remaining: 3,
        },
        placed(cell(1, 1)),
        SolveStep::Contradiction { coord: cell(1, 1) },
    ];

    let folded = fold_trace(&steps);
    let cell = folded[&cell(1, 1)];
    assert!(
        cell.resolved.is_none(),
        "the solver no longer stands behind a contradicted placement"
    );
    assert!(
        cell.contradicted,
        "and the viewer needs to be able to say so"
    );
    assert_eq!(cell.pruned_to, Some(3), "the domain it narrowed to is kept");
    assert_eq!(summarise_trace(&steps).contradictions, 1);
}

#[test]
fn an_untraced_world_reads_as_wholly_collapsed() {
    // The production path does not trace. A viewer that only knew how to fold
    // steps would show an empty facility for a world that is right there.
    let world = HexWfcWorld::generate(0xA11C_E3D0_0000_0023, HexWfcConfig::default())
        .expect("compact fixture solves");
    let cells = cells_from_world(&world);

    assert_eq!(cells.len(), world.placements.len());
    assert!(cells.values().all(|cell| cell.resolved.is_some()));
    for (coord, cell) in &cells {
        assert_eq!(cell.resolved, world.placements.get(coord).copied());
    }

    let roles = cells.values().filter(|cell| cell.role.is_some()).count();
    let blueprint_cells: usize = world.blueprints.iter().map(|b| b.cells.len()).sum();
    assert!(blueprint_cells > 0, "the fixture stamps rooms");
    assert_eq!(roles, blueprint_cells, "every stamped cell keeps its role");
}
