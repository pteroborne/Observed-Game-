//! What could sit next to the module you are looking at.
//!
//! The viewer's ordinary question is *is this tile correct in itself*. This
//! answers the other one an author has while drawing a doorway: *does it meet
//! anything?* Around the selected module it names the ring of cells that touch
//! its footprint, and for each one lists the modules whose ports would mate
//! across that seam.
//!
//! # This is the tile grammar, not the solver
//!
//! `observed_facility::hex_wfc::neighborhood` answers a related question about
//! a *solved facility*, and its module doc calls the version computed here
//! "the cheap version ... wrong in a way that flatters the corpus", because it
//! ignores everything except the two tiles at the seam. That criticism is
//! correct and it does not apply: there is no facility in this tool, only one
//! parsed file, so the port-grammar answer is the only honest one available.
//! It is the same number that query names `FaceDomain::from_centre`, and the
//! panel says so in those words — which also makes the eventual controlled-WFC
//! extension a *narrowing* of this number rather than a rewrite.
//!
//! # Candidates come from the watched directory, not the compiled catalog
//!
//! This tool is a file watcher; its whole identity is that a module you saved
//! ten seconds ago is on screen. `compiled_catalog.ron` is a build artifact, so
//! sourcing candidates from it would give a live centre and a stale ring — a
//! module authored five minutes ago would not appear as a neighbour at all.
//! The catalog's extra breadth is mostly illusory anyway: it stores the same
//! rotated hulls once per architecture register, so its registers multiply the
//! entry count without adding a single distinct shape. **Registers are ignored
//! here for exactly that reason.** What the catalog genuinely adds is the six
//! turns, and those are recovered by turning signatures locally through
//! `observed_authoring::rotation` — the transform the catalog itself uses.
//!
//! # Only validated single-cell modules are offered
//!
//! A candidate must have a validated [`AuthoredModule`], because that is what
//! carries the two facts that make an offer honest: which turns the module
//! actually accepts, and whether it is one cell or a whole room. A module that
//! fails validation is not offered — the viewer is already telling you it is
//! broken, and proposing it as a neighbour would be arguing with itself. Rooms
//! are not offered either: a room occupies several cells and cannot be dropped
//! into one, and `TilePrototype::signature` only describes its origin cell, so
//! treating one as a single-cell tile would be reading a signature that does
//! not describe what would be placed.

use std::collections::{BTreeMap, BTreeSet};

use observed_authoring::rotation::{TURNS, rotate_signature};
use observed_authoring::source::{ModuleKind, RotationPolicy};
use observed_hex::{HexFace, PortClass, PortSignature, ports_compatible};

use crate::module::diagnose::Diagnosis;

/// One face of the centre's footprint that a ring cell answers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seam {
    /// The footprint cell this seam belongs to, in module-local axial plan
    /// coordinates.
    pub from: (i32, i32),
    /// The face of `from` the ring cell sits across.
    pub face: HexFace,
    /// What the centre offers there.
    pub class: PortClass,
}

/// One legal occupant of a ring cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Index into the corpus slice `build_ring` was given — which is
    /// `ModuleState::diagnoses`. The renderer reaches the hulls through this
    /// rather than cloning geometry into the ring.
    pub source: usize,
    /// The module's own name, carried so ordering and dedup do not have to
    /// reach back into the corpus for a string.
    pub name: String,
    /// Sixth-turns clockwise, `0..6`. Always 0 for a `RotationPolicy::None`
    /// module.
    pub turn: u8,
    /// The signature *after* the turn, so a gate can re-check the mating claim
    /// without redoing the rotation.
    pub signature: PortSignature,
    pub weight: u16,
}

/// A cell touching the centre's footprint, and what could stand in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RingCell {
    pub coord: (i32, i32),
    /// Every footprint face this cell answers to. More than one when the cell
    /// sits in a concavity of a multi-cell room.
    pub seams: Vec<Seam>,
    /// Weight-ordered, heaviest first, deduplicated. Empty means nothing in
    /// the corpus fits — an authoring hole, and reported as one.
    pub candidates: Vec<Candidate>,
}

impl RingCell {
    /// The solver would have no choice here.
    #[must_use]
    pub fn is_forced(&self) -> bool {
        self.candidates.len() == 1
    }

    /// Whether every seam this cell answers to is sealed.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.seams
            .iter()
            .all(|seam| seam.class == PortClass::Sealed)
    }

    /// The face label to show, which is the first seam's. A cell answering two
    /// seams is named by the first; the panel prints the count beside it.
    #[must_use]
    pub fn face(&self) -> Option<HexFace> {
        self.seams.first().map(|seam| seam.face)
    }
}

/// The ring around one module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NeighbourRing {
    pub cells: Vec<RingCell>,
    /// Why there is no ring, verbatim. `Some` when the centre cannot be read.
    pub refusal: Option<String>,
}

impl NeighbourRing {
    /// A ring that refuses, rather than an empty one that looks like an answer.
    fn refused(reason: impl Into<String>) -> Self {
        Self {
            cells: Vec::new(),
            refusal: Some(reason.into()),
        }
    }

    /// Ring cells with nothing that fits.
    #[must_use]
    pub fn empty_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.candidates.is_empty())
            .count()
    }
}

/// The centre's footprint and what it offers on each face.
struct Centre {
    /// Plan cells of the footprint, deduplicated across levels.
    cells: BTreeSet<(i32, i32)>,
    /// `(cell, face) -> class`, absent meaning `Sealed`.
    classes: BTreeMap<((i32, i32), usize), PortClass>,
}

impl Centre {
    fn class(&self, cell: (i32, i32), face: HexFace) -> PortClass {
        self.classes
            .get(&(cell, face.index()))
            .copied()
            .unwrap_or(PortClass::Sealed)
    }
}

/// Read the centre's footprint and per-face classes.
///
/// The fallback chain is the one `render::cell_centres` already uses, and for
/// the same reason: the moment you most want to look at a module is when
/// validation failed, which is exactly when `module` is `None`.
///
/// It matters that the *validated* module is preferred rather than the
/// prototype. `parse_tile` records only the **origin cell's** ports into
/// `TilePrototype::signature` and says so in a comment, so a room's other faces
/// are simply absent from it. Reading the signature for a multi-cell module
/// would silently seal every face except six.
fn read_centre(centre: &Diagnosis) -> Result<Centre, String> {
    if let Some(module) = centre.module.as_ref() {
        let cells: BTreeSet<(i32, i32)> = module
            .footprint
            .iter()
            .map(|cell| (i32::from(cell.q), i32::from(cell.r)))
            .collect();
        if cells.is_empty() {
            return Err(String::from("this module declares no footprint cells"));
        }
        let mut classes = BTreeMap::new();
        for port in &module.ports {
            // Lateral faces only. A ramp's real partner is above it, and the
            // plan ring cannot show that; the panel says so rather than this
            // quietly folding a vertical port into a lateral neighbour.
            if !port.face.is_lateral() {
                continue;
            }
            classes.insert(
                (
                    (i32::from(port.cell.q), i32::from(port.cell.r)),
                    port.face.index(),
                ),
                port.class,
            );
        }
        return Ok(Centre { cells, classes });
    }

    // No validated module: fall back to the origin cell the prototype does
    // describe, and to nothing else.
    if let Some(prototype) = centre.prototype.as_ref() {
        let mut classes = BTreeMap::new();
        for face in HexFace::LATERAL {
            classes.insert(((0, 0), face.index()), prototype.signature.port(face));
        }
        return Ok(Centre {
            cells: BTreeSet::from([(0, 0)]),
            classes,
        });
    }

    // Deliberately not a fallback to an all-sealed signature. A plausible ring
    // around a module that does not parse is the worst output this could give:
    // it would look like an answer.
    Err(String::from(
        "this module does not parse, so it has no ports to meet",
    ))
}

/// Every turn a module legally accepts.
fn legal_turns(policy: RotationPolicy) -> std::ops::Range<u8> {
    match policy {
        RotationPolicy::None => 0..1,
        RotationPolicy::SixFold => 0..TURNS,
    }
}

/// The ring around `centre`, and what could stand in each of its cells.
///
/// Pure: same corpus in, same ring out, no IO and no Bevy. That is what lets
/// every gate on this mode run without an `App`.
#[must_use]
pub fn build_ring(centre: &Diagnosis, corpus: &[Diagnosis]) -> NeighbourRing {
    let centre = match read_centre(centre) {
        Ok(centre) => centre,
        Err(reason) => return NeighbourRing::refused(reason),
    };

    // The ring is the perimeter of the footprint, not a hardcoded six: one
    // cell gives exactly six, a two-cell room gives eight, and both fall out
    // of the same loop.
    let mut seams_by_cell: BTreeMap<(i32, i32), Vec<Seam>> = BTreeMap::new();
    for &from in &centre.cells {
        for face in HexFace::LATERAL {
            let (dq, dr, _) = face.delta();
            let coord = (from.0 + dq, from.1 + dr);
            if centre.cells.contains(&coord) {
                continue;
            }
            seams_by_cell.entry(coord).or_default().push(Seam {
                from,
                face,
                class: centre.class(from, face),
            });
        }
    }

    // Offers are computed once and reused for every ring cell: turning a
    // signature is a bit shuffle, but doing it per cell as well as per module
    // would multiply it by the ring size for nothing.
    let offers = collect_offers(corpus);

    let cells = seams_by_cell
        .into_iter()
        .map(|(coord, seams)| {
            let mut candidates: Vec<Candidate> = offers
                .iter()
                .filter(|candidate| {
                    seams.iter().all(|seam| {
                        ports_compatible(seam.class, candidate.signature.port(seam.face.opposite()))
                    })
                })
                .cloned()
                .collect();
            order_and_dedup(&mut candidates);
            RingCell {
                coord,
                seams,
                candidates,
            }
        })
        .collect();

    NeighbourRing {
        cells,
        refusal: None,
    }
}

/// Every `(module, turn)` placement the corpus can offer.
fn collect_offers(corpus: &[Diagnosis]) -> Vec<Candidate> {
    let mut offers = Vec::new();
    for (source, diagnosis) in corpus.iter().enumerate() {
        let (Some(module), Some(prototype)) =
            (diagnosis.module.as_ref(), diagnosis.prototype.as_ref())
        else {
            continue;
        };
        if module.kind != ModuleKind::Cell || module.footprint.len() != 1 {
            continue;
        }
        let name = diagnosis.name();
        for turn in legal_turns(module.rotation) {
            offers.push(Candidate {
                source,
                name: name.clone(),
                turn,
                signature: rotate_signature(prototype.signature, turn),
                weight: prototype.weight,
            });
        }
    }
    offers
}

/// Heaviest first, then by name, then by turn; one entry per distinct
/// `(name, turned signature)`.
///
/// Weight ordering because the head of the list is then what a facility would
/// most often actually build there. Dedup because a module whose ports are
/// rotationally symmetric otherwise contributes the same offer six times, and
/// six identical entries is the single noisiest thing this list can contain.
/// The cost is that such a module is only ever previewed at turn 0 even if its
/// *geometry* is asymmetric; the panel prints the turn count so it is stated
/// rather than hidden.
fn order_and_dedup(candidates: &mut Vec<Candidate>) {
    candidates.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.turn.cmp(&b.turn))
    });
    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| seen.insert((candidate.name.clone(), candidate.signature)));
}

/// How many distinct turns of `name` survive on this cell, for the panel.
#[must_use]
pub fn turns_offered(cell: &RingCell, name: &str) -> usize {
    cell.candidates
        .iter()
        .filter(|candidate| candidate.name == name)
        .count()
}
