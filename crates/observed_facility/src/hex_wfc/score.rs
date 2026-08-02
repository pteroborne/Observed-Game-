//! Deterministic post-hoc scoring of a solved [`HexWfcWorld`].
//!
//! WFC's `collapse` already answers "is this layout legal?" (Phase 90+).
//! This module answers the orthogonal question "which legal layout is the
//! most interesting one?" so [`super::HexWfcWorld::generate_best`] can solve
//! several candidates and keep the best. [`score_layout`] is a pure function
//! of an already-solved world's existing public accessors: it never re-solves,
//! mutates, or reaches for wall-clock time or non-deterministic randomness.

use std::collections::BTreeMap;

use observed_hex::{HexCoord, travel_distance};

use super::profile::ScoreWeights;
use super::{HexArchetype, HexSpace, HexWfcWorld};

/// Fixed component weights folded into [`LayoutScore::total`]. Tuned by feel
/// rather than derived; kept as named constants so retuning is a one-line
/// diff instead of a magic-number hunt through `score_layout`.
pub(super) const WEIGHT_CONNECTIVITY: f64 = 3.0;
pub(super) const WEIGHT_ELEVATION: f64 = 1.5;
pub(super) const WEIGHT_ROOM_WHOLENESS: f64 = 2.0;
pub(super) const WEIGHT_VARIETY: f64 = 1.5;
pub(super) const WEIGHT_RHYTHM: f64 = 1.0;

/// The traversal-grammar kinds `archetype_variety` distributes probability
/// mass over (every non-`Void` [`HexArchetype`] variant). Used to normalize
/// the raw Shannon entropy into a stable `0.0..=1.0` range.
// Room, Straight, Corner, Junction, RampUp, RampHead, Shaft, Expanse
const SCOREABLE_ARCHETYPE_KINDS: u32 = 8;

/// Breakdown of a solved layout's "interestingness". Every field is a pure
/// function of [`HexWfcWorld`]'s existing public accessors, so two calls
/// against the same world always agree (see the `determinism` tests below).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutScore {
    /// Fraction of stamped rooms reachable from spawn (`0.0..=1.0`) plus a
    /// small bonus for having more distinct hall components, a cheap proxy
    /// for redundant routing rather than one single braided path.
    pub connectivity: f64,
    /// Distinct vertical levels touched by any live placement, as a fraction
    /// of `config.levels` (`0.0..=1.0`).
    pub elevation: f64,
    /// Share of `Room`-space cells that belong to a fully stamped blueprint
    /// rather than a per-cell fallback placement (`0.0..=1.0`).
    pub room_wholeness: f64,
    /// Normalized Shannon entropy (`0.0..=1.0`) of the archetype-frequency
    /// distribution across live cells — higher means a more varied mix of
    /// traversal shapes instead of one archetype dominating.
    pub archetype_variety: f64,
    /// Evenness of nearest-neighbor spacing between junction/shaft
    /// "landmark" cells (`0.0..=1.0`): 1.0 is perfectly even spacing, 0.0 is
    /// maximally clumped or too few landmarks to judge.
    pub junction_rhythm: f64,
    /// Fixed-weight sum of the components above. The only field candidate
    /// selection compares; see [`super::HexWfcWorld::generate_best`].
    pub total: f64,
}

/// Score a solved layout with the shipped weights. Pure and total-ordering
/// stable: identical worlds (same seed/config) always yield a bit-identical
/// [`LayoutScore`].
#[must_use]
pub fn score_layout(world: &HexWfcWorld) -> LayoutScore {
    score_layout_with(world, ScoreWeights::baseline())
}

/// Score a solved layout with authored component weights.
///
/// The five components are unchanged — only how much each counts toward
/// [`LayoutScore::total`] moves — so a profile retunes what "best" means
/// without changing what is measured. Passing
/// [`ScoreWeights::baseline`] is exactly [`score_layout`].
#[must_use]
pub fn score_layout_with(world: &HexWfcWorld, weights: ScoreWeights) -> LayoutScore {
    let connectivity = connectivity_score(world);
    let elevation = elevation_score(world);
    let room_wholeness = room_wholeness_score(world);
    let archetype_variety = archetype_variety_score(world);
    let junction_rhythm = junction_rhythm_score(world);

    let total = weights.connectivity * connectivity
        + weights.elevation * elevation
        + weights.room_wholeness * room_wholeness
        + weights.variety * archetype_variety
        + weights.rhythm * junction_rhythm;

    LayoutScore {
        connectivity,
        elevation,
        room_wholeness,
        archetype_variety,
        junction_rhythm,
        total,
    }
}

/// Reachable-room fraction from spawn (flood fill over open doors/vertical
/// ports, matching [`HexWfcWorld::route_between`]'s connectivity notion),
/// scaled up slightly by the number of distinct hall components.
fn connectivity_score(world: &HexWfcWorld) -> f64 {
    if world.blueprints.is_empty() {
        return 0.0;
    }
    let component =
        super::topology::active_component(world.config, &world.placements, world.config.spawn());
    let reachable = world
        .blueprints
        .iter()
        .filter(|blueprint| blueprint.cells.iter().any(|cell| component.contains(cell)))
        .count();
    let fraction = f64::from(u32::try_from(reachable).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(world.blueprints.len()).unwrap_or(1));
    let corridor_bonus = (world.corridors().len() as f64).sqrt() * 0.1;
    fraction + corridor_bonus
}

/// Distinct vertical levels touched by any live (non-`Void`) placement, as a
/// fraction of the configured level count.
fn elevation_score(world: &HexWfcWorld) -> f64 {
    let levels = world
        .placements
        .values()
        .filter(|placement| placement.space != HexSpace::Void)
        .map(|placement| placement.coord.level)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let denom = f64::from(world.config.levels.max(1));
    f64::from(u32::try_from(levels).unwrap_or(0)) / denom
}

/// Share of `Room`-space cells covered by a full stamped blueprint rather
/// than an unattached single-cell `Room` placement.
fn room_wholeness_score(world: &HexWfcWorld) -> f64 {
    let room_cells = world.room_count();
    if room_cells == 0 {
        return 0.0;
    }
    let blueprint_cells: usize = world.blueprints.iter().map(|b| b.cells.len()).sum();
    (blueprint_cells as f64 / room_cells as f64).min(1.0)
}

/// How varied the facility's composition is — measured **within** districts and
/// then averaged, not globally.
///
/// A global entropy over every cell rewards a facility that is uniformly mixed
/// everywhere, which is exactly the layout Arc O is trying to stop producing.
/// Since Phase 107 each district deliberately specialises: Liminal Grid is
/// junction-heavy, Overlit Grid is turns, Wellshaft is vertical. Scored
/// globally, a facility whose districts are strongly characterised looks *worse*
/// than mush, so the score would push `generate_best` to pick the mush.
///
/// Measuring inside each district and averaging asks the question that still
/// matters — is any one neighbourhood monotonous — while leaving districts free
/// to differ from each other.
fn archetype_variety_score(world: &HexWfcWorld) -> f64 {
    let mut per_district: BTreeMap<
        observed_content::ArchitectureRegister,
        BTreeMap<HexArchetype, u32>,
    > = BTreeMap::new();
    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        let Some(register) = world.architecture.get(coord) else {
            continue;
        };
        *per_district
            .entry(*register)
            .or_default()
            .entry(placement.archetype)
            .or_insert(0) += 1;
    }
    if per_district.is_empty() {
        return 0.0;
    }
    let max_entropy = f64::from(SCOREABLE_ARCHETYPE_KINDS).log2();
    if max_entropy <= 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    for counts in per_district.values() {
        let total: u32 = counts.values().sum();
        if total == 0 {
            continue;
        }
        let entropy: f64 = counts
            .values()
            .map(|&count| {
                let p = f64::from(count) / f64::from(total);
                -p * p.log2()
            })
            .sum();
        sum += (entropy / max_entropy).clamp(0.0, 1.0);
    }
    #[allow(clippy::cast_precision_loss)]
    let mean = sum / per_district.len() as f64;
    mean
}

/// Evenness of nearest-neighbor spacing between junction/shaft cells: the
/// inverse of the coefficient of variation of nearest-neighbor distances, so
/// a tight, regular rhythm scores near 1.0 and a clumpy one scores near 0.0.
fn junction_rhythm_score(world: &HexWfcWorld) -> f64 {
    let landmarks: Vec<HexCoord> = world
        .placements
        .values()
        .filter(|placement| {
            matches!(
                placement.archetype,
                HexArchetype::Junction | HexArchetype::Shaft
            )
        })
        .map(|placement| placement.coord)
        .collect();
    if landmarks.len() < 2 {
        return 0.0;
    }
    let nearest_distances: Vec<f64> = landmarks
        .iter()
        .enumerate()
        .map(|(index, &a)| {
            landmarks
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != index)
                .map(|(_, &b)| travel_distance(a, b))
                .min()
                .unwrap_or(0)
        })
        .map(f64::from)
        .collect();
    let mean = nearest_distances.iter().sum::<f64>() / nearest_distances.len() as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let variance = nearest_distances
        .iter()
        .map(|distance| (distance - mean).powi(2))
        .sum::<f64>()
        / nearest_distances.len() as f64;
    let coefficient_of_variation = variance.sqrt() / mean;
    (1.0 / (1.0 + coefficient_of_variation)).clamp(0.0, 1.0)
}

/// Deterministic per-candidate seed derivation used by
/// [`super::HexWfcWorld::generate_best`]. Independent from the collapse
/// solver's own internal `(seed, generation, attempt)` mixing in
/// `collapse.rs`: this only needs to fan a single caller seed out into
/// `candidates` distinct, reproducible solver seeds.
pub(super) fn candidate_seed(seed: u64, candidate: u32) -> u64 {
    let mut rng = observed_core::SplitMix::new(
        seed ^ u64::from(candidate).wrapping_mul(0xD1B5_4A32_D192_ED03),
    );
    rng.next_u64()
}

#[cfg(test)]
mod tests {
    use super::super::HexWfcConfig;
    use super::super::profile::HexCompositionProfile;
    use super::*;

    /// Small-but-solvable config so the determinism tests below stay fast.
    fn small_config() -> HexWfcConfig {
        HexWfcConfig {
            cols: 6,
            rows: 6,
            levels: 2,
            min_rooms: 2,
            max_rooms: 4,
            retry_budget: 100,
            min_room_distance: 2,
        }
    }

    #[test]
    fn score_layout_is_deterministic_for_a_fixed_seed_and_config() {
        let config = small_config();
        let world = HexWfcWorld::generate(1234, config).expect("fixed seed must solve");
        let first = score_layout(&world);
        let second = score_layout(&world);
        assert_eq!(first, second);

        // Re-solving from scratch with the same (seed, config) must also
        // reproduce the identical score, not just the identical world.
        let world_again = HexWfcWorld::generate(1234, config).expect("fixed seed must solve");
        assert_eq!(first, score_layout(&world_again));
    }

    #[test]
    fn generate_best_is_deterministic_and_solves() {
        let config = small_config();
        let first = HexWfcWorld::generate_best(99, config, 4).expect("candidates must solve");
        let second = HexWfcWorld::generate_best(99, config, 4).expect("candidates must solve");
        assert_eq!(first.placements, second.placements);
        assert_eq!(first.blueprints, second.blueprints);
        assert_eq!(first.seed, second.seed);
    }

    /// Candidate 0 is `seed` itself, so raising the candidate count can only
    /// improve on the layout you already had - never replace it with something
    /// worse or merely different.
    #[test]
    fn searching_never_returns_a_layout_worse_than_the_plain_seed() {
        let config = small_config();
        let plain = HexWfcWorld::generate(99, config).expect("the seed itself must solve");
        let plain_score = score_layout(&plain);

        let best = HexWfcWorld::generate_best(99, config, 4).expect("candidates must solve");
        assert!(
            score_layout(&best).total >= plain_score.total,
            "search returned a worse layout than solving the seed directly"
        );
    }

    /// The seed a single candidate solves at must be the seed itself, or every
    /// facility in the repo silently moves the moment this path is taken.
    #[test]
    fn one_candidate_is_exactly_a_plain_solve() {
        let config = small_config();
        let profile = HexCompositionProfile::baseline();
        let (searched, ladder) =
            HexWfcWorld::generate_searched_with_profile(99, config, None, &profile)
                .expect("must solve");
        let plain = HexWfcWorld::generate(99, config).expect("must solve");

        assert_eq!(profile.search.candidates, 1, "baseline must not search");
        assert_eq!(searched.placements, plain.placements);
        assert_eq!(searched.seed, plain.seed);
        assert_eq!(ladder.len(), 1);
        assert_eq!(ladder[0].seed, 99, "candidate 0 must be the seed itself");
    }

    /// The ladder is the tool's whole justification for spending N solves, so
    /// it must account for every candidate and name exactly one winner.
    #[test]
    fn the_ladder_records_every_candidate_and_exactly_one_winner() {
        let config = small_config();
        let mut profile = HexCompositionProfile::baseline();
        profile.search.candidates = 4;

        let (world, ladder) =
            HexWfcWorld::generate_searched_with_profile(99, config, None, &profile)
                .expect("must solve");

        assert_eq!(ladder.len(), 4);
        assert_eq!(ladder.iter().filter(|c| c.winner).count(), 1);
        for (position, candidate) in ladder.iter().enumerate() {
            assert_eq!(candidate.index as usize, position, "ladder is out of order");
        }

        let winner = ladder.iter().find(|c| c.winner).expect("a winner");
        assert_eq!(
            winner.seed, world.seed,
            "the returned world must be the candidate the ladder names"
        );
        // The winner is the maximum, and ties resolve to the lower index.
        let best_total = ladder
            .iter()
            .filter_map(|c| c.score.as_ref())
            .fold(f64::MIN, |acc, s| acc.max(s.total));
        assert!((winner.score.as_ref().expect("winner scored").total - best_total).abs() < 1e-9);
    }

    /// The search must optimise for the *authored* weights. Scoring with the
    /// compile-time defaults instead would still return a plausible layout and
    /// would silently ignore every score slider in the studio.
    #[test]
    fn the_search_scores_with_the_profiles_own_weights() {
        let config = small_config();
        let mut profile = HexCompositionProfile::baseline();
        profile.search.candidates = 3;
        profile.score.connectivity = profile.score.connectivity * 3.0 + 1.0;
        profile.score.variety = 0.0;

        let (world, ladder) =
            HexWfcWorld::generate_searched_with_profile(99, config, None, &profile)
                .expect("must solve");
        let winner = ladder
            .iter()
            .find(|c| c.winner)
            .and_then(|c| c.score.as_ref())
            .expect("a scored winner");

        assert_eq!(
            *winner,
            score_layout_with(&world, profile.score),
            "the ladder must report the authored score"
        );
        assert_ne!(
            winner.total,
            score_layout(&world).total,
            "these weights differ from the defaults, so the totals must differ"
        );
    }
}
