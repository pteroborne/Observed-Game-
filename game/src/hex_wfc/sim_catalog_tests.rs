use std::collections::BTreeSet;

use observed_authoring::RuntimeHexCatalog;
use observed_content::ArchitectureRegister;

use super::{load_authoring_corpus, tile_dir};

#[test]
fn merged_authoring_corpus_covers_every_wfc_geometry_demand_exactly() {
    let corpus = load_authoring_corpus();
    for demand in observed_facility::hex_wfc::geometry_demands() {
        for register in ArchitectureRegister::ALL {
            let register = register.slug();
            assert!(
                corpus.cells.iter().any(|tile| {
                    tile.key.archetype == demand.archetype
                        && tile.signature == demand.signature
                        && (tile.key.register == register || tile.key.register == "generic")
                }),
                "missing exact tile coverage for archetype={} register={} signature={:?}",
                demand.archetype,
                register,
                demand.signature
            );
        }

        // `expanse` joins `stair_tower` on this exemption, and both are debts
        // rather than decisions. Liminal Grid is authored as `.map` modules, and
        // Phase 108 shipped `expanse` through the generated kit only, so the
        // district that most wants expanses is the one district without exact
        // tiles for them. Scheduled to Phase 110 with the rest of the
        // district-exclusive authoring (backlog #20). The `stair_tower`
        // exemption above hid backlog #13 for an entire arc by being silent —
        // this one is named in the backlog so it cannot.
        if demand.archetype != "stair_tower" && demand.archetype != "expanse" {
            let exact_liminal = corpus
                .cells
                .iter()
                .filter(|tile| {
                    tile.key.archetype == demand.archetype
                        && tile.signature == demand.signature
                        && tile.key.register == ArchitectureRegister::LiminalGrid.slug()
                })
                .collect::<Vec<_>>();
            assert!(
                !exact_liminal.is_empty(),
                "Liminal Grid must not use generic fallback for {} {:?}",
                demand.archetype,
                demand.signature
            );
            if demand.archetype != "hall_ramp" {
                let weights = exact_liminal
                    .iter()
                    .map(|tile| tile.weight)
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    weights,
                    BTreeSet::from([2, 3]),
                    "{} {:?} lacks both Liminal layouts",
                    demand.archetype,
                    demand.signature
                );
                assert!(
                    exact_liminal.iter().all(|tile| tile.hulls.len() <= 28),
                    "{} {:?} exceeds the Liminal hull budget",
                    demand.archetype,
                    demand.signature
                );
            }
        }
    }
}

#[test]
fn committed_catalog_hash_is_stable_and_verified_by_the_runtime_loader() {
    let first = RuntimeHexCatalog::load(
        &tile_dir(),
        &ArchitectureRegister::ALL.map(ArchitectureRegister::slug),
    )
    .expect("first load");
    let second = RuntimeHexCatalog::load(
        &tile_dir(),
        &ArchitectureRegister::ALL.map(ArchitectureRegister::slug),
    )
    .expect("second load");
    assert_eq!(
        first.simulation_content_hash,
        second.simulation_content_hash
    );
    assert_ne!(first.simulation_content_hash, [0; 32]);
    assert_eq!(first.cells, second.cells);
}
