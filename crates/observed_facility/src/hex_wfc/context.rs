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

/// Weight multiplier at the central axis for vertical archetypes.
const VERTICAL_CENTER_BOOST: f64 = 1.6;
/// Weight multiplier at the grid edge for vertical archetypes.
const VERTICAL_EDGE_FALLOFF: f64 = 0.7;
/// Weight multiplier for rooms at the lowest level.
const ROOM_LOW_LEVEL: f64 = 0.8;
/// Weight multiplier for rooms at the highest level.
const ROOM_HIGH_LEVEL: f64 = 1.5;

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

/// The effective weight for the weighted lottery: the static `weight` scaled by
/// [`context_multiplier`]. A zero static weight stays zero (never selectable, by
/// design); any positive weight keeps a floor of `1`, so a legal variant can
/// always still be chosen and solvability is preserved.
#[must_use]
pub(super) fn effective_weight(
    coord: HexCoord,
    archetype: HexArchetype,
    weight: u32,
    config: HexWfcConfig,
) -> u64 {
    if weight == 0 {
        return 0;
    }
    let scaled = (f64::from(weight) * context_multiplier(coord, archetype, config)).round();
    // `scaled` is finite and >= 0 here (multiplier is bounded positive), so the
    // cast is well-defined; floor at 1 to preserve selectability.
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
                let edge = effective_weight(coord(0, 0, level), archetype, 4, cfg);
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
            effective_weight(coord(5, 5, 2), HexArchetype::Straight, 0, config()),
            0
        );
    }

    #[test]
    fn verticals_are_favored_near_the_central_axis() {
        let cfg = config();
        let center = coord(cfg.cols / 2, cfg.rows / 2, 1);
        let edge = coord(0, 0, 1);
        let center_w = effective_weight(center, HexArchetype::Shaft, 10, cfg);
        let edge_w = effective_weight(edge, HexArchetype::Shaft, 10, cfg);
        assert!(
            center_w > edge_w,
            "shaft weight should be higher at the axis ({center_w}) than the edge ({edge_w})"
        );
    }

    #[test]
    fn rooms_are_favored_on_upper_levels() {
        let cfg = config();
        let low = coord(5, 5, 0);
        let high = coord(5, 5, cfg.levels - 1);
        let low_w = effective_weight(low, HexArchetype::Room, 10, cfg);
        let high_w = effective_weight(high, HexArchetype::Room, 10, cfg);
        assert!(
            high_w > low_w,
            "room weight should be higher up high ({high_w}) than down low ({low_w})"
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
