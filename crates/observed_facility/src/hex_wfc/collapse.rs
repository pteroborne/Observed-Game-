//! Domain construction, AC-3 propagation, and min-entropy collapse.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, PortClass, PortSignature};

use super::blueprint::StampedBlueprint;
use super::constraints::{
    all_coords, forced_route_edges, stamp_blueprints_with_pins, stamped_signatures,
};
use super::trace::SolveStep;
use super::validate::layout_failure;
use super::variants::{HexVariant, catalogue, variants_compatible};
use super::{HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcError};

type CollapseOutput = (BTreeMap<HexCoord, HexPlacement>, Vec<StampedBlueprint>, u32);

/// Fixed-width bitset over catalogue variant indices. The catalogue currently
/// holds 382 variants; `solver_tables` asserts the capacity still fits.
const MASK_WORDS: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VariantSet([u64; MASK_WORDS]);

impl VariantSet {
    const EMPTY: Self = Self([0; MASK_WORDS]);

    fn only(index: usize) -> Self {
        let mut set = Self::EMPTY;
        set.insert(index);
        set
    }

    fn insert(&mut self, index: usize) {
        self.0[index / 64] |= 1 << (index % 64);
    }

    fn is_empty(&self) -> bool {
        self.0.iter().all(|&word| word == 0)
    }

    fn len(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }

    /// The sole member of a collapsed (single-variant) domain.
    fn single(&self) -> Option<usize> {
        (self.len() == 1).then(|| self.iter().next().expect("len is 1"))
    }

    fn union_with(&mut self, other: &Self) {
        for (word, extra) in self.0.iter_mut().zip(&other.0) {
            *word |= extra;
        }
    }

    /// Intersect in place; returns whether the set shrank.
    fn intersect_with(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (word, allowed) in self.0.iter_mut().zip(&other.0) {
            let next = *word & allowed;
            changed |= next != *word;
            *word = next;
        }
        changed
    }

    /// Member variant indices in ascending order — the same order the old
    /// `Vec<usize>` domains kept, so RNG-weighted picks are unchanged.
    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(word_index, &bits)| {
            let mut bits = bits;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(word_index * 64 + bit)
            })
        })
    }
}

/// The variant catalogue plus, for every `(variant, face)`, the bitmask of
/// neighbor variants compatible across that face. Built once per process:
/// the catalogue is a deterministic constant.
struct SolverTables {
    variants: Vec<HexVariant>,
    /// Indexed `variant * 8 + face.index()`.
    compat: Vec<VariantSet>,
}

impl SolverTables {
    fn compat(&self, variant: usize, face: HexFace) -> &VariantSet {
        &self.compat[variant * 8 + face.index()]
    }
}

fn solver_tables() -> &'static SolverTables {
    static TABLES: OnceLock<SolverTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let variants = catalogue();
        assert!(
            variants.len() <= MASK_WORDS * 64,
            "catalogue outgrew VariantSet capacity; raise MASK_WORDS"
        );
        let mut compat = vec![VariantSet::EMPTY; variants.len() * 8];
        for (a, &source) in variants.iter().enumerate() {
            for face in HexFace::ALL {
                let mask = &mut compat[a * 8 + face.index()];
                for (b, &candidate) in variants.iter().enumerate() {
                    if variants_compatible(source, candidate, face) {
                        mask.insert(b);
                    }
                }
            }
        }
        SolverTables { variants, compat }
    })
}

pub(super) struct CollapseAttempt {
    pub placements: BTreeMap<HexCoord, HexPlacement>,
    pub blueprints: Vec<StampedBlueprint>,
}

pub(super) fn collapse(
    seed: u64,
    generation: u32,
    config: HexWfcConfig,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseOutput, HexWfcError> {
    let mut last_failure: Option<&'static str> = None;
    let no_pins = BTreeSet::new();
    for attempt in 0..config.retry_budget {
        match collapse_attempt(
            seed,
            generation,
            attempt,
            config,
            None,
            &no_pins,
            trace.as_deref_mut(),
        ) {
            Ok(solved) => return Ok((solved.placements, solved.blueprints, attempt + 1)),
            Err(reason) => last_failure = Some(reason),
        }
    }
    Err(HexWfcError::RetryBudgetExhausted {
        attempts: config.retry_budget,
        last_failure,
    })
}

/// Exactly one deterministic `(seed, generation, attempt)` collapse. Relayout
/// calls this once per simulation tick with the previous placements and the
/// complete pin set; ordinary generation uses the same path without pins.
pub(super) fn collapse_attempt(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    previous: Option<&BTreeMap<HexCoord, HexPlacement>>,
    pinned: &BTreeSet<HexCoord>,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseAttempt, &'static str> {
    let tables = solver_tables();
    emit(&mut trace, SolveStep::AttemptStart { attempt });
    let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
    collapse_attempt_with_blueprints(config, tables, &mut rng, previous, pinned, &[], trace)
}

/// Relayout variant of [`collapse_attempt`] that also freezes complete room
/// blueprints. Kept separate so the base generation call remains minimal.
pub(super) fn collapse_relayout_attempt(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    previous: &BTreeMap<HexCoord, HexPlacement>,
    pinned: &BTreeSet<HexCoord>,
    locked_blueprints: &[StampedBlueprint],
) -> Result<CollapseAttempt, &'static str> {
    let tables = solver_tables();
    let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
    collapse_attempt_with_blueprints(
        config,
        tables,
        &mut rng,
        Some(previous),
        pinned,
        locked_blueprints,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn collapse_attempt_with_blueprints(
    config: HexWfcConfig,
    tables: &SolverTables,
    rng: &mut SplitMix,
    previous: Option<&BTreeMap<HexCoord, HexPlacement>>,
    pinned: &BTreeSet<HexCoord>,
    locked_blueprints: &[StampedBlueprint],
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseAttempt, &'static str> {
    let variants = &tables.variants[..];
    let blueprints = stamp_blueprints_with_pins(config, rng, locked_blueprints);
    let signatures = stamped_signatures(config, &blueprints);
    let Some((forced_doors, forced_up, forced_down)) =
        forced_route_edges(config, &blueprints, &signatures, rng)
    else {
        return Err("blueprints blocked every forced route");
    };
    emit_blueprints(&blueprints, &forced_doors, &mut trace);

    let mut domains = initial_domains(
        config,
        variants,
        &forced_doors,
        &forced_up,
        &forced_down,
        &signatures,
        previous,
        pinned,
    );
    if domains.iter().any(VariantSet::is_empty) {
        return Err("pinned cell contradicts blueprint or forced route");
    }
    emit_precollapsed(config, variants, &domains, &signatures, &mut trace);
    // The initial pass seeds every cell; later passes are incremental.
    let all_cells: VecDeque<usize> = (0..domains.len()).collect();
    if !propagate(config, tables, &mut domains, all_cells, &mut trace) {
        return Err("propagation contradiction");
    }
    if !collapse_domains(config, tables, &mut domains, rng, &mut trace) {
        return Err("collapse contradiction");
    }
    let mut placements = materialize(config, variants, &domains);
    prune_disconnected(config, &mut placements);
    if let Some(reason) = layout_failure(config, &placements, &blueprints) {
        return Err(reason);
    }
    let rooms = placements
        .values()
        .filter(|placement| placement.space == HexSpace::Room)
        .count();
    let halls = placements
        .values()
        .filter(|placement| placement.space == HexSpace::Hall)
        .count();
    emit(
        &mut trace,
        SolveStep::Completed {
            rooms: rooms as u16,
            halls: halls as u16,
        },
    );
    Ok(CollapseAttempt {
        placements,
        blueprints,
    })
}

fn emit_blueprints(
    blueprints: &[StampedBlueprint],
    forced_doors: &BTreeMap<HexCoord, u8>,
    trace: &mut Option<&mut Vec<SolveStep>>,
) {
    let Some(steps) = trace.as_deref_mut() else {
        return;
    };
    for stamped in blueprints {
        for &coord in &stamped.cells {
            steps.push(SolveStep::BlueprintCell {
                coord,
                role: stamped.role,
            });
        }
    }
    for (&coord, &required) in forced_doors {
        steps.push(SolveStep::ForcedCell { coord, required });
    }
}

fn emit_precollapsed(
    config: HexWfcConfig,
    variants: &[HexVariant],
    domains: &[VariantSet],
    signatures: &BTreeMap<HexCoord, PortSignature>,
    trace: &mut Option<&mut Vec<SolveStep>>,
) {
    let Some(steps) = trace.as_deref_mut() else {
        return;
    };
    let grid = config.grid();
    for &coord in signatures.keys() {
        let domain = &domains[grid.index(coord)];
        if let Some(only) = domain.single() {
            let variant = variants[only];
            steps.push(SolveStep::Collapsed {
                coord,
                space: variant.space,
                archetype: variant.archetype,
                doors: variant.doors,
                up: variant.up,
                down: variant.down,
            });
        }
    }
}

fn emit(trace: &mut Option<&mut Vec<SolveStep>>, step: SolveStep) {
    if let Some(steps) = trace.as_deref_mut() {
        steps.push(step);
    }
}

fn signature_doors(signature: PortSignature) -> u8 {
    let mut mask = 0u8;
    for face in HexFace::LATERAL {
        if signature.port(face) == PortClass::Door {
            mask |= super::lateral_bit(face);
        }
    }
    mask
}

/// The starting domain of every cell: boundary-safe variants that satisfy the
/// forced route, with blueprint cells pinned to their exact stamped signature
/// and rooms excluded everywhere else.
#[allow(clippy::too_many_arguments)] // Solver constraints are separate typed maps by face class.
fn initial_domains(
    config: HexWfcConfig,
    variants: &[HexVariant],
    forced_doors: &BTreeMap<HexCoord, u8>,
    forced_up: &BTreeMap<HexCoord, PortClass>,
    forced_down: &BTreeMap<HexCoord, PortClass>,
    signatures: &BTreeMap<HexCoord, PortSignature>,
    previous: Option<&BTreeMap<HexCoord, HexPlacement>>,
    pinned: &BTreeSet<HexCoord>,
) -> Vec<VariantSet> {
    let grid = config.grid();
    all_coords(config)
        .map(|coord| {
            let required_doors = forced_doors.get(&coord).copied().unwrap_or(0);
            let required_up = forced_up.get(&coord).copied().unwrap_or(PortClass::Sealed);
            let required_down = forced_down
                .get(&coord)
                .copied()
                .unwrap_or(PortClass::Sealed);
            let blueprint_signature = signatures.get(&coord);

            variants
                .iter()
                .enumerate()
                .filter(|(_, variant)| {
                    if pinned.contains(&coord)
                        && previous.is_none_or(|placements| {
                            let locked = placements[&coord];
                            variant.space != locked.space
                                || variant.archetype != locked.archetype
                                || variant.doors != locked.doors
                                || variant.up != locked.up
                                || variant.down != locked.down
                        })
                    {
                        return false;
                    }
                    // No opening may face the lattice boundary.
                    if HexFace::ALL.iter().any(|&face| {
                        grid.neighbor(coord, face).is_none()
                            && ((face.is_lateral()
                                && variant.doors & super::lateral_bit(face) != 0)
                                || (face == HexFace::Up && variant.up != PortClass::Sealed)
                                || (face == HexFace::Down && variant.down != PortClass::Sealed))
                    }) {
                        return false;
                    }
                    if variant.doors & required_doors != required_doors {
                        return false;
                    }
                    if required_up != PortClass::Sealed && variant.up != required_up {
                        return false;
                    }
                    if required_down != PortClass::Sealed && variant.down != required_down {
                        return false;
                    }
                    match blueprint_signature {
                        // Blueprint cells collapse to exactly the stamped
                        // exterior signature.
                        Some(&signature) => {
                            variant.space == HexSpace::Room
                                && variant.doors == signature_doors(signature)
                                && variant.up == signature.port(HexFace::Up)
                                && variant.down == signature.port(HexFace::Down)
                        }
                        // Rooms exist only inside blueprint footprints;
                        // everything else is connective fabric.
                        None => variant.space != HexSpace::Room,
                    }
                })
                .fold(VariantSet::EMPTY, |mut domain, (index, _)| {
                    domain.insert(index);
                    domain
                })
        })
        .collect()
}

/// AC-3 style arc consistency over all eight faces: retain only variants with
/// a compatible neighbor variant across every face. Returns false on an
/// emptied domain (contradiction).
///
/// The worklist is caller-seeded: the initial pass after `initial_domains`
/// seeds every cell; the pass after observing one cell seeds only that cell
/// (an arc-consistent grid stays consistent everywhere else, and AC-3 reaches
/// the same unique fixpoint from either seeding).
fn propagate(
    config: HexWfcConfig,
    tables: &SolverTables,
    domains: &mut [VariantSet],
    mut queue: VecDeque<usize>,
    trace: &mut Option<&mut Vec<SolveStep>>,
) -> bool {
    let grid = config.grid();
    while let Some(index) = queue.pop_front() {
        let coord = grid.coord(index);
        for face in HexFace::ALL {
            let Some(neighbor) = grid.neighbor(coord, face) else {
                continue;
            };
            let neighbor_index = grid.index(neighbor);
            let source = domains[index];
            let mut allowed = VariantSet::EMPTY;
            for variant in source.iter() {
                allowed.union_with(tables.compat(variant, face));
            }
            let changed = domains[neighbor_index].intersect_with(&allowed);
            if domains[neighbor_index].is_empty() {
                emit(trace, SolveStep::Contradiction { coord: neighbor });
                return false;
            }
            if changed {
                emit_narrowing(trace, &tables.variants, domains, neighbor, neighbor_index);
                queue.push_back(neighbor_index);
            }
        }
    }
    true
}

/// Emit the right trace event for a narrowed domain: `Collapsed` when it hit
/// a single variant, `DomainPruned` otherwise.
fn emit_narrowing(
    trace: &mut Option<&mut Vec<SolveStep>>,
    variants: &[HexVariant],
    domains: &[VariantSet],
    coord: HexCoord,
    index: usize,
) {
    let remaining = domains[index].len();
    if let Some(only) = domains[index].single() {
        let variant = variants[only];
        emit(
            trace,
            SolveStep::Collapsed {
                coord,
                space: variant.space,
                archetype: variant.archetype,
                doors: variant.doors,
                up: variant.up,
                down: variant.down,
            },
        );
    } else {
        emit(
            trace,
            SolveStep::DomainPruned {
                coord,
                remaining: remaining as u16,
            },
        );
    }
}

/// Min-entropy observe/propagate loop. Returns false on contradiction.
fn collapse_domains(
    config: HexWfcConfig,
    tables: &SolverTables,
    domains: &mut [VariantSet],
    rng: &mut SplitMix,
    trace: &mut Option<&mut Vec<SolveStep>>,
) -> bool {
    let variants = &tables.variants[..];
    let grid = config.grid();
    loop {
        let min_size = domains
            .iter()
            .map(VariantSet::len)
            .filter(|&len| len > 1)
            .min();
        let Some(min_size) = min_size else {
            return true; // fully collapsed
        };
        let candidates: Vec<usize> = (0..domains.len())
            .filter(|&index| domains[index].len() == min_size)
            .collect();
        let cell = candidates[(rng.next_u64() % candidates.len() as u64) as usize];

        let total: u64 = domains[cell]
            .iter()
            .map(|variant| u64::from(variants[variant].weight))
            .sum();
        let first = domains[cell].iter().next().expect("candidate is non-empty");
        let picked = if total == 0 {
            first
        } else {
            let mut roll = rng.next_u64() % total;
            let mut chosen = first;
            for variant in domains[cell].iter() {
                let weight = u64::from(variants[variant].weight);
                if roll < weight {
                    chosen = variant;
                    break;
                }
                roll -= weight;
            }
            chosen
        };
        domains[cell] = VariantSet::only(picked);
        let variant = variants[picked];
        emit(
            trace,
            SolveStep::Collapsed {
                coord: grid.coord(cell),
                space: variant.space,
                archetype: variant.archetype,
                doors: variant.doors,
                up: variant.up,
                down: variant.down,
            },
        );
        // The rest of the grid was already arc-consistent, so the observed
        // cell alone seeds the incremental propagation.
        if !propagate(config, tables, domains, VecDeque::from([cell]), trace) {
            return false;
        }
    }
}

fn materialize(
    config: HexWfcConfig,
    variants: &[HexVariant],
    domains: &[VariantSet],
) -> BTreeMap<HexCoord, HexPlacement> {
    let grid = config.grid();
    domains
        .iter()
        .enumerate()
        .map(|(index, domain)| {
            let variant = variants[domain.single().expect("solved domain is a singleton")];
            let coord = grid.coord(index);
            (
                coord,
                HexPlacement {
                    coord,
                    space: variant.space,
                    archetype: variant.archetype,
                    doors: variant.doors,
                    up: variant.up,
                    down: variant.down,
                },
            )
        })
        .collect()
}

/// Void every cell unreachable from spawn (their doors pointed only at each
/// other, so edge consistency survives).
fn prune_disconnected(config: HexWfcConfig, placements: &mut BTreeMap<HexCoord, HexPlacement>) {
    let keep = super::topology::active_component(config, placements, config.spawn());
    for (coord, placement) in placements.iter_mut() {
        if placement.space != HexSpace::Void && !keep.contains(coord) {
            placement.space = HexSpace::Void;
            placement.archetype = HexArchetype::Void;
            placement.doors = 0;
            placement.up = PortClass::Sealed;
            placement.down = PortClass::Sealed;
        }
    }
}

fn mixed(seed: u64, generation: u32, attempt: u32, domain: u64) -> u64 {
    let mut rng = SplitMix::new(
        seed ^ u64::from(generation).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(attempt).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ domain.wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    rng.next_u64()
}
