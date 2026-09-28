//! Where on a production facility a push can kill a minor.
//!
//! The tool kills nothing; the architecture does, and only by a fall out of the facility
//! (`released`). `wfc_kinetic_lab` found the corpus rails every face onto void on a small
//! solved floor, which would leave a push nothing to push a minor off. This measures the
//! production facility the game plays, with the committed tile catalogue: from points a
//! body can stand on, level pushes in twelve directions, walked through the colliders a
//! minor's capsule meets, to the first place with no floor under it. A drop is **lethal**
//! when nothing below catches the body before it is out of the facility, or what does is
//! further down than a minor survives (`MINOR_BREAKING_DROP`), and a **landing** when a
//! lower deck catches it sooner.
//!
//! The probe only nominates: every cell it calls lethal is replayed as up to three real
//! shoves on the simulation, and only a shove that ends in `GuardianLost` counts. (The
//! probe alone overcounts several times over: a railing just past a slab's edge, or a
//! fall that carries forward onto a roof the straight-down ray missed.)
//!
//! A retraction is measured the same way: a hall is retracted as the rules retract one,
//! and the doorways into it are probed from the halls beside it.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_facility::hex_wfc::{HexArchetype, HexPlacement, HexSpace, HexWfcConfig};
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, HexFace, PortClass, hex_origin};

use super::drops::{
    DIRECTIONS, PushEnd as Push, PushTrials as Trials, direction, linked_vertically as vertical,
};
use super::*;
use crate::hex_wfc::model::HexMatchConfig;

fn production(seed: u64) -> HexWfcMatch {
    let content = std::sync::Arc::new(crate::hex_wfc::HexMatchContent::from_runtime_catalog(
        crate::hex_wfc::test_catalog().clone(),
    ));
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig::arc_default(),
    };
    HexWfcMatch::new_with_content(seed, config, content).expect("a production facility solves")
}

fn stands(game: &HexWfcMatch, cell: HexCoord) -> Vec<Vec3> {
    game.standing_points(cell)
}

fn probe(game: &HexWfcMatch, feet: Vec3, direction: Vec3) -> Push {
    game.push_end(feet, direction)
}

#[derive(Default, Debug)]
struct Tally {
    cells: usize,
    points: usize,
    pushes: usize,
    landings: usize,
    lethal: usize,
    /// Cells with a lethal push from at least one point.
    lethal_cells: BTreeSet<HexCoord>,
    lethal_cells_per_level: BTreeMap<u8, usize>,
    cells_per_level: BTreeMap<u8, usize>,
}

#[test]
#[ignore = "evidence, not regression cover: probes three production facilities and \
            replays sampled shoves (~2 min); prints where a push can kill a minor"]
fn where_a_push_can_kill_a_minor_on_a_production_facility() {
    for seed in [1u64, 2, 3] {
        let game = production(seed);
        let mut tally = Tally::default();
        let mut sites: BTreeMap<HexCoord, Vec<(Vec3, Vec3)>> = BTreeMap::new();
        let cells: Vec<HexCoord> = game
            .facility
            .placements
            .iter()
            .filter(|&(&cell, placement)| {
                placement.space.built() && !vertical(&game.facility, cell)
            })
            .map(|(&cell, _)| cell)
            .collect();
        for &cell in &cells {
            tally.cells += 1;
            *tally.cells_per_level.entry(cell.level).or_default() += 1;
            for feet in stands(&game, cell) {
                tally.points += 1;
                for step in 0..DIRECTIONS {
                    tally.pushes += 1;
                    match probe(&game, feet, direction(step)) {
                        Push::Held => {}
                        Push::Landing => tally.landings += 1,
                        Push::Lethal => {
                            tally.lethal += 1;
                            if tally.lethal_cells.insert(cell) {
                                *tally.lethal_cells_per_level.entry(cell.level).or_default() += 1;
                            }
                            let tried = sites.entry(cell).or_default();
                            if tried.len() < 3 {
                                tried.push((feet, direction(step)));
                            }
                        }
                    }
                }
            }
        }
        // Every cell the probe calls lethal, replayed as up to three real shoves.
        let mut trials = Trials::new(&game);
        let mut confirmed: BTreeMap<u8, usize> = BTreeMap::new();
        let (mut replayed, mut killed) = (0, 0);
        for (&cell, tries) in &sites {
            let mut dies = false;
            for &(feet, direction) in tries {
                replayed += 1;
                if trials.kills(cell, feet, direction) {
                    killed += 1;
                    dies = true;
                }
            }
            if dies {
                *confirmed.entry(cell.level).or_default() += 1;
            }
        }
        let confirmed_cells: usize = confirmed.values().sum();
        eprintln!(
            "seed {seed}: {} walkable non-stair cells, {} standing points, {} pushes: \
             {} held, {} land on a lower deck, {} lethal by the probe",
            tally.cells,
            tally.points,
            tally.pushes,
            tally.pushes - tally.landings - tally.lethal,
            tally.landings,
            tally.lethal,
        );
        eprintln!(
            "  cells the probe calls lethal: {} of {}; per level {:?} of {:?}",
            tally.lethal_cells.len(),
            tally.cells,
            tally.lethal_cells_per_level,
            tally.cells_per_level,
        );
        eprintln!(
            "  replayed {replayed} shoves from them, {killed} killed: cells where a push \
             really kills {confirmed_cells} of {} ({:.1}%), per level {confirmed:?}",
            tally.cells,
            100.0 * confirmed_cells as f32 / tally.cells.max(1) as f32,
        );
        retractions(&game);
    }
}

/// Retract halls as the rules do and probe the doorways into each from beside it.
fn retractions(game: &HexWfcMatch) {
    let grid = game.facility.config.grid();
    let candidates: Vec<HexCoord> = game
        .facility
        .placements
        .iter()
        .filter(|&(&cell, placement)| {
            placement.space == HexSpace::Hall
                && !vertical(&game.facility, cell)
                && game
                    .facility
                    .blueprints
                    .iter()
                    .all(|b| !b.cells.contains(&cell))
        })
        .map(|(&cell, _)| cell)
        .collect();
    let picked: Vec<HexCoord> = (0..24)
        .map(|i| candidates[i * candidates.len() / 24])
        .collect();
    let (mut doorways, mut lethal, mut landing, mut held, mut killed, mut replayed) =
        (0, 0, 0, 0, 0, 0);
    let mut by_level: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    for cell in picked {
        let mut after = game.clone();
        let hole = HexPlacement {
            coord: cell,
            space: HexSpace::Void,
            archetype: HexArchetype::Void,
            doors: 0,
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        };
        if after
            .apply_directed_change(BTreeMap::from([(cell, hole)]))
            .is_err()
        {
            continue;
        }
        let into = Vec3::from_array(hex_origin(cell));
        for face in HexFace::LATERAL {
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            // A neighbour whose doorway led into the retracted hall.
            let Some(placement) = after.facility.placements.get(&next) else {
                continue;
            };
            if !placement.space.built() || !placement.is_open(face.opposite()) {
                continue;
            }
            // Stand where a player would push from: a couple of metres short of the
            // doorway, so the slide carries through it.
            let from = Vec3::from_array(hex_origin(next));
            let toward = (into - from).with_y(0.0).normalize();
            let doorway = (from + into) * 0.5 + Vec3::Y * FLOOR_SLAB_TOP;
            let Some(feet) = after.stands_at(doorway - toward * 2.5) else {
                continue;
            };
            doorways += 1;
            let entry = by_level.entry(cell.level).or_default();
            entry.0 += 1;
            match probe(&after, feet, toward) {
                Push::Held => held += 1,
                Push::Landing => landing += 1,
                Push::Lethal => {
                    lethal += 1;
                    entry.1 += 1;
                    if replayed < 8 {
                        replayed += 1;
                        if Trials::new(&after).kills(next, feet, toward) {
                            killed += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "  retracting 24 halls: {doorways} doorways into them; a push through one is \
         held {held}, lands on a lower deck {landing}, lethal {lethal}; \
         lethal per level (doorways, lethal) {by_level:?}; replayed {killed} of {replayed} killed"
    );
}
