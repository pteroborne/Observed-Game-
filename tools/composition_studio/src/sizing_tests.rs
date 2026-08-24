//! Facility-sizing instruments for Arc T's T-4.
//!
//! A file of its own rather than another section of [`super::authoring_tests`],
//! which the addition took past the 600-line review budget the rest of the WFC
//! path lives under. The same reason `widget_tests` exists.

use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};

/// T-4's instrument, run at production scale rather than at the studio default.
///
/// Backlog #33 asks how much of the facility the corpus can meaningfully fill.
/// "Meaningfully" is the whole question, and the projector's verdict does not
/// answer it: that only says every demand resolved to *something*. A facility
/// can project perfectly and still be one shape repeated five thousand times.
///
/// So three numbers rather than one. How many cells the corpus answers with
/// geometry authored for their own register (`exact`) against the shared
/// generic kit (`generic`); how many distinct authored tiles actually appear;
/// and how many cells each of those tiles has to cover.
#[test]
#[ignore = "T-4 sizing instrument"]
fn survey_how_much_of_the_facility_the_corpus_can_fill() {
    use observed_facility::hex_wfc::{HexRoomQuotas, HexSpace};
    use std::collections::BTreeSet;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    println!(
        "lattice {}x{}x{} = {} cells",
        config.cols,
        config.rows,
        config.levels,
        usize::from(config.cols) * usize::from(config.rows) * usize::from(config.levels),
    );
    println!(
        "seed              hall  room  void   placed  exact%  generic%  missing  \
         keys  modules  shapes  cells/module"
    );
    for seed in (0..6u64).map(|i| 0xA11C_E3D0_0000_0000 ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15)) {
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            println!("{seed:016x}  UNSOLVED");
            continue;
        };
        let projected = observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(
            &world, cells, rooms,
        );
        let (snapshot, error) = match projected {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error)),
        };
        let coverage = crate::coverage::CoverageReport::build(
            &world,
            cells,
            rooms,
            snapshot.as_ref(),
            error.as_ref(),
        );
        let count = |space| {
            world
                .placements
                .values()
                .filter(|p| p.space == space)
                .count()
        };
        let by = |want: crate::coverage::Supply| -> usize {
            coverage
                .demanded
                .iter()
                .filter(|row| row.supply == want)
                .map(|row| row.cells as usize)
                .sum()
        };
        let placed: usize = coverage.demanded.iter().map(|row| row.cells as usize).sum();
        // Distinct authored tiles the projector actually selected. This is the
        // number the "amateur, samey" reports are really about.
        //
        // Counted three ways, because the first one flatters. A `TileKey` is
        // (archetype, register, variant) and the compiler expands one authored
        // module into many variants, so counting keys counts *variation* and
        // reports thousands. What a player can tell apart is nearer the shape
        // and the dressing, so the module and the archetype are counted beside
        // it and the three are meant to be read together.
        let distinct = |f: fn(&observed_authoring::TileKey) -> (String, String, u16)| -> usize {
            snapshot
                .as_ref()
                .map(|snapshot| {
                    snapshot
                        .pieces
                        .iter()
                        .filter_map(|piece| piece.tile.as_ref())
                        .map(f)
                        .collect::<BTreeSet<_>>()
                        .len()
                })
                .unwrap_or(0)
        };
        let keys = distinct(|t| (t.archetype.clone(), t.register.clone(), t.variant));
        let modules = distinct(|t| (t.archetype.clone(), t.register.clone(), 0u16));
        let shapes = distinct(|t| (t.archetype.clone(), String::new(), 0u16));
        println!(
            "{seed:016x}  {:>4}  {:>4}  {:>4}  {placed:>7}  {:>5.1}%  {:>7.1}%  {:>7}  \
             {keys:>4}  {modules:>7}  {shapes:>6}  {:>12.1}",
            count(HexSpace::Hall),
            count(HexSpace::Room),
            count(HexSpace::Void),
            by(crate::coverage::Supply::Exact) as f64 * 100.0 / placed.max(1) as f64,
            by(crate::coverage::Supply::GenericFallback) as f64 * 100.0 / placed.max(1) as f64,
            by(crate::coverage::Supply::Missing),
            placed as f64 / modules.max(1) as f64,
        );
    }
}
