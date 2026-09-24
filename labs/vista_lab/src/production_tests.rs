//! The vista is hand-built. This asks what the facility the game actually plays
//! offers the same rules: the production lattice, the committed composition profile,
//! solved exactly as a match solves it.
use observed_authoring::RuntimeHexCatalog;
use observed_facility::hex_wfc::exposure::{Form, Overhang, opens_to_air, survey};
use observed_facility::hex_wfc::{HexFace, HexSpace, HexWfcConfig, HexWfcWorld};

use crate::composition::UNSAFE_FROM_LEVEL;

const SEEDS: [u64; 6] = [0x0A1A, 0x0B2B, 0x0C3C, 0x0D4D, 0x0E5E, 0x0F6F];

fn production(seed: u64, catalog: &RuntimeHexCatalog) -> HexWfcWorld {
    // `HexWfcMatch::new` passes no room quotas below 28x20x10, and the arc lattice is
    // below it; mirror that exactly rather than solving a facility nobody plays.
    let mut world = HexWfcWorld::generate_with_profile(
        seed,
        HexWfcConfig::arc_default(),
        None,
        &catalog.composition,
    )
    .expect("the production facility solves");
    let _ = world.mark_open_air();
    world
}

#[test]
fn what_the_production_facility_shows_the_sky() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let catalog = RuntimeHexCatalog::load(&root, &[]).expect("committed catalog loads");
    eprintln!(
        "seed    built  air   rock | exterior  sheer faces | hanging  over void | spans  halls to air | doors onto air"
    );
    for seed in SEEDS {
        let world = production(seed, &catalog);
        let cells = &world.placements;
        let count = |space: HexSpace| cells.values().filter(|p| p.space == space).count();
        let built = cells.values().filter(|p| p.space.built()).count();
        let (air, rock) = (count(HexSpace::Air), count(HexSpace::Void));
        assert_eq!(built + air + rock, cells.len());

        let exposures = survey(&world, UNSAFE_FROM_LEVEL);
        assert_eq!(exposures.len(), built);
        let exterior = exposures.iter().filter(|e| e.sheer_count() > 0).count();
        let sheer: u32 = exposures.iter().map(|e| e.sheer_count()).sum();
        let hanging = exposures.iter().filter(|e| e.overhang.hangs()).count();
        let over_void = exposures
            .iter()
            .filter(|e| matches!(e.overhang, Overhang::Hanging { onto: None, .. }))
            .count();
        let spans = exposures
            .iter()
            .filter(|e| matches!(e.form, Form::Span { .. }))
            .count();
        // `Form::Deck` is any hall that is not a span: in the vista an open terrace,
        // here an enclosed corridor with at least one wall that has sky behind it.
        let halls_to_air = exposures
            .iter()
            .filter(|e| e.form == Form::Deck && e.sheer_count() > 0)
            .count();
        // A door is a matched pair of open faces; the solver never opens one onto air.
        let doors_onto_air = cells
            .values()
            .filter(|p| p.space.built())
            .map(|p| {
                HexFace::LATERAL
                    .into_iter()
                    .filter(|&face| p.is_open(face) && opens_to_air(&world, p.coord, face))
                    .count()
            })
            .sum::<usize>();
        assert_eq!(doors_onto_air, 0, "seed {seed:#x}");
        for e in &exposures {
            for face in HexFace::LATERAL.into_iter().filter(|&f| e.is_sheer(f)) {
                assert!(opens_to_air(&world, e.coord, face));
            }
        }
        eprintln!(
            "{seed:#06x}  {built:5} {air:5} {rock:4} | {exterior:8} {sheer:12} | {hanging:7} {over_void:9} | {spans:5} {halls_to_air:12} | {doors_onto_air:14}"
        );
    }
}

/// What each composition knob does to the air a match is played in: the measurement
/// behind choosing more void, printed rather than asserted.
///
/// For each void share: how often the production lattice still solves, how long it
/// takes, and what open air it offers — air and rock, halls with open edges,
/// walkways, open faces over a deep drop (two storeys or more) split into off the rim
/// and between towers, cells floating above the ground level over true void, and
/// whether the exit is still reachable from the spawn.
#[test]
#[ignore = "composition sweep: six production solves per void share, about a minute; prints, asserts nothing"]
fn sweep_void_share() {
    use observed_facility::hex_wfc::exposure::{Overhang, exposure};
    use observed_match::hex_wfc::open_edges;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let catalog = RuntimeHexCatalog::load(&root, &[]).expect("committed catalog loads");
    eprintln!(
        "void  solved   ms | air%  rock% | open halls  spans | deep rim  deep inner | floating | exit"
    );
    for void in [300.0, 600.0, 1_000.0, 2_000.0, 4_000.0, 10_000.0] {
        let mut profile = catalog.composition.clone();
        profile.space_mix.void = void;
        let (mut solved, mut ms, mut air, mut rock, mut cells) = (0, 0u128, 0, 0, 0);
        let (mut halls, mut spans, mut rim, mut inner, mut floating, mut exits) =
            (0, 0, 0, 0, 0, 0);
        for seed in SEEDS {
            let started = std::time::Instant::now();
            let Ok(mut world) = HexWfcWorld::generate_with_profile(
                seed,
                HexWfcConfig::arc_default(),
                None,
                &profile,
            ) else {
                continue;
            };
            ms += started.elapsed().as_millis();
            solved += 1;
            let _ = world.mark_open_air();
            let grid = world.config.grid();
            cells += world.placements.len();
            air += world
                .placements
                .values()
                .filter(|p| p.space == HexSpace::Air)
                .count();
            rock += world
                .placements
                .values()
                .filter(|p| p.space == HexSpace::Void)
                .count();
            for (&at, placement) in &world.placements {
                if !placement.space.built() {
                    continue;
                }
                if at.level > 0
                    && matches!(
                        exposure(&world, at, UNSAFE_FROM_LEVEL).map(|e| e.overhang),
                        Some(Overhang::Hanging { onto: None, .. })
                    )
                {
                    floating += 1;
                }
                let Some(edges) = open_edges(&world, at) else {
                    continue;
                };
                halls += 1;
                spans += usize::from(edges.span.is_some());
                for face in HexFace::LATERAL.into_iter().filter(|&f| edges.opens(f)) {
                    let Some(mut cell) = grid.neighbor(at, face) else {
                        rim += 1;
                        continue;
                    };
                    let mut depth = 0;
                    while world
                        .placements
                        .get(&cell)
                        .is_some_and(|p| p.space.unbuilt())
                    {
                        depth += 1;
                        match grid.neighbor(cell, HexFace::Down) {
                            Some(below) => cell = below,
                            None => break,
                        }
                    }
                    inner += usize::from(depth >= 2);
                }
            }
            exits += usize::from(
                world
                    .route_between(world.config.spawn(), world.config.exit())
                    .is_some(),
            );
        }
        let n = solved.max(1);
        #[allow(clippy::cast_precision_loss)]
        let pct = |count: usize| 100.0 * count as f32 / cells.max(1) as f32;
        eprintln!(
            "{void:5} {solved}/{}  {:5} | {:4.1}  {:4.1}  | {:10} {:6} | {:8} {:10} | {:8} | {exits}/{solved}",
            SEEDS.len(),
            ms / n as u128,
            pct(air),
            pct(rock),
            halls / n,
            spans / n,
            rim / n,
            inner / n,
            floating / n,
        );
    }
}
