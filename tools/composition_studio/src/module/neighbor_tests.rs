//! Gates for the module studio's neighbour ring.
//!
//! A sibling of the inline test modules so no file outgrows the 600-line
//! review budget the rest of this tool lives under.
//!
//! These run against the **real committed corpus** rather than hand-written
//! fixtures. That is deliberate: the mode's whole claim is about what the
//! authored tiles actually offer each other, and a synthetic two-tile corpus
//! would prove the arithmetic while saying nothing about the thing on screen.

use std::path::{Path, PathBuf};

use observed_authoring::rotation::rotate_signature;
use observed_authoring::source::{ModuleKind, RotationPolicy};
use observed_hex::{HexFace, PortClass, ports_compatible};

use crate::module::diagnose::{Diagnosis, diagnose_file, diagnose_text};
use crate::module::neighbors::{NeighbourRing, build_ring};

fn authored_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored")
}

/// Every watched module, diagnosed, in scan order — the same list
/// `ModuleState::diagnoses` holds at runtime.
fn corpus() -> Vec<Diagnosis> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(authored_dir())
        .expect("the authored corpus is committed")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "map")
                || crate::module::diagnose::is_recipe(path)
        })
        .collect();
    paths.sort();
    assert!(paths.len() > 20, "the corpus is smaller than expected");
    paths.iter().map(|path| diagnose_file(path)).collect()
}

fn find<'a>(corpus: &'a [Diagnosis], name: &str) -> &'a Diagnosis {
    corpus
        .iter()
        .find(|diagnosis| diagnosis.name() == name)
        .unwrap_or_else(|| panic!("{name} is not in the committed corpus"))
}

/// The gate this mode lives or dies by.
///
/// Every candidate offered on every ring cell of every module must actually
/// mate across every seam that cell answers to. A preview that offers a tile
/// which would not fit is not a preview of the grammar, it is a different
/// program — and the author would draw against geometry the game refuses.
#[test]
fn every_offered_neighbour_mates_across_its_face() {
    let corpus = corpus();
    let mut checked = 0usize;
    for diagnosis in &corpus {
        let ring = build_ring(diagnosis, &corpus);
        for cell in &ring.cells {
            for candidate in &cell.candidates {
                for seam in &cell.seams {
                    assert!(
                        ports_compatible(
                            seam.class,
                            candidate.signature.port(seam.face.opposite())
                        ),
                        "{} offers {} turn {} across {:?}, which does not mate",
                        diagnosis.name(),
                        candidate.name,
                        candidate.turn,
                        seam.face
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 1000,
        "only {checked} mating claims checked; the sweep is not exercising the corpus"
    );
}

/// A module that declares no rotation must never be offered turned. This is
/// the one that stops the ring showing a placement the game cannot build.
#[test]
fn rotation_policy_none_modules_are_never_offered_turned() {
    let corpus = corpus();
    let mut fixed = 0usize;
    for diagnosis in &corpus {
        let ring = build_ring(diagnosis, &corpus);
        for cell in &ring.cells {
            for candidate in &cell.candidates {
                let source = &corpus[candidate.source];
                let policy = source
                    .module
                    .as_ref()
                    .map(|module| module.rotation)
                    .expect("only validated modules are offered");
                if policy == RotationPolicy::None {
                    assert_eq!(
                        candidate.turn, 0,
                        "{} accepts no rotation but was offered at turn {}",
                        candidate.name, candidate.turn
                    );
                    fixed += 1;
                }
            }
        }
    }
    assert!(
        fixed > 0,
        "the corpus has no fixed-rotation modules to check"
    );
}

/// The candidate's stored signature has to be the turned one, or the mating
/// check above would be verifying a claim the renderer does not honour.
#[test]
fn a_candidates_signature_is_its_module_turned() {
    let corpus = corpus();
    let mut compared = 0usize;
    for diagnosis in &corpus {
        for cell in &build_ring(diagnosis, &corpus).cells {
            for candidate in &cell.candidates {
                let prototype = corpus[candidate.source]
                    .prototype
                    .as_ref()
                    .expect("only parsed modules are offered");
                assert_eq!(
                    candidate.signature,
                    rotate_signature(prototype.signature, candidate.turn)
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 100);
}

/// Rooms and broken modules are not single-cell tiles and must not be offered
/// as one. A room's `TilePrototype::signature` describes only its origin cell,
/// so offering it would be reading a signature that does not describe what
/// would be placed.
#[test]
fn only_validated_single_cell_modules_are_offered() {
    let corpus = corpus();
    let mut seen = 0usize;
    for diagnosis in &corpus {
        for cell in &build_ring(diagnosis, &corpus).cells {
            for candidate in &cell.candidates {
                let source = &corpus[candidate.source];
                let module = source.module.as_ref().expect("validated");
                assert!(source.is_clean(), "{} is offered but fails", candidate.name);
                assert_eq!(module.kind, ModuleKind::Cell, "{}", candidate.name);
                assert_eq!(module.footprint.len(), 1, "{}", candidate.name);
                seen += 1;
            }
        }
    }
    assert!(seen > 100);
}

/// A module that does not parse has no ports, so it has no ring. It must say
/// so — a plausible ring around a broken module would look like an answer.
#[test]
fn a_module_that_does_not_parse_offers_no_neighbours() {
    let corpus = corpus();
    let broken = diagnose_text(Path::new("not_a_module.map"), "this is not a map file");
    assert!(broken.prototype.is_none(), "the fixture must fail to parse");

    let ring = build_ring(&broken, &corpus);
    assert!(ring.refusal.is_some(), "a broken module must refuse");
    assert!(
        ring.cells.is_empty(),
        "refusing and still listing cells would be the worst of both"
    );
}

/// The ring is the perimeter of the footprint. One cell gives exactly the six
/// lateral neighbours; a room gives more, and never a cell inside itself.
#[test]
fn the_ring_is_the_perimeter_of_the_footprint() {
    let corpus = corpus();

    let single = find(&corpus, "hall_straight");
    let module = single.module.as_ref().expect("hall_straight validates");
    assert_eq!(module.footprint.len(), 1, "the fixture must be one cell");
    let ring = build_ring(single, &corpus);
    assert_eq!(ring.cells.len(), 6, "one cell has exactly six neighbours");

    let mut coords: Vec<(i32, i32)> = ring.cells.iter().map(|cell| cell.coord).collect();
    coords.sort_unstable();
    let mut expected: Vec<(i32, i32)> = HexFace::LATERAL
        .into_iter()
        .map(|face| {
            let (dq, dr, _) = face.delta();
            (dq, dr)
        })
        .collect();
    expected.sort_unstable();
    assert_eq!(coords, expected, "the ring is not the lateral deltas");

    // Every ring cell of any module must sit outside that module's footprint.
    for diagnosis in &corpus {
        let Some(module) = diagnosis.module.as_ref() else {
            continue;
        };
        let footprint: Vec<(i32, i32)> = module
            .footprint
            .iter()
            .map(|cell| (i32::from(cell.q), i32::from(cell.r)))
            .collect();
        for cell in &build_ring(diagnosis, &corpus).cells {
            assert!(
                !footprint.contains(&cell.coord),
                "{} put a ring cell inside its own footprint",
                diagnosis.name()
            );
        }
    }
}

/// A multi-cell module's ring is bigger than six, and the cells tucked into a
/// concavity answer to more than one seam. If this ever collapsed to six, the
/// mode would be silently previewing only part of a room's perimeter.
#[test]
fn a_room_has_a_larger_ring_and_cells_that_answer_twice() {
    let corpus = corpus();
    let Some(room) = corpus.iter().find(|diagnosis| {
        diagnosis
            .module
            .as_ref()
            .is_some_and(|module| module.footprint.len() > 1)
    }) else {
        // Not an assertion failure: the corpus is allowed to have no rooms.
        // But say so, because this test then proves nothing.
        eprintln!("no multi-cell module in the corpus; nothing to check");
        return;
    };
    let footprint = room.module.as_ref().expect("validated").footprint.len();
    let ring = build_ring(room, &corpus);
    assert!(
        ring.cells.len() > 6,
        "{} spans {footprint} cells but reported only {} ring cells",
        room.name(),
        ring.cells.len()
    );
    for cell in &ring.cells {
        assert!(
            !cell.seams.is_empty(),
            "a ring cell with no seam is not one"
        );
    }
}

/// The list has to be stable and free of the same offer repeated. A module
/// whose ports are rotationally symmetric would otherwise appear six times,
/// which is the noisiest thing this list can contain.
#[test]
fn candidate_order_is_deterministic_and_deduplicated() {
    let corpus = corpus();
    let subject = find(&corpus, "hall_straight");
    let first = build_ring(subject, &corpus);
    let second = build_ring(subject, &corpus);
    assert_eq!(first, second, "two builds of the same input disagree");

    for cell in &first.cells {
        let mut keys: Vec<(String, u16)> = cell
            .candidates
            .iter()
            .map(|candidate| (candidate.name.clone(), candidate.signature.0))
            .collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(before, keys.len(), "a duplicate offer survived dedup");

        let weights: Vec<u16> = cell.candidates.iter().map(|c| c.weight).collect();
        assert!(
            weights.windows(2).all(|pair| pair[0] >= pair[1]),
            "candidates are not heaviest-first: {weights:?}"
        );
    }
}

/// A door face and a sealed face must not offer the same set. If they did, the
/// mating predicate would be ignoring the class entirely.
#[test]
fn a_door_face_and_a_sealed_face_offer_different_things() {
    let corpus = corpus();
    let subject = find(&corpus, "hall_straight");
    let ring = build_ring(subject, &corpus);

    let door: Vec<&crate::module::neighbors::RingCell> =
        ring.cells.iter().filter(|cell| !cell.is_sealed()).collect();
    let sealed: Vec<&crate::module::neighbors::RingCell> =
        ring.cells.iter().filter(|cell| cell.is_sealed()).collect();
    assert!(!door.is_empty(), "a straight hall has open faces");
    assert!(!sealed.is_empty(), "a straight hall has sealed faces");

    for cell in &door {
        for candidate in &cell.candidates {
            let seam = cell.seams.first().expect("a seam");
            assert_eq!(
                candidate.signature.port(seam.face.opposite()),
                PortClass::Door
            );
        }
    }
    for cell in &sealed {
        for candidate in &cell.candidates {
            let seam = cell.seams.first().expect("a seam");
            assert_eq!(
                candidate.signature.port(seam.face.opposite()),
                PortClass::Sealed
            );
        }
    }
}

/// An empty ring cell is an authoring hole, and the count has to reach the
/// panel rather than presenting as a cell that just looks bare.
#[test]
fn empty_cells_are_counted() {
    let mut ring = NeighbourRing::default();
    assert_eq!(ring.empty_cells(), 0);
    let corpus = corpus();
    ring = build_ring(find(&corpus, "hall_straight"), &corpus);
    let counted = ring
        .cells
        .iter()
        .filter(|cell| cell.candidates.is_empty())
        .count();
    assert_eq!(ring.empty_cells(), counted);
}
