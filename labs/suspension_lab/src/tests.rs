//! Assertions first, then the two reports.

use crate::measure::{cascade_sizes, sight_comparison, suspended_deck, tower_with_an_atrium};
use crate::{CANTILEVER_REACH, Site, Space};

/// The rule the whole lab rests on: sight crosses air, movement does not.
#[test]
fn sight_crosses_air_and_stops_at_solid() {
    let mut site = Site::new(8, 1, 1);
    site.set((0, 0, 0), Space::Floor);
    for x in 1..6 {
        site.set((x, 0, 0), Space::Air);
    }
    site.set((6, 0, 0), Space::Floor);
    site.set((7, 0, 0), Space::Floor);

    let seen = site.sightline((0, 0, 0), (1, 0), 16);
    assert_eq!(
        seen.len(),
        6,
        "five cells of air and the wall behind them, then nothing past it: {seen:?}"
    );
    assert_eq!(*seen.last().unwrap(), (6, 0, 0));

    assert!(
        site.step_sightline((0, 0, 0), (1, 0)).is_empty(),
        "the movement-shaped rule sees nothing at all across an atrium, which is the defect"
    );
}

/// A deck reaching past the cantilever is not standing on anything.
#[test]
fn a_platform_beyond_the_cantilever_is_unsupported() {
    let deck = suspended_deck(CANTILEVER_REACH + 4, 1, 2);
    let supported = deck.supported();
    let far_end = (CANTILEVER_REACH + 3, 0, 1);
    assert_eq!(deck.get(far_end), Space::Floor);
    assert!(
        !supported.contains(&far_end),
        "a deck {CANTILEVER_REACH}+ cells out over the air has nothing carrying it"
    );
    let near = (1, 0, 1);
    assert!(
        supported.contains(&near),
        "the cell beside the column is carried by it"
    );
}

/// Pull the column and the deck it carried comes down with it.
#[test]
fn retracting_a_column_drops_what_it_carried() {
    let mut deck = suspended_deck(6, 1, 2);
    let standing_before = deck.solids().len();
    let fell = deck.retract((0, 0, 1));
    assert!(
        fell.len() > 1,
        "removing a support must take more than itself: {fell:?}"
    );
    assert!(
        deck.solids().len() < standing_before,
        "the site is smaller afterwards"
    );
}

/// Ground level carries itself, or nothing could ever be built.
#[test]
fn the_ground_carries_itself() {
    let mut site = Site::new(4, 4, 1);
    for x in 0..4 {
        for y in 0..4 {
            site.set((x, y, 0), Space::Floor);
        }
    }
    assert_eq!(site.supported().len(), 16);
    let fell = site.retract((0, 0, 0));
    assert_eq!(fell.len(), 1, "ground does not cascade: {fell:?}");
}

/// Question 1, reported: what does seeing across open air cost?
#[test]
fn report_what_sight_across_air_costs() {
    println!("\n============== SIGHT: STEP RULE vs ACROSS AIR ==============");
    for (name, site) in [
        ("tower 8x4x3", tower_with_an_atrium(8, 4, 3)),
        ("tower 12x6x4", tower_with_an_atrium(12, 6, 4)),
        ("deck 8x3x2", suspended_deck(8, 3, 2)),
    ] {
        for range in [4, 8, 16] {
            let (old, new) = sight_comparison(&site, range);
            let ratio = if old == 0 {
                f64::INFINITY
            } else {
                new as f64 / old as f64
            };
            println!(
                "{name:>14} range {range:>2}: step rule sees {old:>4}, across air {new:>4}  ({ratio:.1}x)"
            );
        }
    }
    println!("===========================================================\n");
}

/// Question 2, reported: does a support cascade read as drama or as a softlock?
#[test]
fn report_what_a_cascade_takes_down() {
    println!("\n============== CASCADE SIZE PER RETRACTION ==============");
    for (name, site) in [
        ("tower 8x4x3", tower_with_an_atrium(8, 4, 3)),
        ("tower 12x6x4", tower_with_an_atrium(12, 6, 4)),
        ("deck 8x3x2", suspended_deck(8, 3, 2)),
    ] {
        let standing = site.solids().len();
        let histogram = cascade_sizes(&site);
        let worst = histogram.keys().copied().max().unwrap_or(0);
        let singles = histogram.get(&1).copied().unwrap_or(0);
        let total = histogram.values().sum::<usize>();
        println!(
            "{name:>14}: {standing} solid cells, worst cascade {worst} ({:.0}% of the site), \
             {singles} of {total} removals take only themselves",
            100.0 * worst as f64 / standing as f64
        );
        println!(
            "{:>14}  sizes (cells down -> how many removals): {histogram:?}",
            ""
        );
    }
    println!("========================================================\n");
}

/// Does the model's 2x survive contact with the real solver?
#[test]
fn report_sight_on_real_facilities() {
    use crate::facility::{facility_sight, scenario_configs, solve};
    println!("\n============== SIGHT ON GENERATED FACILITIES ==============");
    for (name, config) in scenario_configs() {
        for range in [4usize, 8, 16] {
            let mut watchers = 0usize;
            let mut stepped = 0usize;
            let mut across = 0usize;
            let mut solved = 0usize;
            for seed in 0..8u64 {
                let Some(world) = solve(config, seed) else {
                    continue;
                };
                solved += 1;
                let (w, s, a) = facility_sight(&world, range);
                watchers += w;
                stepped += s;
                across += a;
            }
            let ratio = if stepped == 0 {
                f64::INFINITY
            } else {
                across as f64 / stepped as f64
            };
            println!(
                "{name:>12} range {range:>2}: {solved} seeds, {watchers} watchers, \
                 stepped {stepped:>5}, across air {across:>5}  ({ratio:.1}x)"
            );
        }
    }
    println!("===========================================================\n");
}

/// `mark_open_air` must find the sky and leave sealed pockets alone.
#[test]
fn open_air_reaches_the_outside_and_stops_at_sealed_pockets() {
    use crate::facility::{scenario_configs, solve};
    use observed_facility::hex_wfc::HexSpace;

    let (_, config) = scenario_configs()[2];
    let mut world = solve(config, 3).expect("Full Ascent solves");
    let unbuilt_before = world
        .placements
        .values()
        .filter(|p| p.space.unbuilt())
        .count();
    assert!(
        unbuilt_before > 0,
        "the scenario has unbuilt cells to classify"
    );

    let changed = world.mark_open_air();
    assert!(
        changed > 0,
        "a facility in a bounded lattice touches the outside"
    );

    let air = world
        .placements
        .values()
        .filter(|p| p.space == HexSpace::Air)
        .count();
    let rock = world
        .placements
        .values()
        .filter(|p| p.space == HexSpace::Void)
        .count();
    assert_eq!(
        air + rock,
        unbuilt_before,
        "the pass moves cells between the two unbuilt states and touches nothing else"
    );

    // Idempotent: the second pass has nothing left to find.
    assert_eq!(world.mark_open_air(), 0);

    // Deterministic: the same seed classifies the same cells.
    let mut again = solve(config, 3).expect("solves");
    let _ = again.mark_open_air();
    assert_eq!(world.placements, again.placements);
}

/// How much of a facility is open air, and how much is entombed rock?
#[test]
fn report_how_much_is_sky() {
    use crate::facility::{scenario_configs, solve};
    use observed_facility::hex_wfc::HexSpace;
    println!("\n============== AIR vs ROCK vs BUILT ==============");
    for (name, config) in scenario_configs() {
        let (mut air, mut rock, mut built, mut seeds) = (0usize, 0usize, 0usize, 0usize);
        for seed in 0..8u64 {
            let Some(mut world) = solve(config, seed) else {
                continue;
            };
            seeds += 1;
            let _ = world.mark_open_air();
            for placement in world.placements.values() {
                match placement.space {
                    HexSpace::Air => air += 1,
                    HexSpace::Void => rock += 1,
                    HexSpace::Room | HexSpace::Hall => built += 1,
                }
            }
        }
        let total = air + rock + built;
        println!(
            "{name:>12}: {seeds} seeds, {total} cells -> built {built} ({:.0}%), \
             air {air} ({:.0}%), sealed rock {rock} ({:.0}%)",
            100.0 * built as f64 / total as f64,
            100.0 * air as f64 / total as f64,
            100.0 * rock as f64 / total as f64
        );
    }
    println!("==================================================\n");
}
