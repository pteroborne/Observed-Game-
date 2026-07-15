//! Catalogue-v2 success-criteria corpus plus the retained v1 topology regression.
//! Run with `cargo test -p wfc_proc_gen_lab -- --nocapture` to see the summary table.

use std::collections::{BTreeSet, VecDeque};

use observed_facility::map_spec::{ArchitectureRegister, MapSpec, RoomRole};
use observed_facility::wfc::{WfcMapConfig, generate_liminal_map, generate_liminal_map_v2};
use observed_match::facility::CompetitiveFacility;

const CORPUS_SEEDS: std::ops::Range<u64> = 0..50;

struct SeedResult {
    seed: u64,
    room_count: usize,
    edge_count: usize,
    monitor_count: usize,
}

#[test]
fn catalogue_v2_is_deterministic_across_the_corpus() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let first = generate_liminal_map_v2(seed, &config)
            .unwrap_or_else(|error| panic!("v2 seed {seed}: {error:?}"));
        let second = generate_liminal_map_v2(seed, &config)
            .unwrap_or_else(|error| panic!("v2 seed {seed}: {error:?}"));
        assert_eq!(first, second, "v2 seed {seed} must regenerate identically");
    }
}

#[test]
fn catalogue_v2_has_four_to_six_connected_architecture_regions_per_map() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let spec = generate_liminal_map_v2(seed, &config)
            .unwrap_or_else(|error| panic!("v2 seed {seed}: {error:?}"));
        spec.validate()
            .unwrap_or_else(|errors| panic!("v2 seed {seed}: {errors:?}"));
        let designs = spec
            .designs
            .as_ref()
            .expect("v2 carries design assignments");
        assert!(
            (4..=6).contains(&designs.register_count()),
            "v2 seed {seed}: expected 4-6 regions, got {}",
            designs.register_count()
        );

        for register in ArchitectureRegister::ALL {
            let region: BTreeSet<_> = designs
                .rooms
                .values()
                .filter_map(|design| (design.register == register).then_some(design.room))
                .collect();
            if region.is_empty() {
                continue;
            }

            let start = *region.iter().next().expect("non-empty region");
            let mut reached = BTreeSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some(room) = queue.pop_front() {
                for neighbor in spec.neighbors(room) {
                    if region.contains(&neighbor) && reached.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
            assert_eq!(
                reached,
                region,
                "v2 seed {seed}: {} region is disconnected",
                register.label()
            );
        }
    }
}

#[test]
fn catalogue_v2_corpus_exercises_all_nine_architecture_registers() {
    let config = WfcMapConfig::default();
    let mut seen = BTreeSet::new();
    for seed in CORPUS_SEEDS {
        let spec = generate_liminal_map_v2(seed, &config)
            .unwrap_or_else(|error| panic!("v2 seed {seed}: {error:?}"));
        seen.extend(
            spec.designs
                .as_ref()
                .expect("v2 carries design assignments")
                .rooms
                .values()
                .map(|design| design.register),
        );
    }
    assert_eq!(
        seen,
        ArchitectureRegister::ALL.into_iter().collect(),
        "the 50-seed corpus must exercise the complete production catalogue"
    );
}

/// (a) Generation is deterministic by seed: regenerating the same seed produces a
/// byte-identical `MapSpec` (compared via `PartialEq`, which is the crate's own
/// "digest" — every field participates).
#[test]
fn generation_is_deterministic_by_seed_across_the_corpus() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let first =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let second =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        assert_eq!(first, second, "seed {seed} must regenerate identically");
    }
}

/// (b) Every seed in the corpus generates `Ok` within the configured retry budget,
/// with a dense room count in `[24, 32]` and dense `RoomId(0..n)` coverage (no gaps —
/// checked by confirming every id in range resolves via `MapSpec::room`).
#[test]
fn every_corpus_seed_generates_in_range_with_dense_room_ids() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let spec =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        assert!(
            (config.min_rooms..=config.max_rooms).contains(&spec.room_count()),
            "seed {seed}: room count {} out of [{}, {}]",
            spec.room_count(),
            config.min_rooms,
            config.max_rooms
        );
        for id in 0..spec.room_count() as u32 {
            assert!(
                spec.room(observed_core::RoomId(id)).is_some(),
                "seed {seed}: RoomId({id}) missing — dense ids broke"
            );
        }
    }
}

/// (c) `MapSpec::validate()` passes for every corpus seed: unique rooms/endpoints,
/// every required role present exactly once (or at least once, for repeatable
/// roles), connectivity, redundancy (no single-edge-cut anywhere), and objective
/// recovery (keystone/relay/anchor/recovery/dual-station rooms survive any single
/// cut too).
#[test]
fn every_corpus_seed_passes_full_map_spec_validation() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let spec =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        spec.validate()
            .unwrap_or_else(|errors| panic!("seed {seed}: validate() failed: {errors:?}"));
    }
}

/// (d) Monitor paging coverage: every seed has at least `ceil(room_count / 9)`
/// `RoomRole::Monitor` rooms, enough to page every room in the facility at 9-panel
/// density (Arc D's D1 monitor work assumes this floor).
#[test]
fn every_corpus_seed_has_enough_monitor_rooms_to_page_every_room() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let spec =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let monitors = spec.rooms_with_role(RoomRole::Monitor).len();
        let required = spec.room_count().div_ceil(9).max(1);
        assert!(
            monitors >= required,
            "seed {seed}: {monitors} monitor rooms, needs >= {required} for {} rooms",
            spec.room_count()
        );
    }
}

/// (e) Objective-sequence coherence: `CompetitiveFacility::for_map_spec` builds
/// cleanly (which itself panics on an invalid spec via `validate_or_panic`), its
/// `objective_sequence()` is non-empty and ends at the exit room, and
/// `collapse_rooms()` at the opening purge line (`purge_line == -PURGE_GAP`, i.e. no
/// progress yet) is empty — the progress/collapse model must not choke on a freshly
/// generated map before anyone has moved.
#[test]
fn every_corpus_seed_produces_a_coherent_objective_sequence() {
    let config = WfcMapConfig::default();
    for seed in CORPUS_SEEDS {
        let spec =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let exit = spec.exit_room().expect("validated spec has an exit room");

        let facility = CompetitiveFacility::for_map_spec(spec);
        let sequence = facility.objective_sequence();
        assert!(
            !sequence.is_empty(),
            "seed {seed}: objective sequence must not be empty"
        );
        assert_eq!(
            *sequence.last().unwrap(),
            exit,
            "seed {seed}: objective sequence must end at the exit room"
        );

        // At match start no team has progressed, so the collapse has claimed nothing
        // yet — this is the "must not choke on generated maps" floor the phase brief
        // calls out explicitly.
        assert!(
            facility.collapse_rooms().is_empty(),
            "seed {seed}: collapse_rooms() at the opening purge line must be empty"
        );
    }
}

/// (f) Corpus summary table for manual review (`cargo test -p wfc_proc_gen_lab --
/// --nocapture`): room counts (min/mean/max), edge counts, and monitor coverage
/// across the full 50-seed sweep, plus how many seeds needed more than one WFC
/// attempt (the retry budget is a safety net, not the whole strategy — see
/// `observed_facility::wfc`'s module docs).
#[test]
fn corpus_summary_table_for_manual_review() {
    let config = WfcMapConfig::default();
    let mut results = Vec::new();
    for seed in CORPUS_SEEDS {
        let spec: MapSpec =
            generate_liminal_map(seed, &config).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        results.push(SeedResult {
            seed,
            room_count: spec.room_count(),
            edge_count: spec.edges.len(),
            monitor_count: spec.rooms_with_role(RoomRole::Monitor).len(),
        });
    }

    let n = results.len() as f64;
    let room_min = results.iter().map(|r| r.room_count).min().unwrap_or(0);
    let room_max = results.iter().map(|r| r.room_count).max().unwrap_or(0);
    let room_mean = results.iter().map(|r| r.room_count).sum::<usize>() as f64 / n;
    let edge_min = results.iter().map(|r| r.edge_count).min().unwrap_or(0);
    let edge_max = results.iter().map(|r| r.edge_count).max().unwrap_or(0);
    let edge_mean = results.iter().map(|r| r.edge_count).sum::<usize>() as f64 / n;
    let monitor_min = results.iter().map(|r| r.monitor_count).min().unwrap_or(0);
    let monitor_max = results.iter().map(|r| r.monitor_count).max().unwrap_or(0);

    println!(
        "\n=== WFC liminal-map corpus summary ({} seeds) ===",
        results.len()
    );
    println!(
        "rooms:    min={room_min}  mean={room_mean:.1}  max={room_max}  (target [{}, {}])",
        config.min_rooms, config.max_rooms
    );
    println!("edges:    min={edge_min}  mean={edge_mean:.1}  max={edge_max}");
    println!("monitors: min={monitor_min}  max={monitor_max}");
    println!("retry_budget configured: {}", config.retry_budget);
    println!("per-seed room/edge/monitor counts:");
    for r in &results {
        println!(
            "  seed {:>3}: rooms={:>2} edges={:>2} monitors={:>2}",
            r.seed, r.room_count, r.edge_count, r.monitor_count
        );
    }
    println!("=== end corpus summary ===\n");
}
