//! Domain construction, AC-3 propagation, and min-entropy collapse.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, PortClass, PortSignature};

use super::blueprint::StampedBlueprint;
use super::constraints::{
    all_coords, forced_route_edges, stamp_blueprints_with_pins, stamped_signatures,
};
use super::profile::{HexCompositionProfile, PinIntent};
use super::trace::SolveStep;
use super::validate::layout_failure;
use super::variants::{HexVariant, catalogue, variants_compatible};
use super::{HexArchetype, HexPlacement, HexRoomQuotas, HexSpace, HexWfcConfig, HexWfcError};

type CollapseOutput = (BTreeMap<HexCoord, HexPlacement>, Vec<StampedBlueprint>, u32);

/// Fixed-width bitset over catalogue variant indices. The catalogue holds 404
/// variants since Phase 108 added `Expanse`, which needed one more word — six
/// gave 384 slots against 382 in use. `solver_tables` asserts the fit.
const MASK_WORDS: usize = 7;
/// A cadence event refreshes the architecture register across its full
/// target-32 pocket, but only this connected structural core is allowed to
/// change topology. This keeps collision churn bounded independently of
/// authored hull density.
const POCKET_TOPOLOGY_CELLS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct VariantSet([u64; MASK_WORDS]);

impl VariantSet {
    pub(super) const EMPTY: Self = Self([0; MASK_WORDS]);

    fn only(index: usize) -> Self {
        let mut set = Self::EMPTY;
        set.insert(index);
        set
    }

    pub(super) fn insert(&mut self, index: usize) {
        self.0[index / 64] |= 1 << (index % 64);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.iter().all(|&word| word == 0)
    }

    pub(super) fn len(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }

    /// The sole member of a collapsed (single-variant) domain.
    fn single(&self) -> Option<usize> {
        (self.len() == 1).then(|| self.iter().next().expect("len is 1"))
    }

    pub(super) fn union_with(&mut self, other: &Self) {
        for (word, extra) in self.0.iter_mut().zip(&other.0) {
            *word |= extra;
        }
    }

    /// Intersect in place; returns whether the set shrank.
    pub(super) fn intersect_with(&mut self, other: &Self) -> bool {
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
    pub(super) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
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
pub(super) struct SolverTables {
    pub(super) variants: Vec<HexVariant>,
    /// Indexed `variant * 8 + face.index()`.
    compat: Vec<VariantSet>,
}

impl SolverTables {
    pub(super) fn compat(&self, variant: usize, face: HexFace) -> &VariantSet {
        &self.compat[variant * 8 + face.index()]
    }
}

pub(super) fn solver_tables() -> &'static SolverTables {
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
    room_quotas: Option<HexRoomQuotas>,
    profile: &HexCompositionProfile,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseOutput, HexWfcError> {
    let mut last_failure: Option<&'static str> = None;
    let no_pins = BTreeSet::new();

    // Pin pre-flight, before a single attempt. A pin that can never be
    // satisfied — a door facing off the lattice, a room painted onto fabric —
    // would otherwise surface as `RetryBudgetExhausted` a hundred attempts
    // later, naming the budget instead of the mistake.
    let (authored, _) = super::pins::resolved_pins(config, profile);
    if !authored.is_empty()
        && let Some((coord, reason)) =
            super::pins::unsatisfiable(config, &authored, &solver_tables().variants)
    {
        return Err(HexWfcError::PinContradiction { coord, reason });
    }

    let retry_budget = profile
        .search
        .retry_budget_override
        .unwrap_or(config.retry_budget);
    for attempt in 0..retry_budget {
        let solved = if room_quotas.is_none() {
            collapse_attempt(
                seed,
                generation,
                attempt,
                config,
                None,
                &no_pins,
                profile,
                trace.as_deref_mut(),
            )
        } else {
            let tables = solver_tables();
            emit(&mut trace, SolveStep::AttemptStart { attempt });
            let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
            collapse_attempt_with_blueprints(
                seed,
                config,
                tables,
                &mut rng,
                None,
                &no_pins,
                &[],
                room_quotas,
                profile,
                trace.as_deref_mut(),
            )
        };
        match solved {
            Ok(solved) => return Ok((solved.placements, solved.blueprints, attempt + 1)),
            Err(reason) => last_failure = Some(reason),
        }
    }
    Err(HexWfcError::RetryBudgetExhausted {
        attempts: retry_budget,
        last_failure,
    })
}

/// Exactly one deterministic `(seed, generation, attempt)` collapse. Relayout
/// calls this once per simulation tick with the previous placements and the
/// complete pin set; ordinary generation uses the same path without pins.
#[allow(clippy::too_many_arguments)]
pub(super) fn collapse_attempt(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    previous: Option<&BTreeMap<HexCoord, HexPlacement>>,
    pinned: &BTreeSet<HexCoord>,
    profile: &HexCompositionProfile,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseAttempt, &'static str> {
    let tables = solver_tables();
    emit(&mut trace, SolveStep::AttemptStart { attempt });
    let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
    collapse_attempt_with_blueprints(
        seed,
        config,
        tables,
        &mut rng,
        previous,
        pinned,
        &[],
        None,
        profile,
        trace,
    )
}

/// Collapse only a bounded mutation pocket. Live outside neighbors become
/// exact boundary constraints, room cells remain tied to their stamped
/// blueprint signatures, and all propagation/entropy work is proportional to
/// the pocket rather than the full production lattice.
#[allow(clippy::too_many_arguments)]
pub(super) fn collapse_pocket_attempt(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    previous: &BTreeMap<HexCoord, HexPlacement>,
    blueprints: &[StampedBlueprint],
    region: &super::relayout::HexMutationRegion,
    influence: Option<&super::context::HexInfluenceField>,
) -> Result<CollapseAttempt, &'static str> {
    let tables = solver_tables();
    let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
    let topology_core =
        select_topology_core(seed, generation, attempt, config, previous, &region.cells);
    let mut domains = region
        .cells
        .iter()
        .copied()
        .map(|coord| {
            let before = previous[&coord];
            let domain = tables
                .variants
                .iter()
                .enumerate()
                .filter(|(_, variant)| {
                    if !topology_core.contains(&coord) {
                        return variant_matches(**variant, before);
                    }
                    if !variant_is_mutable_topology(**variant) {
                        return false;
                    }
                    HexFace::ALL
                        .iter()
                        .all(|&face| match config.grid().neighbor(coord, face) {
                            None => variant.signature().port(face) == PortClass::Sealed,
                            Some(neighbor) if !region.cells.contains(&neighbor) => {
                                variants_compatible(
                                    **variant,
                                    placement_variant(previous[&neighbor]),
                                    face,
                                )
                            }
                            Some(_) => true,
                        })
                })
                .fold(VariantSet::EMPTY, |mut domain, (index, _)| {
                    domain.insert(index);
                    domain
                });
            (coord, domain)
        })
        .collect::<BTreeMap<_, _>>();
    if domains.values().any(VariantSet::is_empty) {
        return Err("pocket boundary contradiction");
    }
    if !propagate_pocket(tables, region, &mut domains) {
        return Err("pocket propagation contradiction");
    }
    if !collapse_pocket_domains(tables, region, &mut domains, &mut rng, influence, previous) {
        return Err("pocket collapse contradiction");
    }
    let placements = domains
        .iter()
        .map(|(&coord, domain)| {
            let variant = tables.variants[domain.single().expect("solved pocket domain")];
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
        .collect();
    let blueprints = blueprints
        .iter()
        .filter(|blueprint| {
            blueprint
                .cells
                .iter()
                .all(|cell| region.cells.contains(cell))
        })
        .cloned()
        .collect();
    Ok(CollapseAttempt {
        placements,
        blueprints,
    })
}

fn select_topology_core(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    previous: &BTreeMap<HexCoord, HexPlacement>,
    region: &BTreeSet<HexCoord>,
) -> BTreeSet<HexCoord> {
    let eligible = region
        .iter()
        .copied()
        .filter(|coord| placement_is_mutable_topology(previous[coord]))
        .collect::<BTreeSet<_>>();
    let mut seeds = eligible.iter().copied().collect::<Vec<_>>();
    seeds.sort_by_key(|coord| {
        mixed(
            seed,
            generation,
            attempt,
            u64::try_from(config.grid().index(*coord)).expect("grid index fits u64")
                ^ 0xC011_1DE2_C0DE_0004,
        )
    });
    let mut best = BTreeSet::new();
    for seed_coord in seeds {
        let mut core = BTreeSet::new();
        let mut queued = BTreeSet::from([seed_coord]);
        let mut queue = VecDeque::from([seed_coord]);
        while let Some(coord) = queue.pop_front() {
            core.insert(coord);
            if core.len() == POCKET_TOPOLOGY_CELLS {
                return core;
            }
            let mut neighbors = HexFace::LATERAL
                .iter()
                .filter_map(|&face| config.grid().neighbor(coord, face))
                .filter(|neighbor| eligible.contains(neighbor) && queued.insert(*neighbor))
                .collect::<Vec<_>>();
            neighbors.sort_by_key(|coord| {
                mixed(
                    seed,
                    generation,
                    attempt,
                    u64::try_from(config.grid().index(*coord)).expect("grid index fits u64")
                        ^ 0xC011_1DE2_C0DE_0004,
                )
            });
            queue.extend(neighbors);
        }
        if core.len() > best.len() {
            best = core;
        }
    }
    best
}

fn placement_is_mutable_topology(placement: HexPlacement) -> bool {
    matches!(
        placement.archetype,
        HexArchetype::Void
            | HexArchetype::Straight
            | HexArchetype::Corner
            | HexArchetype::Junction
            // Open floor with nothing authored to protect: relayout is free to
            // reshape an expanse exactly as it reshapes a corridor.
            | HexArchetype::Expanse
    )
}

fn variant_is_mutable_topology(variant: HexVariant) -> bool {
    matches!(
        variant.archetype,
        HexArchetype::Void
            | HexArchetype::Straight
            | HexArchetype::Corner
            | HexArchetype::Junction
            | HexArchetype::Expanse
    )
}

/// The catalogue index of the variant a placement corresponds to.
///
/// Every placement the solver produces came out of the catalogue, so `None`
/// means the placement was invented somewhere else — worth refusing rather
/// than approximating.
pub(super) fn variant_index(tables: &SolverTables, placement: HexPlacement) -> Option<usize> {
    tables
        .variants
        .iter()
        .position(|&variant| variant_matches(variant, placement))
}

pub(super) fn placement_variant(placement: HexPlacement) -> HexVariant {
    HexVariant {
        space: placement.space,
        archetype: placement.archetype,
        doors: placement.doors,
        up: placement.up,
        down: placement.down,
        weight: 0,
    }
}

/// The seed-specific constraints one attempt of a solve ran under.
///
/// Blueprint stamping and the forced spawn→exit route are drawn from the
/// attempt's own RNG, so they are not recoverable from a solved world by
/// inspection. [`replay_constraints`] re-derives them by walking the same RNG
/// the same way — and its caller checks the replayed blueprints against the
/// world's own before believing a word of it.
pub(super) struct AttemptConstraints {
    pub(super) blueprints: Vec<StampedBlueprint>,
    pub(super) signatures: BTreeMap<HexCoord, PortSignature>,
    pub(super) forced_doors: BTreeMap<HexCoord, u8>,
    pub(super) forced_up: BTreeMap<HexCoord, PortClass>,
    pub(super) forced_down: BTreeMap<HexCoord, PortClass>,
}

/// Re-derive the constraints of one attempt, exactly as `collapse` produced
/// them.
///
/// This is a replay, not a reconstruction: it seeds the same `SplitMix` from
/// the same `(seed, generation, attempt)` and draws from it in the same order,
/// so a matching `attempt` yields byte-identical blueprints and route. A
/// mismatched one yields a *different valid-looking* answer, which is why the
/// caller must verify the blueprints against the world rather than trusting
/// the attempt number it was given.
pub(super) fn replay_constraints(
    seed: u64,
    generation: u32,
    attempt: u32,
    config: HexWfcConfig,
    room_quotas: Option<HexRoomQuotas>,
) -> Option<AttemptConstraints> {
    let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
    let districts = super::relayout::district_sites(seed, config);
    let blueprints = stamp_blueprints_with_pins(config, &mut rng, &[], &districts, room_quotas);
    let signatures = stamped_signatures(config, &blueprints);
    let (forced_doors, forced_up, forced_down) =
        forced_route_edges(config, &blueprints, &signatures, &mut rng)?;
    Some(AttemptConstraints {
        blueprints,
        signatures,
        forced_doors,
        forced_up,
        forced_down,
    })
}

pub(super) fn variant_matches(variant: HexVariant, placement: HexPlacement) -> bool {
    variant.space == placement.space
        && variant.archetype == placement.archetype
        && variant.doors == placement.doors
        && variant.up == placement.up
        && variant.down == placement.down
}

fn propagate_pocket(
    tables: &SolverTables,
    region: &super::relayout::HexMutationRegion,
    domains: &mut BTreeMap<HexCoord, VariantSet>,
) -> bool {
    let mut queue = region.cells.iter().copied().collect::<VecDeque<_>>();
    while let Some(coord) = queue.pop_front() {
        let source = domains[&coord];
        for face in HexFace::ALL {
            let Some(neighbor) = region.cells.iter().copied().find(|candidate| {
                let delta = face.delta();
                i32::from(candidate.q) == i32::from(coord.q) + delta.0
                    && i32::from(candidate.r) == i32::from(coord.r) + delta.1
                    && i32::from(candidate.level) == i32::from(coord.level) + delta.2
            }) else {
                continue;
            };
            let mut allowed = VariantSet::EMPTY;
            for variant in source.iter() {
                allowed.union_with(tables.compat(variant, face));
            }
            let neighbor_domain = domains.get_mut(&neighbor).expect("pocket neighbor domain");
            let changed = neighbor_domain.intersect_with(&allowed);
            if neighbor_domain.is_empty() {
                return false;
            }
            if changed {
                queue.push_back(neighbor);
            }
        }
    }
    true
}

fn collapse_pocket_domains(
    tables: &SolverTables,
    region: &super::relayout::HexMutationRegion,
    domains: &mut BTreeMap<HexCoord, VariantSet>,
    rng: &mut SplitMix,
    influence: Option<&super::context::HexInfluenceField>,
    previous: &BTreeMap<HexCoord, HexPlacement>,
) -> bool {
    loop {
        let Some(min_size) = domains
            .values()
            .map(VariantSet::len)
            .filter(|&len| len > 1)
            .min()
        else {
            return true;
        };
        let candidates = domains
            .iter()
            .filter_map(|(&coord, domain)| (domain.len() == min_size).then_some(coord))
            .collect::<Vec<_>>();
        let coord = candidates[(rng.next_u64() % candidates.len() as u64) as usize];
        let domain = domains[&coord];
        // Only a driven relayout weights the pocket, and only by the influence
        // bias (no geometry context). An ordinary relayout (`influence == None`)
        // keeps the exact static-weight lottery, so its output is byte-identical
        // to before this feature, and a `neutral` field matches it too.
        let adjacent_expanses = lateral_expanse_neighbors_map(coord, domains, tables, previous);
        let weight_of = |variant: usize| {
            let base = match influence {
                Some(field) => super::context::influenced_weight(
                    tables.variants[variant].archetype,
                    tables.variants[variant].weight,
                    field,
                ),
                None => u64::from(tables.variants[variant].weight),
            };
            clustered_expanse_weight(base, tables.variants[variant].archetype, adjacent_expanses)
        };
        let total = domain.iter().map(weight_of).sum::<u64>();
        let mut roll = rng.next_u64() % total.max(1);
        let mut picked = domain.iter().next().expect("non-empty pocket domain");
        for variant in domain.iter() {
            let weight = weight_of(variant);
            if roll < weight {
                picked = variant;
                break;
            }
            roll = roll.saturating_sub(weight);
        }
        domains.insert(coord, VariantSet::only(picked));
        if !propagate_pocket(tables, region, domains) {
            return false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collapse_attempt_with_blueprints(
    seed: u64,
    config: HexWfcConfig,
    tables: &SolverTables,
    rng: &mut SplitMix,
    previous: Option<&BTreeMap<HexCoord, HexPlacement>>,
    pinned: &BTreeSet<HexCoord>,
    locked_blueprints: &[StampedBlueprint],
    room_quotas: Option<HexRoomQuotas>,
    profile: &HexCompositionProfile,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseAttempt, &'static str> {
    let variants = &tables.variants[..];
    // Districts are a pure function of (seed, config), so they are known before
    // anything is stamped — which is what lets a room role belong to a district.
    let districts = super::relayout::district_sites(seed, config);
    let blueprints =
        stamp_blueprints_with_pins(config, rng, locked_blueprints, &districts, room_quotas);
    let signatures = stamped_signatures(config, &blueprints);
    let Some((forced_doors, forced_up, forced_down)) =
        forced_route_edges(config, &blueprints, &signatures, rng)
    else {
        return Err("blueprints blocked every forced route");
    };
    emit_blueprints(&blueprints, &forced_doors, &mut trace);

    // Authored pins are part of the profile, so they resolve from it rather
    // than riding another parameter down the call chain. Resolution is a few
    // map inserts against a full solve.
    let (authored, _) = super::pins::resolved_pins(config, profile);
    let mut domains = initial_domains(
        config,
        variants,
        &forced_doors,
        &forced_up,
        &forced_down,
        &signatures,
        previous,
        pinned,
        &authored,
    );
    if domains.iter().any(VariantSet::is_empty) {
        return Err("pinned cell contradicts blueprint, forced route, or an authored pin");
    }
    emit_precollapsed(config, variants, &domains, &signatures, &mut trace);
    // The initial pass seeds every cell; later passes are incremental.
    let all_cells: VecDeque<usize> = (0..domains.len()).collect();
    if !propagate(config, tables, &mut domains, all_cells, &mut trace) {
        return Err("propagation contradiction");
    }
    if !collapse_domains(
        config,
        tables,
        &mut domains,
        rng,
        &districts,
        profile,
        &mut trace,
    ) {
        return Err("collapse contradiction");
    }
    let mut placements = materialize(config, variants, &domains);
    prune_disconnected(config, &mut placements, &mut trace);
    if let Some(reason) = layout_failure(config, &placements, &blueprints) {
        return Err(reason);
    }
    if let Some(quotas) = room_quotas {
        if let Some(reason) = super::validate::room_quota_failure(&blueprints, quotas) {
            return Err(reason);
        }
        if let Some(reason) = super::validate::open_volume_failure(config, &placements, &blueprints)
        {
            return Err(reason);
        }
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
    authored: &BTreeMap<HexCoord, PinIntent>,
) -> Vec<VariantSet> {
    all_coords(config)
        .map(|coord| {
            let is_pinned = pinned.contains(&coord);
            let locked_to = if is_pinned {
                previous.map(|placements| placements[&coord])
            } else {
                None
            };
            initial_domain_for(
                config,
                variants,
                coord,
                forced_doors.get(&coord).copied().unwrap_or(0),
                forced_up.get(&coord).copied().unwrap_or(PortClass::Sealed),
                forced_down
                    .get(&coord)
                    .copied()
                    .unwrap_or(PortClass::Sealed),
                signatures.get(&coord).copied(),
                locked_to,
                is_pinned,
                authored.get(&coord),
            )
        })
        .collect()
}

/// One cell's starting domain, before any propagation.
///
/// Split out of [`initial_domains`] because the neighbourhood query re-opens a
/// single cell with the rest of a solved facility held fixed, and it has to
/// start from *this* rule rather than from a second copy of it. A re-opened
/// cell whose starting domain was slightly wider than the solver's would show
/// an author variety the game will never produce, which is the one failure
/// mode a preview of the solver must not have.
#[allow(clippy::too_many_arguments)] // Solver constraints are separate typed values by face class.
pub(super) fn initial_domain_for(
    config: HexWfcConfig,
    variants: &[HexVariant],
    coord: HexCoord,
    required_doors: u8,
    required_up: PortClass,
    required_down: PortClass,
    blueprint_signature: Option<PortSignature>,
    locked_to: Option<HexPlacement>,
    pinned: bool,
    authored: Option<&PinIntent>,
) -> VariantSet {
    let grid = config.grid();
    variants
        .iter()
        .enumerate()
        .filter(|(_, variant)| {
            // Authored intent narrows the domain like any other constraint
            // here — except where a stamped blueprint already fixes the cell.
            // Room footprints and named ports are a frozen contract, so the
            // blueprint wins and `pins::diagnose_pins` reports the collision
            // rather than this emptying the domain and calling it a
            // contradiction.
            if blueprint_signature.is_none()
                && let Some(intent) = authored
                && !super::pins::pin_admits(intent, variant)
            {
                return false;
            }
            if pinned && locked_to.is_none_or(|locked| !variant_matches(**variant, locked)) {
                return false;
            }
            // No opening may face the lattice boundary.
            if HexFace::ALL.iter().any(|&face| {
                grid.neighbor(coord, face).is_none()
                    && ((face.is_lateral() && variant.doors & super::lateral_bit(face) != 0)
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
                // Blueprint cells collapse to exactly the stamped sibling-seam
                // and exterior-threshold signature.
                Some(signature) => {
                    variant.space == HexSpace::Room
                        && variant.doors == signature_doors(signature)
                        && variant.up == signature.port(HexFace::Up)
                        && variant.down == signature.port(HexFace::Down)
                }
                // Rooms exist only inside blueprint footprints; everything else
                // is connective fabric.
                None => variant.space != HexSpace::Room,
            }
        })
        .fold(VariantSet::EMPTY, |mut domain, (index, _)| {
            domain.insert(index);
            domain
        })
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
    districts: &[super::relayout::DistrictSite],
    profile: &HexCompositionProfile,
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

        // Contextual composition: scale each variant's static weight by its
        // position in the grid and by the district it stands in. Never zeroes a
        // legal variant, and draws the same RNG values in the same order, so
        // determinism is preserved.
        let cell_coord = grid.coord(cell);
        let adjacent_expanses = HexFace::LATERAL
            .iter()
            .filter_map(|&face| grid.neighbor(cell_coord, face))
            .filter(|neighbor| {
                domains[grid.index(*neighbor)]
                    .single()
                    .is_some_and(|variant| variants[variant].archetype == HexArchetype::Expanse)
            })
            .count();
        let weight_of = |variant: usize| {
            let base = super::context::effective_weight(
                cell_coord,
                variants[variant].archetype,
                variants[variant].weight,
                config,
                super::relayout::district_of(cell_coord, districts),
                None,
                profile,
            );
            clustered_expanse_weight(base, variants[variant].archetype, adjacent_expanses)
        };
        let total: u64 = domains[cell].iter().map(weight_of).sum();
        let first = domains[cell].iter().next().expect("candidate is non-empty");
        let picked = if total == 0 {
            first
        } else {
            let mut roll = rng.next_u64() % total;
            let mut chosen = first;
            for variant in domains[cell].iter() {
                let weight = weight_of(variant);
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

fn clustered_expanse_weight(base: u64, archetype: HexArchetype, adjacent: usize) -> u64 {
    if archetype != HexArchetype::Expanse {
        return base;
    }
    match adjacent {
        0 => base,
        1 => base.saturating_mul(5) / 2,
        _ => base.saturating_mul(4),
    }
}

fn lateral_expanse_neighbors_map(
    coord: HexCoord,
    domains: &BTreeMap<HexCoord, VariantSet>,
    tables: &SolverTables,
    previous: &BTreeMap<HexCoord, HexPlacement>,
) -> usize {
    HexFace::LATERAL
        .iter()
        .filter_map(|&face| {
            let (dq, dr, dl) = face.delta();
            let q = i32::from(coord.q) + dq;
            let r = i32::from(coord.r) + dr;
            let level = i32::from(coord.level) + dl;
            (q >= 0 && r >= 0 && level >= 0).then_some(HexCoord {
                q: q as u16,
                r: r as u16,
                level: level as u8,
            })
        })
        .filter(|neighbor| {
            domains.get(neighbor).map_or_else(
                || {
                    previous
                        .get(neighbor)
                        .is_some_and(|placement| placement.archetype == HexArchetype::Expanse)
                },
                |domain| {
                    domain.single().is_some_and(|variant| {
                        tables.variants[variant].archetype == HexArchetype::Expanse
                    })
                },
            )
        })
        .count()
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
/// Void everything the spawn cannot reach, and say so in the trace.
///
/// This runs *after* the last collapse step, so it is the one place the solver
/// changes a cell without the trace hearing about it. That was invisible for as
/// long as no traced seed actually had anything to prune, and it stopped being
/// invisible the moment one did: the fold replays the collapse steps, the solver
/// returns the pruned world, and the two disagree by exactly the cells voided
/// here. A step-by-step viewer would draw a facility the solver never returned.
///
/// So the prune emits the same `Collapsed` step any other resolution does. The
/// cell really is resolved, and it really is `Void`; re-resolving a coord is
/// something `fold_trace` already handles, and the summary counts distinct
/// resolved cells rather than steps, so nothing double-counts.
fn prune_disconnected(
    config: HexWfcConfig,
    placements: &mut BTreeMap<HexCoord, HexPlacement>,
    trace: &mut Option<&mut Vec<SolveStep>>,
) {
    let keep = super::topology::active_component(config, placements, config.spawn());
    for (coord, placement) in placements.iter_mut() {
        if placement.space != HexSpace::Void && !keep.contains(coord) {
            placement.space = HexSpace::Void;
            placement.archetype = HexArchetype::Void;
            placement.doors = 0;
            placement.up = PortClass::Sealed;
            placement.down = PortClass::Sealed;
            emit(
                trace,
                SolveStep::Collapsed {
                    coord: *coord,
                    space: HexSpace::Void,
                    archetype: HexArchetype::Void,
                    doors: 0,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                },
            );
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
