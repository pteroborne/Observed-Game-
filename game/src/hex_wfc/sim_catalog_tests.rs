use std::collections::BTreeSet;

use observed_authoring::RuntimeHexCatalog;
use observed_content::ArchitectureRegister;

/// Collider IDs the projector reserves per cell
/// (`observed_match::hex_wfc::geometry::COLLIDER_STRIDE`), and so the hard cap
/// on hulls in one tile.
const COLLIDER_STRIDE: usize = 128;

use super::{load_authoring_corpus, tile_dir};

/// Every district can build every shape the solver asks for, out of its own
/// geometry.
///
/// This assertion used to carry two exemptions, and both were load-bearing.
/// `stair_tower` was excused because no authored tower existed anywhere, which
/// silently hid backlog #13 for an entire arc — half the facility was one
/// procedural switchback and the coverage gate said nothing. `expanse` was
/// excused in Phase 108 for Liminal Grid, the one district whose identity *is*
/// open space. Phase 110 gives every register its own generated kit, so both
/// come down and the gate is unconditional again.
///
/// "Exact" is the point. A tile keyed `generic` satisfies the solver but is
/// drawn in another district's style, so a facility can be fully covered and
/// still read as one place — which is precisely what nine of the ten districts
/// did until this phase.
#[test]
fn every_district_covers_every_wfc_geometry_demand_with_its_own_geometry() {
    let corpus = load_authoring_corpus();
    for demand in observed_facility::hex_wfc::geometry_demands() {
        for register in ArchitectureRegister::ALL {
            let slug = register.slug();
            let exact = corpus
                .cells
                .iter()
                .filter(|tile| {
                    tile.key.archetype == demand.archetype
                        && tile.signature == demand.signature
                        && tile.key.register == slug
                })
                .collect::<Vec<_>>();
            assert!(
                !exact.is_empty(),
                "{slug} falls back to another district's geometry for archetype={} signature={:?}",
                demand.archetype,
                demand.signature
            );
            // The projector reserves a fixed collider ID range per cell and
            // refuses a tile that overruns it, so this is the real ceiling
            // rather than a stylistic one.
            assert!(
                exact.iter().all(|tile| tile.hulls.len() <= COLLIDER_STRIDE),
                "{slug} {} {:?} needs more than {COLLIDER_STRIDE} hulls, which the                  projector cannot give one cell",
                demand.archetype,
                demand.signature
            );
        }

        // Liminal Grid is the one district with hand-authored modules, and the
        // generated kit sits *under* them rather than replacing them. Where it
        // has authored layouts, both must still be reachable, or the floor has
        // quietly become the ceiling.
        //
        // Which archetypes those are is read from the corpus rather than listed
        // here. A list would have to grow every time the kit gains a family the
        // authored corpus does not cover — it already would have, twice — and a
        // stale exclusion is indistinguishable from a real gap.
        let liminal = corpus
            .cells
            .iter()
            .filter(|tile| {
                tile.key.archetype == demand.archetype
                    && tile.signature == demand.signature
                    && tile.key.register == ArchitectureRegister::LiminalGrid.slug()
            })
            .collect::<Vec<_>>();
        // Weights 2 and 3 are how the two-layout hall families are marked; the
        // authored ramp carries 10 and generated tiles carry 1, so seeing either
        // marker is what says this archetype has a pair to keep.
        let weights = liminal
            .iter()
            .map(|tile| tile.weight)
            .collect::<BTreeSet<_>>();
        if weights.contains(&2) || weights.contains(&3) {
            assert!(
                weights.contains(&2) && weights.contains(&3),
                "{} {:?} lost one of Liminal Grid's authored layouts: {weights:?}",
                demand.archetype,
                demand.signature
            );
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
