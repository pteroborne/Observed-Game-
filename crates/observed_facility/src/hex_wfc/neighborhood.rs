//! What could have stood next to this cell.
//!
//! A solved facility shows one answer. An author tuning a composition profile
//! needs the other question: *given this tile, what is the range of things the
//! solver could legally have put around it, and how likely is each one?* That
//! is the difference between "this seed produced a corner here" and "every seed
//! will produce a corner here", and it is the difference between tuning and
//! guessing.
//!
//! # This re-opens the ring; it does not enumerate the alphabet
//!
//! The cheap version of this question — "every variant compatible with the
//! centre tile across this face" — is wrong in a way that flatters the corpus.
//! It ignores the rest of the facility, and the rest of the facility is most of
//! the constraint: a cell's other neighbours are already placed, its faces at
//! the lattice edge must be sealed, a blueprint may own it outright, and the
//! six lateral ring cells are neighbours *of each other*, so they constrain
//! each other too. An author shown that set would tune against variety that
//! cannot occur.
//!
//! So this re-opens the ring properly: the centre and everything outside the
//! ring stay exactly as solved, every ring cell goes back to its starting
//! domain, and the solver's own AC-3 propagation runs to a fixpoint over them.
//! What survives is what could actually have been there. Both numbers are
//! reported — [`FaceDomain::from_centre`] is the flattering one and
//! [`FaceDomain::candidates`] is the true one — because the gap between them is
//! itself the interesting fact: it says how much of this cell's character comes
//! from its own tile and how much from where it happens to stand.
//!
//! # Nothing here restates a solver rule
//!
//! Starting domains come from [`collapse::initial_domain_for`], compatibility
//! from the solver's own `(variant, face)` tables, propagation from the same
//! AC-3 pass, and weights from [`context::effective_weight`] — the function the
//! collapse lottery itself calls. The seed-specific constraints (stamped
//! blueprints, the forced spawn→exit route) are not recoverable from a solved
//! world by inspection, so they are *replayed* from the attempt's own RNG and
//! then checked against the world's own blueprints. A replay that disagrees is
//! [`NeighborhoodError::ConstraintReplayFailed`] rather than a plausible
//! answer, for the reason the whole studio is built on: a preview that quietly
//! diverges from the solver is worse than no preview.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_core::SplitMix;

use super::collapse::{
    VariantSet, initial_domain_for, replay_constraints, solver_tables, variant_index,
};
use super::context;
use super::profile::HexCompositionProfile;
use super::relayout::{district_of, district_sites};
use super::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexRoomQuotas, HexWfcConfig, HexWfcWorld,
};

/// Why a neighbourhood could not be described.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeighborhoodError {
    /// The coordinate is not a cell of this world.
    NotPlaced(HexCoord),
    /// The attempt's blueprints and route could not be re-derived, or the
    /// replay produced blueprints the world does not have. Either way the
    /// constraint set is unknown, and reporting a domain without it would
    /// over-report.
    ConstraintReplayFailed,
    /// Re-opening the ring emptied a domain. This should be unreachable — the
    /// solved placements are themselves a witness that the ring is satisfiable
    /// — so it means a solver rule and this query have drifted apart.
    Contradiction(HexCoord),
    /// A solved placement is not in the variant catalogue.
    UnknownPlacement(HexCoord),
}

/// One thing that could have stood in a neighbouring cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeighborCandidate {
    pub placement: HexPlacement,
    /// The profile-adjusted weight the collapse lottery would give this
    /// variant *in this cell* — position, district, and archetype bias all
    /// applied. This is the number that moves when a slider moves.
    pub weight: u64,
    /// Whether this is what the current seed actually produced.
    pub is_actual: bool,
}

impl NeighborCandidate {
    /// This candidate's share of the lottery, in its own cell.
    #[must_use]
    pub fn share(&self, total: u64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.weight as f64 / total as f64
        }
    }
}

/// Everything one neighbouring cell could have been.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceDomain {
    pub face: HexFace,
    pub coord: HexCoord,
    /// What the centre tile alone permits across this face, before the rest of
    /// the facility is consulted. Always at least `candidates.len()`, and the
    /// gap between them is how much this cell's character comes from its
    /// surroundings rather than from its neighbour.
    pub from_centre: usize,
    /// What survives once the locked facility outside the ring and the other
    /// ring cells have had their say. Weight-ordered, heaviest first, so the
    /// head of the list is what an author will actually see most often.
    pub candidates: Vec<NeighborCandidate>,
    /// What this seed placed here. Always a member of `candidates`.
    pub actual: HexPlacement,
    /// Summed weight over `candidates`, for turning a weight into a share.
    pub total_weight: u64,
}

impl FaceDomain {
    /// Whether the solver had no choice here at all.
    #[must_use]
    pub fn is_forced(&self) -> bool {
        self.candidates.len() <= 1
    }

    /// Distinct archetypes among the candidates, heaviest first — the coarse
    /// answer to "what kind of thing can be here", which is usually the
    /// question before "which exact variant".
    #[must_use]
    pub fn archetype_spread(&self) -> Vec<(HexArchetype, u64, usize)> {
        let mut by_archetype: BTreeMap<HexArchetype, (u64, usize)> = BTreeMap::new();
        for candidate in &self.candidates {
            let entry = by_archetype
                .entry(candidate.placement.archetype)
                .or_insert((0, 0));
            entry.0 += candidate.weight;
            entry.1 += 1;
        }
        let mut spread: Vec<(HexArchetype, u64, usize)> = by_archetype
            .into_iter()
            .map(|(archetype, (weight, count))| (archetype, weight, count))
            .collect();
        spread.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        spread
    }
}

/// The re-opened ring around one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Neighborhood {
    pub centre: HexCoord,
    pub centre_placement: HexPlacement,
    /// One entry per in-grid face, in [`HexFace::ALL`] order.
    pub faces: Vec<FaceDomain>,
}

impl Neighborhood {
    /// The face domain for one face, if that face has an in-grid neighbour.
    #[must_use]
    pub fn face(&self, face: HexFace) -> Option<&FaceDomain> {
        self.faces.iter().find(|domain| domain.face == face)
    }

    /// How much freedom this cell's surroundings have, as one number: the
    /// product of the per-face candidate counts, saturating. The count of
    /// distinct rings, if the ring cells did not constrain each other — an
    /// upper bound, and the honest way to say "about this many".
    #[must_use]
    pub fn ring_upper_bound(&self) -> u128 {
        self.faces.iter().fold(1u128, |total, domain| {
            total.saturating_mul(domain.candidates.len().max(1) as u128)
        })
    }

    /// One complete ring the solver could have produced, drawn from `roll`.
    ///
    /// This is a real min-entropy collapse over the re-opened ring using the
    /// same weighted lottery the solver uses, so successive rolls walk the
    /// space of *consistent* rings rather than picking each face independently
    /// — which would routinely produce rings that cannot exist, because two
    /// lateral ring cells are neighbours of each other.
    ///
    /// `None` only on a contradiction, which the constructor has already ruled
    /// out; it is returned rather than panicked on so a preview never takes
    /// down the tool.
    #[must_use]
    pub fn sample_ring(
        &self,
        world: &HexWfcWorld,
        profile: &HexCompositionProfile,
        roll: u64,
    ) -> Option<BTreeMap<HexCoord, HexPlacement>> {
        let tables = solver_tables();
        let config = world.config;
        let ring: BTreeSet<HexCoord> = self.faces.iter().map(|domain| domain.coord).collect();
        let districts = district_sites(world.seed, config);

        let mut domains: BTreeMap<HexCoord, VariantSet> = BTreeMap::new();
        for domain in &self.faces {
            let mut set = VariantSet::EMPTY;
            for candidate in &domain.candidates {
                set.insert(variant_index(tables, candidate.placement)?);
            }
            domains.insert(domain.coord, set);
        }

        let mut rng = SplitMix::new(roll);
        // Min-entropy observe/propagate, exactly as `collapse_domains` runs it,
        // until every ring cell holds one variant.
        while let Some(min_size) = domains
            .values()
            .map(VariantSet::len)
            .filter(|&len| len > 1)
            .min()
        {
            let open: Vec<HexCoord> = domains
                .iter()
                .filter_map(|(&coord, set)| (set.len() == min_size).then_some(coord))
                .collect();
            #[allow(clippy::cast_possible_truncation)]
            let coord = open[(rng.next_u64() % open.len() as u64) as usize];
            let domain = domains[&coord];
            let weight_of = |variant: usize| {
                context::effective_weight(
                    coord,
                    tables.variants[variant].archetype,
                    tables.variants[variant].weight,
                    config,
                    district_of(coord, &districts),
                    None,
                    profile,
                )
            };
            let total: u64 = domain.iter().map(weight_of).sum();
            let first = domain.iter().next()?;
            let picked = if total == 0 {
                first
            } else {
                let mut remaining = rng.next_u64() % total;
                let mut chosen = first;
                for variant in domain.iter() {
                    let weight = weight_of(variant);
                    if remaining < weight {
                        chosen = variant;
                        break;
                    }
                    remaining -= weight;
                }
                chosen
            };
            domains.insert(coord, single(picked));
            if !propagate_ring(&ring, &mut domains, config) {
                return None;
            }
        }

        let mut placements = BTreeMap::new();
        for (&coord, set) in &domains {
            let index = set.iter().next()?;
            placements.insert(coord, placement_of(tables.variants[index], coord));
        }
        Some(placements)
    }
}

fn single(index: usize) -> VariantSet {
    let mut set = VariantSet::EMPTY;
    set.insert(index);
    set
}

fn placement_of(variant: super::variants::HexVariant, coord: HexCoord) -> HexPlacement {
    HexPlacement {
        coord,
        space: variant.space,
        archetype: variant.archetype,
        doors: variant.doors,
        up: variant.up,
        down: variant.down,
    }
}

/// AC-3 over the ring cells only. Everything outside is already folded into
/// the starting domains as a fixed constraint, so it never needs revisiting.
fn propagate_ring(
    ring: &BTreeSet<HexCoord>,
    domains: &mut BTreeMap<HexCoord, VariantSet>,
    config: HexWfcConfig,
) -> bool {
    let tables = solver_tables();
    let grid = config.grid();
    let mut queue: VecDeque<HexCoord> = ring.iter().copied().collect();
    while let Some(coord) = queue.pop_front() {
        let source = domains[&coord];
        for face in HexFace::ALL {
            let Some(neighbor) = grid.neighbor(coord, face) else {
                continue;
            };
            if !ring.contains(&neighbor) {
                continue;
            }
            let mut allowed = VariantSet::EMPTY;
            for variant in source.iter() {
                allowed.union_with(tables.compat(variant, face));
            }
            let target = domains.get_mut(&neighbor).expect("ring domain");
            let changed = target.intersect_with(&allowed);
            if target.is_empty() {
                return false;
            }
            if changed {
                queue.push_back(neighbor);
            }
        }
    }
    true
}

/// Re-open the ring around `centre` and report what each neighbour could have
/// been.
///
/// `quotas` must be the quotas the world was solved under; they take part in
/// blueprint stamping, so passing the wrong ones makes the constraint replay
/// fail loudly rather than answer wrongly.
///
/// # Errors
///
/// See [`NeighborhoodError`]. The interesting one is
/// [`NeighborhoodError::ConstraintReplayFailed`]: the seed-specific
/// constraints could not be recovered, so no honest domain can be reported.
pub fn neighborhood(
    world: &HexWfcWorld,
    profile: &HexCompositionProfile,
    quotas: Option<HexRoomQuotas>,
    centre: HexCoord,
) -> Result<Neighborhood, NeighborhoodError> {
    let tables = solver_tables();
    let config = world.config;
    let grid = config.grid();
    let centre_placement = *world
        .placements
        .get(&centre)
        .ok_or(NeighborhoodError::NotPlaced(centre))?;
    let centre_variant = variant_index(tables, centre_placement)
        .ok_or(NeighborhoodError::UnknownPlacement(centre))?;

    // `last_attempts` is 1-based and counts the accepted attempt, so the
    // accepted attempt's index is one less. Verified below rather than
    // assumed: a replay at the wrong attempt produces a different, entirely
    // plausible constraint set.
    let attempt = world.last_attempts.saturating_sub(1);
    let constraints = replay_constraints(world.seed, world.generation, attempt, config, quotas)
        .ok_or(NeighborhoodError::ConstraintReplayFailed)?;
    if constraints.blueprints != world.blueprints {
        return Err(NeighborhoodError::ConstraintReplayFailed);
    }

    let (authored, _) = super::pins::resolved_pins(config, profile);
    let districts = district_sites(world.seed, config);

    let ring: BTreeSet<HexCoord> = HexFace::ALL
        .iter()
        .filter_map(|&face| grid.neighbor(centre, face))
        .filter(|coord| world.placements.contains_key(coord))
        .collect();

    // Starting domains, from the solver's own rule, then narrowed by every
    // fixed thing around the cell: the centre across the shared face, and each
    // locked neighbour outside the ring across theirs.
    let mut domains: BTreeMap<HexCoord, VariantSet> = BTreeMap::new();
    let mut from_centre: BTreeMap<HexCoord, usize> = BTreeMap::new();
    for &coord in &ring {
        let mut domain = initial_domain_for(
            config,
            &tables.variants,
            coord,
            constraints.forced_doors.get(&coord).copied().unwrap_or(0),
            constraints
                .forced_up
                .get(&coord)
                .copied()
                .unwrap_or(super::PortClass::Sealed),
            constraints
                .forced_down
                .get(&coord)
                .copied()
                .unwrap_or(super::PortClass::Sealed),
            constraints.signatures.get(&coord).copied(),
            Some(world.placements[&coord]),
            world.authored_pins.contains(&coord),
            authored.get(&coord),
        );

        // The centre's own contribution, recorded before anything else narrows
        // the domain — this is the flattering number, and it is worth showing
        // exactly because it is the one a simpler tool would have shown alone.
        let face = face_from(centre, coord, grid).ok_or(NeighborhoodError::NotPlaced(coord))?;
        domain.intersect_with(tables.compat(centre_variant, face));
        from_centre.insert(coord, domain.len());

        for other in HexFace::ALL {
            let Some(neighbor) = grid.neighbor(coord, other) else {
                continue;
            };
            if neighbor == centre || ring.contains(&neighbor) {
                continue;
            }
            let Some(placement) = world.placements.get(&neighbor) else {
                continue;
            };
            let index = variant_index(tables, *placement)
                .ok_or(NeighborhoodError::UnknownPlacement(neighbor))?;
            // Compatibility is symmetric across the shared face: what the
            // locked neighbour permits of this cell is what this cell permits
            // of it, seen from the other side.
            domain.intersect_with(tables.compat(index, other.opposite()));
        }
        if domain.is_empty() {
            return Err(NeighborhoodError::Contradiction(coord));
        }
        domains.insert(coord, domain);
    }

    if !propagate_ring(&ring, &mut domains, config) {
        return Err(NeighborhoodError::Contradiction(centre));
    }

    let mut faces = Vec::new();
    for face in HexFace::ALL {
        let Some(coord) = grid.neighbor(centre, face) else {
            continue;
        };
        let Some(domain) = domains.get(&coord) else {
            continue;
        };
        let actual = world.placements[&coord];
        let district = district_of(coord, &districts);
        let mut candidates: Vec<NeighborCandidate> = domain
            .iter()
            .map(|index| {
                let variant = tables.variants[index];
                let placement = placement_of(variant, coord);
                NeighborCandidate {
                    placement,
                    weight: context::effective_weight(
                        coord,
                        variant.archetype,
                        variant.weight,
                        config,
                        district,
                        None,
                        profile,
                    ),
                    is_actual: placement == actual,
                }
            })
            .collect();
        // Heaviest first, so the head of the list is what an author will
        // mostly see. Ties break on the placement itself rather than on
        // catalogue order, so the list does not reshuffle when the alphabet
        // grows.
        candidates.sort_by(|a, b| {
            b.weight.cmp(&a.weight).then_with(|| {
                (a.placement.archetype, a.placement.doors)
                    .cmp(&(b.placement.archetype, b.placement.doors))
            })
        });
        let total_weight = candidates.iter().map(|candidate| candidate.weight).sum();
        faces.push(FaceDomain {
            face,
            coord,
            from_centre: from_centre.get(&coord).copied().unwrap_or(0),
            candidates,
            actual,
            total_weight,
        });
    }

    Ok(Neighborhood {
        centre,
        centre_placement,
        faces,
    })
}

/// The face of `from` that `to` sits across, by exact grid step.
fn face_from(from: HexCoord, to: HexCoord, grid: super::HexGridSize) -> Option<HexFace> {
    HexFace::ALL
        .into_iter()
        .find(|&face| grid.neighbor(from, face) == Some(to))
}
