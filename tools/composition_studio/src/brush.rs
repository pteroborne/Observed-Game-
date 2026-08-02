//! Painting authored intent onto the lattice.
//!
//! This is the line where the studio stops being a tuner. A bias says "lean
//! toward shafts"; a pin says "a shaft goes **here**", and the solver is
//! required to honour it rather than merely made likely to.
//!
//! Pins live in the profile, so painting one is an ordinary profile edit: it
//! marks the profile unsaved, moves the simulation hash, and re-solves.

use std::collections::BTreeSet;

use observed_facility::hex_wfc::profile::{HexPin, PinIntent, PinSet};
use observed_facility::hex_wfc::{HexArchetype, HexCompositionProfile, HexSpace, HexWfcConfig};
use observed_hex::HexCoord;

/// The pin set the studio paints into. One named set keeps the artifact
/// readable; multiple sets are an authoring convention, not a tool feature.
pub const STUDIO_PIN_SET: &str = "studio";

/// What the brush writes.
///
/// Deliberately a short, meaningful list rather than every representable
/// `PinIntent`: `Ports` is expressible in the profile but is not something a
/// person can usefully paint with a mouse, so it is left to hand-authored RON.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Brush {
    #[default]
    Junction,
    Straight,
    Corner,
    Shaft,
    RampUp,
    Expanse,
    /// "Never a vertical here" — the checked form of a prohibition, which a
    /// weight can never express.
    ForbidVertical,
    /// "Keep this connective" — hall space, whatever shape.
    Hall,
}

impl Brush {
    pub const ALL: [Self; 8] = [
        Self::Junction,
        Self::Straight,
        Self::Corner,
        Self::Shaft,
        Self::RampUp,
        Self::Expanse,
        Self::ForbidVertical,
        Self::Hall,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Junction => "junction",
            Self::Straight => "straight",
            Self::Corner => "corner",
            Self::Shaft => "shaft",
            Self::RampUp => "ramp up",
            Self::Expanse => "expanse",
            Self::ForbidVertical => "forbid vertical",
            Self::Hall => "hall (any shape)",
        }
    }

    #[must_use]
    pub fn intent(self) -> PinIntent {
        match self {
            Self::Junction => PinIntent::Archetype(HexArchetype::Junction),
            Self::Straight => PinIntent::Archetype(HexArchetype::Straight),
            Self::Corner => PinIntent::Archetype(HexArchetype::Corner),
            Self::Shaft => PinIntent::Archetype(HexArchetype::Shaft),
            Self::RampUp => PinIntent::Archetype(HexArchetype::RampUp),
            Self::Expanse => PinIntent::Archetype(HexArchetype::Expanse),
            Self::ForbidVertical => PinIntent::Forbid(vec![
                HexArchetype::Shaft,
                HexArchetype::RampUp,
                HexArchetype::RampHead,
            ]),
            Self::Hall => PinIntent::Space(HexSpace::Hall),
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|brush| *brush == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    #[must_use]
    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|brush| *brush == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// The studio's pin set in `profile`, created on first use.
fn studio_set(profile: &mut HexCompositionProfile, config: HexWfcConfig) -> &mut PinSet {
    if !profile.pin_sets.iter().any(|set| set.id == STUDIO_PIN_SET) {
        profile.pin_sets.push(PinSet {
            id: String::from(STUDIO_PIN_SET),
            note: String::from("painted in composition_studio"),
            cols: config.cols,
            rows: config.rows,
            levels: config.levels,
            pins: Vec::new(),
        });
    }
    profile
        .pin_sets
        .iter_mut()
        .find(|set| set.id == STUDIO_PIN_SET)
        .expect("just ensured")
}

/// Pin `coord` to `brush`. Returns whether anything changed, so a drag across
/// an already-painted cell does not queue a pointless re-solve.
pub fn paint(
    profile: &mut HexCompositionProfile,
    config: HexWfcConfig,
    coord: HexCoord,
    brush: Brush,
) -> bool {
    let intent = brush.intent();
    let set = studio_set(profile, config);
    if let Some(existing) = set
        .pins
        .iter_mut()
        .find(|pin| pin.q == coord.q && pin.r == coord.r && pin.level == coord.level)
    {
        if existing.intent == intent {
            return false;
        }
        existing.intent = intent;
        return true;
    }
    set.pins.push(HexPin {
        q: coord.q,
        r: coord.r,
        level: coord.level,
        intent,
    });
    true
}

/// Remove any pin on `coord`. Returns whether anything was removed.
pub fn unpin(profile: &mut HexCompositionProfile, coord: HexCoord) -> bool {
    let mut removed = false;
    for set in &mut profile.pin_sets {
        let before = set.pins.len();
        set.pins
            .retain(|pin| !(pin.q == coord.q && pin.r == coord.r && pin.level == coord.level));
        removed |= set.pins.len() != before;
    }
    // Drop the studio set once it is empty, so an author who clears their pins
    // gets a profile byte-identical to one that never had any.
    profile
        .pin_sets
        .retain(|set| set.id != STUDIO_PIN_SET || !set.pins.is_empty());
    removed
}

/// Remove every pin the studio painted.
pub fn clear(profile: &mut HexCompositionProfile) -> usize {
    let removed = profile
        .pin_sets
        .iter()
        .filter(|set| set.id == STUDIO_PIN_SET)
        .map(|set| set.pins.len())
        .sum();
    profile.pin_sets.retain(|set| set.id != STUDIO_PIN_SET);
    removed
}

/// Every pinned coordinate in the profile, for the drawer's legend.
#[must_use]
pub fn pinned_coords(profile: &HexCompositionProfile) -> BTreeSet<HexCoord> {
    profile
        .pin_sets
        .iter()
        .flat_map(|set| set.pins.iter())
        .map(|pin| HexCoord {
            q: pin.q,
            r: pin.r,
            level: pin.level,
        })
        .collect()
}

/// The intent pinned at `coord`, if any.
#[must_use]
pub fn intent_at(profile: &HexCompositionProfile, coord: HexCoord) -> Option<&PinIntent> {
    profile
        .pin_sets
        .iter()
        .flat_map(|set| set.pins.iter())
        .find(|pin| pin.q == coord.q && pin.r == coord.r && pin.level == coord.level)
        .map(|pin| &pin.intent)
}

/// One-line description of an intent, for panels.
#[must_use]
pub fn describe(intent: &PinIntent) -> String {
    match intent {
        PinIntent::Space(space) => format!("space {space:?}"),
        PinIntent::Archetype(archetype) => format!("{archetype:?}"),
        PinIntent::Forbid(list) => format!(
            "not {}",
            list.iter()
                .map(|archetype| format!("{archetype:?}"))
                .collect::<Vec<_>>()
                .join("/")
        ),
        PinIntent::Ports { doors, up, down } => {
            format!("ports {doors:#08b} up {up:?} down {down:?}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HexWfcConfig {
        HexWfcConfig::default()
    }

    fn coord(q: u16, r: u16) -> HexCoord {
        HexCoord { q, r, level: 0 }
    }

    #[test]
    fn painting_creates_the_studio_set_and_pins_the_cell() {
        let mut profile = HexCompositionProfile::baseline();
        assert!(paint(&mut profile, config(), coord(3, 3), Brush::Shaft));
        assert_eq!(
            intent_at(&profile, coord(3, 3)),
            Some(&PinIntent::Archetype(HexArchetype::Shaft))
        );
        assert_eq!(profile.validate(), Ok(()));
    }

    /// Dragging across a cell already holding this brush must not report a
    /// change, or every frame of a drag would queue another solve.
    #[test]
    fn repainting_the_same_intent_reports_no_change() {
        let mut profile = HexCompositionProfile::baseline();
        assert!(paint(&mut profile, config(), coord(3, 3), Brush::Shaft));
        assert!(!paint(&mut profile, config(), coord(3, 3), Brush::Shaft));
        assert!(paint(&mut profile, config(), coord(3, 3), Brush::Junction));
    }

    /// Clearing every pin must leave a profile identical to one that never had
    /// any — otherwise an empty `pin_sets` entry would move the content hash
    /// and lock out LAN peers for nothing.
    #[test]
    fn unpinning_the_last_pin_restores_the_baseline_profile() {
        let mut profile = HexCompositionProfile::baseline();
        paint(&mut profile, config(), coord(3, 3), Brush::Shaft);
        assert!(!profile.is_baseline());
        assert!(unpin(&mut profile, coord(3, 3)));
        assert!(
            profile.is_baseline(),
            "an emptied pin set must not linger in the artifact"
        );
    }

    #[test]
    fn clear_removes_every_studio_pin() {
        let mut profile = HexCompositionProfile::baseline();
        for q in 2..6 {
            paint(&mut profile, config(), coord(q, 3), Brush::Junction);
        }
        assert_eq!(pinned_coords(&profile).len(), 4);
        assert_eq!(clear(&mut profile), 4);
        assert!(profile.is_baseline());
    }

    #[test]
    fn every_brush_produces_a_valid_profile() {
        for brush in Brush::ALL {
            let mut profile = HexCompositionProfile::baseline();
            paint(&mut profile, config(), coord(4, 4), brush);
            assert_eq!(
                profile.validate(),
                Ok(()),
                "{} produced an invalid profile",
                brush.label()
            );
        }
    }

    #[test]
    fn the_brush_cycle_visits_every_entry_and_wraps() {
        let mut seen = Vec::new();
        let mut brush = Brush::default();
        for _ in 0..Brush::ALL.len() {
            seen.push(brush);
            brush = brush.next();
        }
        assert_eq!(brush, Brush::default(), "the cycle must wrap");
        assert_eq!(seen.len(), Brush::ALL.len());
        for entry in Brush::ALL {
            assert!(seen.contains(&entry), "{} was skipped", entry.label());
        }
        assert_eq!(Brush::default().next().previous(), Brush::default());
    }
}
