//! Geometry-only contextual composition weighting.
//!
//! The WFC weighted lottery picks a variant for each collapsing cell in
//! proportion to that variant's static `weight`. This module scales that weight
//! by two deterministic factors: the cell's **position in the grid**, and the
//! **district** it stands in.
//!
//! The second of those is new in Arc O Phase 107 and reverses an earlier rule.
//! The register used to be kept out of structural identity so that atmosphere
//! and structure stayed separable. That separation turned out to be the reason
//! districts were only lighting: a neighbourhood you can name has to be built
//! differently, not merely lit differently, and no amount of palette work
//! delivers that. `DistrictPalette` remains atmosphere-only — style still never
//! decides structure — but the *register*, which is a semantic label the solver
//! owns, now biases what gets built there.
//!
//! This is only possible because Phase 106 made districts spatial. Under the old
//! per-hex lottery a register was already knowable before the solve, but
//! weighting by it would have produced noise at cell granularity; weighting by a
//! contiguous district produces a neighbourhood.
//!
//! The bias is a set of *tendencies*, not hard rules: verticals cluster toward
//! the central axis, atria (rooms) favour the upper levels. Crucially a legal
//! variant is never driven to zero weight — [`effective_weight`] keeps a floor
//! of 1 for any positive static weight, so every solve the un-weighted selector
//! could complete still completes (solvability is preserved). Determinism is
//! preserved too: the factor is pure arithmetic of integer grid coordinates, so
//! the same seed still yields the same layout, and the RNG draw sequence is
//! unchanged (only the per-variant weight each draw is compared against moves).

use observed_hex::HexCoord;

use observed_content::ArchitectureRegister;

use super::{HexArchetype, HexWfcConfig};

/// Whether the geometry composition tendencies below are applied to the solve.
///
/// **Re-enabled in Arc O Phase 107**, after the failure that disabled them was
/// tracked to its actual cause. It was switched off because enabling it made one
/// layout stall all four bots, and softening the constants only moved the
/// failure around — correctly read at the time as "the root cause is
/// navigation, not the weighting".
///
/// The specific cause: the tendency shifted exactly one layout in the soak
/// corpus into routing through a **two-level room's internal vertical link**,
/// and nothing can climb one. Measured across the 28 routable soak layouts, the
/// old weighting produced 0 such routes and the tendency produced 1 — the very
/// seed that stalled. `topology::is_connection_open` no longer treats a
/// room-to-room vertical port as a connection, so no route promises that climb,
/// and the soak passes with tendencies on.
const COMPOSITION_TENDENCIES_ENABLED: bool = true;

/// Weight multiplier at the central axis for vertical archetypes.
const VERTICAL_CENTER_BOOST: f64 = 1.2;
/// Weight multiplier at the grid edge for vertical archetypes.
const VERTICAL_EDGE_FALLOFF: f64 = 0.9;
/// Weight multiplier for rooms at the lowest level.
const ROOM_LOW_LEVEL: f64 = 0.9;
/// Weight multiplier for rooms at the highest level.
const ROOM_HIGH_LEVEL: f64 = 1.2;

/// Linear interpolation from `a` (at `t = 0`) to `b` (at `t = 1`).
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Fraction of the way from the grid's central axis (`0.0`) to its farthest
/// corner (`1.0`), in the axial plane. Level is ignored — this is the radial
/// (plan-view) distance only.
fn radial_fraction(coord: HexCoord, config: HexWfcConfig) -> f64 {
    let center_q = f64::from(config.cols) / 2.0;
    let center_r = f64::from(config.rows) / 2.0;
    let dq = f64::from(coord.q) - center_q;
    let dr = f64::from(coord.r) - center_r;
    let dist = (dq * dq + dr * dr).sqrt();
    let max = (center_q * center_q + center_r * center_r).sqrt();
    if max <= f64::EPSILON {
        0.0
    } else {
        (dist / max).clamp(0.0, 1.0)
    }
}

/// Fraction of the way from the lowest level (`0.0`) to the highest (`1.0`).
/// A single-level facility is treated as neutral (`0.5`).
fn level_fraction(coord: HexCoord, config: HexWfcConfig) -> f64 {
    if config.levels <= 1 {
        return 0.5;
    }
    f64::from(coord.level) / f64::from(config.levels - 1)
}

/// The geometry-context weight multiplier for placing `archetype` at `coord`.
/// Always strictly positive; tendencies only (see module docs).
#[must_use]
pub(super) fn context_multiplier(
    coord: HexCoord,
    archetype: HexArchetype,
    config: HexWfcConfig,
) -> f64 {
    match archetype {
        // Verticals (ramps, shafts) cluster toward the central axis and thin
        // out toward the edges, so the facility grows a legible vertical core.
        HexArchetype::RampUp | HexArchetype::RampHead | HexArchetype::Shaft => lerp(
            VERTICAL_CENTER_BOOST,
            VERTICAL_EDGE_FALLOFF,
            radial_fraction(coord, config),
        ),
        // Rooms (the atria) favour the upper levels; heavier connective
        // structure fills the lower ones.
        HexArchetype::Room => lerp(
            ROOM_LOW_LEVEL,
            ROOM_HIGH_LEVEL,
            level_fraction(coord, config),
        ),
        // Halls (straight/corner/junction) and void stay neutral.
        HexArchetype::Void
        | HexArchetype::Straight
        | HexArchetype::Corner
        | HexArchetype::Junction => 1.0,
    }
}

/// The narrowest a bounded influence bias may drive a category's weight.
const INFLUENCE_MIN: f64 = 0.25;
/// The widest a bounded influence bias may drive a category's weight.
const INFLUENCE_MAX: f64 = 4.0;

/// The number of biasable archetypes (every [`HexArchetype`] except `Void`,
/// which is empty space and always neutral).
const INFLUENCE_SLOTS: usize = 7;

/// A bounded, per-archetype weight bias an eliminated team applies to *drive*
/// the facility's refactoring (Phase 4 feasibility): it perturbs which
/// archetypes the WFC tends to place during a driven relayout, rather than
/// placing exact tiles. Every multiplier is clamped to
/// `[INFLUENCE_MIN, INFLUENCE_MAX]`, so an archetype is never zeroed — the WFC
/// still guarantees routes, anchors, observation locks, and legal geometry; the
/// team biases *tendencies* only.
///
/// Bias is per-archetype (not lumped by category) so a driven pocket can shift
/// even among connective halls — e.g. fewer junctions, more caps for a
/// dead-ended layout — which is what makes the effect observable in the small,
/// heavily-constrained relayout pockets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HexInfluenceField {
    /// Per-archetype multipliers, indexed by [`slot`]; `Void` has no slot.
    biases: [f64; INFLUENCE_SLOTS],
}

/// The bias slot for an archetype, or `None` for `Void` (always neutral).
fn slot(archetype: HexArchetype) -> Option<usize> {
    Some(match archetype {
        HexArchetype::Room => 0,
        HexArchetype::Straight => 1,
        HexArchetype::Corner => 2,
        HexArchetype::Junction => 3,
        HexArchetype::RampUp => 4,
        HexArchetype::RampHead => 5,
        HexArchetype::Shaft => 6,
        HexArchetype::Void => return None,
    })
}

impl Default for HexInfluenceField {
    fn default() -> Self {
        Self::neutral()
    }
}

impl HexInfluenceField {
    /// No bias — every archetype at its natural weight.
    #[must_use]
    pub fn neutral() -> Self {
        Self {
            biases: [1.0; INFLUENCE_SLOTS],
        }
    }

    /// Set one archetype's bias, clamped to the safe range. Chainable.
    #[must_use]
    pub fn with_bias(mut self, archetype: HexArchetype, factor: f64) -> Self {
        if let Some(index) = slot(archetype) {
            self.biases[index] = factor.clamp(INFLUENCE_MIN, INFLUENCE_MAX);
        }
        self
    }

    /// Push the facility taller: favour ramps and shafts.
    #[must_use]
    pub fn encourage_verticality() -> Self {
        Self::neutral()
            .with_bias(HexArchetype::RampUp, 2.5)
            .with_bias(HexArchetype::RampHead, 2.5)
            .with_bias(HexArchetype::Shaft, 2.5)
    }

    /// Make the facility less forgiving: starve recovery rooms.
    #[must_use]
    pub fn discourage_recovery_rooms() -> Self {
        Self::neutral().with_bias(HexArchetype::Room, 0.4)
    }

    /// Dead-ends: starve junctions and favour caps/corners for a more
    /// dead-ended, harder-to-cross layout.
    #[must_use]
    pub fn encourage_dead_ends() -> Self {
        Self::neutral()
            .with_bias(HexArchetype::Junction, 0.4)
            .with_bias(HexArchetype::Corner, 1.8)
    }

    /// Decay: thin junctions and rooms for a sparser layout.
    #[must_use]
    pub fn encourage_decay() -> Self {
        Self::neutral()
            .with_bias(HexArchetype::Junction, 0.5)
            .with_bias(HexArchetype::Room, 0.6)
    }

    /// The bias multiplier for `archetype` (already clamped on assignment).
    fn multiplier(&self, archetype: HexArchetype) -> f64 {
        slot(archetype).map_or(1.0, |index| self.biases[index])
    }
}

/// How strongly a district may bend the static weights. Shared with
/// [`HexInfluenceField`] so no single input can dominate the lottery or drive a
/// legal variant out of contention.
const PROFILE_MIN: f64 = 0.25;
const PROFILE_MAX: f64 = 4.0;

/// What a district builds, as a multiplier on each archetype's static weight.
///
/// These are the identities Arc O set out to deliver, expressed as the only
/// thing the solver actually understands — relative weight. They are tendencies,
/// not rules: [`effective_weight`] floors every positive weight at 1, so a
/// district that suppresses shafts still gets one where the port signatures
/// demand it, and solvability is untouched.
///
/// **No district boosts shafts above baseline, and that is deliberate.** The
/// facility is already roughly 47 % shaft before any of this applies (backlog
/// #13), so a profile that added more would work against the arc's own goal and,
/// worse, put more of the fragile generic switchback on the routes bots follow.
/// Verticality is therefore expressed by suppressing shafts *less* than the
/// neighbours do — Wellshaft holds them at baseline while an open district cuts
/// them to a third — and by ramps, which are genuinely traversable, where a
/// district wants a built ascent. The relative reading is identical and the
/// absolute shaft count falls everywhere.
#[must_use]
fn district_multiplier(register: ArchitectureRegister, archetype: HexArchetype) -> f64 {
    use ArchitectureRegister as R;
    use HexArchetype as A;
    let value = match register {
        // Vast and open: junctions and flat runs, verticals pushed down hard.
        R::LiminalGrid => match archetype {
            A::Straight => 1.5,
            A::Corner => 0.9,
            A::Junction => 2.4,
            A::RampUp | A::RampHead => 0.6,
            A::Shaft => 0.3,
            A::Room => 1.2,
            A::Void => 1.0,
        },
        // Winding: turns and runs, junctions suppressed so a path commits.
        R::OverlitGrid => match archetype {
            A::Straight => 1.8,
            A::Corner => 2.4,
            A::Junction => 0.45,
            A::RampUp | A::RampHead => 0.8,
            A::Shaft => 0.4,
            A::Room => 1.0,
            A::Void => 1.0,
        },
        // The vertical districts. Wellshaft is shafts; Megastructure climbs on
        // ramps, so it reads as a built ascent rather than a stack of towers.
        R::Wellshaft => match archetype {
            A::Straight => 0.7,
            A::Corner => 0.8,
            A::Junction => 0.8,
            A::RampUp | A::RampHead => 1.6,
            A::Shaft => 1.0,
            A::Room => 0.9,
            A::Void => 1.0,
        },
        R::Megastructure => match archetype {
            A::Straight => 0.8,
            A::Corner => 0.8,
            A::Junction => 1.1,
            A::RampUp | A::RampHead => 2.6,
            A::Shaft => 0.8,
            A::Room => 1.0,
            A::Void => 1.0,
        },
        // The remaining registers take mild characters, so the strong four read
        // as deliberate rather than as the only districts with any identity.
        R::ShadowScreen => match archetype {
            A::Corner => 1.4,
            A::Junction => 0.8,
            A::Shaft => 0.5,
            _ => 1.0,
        },
        R::Monolith => match archetype {
            A::Straight => 1.4,
            A::Junction => 0.8,
            A::Room => 1.2,
            A::Shaft => 0.5,
            _ => 1.0,
        },
        R::Institutional => match archetype {
            A::Straight => 1.2,
            A::Junction => 1.3,
            A::Shaft => 0.45,
            _ => 1.0,
        },
        R::FacetMonument => match archetype {
            A::Corner => 1.5,
            A::Room => 1.2,
            A::Shaft => 0.5,
            _ => 1.0,
        },
        R::InfiniteGallery => match archetype {
            A::Straight => 2.0,
            A::Corner => 0.7,
            A::Junction => 0.7,
            A::Shaft => 0.4,
            _ => 1.0,
        },
        R::Thinning => match archetype {
            A::Junction => 0.6,
            A::Room => 0.7,
            A::Shaft => 0.6,
            _ => 1.0,
        },
    };
    f64::clamp(value, PROFILE_MIN, PROFILE_MAX)
}

/// The effective weight for the weighted lottery: the static `weight` scaled by
/// the geometry [`context_multiplier`] and, when a driven relayout supplies one,
/// a bounded [`HexInfluenceField`] bias. A zero static weight stays zero (never
/// selectable, by design); any positive weight keeps a floor of `1`, so a legal
/// variant can always still be chosen and solvability is preserved.
#[must_use]
pub(super) fn effective_weight(
    coord: HexCoord,
    archetype: HexArchetype,
    weight: u32,
    config: HexWfcConfig,
    district: Option<ArchitectureRegister>,
    influence: Option<&HexInfluenceField>,
) -> u64 {
    if weight == 0 {
        return 0;
    }
    let geometry = if COMPOSITION_TENDENCIES_ENABLED {
        context_multiplier(coord, archetype, config)
    } else {
        1.0
    };
    let profile = district.map_or(1.0, |register| district_multiplier(register, archetype));
    let bias = influence.map_or(1.0, |field| field.multiplier(archetype));
    let scaled = (f64::from(weight) * geometry * profile * bias).round();
    // `scaled` is finite and >= 0 here (both factors are bounded positive), so
    // the cast is well-defined; floor at 1 to preserve selectability.
    (scaled as u64).max(1)
}

/// The effective weight for a *driven relayout* pocket: the static `weight`
/// scaled by the influence bias **only** — no geometry context. Relayout pockets
/// deliberately do not apply the initial-solve geometry tendencies (that would
/// perturb every ordinary relayout too), so a `neutral` field leaves the pocket
/// byte-identical to the undriven path and the influence is the sole driven
/// difference. Zero stays zero; any positive weight keeps a floor of `1`.
#[must_use]
pub(super) fn influenced_weight(
    archetype: HexArchetype,
    weight: u32,
    influence: &HexInfluenceField,
) -> u64 {
    if weight == 0 {
        return 0;
    }
    let scaled = (f64::from(weight) * influence.multiplier(archetype)).round();
    (scaled as u64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HexWfcConfig {
        HexWfcConfig::arc_default()
    }

    fn coord(q: u16, r: u16, level: u8) -> HexCoord {
        HexCoord { q, r, level }
    }

    #[test]
    fn multiplier_is_deterministic() {
        let c = coord(5, 5, 2);
        let a = context_multiplier(c, HexArchetype::Shaft, config());
        let b = context_multiplier(c, HexArchetype::Shaft, config());
        assert_eq!(a, b);
    }

    #[test]
    fn positive_weight_never_zeroed() {
        let cfg = config();
        for level in 0..cfg.levels {
            for &archetype in &[
                HexArchetype::Room,
                HexArchetype::Shaft,
                HexArchetype::Straight,
                HexArchetype::RampUp,
            ] {
                let edge = effective_weight(coord(0, 0, level), archetype, 4, cfg, None, None);
                assert!(
                    edge >= 1,
                    "{archetype:?} @ level {level} zeroed a legal variant"
                );
            }
        }
    }

    #[test]
    fn zero_weight_stays_zero() {
        assert_eq!(
            effective_weight(
                coord(5, 5, 2),
                HexArchetype::Straight,
                0,
                config(),
                None,
                None
            ),
            0
        );
    }

    // The next two pin the *designed* tendency via `context_multiplier`, which
    // stays live even while `COMPOSITION_TENDENCIES_ENABLED` keeps it out of the
    // solve — so the design is still covered and re-enabling is a one-line change.
    #[test]
    fn verticals_are_favored_near_the_central_axis() {
        let cfg = config();
        let center = context_multiplier(
            coord(cfg.cols / 2, cfg.rows / 2, 1),
            HexArchetype::Shaft,
            cfg,
        );
        let edge = context_multiplier(coord(0, 0, 1), HexArchetype::Shaft, cfg);
        assert!(
            center > edge,
            "shaft tendency should be higher at the axis ({center}) than the edge ({edge})"
        );
    }

    #[test]
    fn rooms_are_favored_on_upper_levels() {
        let cfg = config();
        let low = context_multiplier(coord(5, 5, 0), HexArchetype::Room, cfg);
        let high = context_multiplier(coord(5, 5, cfg.levels - 1), HexArchetype::Room, cfg);
        assert!(
            high > low,
            "room tendency should be higher up high ({high}) than down low ({low})"
        );
    }

    /// Arc O Phase 107 turned the tendencies on and added district profiles, so
    /// the effective weight is no longer the static one. If this starts failing,
    /// something has silently neutralised the composition weighting — re-run
    /// `cargo test -p observed_match hex_wfc::model::tests::bot_soak_has_no_stalls`
    /// before accepting whatever caused it.
    #[test]
    fn composition_weighting_reaches_the_solve() {
        let cfg = config();
        let center = coord(cfg.cols / 2, cfg.rows / 2, 1);
        assert!(context_multiplier(center, HexArchetype::Shaft, cfg) > 1.0);
        assert!(
            effective_weight(center, HexArchetype::Shaft, 10, cfg, None, None) > 10,
            "the geometry tendency must reach the weight"
        );
        // And a district bends it further, in the direction its identity says.
        let open = effective_weight(
            center,
            HexArchetype::Shaft,
            10,
            cfg,
            Some(ArchitectureRegister::LiminalGrid),
            None,
        );
        let vertical = effective_weight(
            center,
            HexArchetype::Shaft,
            10,
            cfg,
            Some(ArchitectureRegister::Wellshaft),
            None,
        );
        assert!(
            open < vertical,
            "an open district must want fewer shafts than a vertical one ({open} vs {vertical})"
        );
    }

    /// No district may drive a legal variant out of contention: solvability is
    /// not something a composition profile is allowed to trade away.
    #[test]
    fn no_district_can_starve_a_legal_variant() {
        let cfg = config();
        let center = coord(cfg.cols / 2, cfg.rows / 2, 1);
        for register in ArchitectureRegister::ALL {
            for archetype in [
                HexArchetype::Straight,
                HexArchetype::Corner,
                HexArchetype::Junction,
                HexArchetype::Room,
                HexArchetype::RampUp,
                HexArchetype::Shaft,
            ] {
                assert!(
                    effective_weight(center, archetype, 1, cfg, Some(register), None) >= 1,
                    "{register:?}/{archetype:?} starved a legal variant"
                );
                assert!(
                    effective_weight(center, archetype, 0, cfg, Some(register), None) == 0,
                    "{register:?}/{archetype:?} revived an illegal variant"
                );
            }
        }
    }

    /// The profiles are the arc's stated district identities, so they are pinned
    /// as claims rather than left as constants nobody checks.
    #[test]
    fn the_district_profiles_say_what_the_arc_says_they_say() {
        use ArchitectureRegister as R;
        use HexArchetype as A;
        let m = district_multiplier;
        // Liminal Grid: vast and open.
        assert!(m(R::LiminalGrid, A::Junction) > m(R::OverlitGrid, A::Junction));
        assert!(m(R::LiminalGrid, A::Shaft) < 0.5);
        // Overlit Grid: winding, so turns over branches.
        assert!(m(R::OverlitGrid, A::Corner) > m(R::OverlitGrid, A::Junction) * 3.0);
        // The vertical pair, expressed without adding shafts anywhere.
        assert!(m(R::Wellshaft, A::Shaft) > m(R::LiminalGrid, A::Shaft));
        assert!(m(R::Megastructure, A::RampUp) > m(R::LiminalGrid, A::RampUp));
        for register in R::ALL {
            assert!(
                m(register, A::Shaft) <= 1.0,
                "{register:?} boosts shafts above baseline; the facility is already                  half shafts and the generic switchback is the fragile tile"
            );
        }
    }

    #[test]
    fn context_weighted_solve_is_deterministic_and_solves() {
        use crate::hex_wfc::{HexWfcConfig, HexWfcWorld};
        let cfg = HexWfcConfig::arc_default();
        let seed = 0xC047_0000_0000_0000;
        let a = HexWfcWorld::generate(seed, cfg).expect("context-weighted solve completes");
        let b = HexWfcWorld::generate(seed, cfg).expect("context-weighted solve completes");
        assert_eq!(
            a.placements, b.placements,
            "same seed must reproduce exactly"
        );
    }

    #[test]
    fn influence_biases_the_targeted_category_only() {
        let cfg = config();
        let c = coord(5, 5, 1);
        let vertical = HexInfluenceField::encourage_verticality();
        // Shaft weight rises under encourage_verticality; a hall is untouched.
        let shaft_base = effective_weight(c, HexArchetype::Shaft, 10, cfg, None, None);
        let shaft_driven = effective_weight(c, HexArchetype::Shaft, 10, cfg, None, Some(&vertical));
        assert!(
            shaft_driven > shaft_base,
            "verticality should raise shaft weight ({shaft_driven} vs {shaft_base})"
        );
        let hall_base = effective_weight(c, HexArchetype::Straight, 10, cfg, None, None);
        let hall_driven =
            effective_weight(c, HexArchetype::Straight, 10, cfg, None, Some(&vertical));
        assert_eq!(hall_base, hall_driven, "verticality must not touch halls");
    }

    #[test]
    fn influence_lowers_but_never_zeroes_a_starved_category() {
        let cfg = config();
        let c = coord(5, 5, 3);
        let starve = HexInfluenceField::discourage_recovery_rooms();
        let base = effective_weight(c, HexArchetype::Room, 10, cfg, None, None);
        let driven = effective_weight(c, HexArchetype::Room, 10, cfg, None, Some(&starve));
        assert!(driven < base, "recovery rooms should be starved");
        assert!(
            driven >= 1,
            "a legal room variant is never zeroed (solvability)"
        );
    }

    #[test]
    fn influence_biases_are_clamped_to_the_safe_range() {
        let cfg = config();
        let c = coord(5, 5, 1);
        // Absurd requested biases clamp to the bounded range, so weight stays sane.
        let extreme = HexInfluenceField::neutral()
            .with_bias(HexArchetype::Shaft, 1_000.0)
            .with_bias(HexArchetype::Room, 0.0);
        let shaft = effective_weight(c, HexArchetype::Shaft, 10, cfg, None, Some(&extreme));
        let room = effective_weight(c, HexArchetype::Room, 10, cfg, None, Some(&extreme));
        // Vertical clamps to <= MAX (4.0) of geometry*static; never explodes.
        let ceiling = (10.0 * INFLUENCE_MAX * VERTICAL_CENTER_BOOST).ceil() as u64;
        assert!(
            shaft <= ceiling,
            "clamped high bias still exploded: {shaft}"
        );
        // Room's 0.0 request clamps up to MIN (0.25), never zero.
        assert!(room >= 1, "clamped low bias must not zero a legal variant");
    }

    #[test]
    fn halls_stay_neutral() {
        let cfg = config();
        assert_eq!(
            context_multiplier(coord(0, 0, 0), HexArchetype::Straight, cfg),
            1.0
        );
        assert_eq!(
            context_multiplier(coord(9, 9, 3), HexArchetype::Junction, cfg),
            1.0
        );
    }
}
