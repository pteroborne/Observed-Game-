//! Geometry-only contextual composition weighting.
//!
//! The WFC weighted lottery picks a variant for each collapsing cell in
//! proportion to that variant's static `weight`. This module scales that weight
//! by a deterministic factor derived purely from the cell's **position in the
//! grid** — never from the architecture register or any presentation signal, so
//! the atmosphere/structure separation (`DistrictPalette` stays atmosphere-only,
//! per agents.md) is preserved and the register deliberately stays out of
//! structural identity (see `variants.rs`).
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

use super::{HexArchetype, HexWfcConfig};

/// Whether the geometry composition tendencies below are actually applied to
/// the solve.
///
/// **Currently `false`, deliberately.** The tendencies are a nice-to-have
/// aesthetic bias, but applying them shifts every layout, and one of the
/// resulting layouts stalls all four bots at a known-fragile spot — breaking
/// `bot_soak_has_no_stalls` (the Arc-K zero-stall invariant, Phase 94 success
/// criterion 2). Softening the constants only moved the failure around; the
/// same cell kept stalling, which says the root cause is bot-navigation
/// fragility rather than the weighting itself. Navigability is a shipping
/// invariant and composition tendency is not, so the tendency yields until
/// bot navigation is hardened.
///
/// The tendency logic and its tests stay live so re-enabling is this one line
/// plus a re-run of `cargo test -p observed_match hex_wfc::model::tests`.
const COMPOSITION_TENDENCIES_ENABLED: bool = false;

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
    let bias = influence.map_or(1.0, |field| field.multiplier(archetype));
    let scaled = (f64::from(weight) * geometry * bias).round();
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
                let edge = effective_weight(coord(0, 0, level), archetype, 4, cfg, None);
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
            effective_weight(coord(5, 5, 2), HexArchetype::Straight, 0, config(), None),
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

    /// Pins the deliberate current state: the geometry tendency is authored and
    /// tested but NOT applied to the solve, because doing so regressed
    /// `bot_soak_has_no_stalls`. If this test starts failing, the tendency was
    /// re-enabled — re-run that soak before accepting it.
    #[test]
    fn composition_tendencies_are_not_applied_to_the_solve_yet() {
        let cfg = config();
        // A shaft at the axis has a non-neutral *tendency* but a neutral
        // *effective weight*, so layouts match the pre-tendency solver exactly.
        let center = coord(cfg.cols / 2, cfg.rows / 2, 1);
        assert!(context_multiplier(center, HexArchetype::Shaft, cfg) > 1.0);
        assert_eq!(
            effective_weight(center, HexArchetype::Shaft, 10, cfg, None),
            10
        );
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
        let shaft_base = effective_weight(c, HexArchetype::Shaft, 10, cfg, None);
        let shaft_driven = effective_weight(c, HexArchetype::Shaft, 10, cfg, Some(&vertical));
        assert!(
            shaft_driven > shaft_base,
            "verticality should raise shaft weight ({shaft_driven} vs {shaft_base})"
        );
        let hall_base = effective_weight(c, HexArchetype::Straight, 10, cfg, None);
        let hall_driven = effective_weight(c, HexArchetype::Straight, 10, cfg, Some(&vertical));
        assert_eq!(hall_base, hall_driven, "verticality must not touch halls");
    }

    #[test]
    fn influence_lowers_but_never_zeroes_a_starved_category() {
        let cfg = config();
        let c = coord(5, 5, 3);
        let starve = HexInfluenceField::discourage_recovery_rooms();
        let base = effective_weight(c, HexArchetype::Room, 10, cfg, None);
        let driven = effective_weight(c, HexArchetype::Room, 10, cfg, Some(&starve));
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
        let shaft = effective_weight(c, HexArchetype::Shaft, 10, cfg, Some(&extreme));
        let room = effective_weight(c, HexArchetype::Room, 10, cfg, Some(&extreme));
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
