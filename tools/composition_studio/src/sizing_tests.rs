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

/// The sizing sweep T-4 actually asks for: what changes when the lattice does.
///
/// Six candidate lattices against the six baseline seeds, reporting the three
/// things #33 puts in tension. **Cost** is the LAN ceiling - the solve runs
/// synchronously on the desync and late-join paths against a 2 s client
/// timeout, so a size that solves in four seconds is not a size. **Traversal**
/// is the finding itself: "a match is spent traversing rather than deciding",
/// which is the spawn-to-exit route in cells. **Density** is what a smaller
/// facility is supposed to buy - fewer cells over the same corpus means each
/// authored module covers less ground.
///
/// Rooms are held at the production quota of thirty throughout. They are
/// gameplay content rather than a size lever: dropping a Decision room to make
/// the facility smaller removes a decision, which is the opposite of the point.
/// A lattice that cannot seat thirty rooms reports `UNSOLVED` and is out.
#[test]
#[ignore = "T-4 sizing sweep"]
fn survey_what_changes_when_the_lattice_does() {
    use observed_facility::hex_wfc::{HexFace, HexRoomQuotas, HexSpace};
    use std::collections::BTreeSet;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let quotas = HexRoomQuotas::for_team_count(2);
    let seeds: Vec<u64> = (0..6u64)
        .map(|i| 0xA11C_E3D0_0000_0000 ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .collect();

    println!(
        "lattice        cells  solved  hall  void%  attempts  slowest  route  \\
         modules  cells/mod  deg2%  deg4+%"
    );
    for &(cols, rows, levels) in &[
        // The current size, then the area axis at fixed height, then the height
        // axis at fixed area. Which of the two drives the cliff is the question
        // - a facility can be made smaller by taking floors off or by taking
        // ground away, and they are not the same edit.
        (28u16, 20u16, 10u8),
        (28, 20, 8),
        (28, 20, 6),
        (24, 17, 10),
        (24, 17, 8),
        (24, 17, 6),
        (22, 16, 8),
        (21, 15, 8),
        (20, 14, 10),
        (20, 14, 8),
        (18, 13, 8),
        (16, 12, 8),
    ] {
        let config = HexWfcConfig {
            cols,
            rows,
            levels,
            ..HexWfcConfig::arc_default()
        };
        let grid = config.grid();
        let total = usize::from(cols) * usize::from(rows) * usize::from(levels);

        let (mut solved, mut halls, mut voids, mut placed_cells) = (0usize, 0usize, 0usize, 0usize);
        let mut worst_attempts = 0u32;
        let mut slowest = std::time::Duration::ZERO;
        let mut route_sum = 0usize;
        let mut modules: BTreeSet<(String, String)> = BTreeSet::new();
        let mut degrees = [0usize; 7];

        for &seed in &seeds {
            let started = std::time::Instant::now();
            let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
                continue;
            };
            slowest = slowest.max(started.elapsed());
            worst_attempts = worst_attempts.max(world.last_attempts);
            solved += 1;
            for (&coord, placement) in &world.placements {
                match placement.space {
                    HexSpace::Void => voids += 1,
                    HexSpace::Hall => {
                        degrees[HexFace::LATERAL
                            .into_iter()
                            .filter(|&face| {
                                placement.is_open(face)
                                    && grid.neighbor(coord, face).is_some_and(|next| {
                                        world
                                            .placements
                                            .get(&next)
                                            .is_some_and(|there| there.is_open(face.opposite()))
                                    })
                            })
                            .count()] += 1;
                        halls += 1;
                    }
                    HexSpace::Room => {}
                }
            }
            route_sum += world
                .route_between(config.spawn(), config.exit())
                .map_or(0, |route| route.len());
            if let Ok(snapshot) =
                observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(
                    &world, cells, rooms,
                )
            {
                // Distinct *cells* that received a tile, not pieces: one cell
                // projects many colliders, and counting those reports a number
                // an order of magnitude too large for the thing it is named for.
                let mut seen: BTreeSet<_> = BTreeSet::new();
                for piece in &snapshot.pieces {
                    if let Some(tile) = &piece.tile {
                        modules.insert((tile.archetype.clone(), tile.register.clone()));
                        seen.insert(piece.source_cell);
                    }
                }
                placed_cells += seen.len();
            }
        }

        if solved == 0 {
            println!("{cols}x{rows}x{levels:<4}  {total:>7}  UNSOLVED at every seed");
            continue;
        }
        let n = solved as f64;
        println!(
            "{:<12}  {total:>5}  {solved:>6}  {:>4}  {:>4.1}  {worst_attempts:>8}  {:>6.2}s  \
             {:>5.0}  {:>7}  {:>9.1}  {:>5.1}  {:>6.1}",
            format!("{cols}x{rows}x{levels}"),
            (halls as f64 / n) as usize,
            voids as f64 * 100.0 / (total * solved) as f64,
            slowest.as_secs_f64(),
            route_sum as f64 / n,
            modules.len(),
            placed_cells as f64 / n / modules.len().max(1) as f64,
            degrees[2] as f64 * 100.0 / halls.max(1) as f64,
            degrees[4..].iter().sum::<usize>() as f64 * 100.0 / halls.max(1) as f64,
        );
    }
}

/// What a facility is actually built out of, by shape.
///
/// The companion to the density number: `cells/module` says how hard each
/// authored thing is working, and this says what the things *are*. T-5 and T-6
/// both start here, because "the corpus cannot compose places you can tell
/// apart" is a claim about this list rather than about the catalog's size.
#[test]
#[ignore = "T-4 sizing instrument"]
fn survey_what_shapes_the_facility_is_built_from() {
    use observed_facility::hex_wfc::HexRoomQuotas;
    use std::collections::BTreeMap;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    let mut by_shape: BTreeMap<String, (usize, std::collections::BTreeSet<String>)> =
        BTreeMap::new();
    let mut seeds = 0usize;
    for i in 0..6u64 {
        let seed = 0xA11C_E3D0_0000_0000 ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            continue;
        };
        let Ok(snapshot) = observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(
            &world, cells, rooms,
        ) else {
            continue;
        };
        seeds += 1;
        let mut seen: BTreeMap<String, std::collections::BTreeSet<_>> = BTreeMap::new();
        for piece in &snapshot.pieces {
            if let Some(tile) = &piece.tile {
                let entry = by_shape.entry(tile.archetype.clone()).or_default();
                entry.1.insert(tile.register.clone());
                seen.entry(tile.archetype.clone())
                    .or_default()
                    .insert(piece.source_cell);
            }
        }
        for (archetype, cells) in seen {
            by_shape.entry(archetype).or_default().0 += cells.len();
        }
    }

    let total: usize = by_shape.values().map(|(n, _)| *n).sum();
    println!("shape                   cells/facility   share  registers");
    for (archetype, (n, registers)) in &by_shape {
        println!(
            "{archetype:<22}  {:>13.0}  {:>5.1}%  {:>9}",
            *n as f64 / seeds.max(1) as f64,
            *n as f64 * 100.0 / total.max(1) as f64,
            registers.len(),
        );
    }
    println!(
        "\n{} distinct shapes across {seeds} facilities",
        by_shape.len()
    );
}

/// Which *reading* of each archetype the facility actually places.
///
/// The generated kit gives several archetypes more than one interior: an
/// expanse is either a pure volume or a pair of off-centre piers, a junction
/// either carries the waypoint pylon or leaves the crossing clear. Those are
/// the corpus's own answer to "places you can tell apart", and whether they
/// reach a facility in any useful proportion is a question about the *lottery*
/// rather than about the geometry.
///
/// Runtime variant encodes the reading above `READING_STRIDE` (64) and the door
/// mask below it, so the reading is the quotient.
#[test]
#[ignore = "T-4 sizing instrument"]
fn survey_which_readings_reach_the_facility() {
    use observed_facility::hex_wfc::HexRoomQuotas;
    use std::collections::BTreeMap;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);
    const READING_STRIDE: u16 = 64;

    let mut tally: BTreeMap<(String, u16), usize> = BTreeMap::new();
    for i in 0..6u64 {
        let seed = 0xA11C_E3D0_0000_0000 ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            continue;
        };
        let Ok(snapshot) = observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(
            &world, cells, rooms,
        ) else {
            continue;
        };
        let mut seen: BTreeMap<(String, u16), std::collections::BTreeSet<_>> = BTreeMap::new();
        for piece in &snapshot.pieces {
            if let Some(tile) = &piece.tile {
                seen.entry((tile.archetype.clone(), tile.variant / READING_STRIDE))
                    .or_default()
                    .insert(piece.source_cell);
            }
        }
        for (key, cells) in seen {
            *tally.entry(key).or_default() += cells.len();
        }
    }
    println!("archetype               reading   cells/facility");
    for ((archetype, reading), n) in &tally {
        println!("{archetype:<22}  {reading:>7}  {:>14.0}", *n as f64 / 6.0);
    }
}
