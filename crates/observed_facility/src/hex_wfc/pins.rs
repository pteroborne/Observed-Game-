//! Authored intent: cells a person decided about, which the solver must honour.
//!
//! A composition profile's biases are *tendencies* — they bend the weighted
//! lottery and can never zero a legal variant, which is what keeps every solve
//! solvable. That deliberately leaves one thing unsayable: "a shaft goes
//! **here**". A pin says it, and unlike a weight of zero it is *checked*.
//!
//! # How a pin reaches the solve
//!
//! [`pin_admits`] is one more predicate in the filter that already builds each
//! cell's initial domain in [`super::collapse::initial_domains`]. Pins compose
//! with the boundary rule, the forced-route rule and the blueprint-signature
//! rule by plain intersection, exactly as those three already compose with each
//! other. There is no second solver and no post-pass.
//!
//! # Blueprints win
//!
//! Room footprints and their named ports are a frozen contract
//! (`docs/arc_l/phase_90_91_alignment.md`). Where a stamped blueprint already
//! fixes a cell, the pin is **ignored** rather than intersected — intersecting
//! would empty the domain and report a contradiction, when the truthful answer
//! is "a room landed on your pin". [`diagnose_pins`] reports that as
//! [`PinDiagnostic::BlueprintCollision`].
//!
//! # Pins change the layout, by design
//!
//! A pin filters a cell's initial domain, which changes domain *lengths*, which
//! changes the min-entropy tie-break candidate set, which changes the RNG walk.
//! So unlike every other part of a profile, **a pin set produces a different
//! layout for the same seed**. That is intended and safe: pins are hashed
//! content, so a layout is still reproducible from `(seed, profile)`.

use std::collections::BTreeMap;

use observed_hex::HexCoord;

use super::profile::{HexCompositionProfile, PinIntent, PinSet};
use super::variants::HexVariant;
use super::{HexArchetype, HexSpace, HexWfcConfig};
use crate::RoomRole;

/// Why a pin can never be satisfied, independent of any particular seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinFailure {
    /// The intent needs an opening through a face that is the lattice edge.
    BoundaryOpening,
    /// `Space(Room)` outside a stamped blueprint. Rooms exist only inside
    /// blueprint footprints; everything else is connective fabric.
    RoomOutsideBlueprint,
    /// A `Forbid` list that rules out every variant this cell could take.
    EmptyForbidResidue,
    /// No variant in the catalogue matches the intent at all.
    NoSuchVariant,
}

impl PinFailure {
    #[must_use]
    pub fn explain(self) -> &'static str {
        match self {
            Self::BoundaryOpening => {
                "needs an opening through the lattice edge; move it inward or seal that face"
            }
            Self::RoomOutsideBlueprint => {
                "pins Room space outside a blueprint footprint; rooms are stamped, not painted"
            }
            Self::EmptyForbidResidue => "forbids every archetype this cell could legally take",
            Self::NoSuchVariant => "no variant in the catalogue matches this intent",
        }
    }
}

/// Something wrong with an authored pin set, phrased so a tool can point at it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinDiagnostic {
    /// The pin names a cell outside the active lattice.
    OutOfGrid { set: String, coord: HexCoord },
    /// The set was drawn on a different lattice. Never reinterpreted silently:
    /// the same `(q, r, level)` is a different place on a different grid.
    GridMismatch {
        set: String,
        authored: (u16, u16, u8),
        active: (u16, u16, u8),
    },
    /// Two sets pin the same cell. The later one wins; this says so out loud
    /// rather than letting one edit quietly shadow another.
    Shadowed { coord: HexCoord, winner: String },
    /// The pin admits no boundary-legal variant, whatever the seed.
    Unsatisfiable {
        set: String,
        coord: HexCoord,
        reason: PinFailure,
    },
    /// A stamped room landed on this pin, so the blueprint took the cell and
    /// the pin had no effect there. Where rooms land is seed-dependent, so this
    /// is a report about the probe seed, not a permanent property.
    BlueprintCollision { coord: HexCoord, role: RoomRole },
    /// The corridor router claimed this cell, so its exact door mask took it and
    /// the pin had no effect there. Seed-dependent in the same way and for the
    /// same reason as [`Self::BlueprintCollision`]: where the skeleton runs
    /// depends on where the rooms were stamped.
    CorridorCollision { coord: HexCoord, doors: u8 },
    /// Adding `culprit` is what first emptied `at`'s domain. Attribution: the
    /// pin an author should actually go and change.
    Conflict {
        at: HexCoord,
        culprit: HexCoord,
        prior: Vec<HexCoord>,
    },
}

impl core::fmt::Display for PinDiagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfGrid { set, coord } => write!(
                f,
                "{set:?}: ({},{},{}) is outside the lattice",
                coord.q, coord.r, coord.level
            ),
            Self::GridMismatch {
                set,
                authored,
                active,
            } => write!(
                f,
                "{set:?} was drawn on {}x{}x{} but the facility is {}x{}x{}",
                authored.0, authored.1, authored.2, active.0, active.1, active.2
            ),
            Self::Shadowed { coord, winner } => write!(
                f,
                "({},{},{}) is pinned more than once; {winner:?} wins",
                coord.q, coord.r, coord.level
            ),
            Self::Unsatisfiable { set, coord, reason } => write!(
                f,
                "{set:?}: ({},{},{}) {}",
                coord.q,
                coord.r,
                coord.level,
                reason.explain()
            ),
            Self::CorridorCollision { coord, doors } => write!(
                f,
                "({},{},{}) is a routed corridor with doors {doors:06b}; the route wins \
                 and the pin has no effect there",
                coord.q, coord.r, coord.level
            ),
            Self::BlueprintCollision { coord, role } => write!(
                f,
                "({},{},{}) is inside a stamped {role:?} room, so the blueprint wins and the pin is ignored",
                coord.q, coord.r, coord.level
            ),
            Self::Conflict { at, culprit, prior } => write!(
                f,
                "({},{},{}) has no legal variant once ({},{},{}) is pinned ({} earlier pin(s) involved)",
                at.q,
                at.r,
                at.level,
                culprit.q,
                culprit.r,
                culprit.level,
                prior.len()
            ),
        }
    }
}

/// Whether `variant` satisfies `intent`.
///
/// The one place a pin's meaning is defined. Adding a [`PinIntent`] variant
/// fails this match until somebody decides what it constrains.
#[must_use]
pub(super) fn pin_admits(intent: &PinIntent, variant: &HexVariant) -> bool {
    match intent {
        PinIntent::Space(space) => variant.space == *space,
        PinIntent::Archetype(archetype) => variant.archetype == *archetype,
        PinIntent::Forbid(list) => !list.contains(&variant.archetype),
        PinIntent::Ports { doors, up, down } => {
            variant.doors == *doors && variant.up == (*up).into() && variant.down == (*down).into()
        }
    }
}

/// Does this pin need an opening through `face`?
fn intent_opens(intent: &PinIntent, lateral_bit: u8, vertical: bool) -> bool {
    match intent {
        PinIntent::Ports { doors, up, down } => {
            if vertical {
                *up != super::profile::PinPortClass::Sealed
                    || *down != super::profile::PinPortClass::Sealed
            } else {
                doors & lateral_bit != 0
            }
        }
        // Space/Archetype/Forbid do not demand a specific opening, so they can
        // always be satisfied by some sealed-enough variant near the edge.
        _ => false,
    }
}

/// Flatten every pin set in `profile` onto the active lattice.
///
/// Returns the resolved map even when diagnostics are non-empty: an author with
/// one bad pin should still see the other twenty take effect.
#[must_use]
pub fn resolved_pins(
    config: HexWfcConfig,
    profile: &HexCompositionProfile,
) -> (BTreeMap<HexCoord, PinIntent>, Vec<PinDiagnostic>) {
    let mut resolved = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let active = (config.cols, config.rows, config.levels);

    for set in &profile.pin_sets {
        let authored = (set.cols, set.rows, set.levels);
        if authored != active {
            diagnostics.push(PinDiagnostic::GridMismatch {
                set: set.id.clone(),
                authored,
                active,
            });
            continue;
        }
        for pin in &set.pins {
            let coord = HexCoord {
                q: pin.q,
                r: pin.r,
                level: pin.level,
            };
            if pin.q >= config.cols || pin.r >= config.rows || pin.level >= config.levels {
                diagnostics.push(PinDiagnostic::OutOfGrid {
                    set: set.id.clone(),
                    coord,
                });
                continue;
            }
            if resolved.insert(coord, pin.intent.clone()).is_some() {
                diagnostics.push(PinDiagnostic::Shadowed {
                    coord,
                    winner: set.id.clone(),
                });
            }
        }
    }
    (resolved, diagnostics)
}

/// Seed-independent pre-flight: pins that can never be satisfied.
///
/// Runs before the retry loop and consumes no retry budget, so an impossible
/// pin reports itself immediately instead of surfacing as
/// `RetryBudgetExhausted` a hundred attempts later.
///
/// This checks each pin **in isolation** against the boundary rule and the
/// room-placement rule. Interactions between pins are seed-dependent and cost a
/// propagation pass each; [`diagnose_pins`] does that at authoring time.
#[must_use]
pub(super) fn unsatisfiable(
    config: HexWfcConfig,
    pins: &BTreeMap<HexCoord, PinIntent>,
    variants: &[HexVariant],
) -> Option<(HexCoord, PinFailure)> {
    let grid = config.grid();
    for (coord, intent) in pins {
        // Room space is stamped by a blueprint, never painted onto fabric.
        if matches!(intent, PinIntent::Space(HexSpace::Room))
            || matches!(intent, PinIntent::Archetype(HexArchetype::Room))
        {
            return Some((*coord, PinFailure::RoomOutsideBlueprint));
        }

        // An opening that faces off the lattice can never be built.
        for face in observed_hex::HexFace::ALL {
            if grid.neighbor(*coord, face).is_some() {
                continue;
            }
            let bit = if face.is_lateral() {
                super::lateral_bit(face)
            } else {
                0
            };
            if intent_opens(intent, bit, !face.is_lateral()) {
                return Some((*coord, PinFailure::BoundaryOpening));
            }
        }

        // And the intent has to describe something the catalogue contains.
        let admitted = variants
            .iter()
            .any(|variant| variant.space != HexSpace::Room && pin_admits(intent, variant));
        if !admitted {
            let reason = if matches!(intent, PinIntent::Forbid(_)) {
                PinFailure::EmptyForbidResidue
            } else {
                PinFailure::NoSuchVariant
            };
            return Some((*coord, reason));
        }
    }
    None
}

/// Everything wrong with this profile's pins on this lattice, for the authoring
/// tool. Includes the isolation checks plus per-pin attribution.
///
/// Attribution is `O(pins)` full solves, which is unacceptable on the hot path
/// and entirely fine while somebody is looking at a screen. It answers the
/// question an author actually has — *which pin do I change?* — which a bare
/// "contradiction at (4,7,2)" does not.
#[must_use]
pub fn diagnose_pins(config: HexWfcConfig, profile: &HexCompositionProfile) -> Vec<PinDiagnostic> {
    let (resolved, mut diagnostics) = resolved_pins(config, profile);
    if resolved.is_empty() {
        return diagnostics;
    }

    let variants = super::variants::catalogue();
    if let Some((coord, reason)) = unsatisfiable(config, &resolved, &variants) {
        diagnostics.push(PinDiagnostic::Unsatisfiable {
            set: owning_set(profile, coord).unwrap_or_else(|| String::from("<unknown>")),
            coord,
            reason,
        });
        return diagnostics;
    }

    // A stamped room may land on a pin. The blueprint wins (frozen contract),
    // so the pin is quietly ignored at solve time — which is exactly the sort
    // of silence an author must not be left in. Solving once and asking which
    // pinned cells ended up inside a room turns it into a stated outcome.
    //
    // Room siting is seed-dependent, so this is "at the probe seed" rather than
    // "always"; the wording in `PinDiagnostic` says so.
    if let Ok(world) = super::HexWfcWorld::generate_with_profile(PROBE_SEED, config, None, profile)
    {
        // Recomputed rather than read off the world: the skeleton is a function
        // of the config and the stamped blueprints, and the probe world pins
        // both, so this is the same map the solve used.
        let skeleton = profile
            .route_corridors
            .then(|| {
                super::constraints::corridor_skeleton(
                    config,
                    &world.blueprints,
                    profile.carve_unrouted,
                )
            })
            .flatten()
            .map(|(doors, _, _)| doors)
            .unwrap_or_default();
        for (coord, intent) in &resolved {
            if world.room_id_at(*coord).is_none() {
                // Only where the route and the pin actually disagree. A skeleton
                // that made a cell the very `Straight` its pin asked for has
                // overruled nothing, and a panel that said so on every routed
                // pin would be the always-complaining view this module's other
                // gate exists to prevent.
                if let Some(&doors) = skeleton.get(coord)
                    && !variants.iter().any(|variant| {
                        variant.space != HexSpace::Room
                            && variant.doors == doors
                            && pin_admits(intent, variant)
                    })
                {
                    diagnostics.push(PinDiagnostic::CorridorCollision {
                        coord: *coord,
                        doors,
                    });
                }
                continue;
            }
            let role = world
                .blueprints
                .iter()
                .find(|stamped| stamped.cells.contains(coord))
                .map(|stamped| stamped.role);
            if let Some(role) = role {
                diagnostics.push(PinDiagnostic::BlueprintCollision {
                    coord: *coord,
                    role,
                });
            }
        }
    }

    // Add pins one at a time and stop at the first that makes the facility
    // unsolvable. The pin that broke it is the one to change.
    let mut accumulated: BTreeMap<HexCoord, PinIntent> = BTreeMap::new();
    let mut prior: Vec<HexCoord> = Vec::new();
    for (coord, intent) in &resolved {
        accumulated.insert(*coord, intent.clone());
        let mut probe = profile.clone();
        probe.pin_sets = vec![PinSet {
            id: String::from("<probe>"),
            note: String::new(),
            cols: config.cols,
            rows: config.rows,
            levels: config.levels,
            pins: accumulated
                .iter()
                .map(|(coord, intent)| super::profile::HexPin {
                    q: coord.q,
                    r: coord.r,
                    level: coord.level,
                    intent: intent.clone(),
                })
                .collect(),
        }];
        if super::HexWfcWorld::generate_with_profile(PROBE_SEED, config, None, &probe).is_err() {
            diagnostics.push(PinDiagnostic::Conflict {
                at: *coord,
                culprit: *coord,
                prior: prior.clone(),
            });
            break;
        }
        prior.push(*coord);
    }
    diagnostics
}

/// A fixed seed for attribution probes, so `diagnose_pins` is deterministic.
const PROBE_SEED: u64 = 0x9111_0000_0000_0001;

fn owning_set(profile: &HexCompositionProfile, coord: HexCoord) -> Option<String> {
    profile
        .pin_sets
        .iter()
        .find(|set| {
            set.pins
                .iter()
                .any(|pin| pin.q == coord.q && pin.r == coord.r && pin.level == coord.level)
        })
        .map(|set| set.id.clone())
}

#[cfg(test)]
mod tests {
    use super::super::profile::{HexPin, PinPortClass};
    use super::*;

    fn compact() -> HexWfcConfig {
        HexWfcConfig::default()
    }

    fn profile_with(pins: Vec<HexPin>) -> HexCompositionProfile {
        let config = compact();
        let mut profile = HexCompositionProfile::baseline();
        profile.pin_sets.push(PinSet {
            id: String::from("set"),
            note: String::new(),
            cols: config.cols,
            rows: config.rows,
            levels: config.levels,
            pins,
        });
        profile
    }

    fn pin(q: u16, r: u16, intent: PinIntent) -> HexPin {
        HexPin {
            q,
            r,
            level: 0,
            intent,
        }
    }

    #[test]
    fn an_empty_profile_resolves_to_no_pins() {
        let (resolved, diagnostics) = resolved_pins(compact(), &HexCompositionProfile::baseline());
        assert!(resolved.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_pin_set_drawn_on_another_lattice_is_refused() {
        let mut profile = profile_with(vec![pin(1, 1, PinIntent::Archetype(HexArchetype::Shaft))]);
        profile.pin_sets[0].cols += 1;
        let (resolved, diagnostics) = resolved_pins(compact(), &profile);
        assert!(resolved.is_empty(), "a mismatched set must not be applied");
        assert!(matches!(
            diagnostics.as_slice(),
            [PinDiagnostic::GridMismatch { .. }]
        ));
    }

    #[test]
    fn a_pin_outside_the_lattice_is_refused() {
        let config = compact();
        let profile = profile_with(vec![pin(
            config.cols + 5,
            1,
            PinIntent::Archetype(HexArchetype::Shaft),
        )]);
        let (resolved, diagnostics) = resolved_pins(config, &profile);
        assert!(resolved.is_empty());
        assert!(matches!(
            diagnostics.as_slice(),
            [PinDiagnostic::OutOfGrid { .. }]
        ));
    }

    /// Two pins on one cell resolve to the later one, and say so. A silent
    /// shadow is how an author loses an edit without noticing.
    #[test]
    fn a_doubly_pinned_cell_reports_the_shadow() {
        let profile = profile_with(vec![
            pin(2, 2, PinIntent::Archetype(HexArchetype::Shaft)),
            pin(2, 2, PinIntent::Archetype(HexArchetype::Junction)),
        ]);
        let (resolved, diagnostics) = resolved_pins(compact(), &profile);
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[&HexCoord {
                q: 2,
                r: 2,
                level: 0
            }],
            PinIntent::Archetype(HexArchetype::Junction)
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| matches!(d, PinDiagnostic::Shadowed { .. }))
        );
    }

    #[test]
    fn a_room_pin_is_refused_because_rooms_are_stamped() {
        let variants = super::super::variants::catalogue();
        let profile = profile_with(vec![pin(2, 2, PinIntent::Space(HexSpace::Room))]);
        let (resolved, _) = resolved_pins(compact(), &profile);
        assert_eq!(
            unsatisfiable(compact(), &resolved, &variants).map(|(_, reason)| reason),
            Some(PinFailure::RoomOutsideBlueprint)
        );
    }

    /// A door pinned against the lattice edge can never be built, and must be
    /// reported before the solver burns its whole retry budget discovering it.
    #[test]
    fn an_opening_through_the_lattice_edge_is_unsatisfiable() {
        let variants = super::super::variants::catalogue();
        // (0,0) is a corner; every lateral face there has at least one edge.
        let profile = profile_with(vec![pin(
            0,
            0,
            PinIntent::Ports {
                doors: 0b0011_1111,
                up: PinPortClass::Sealed,
                down: PinPortClass::Sealed,
            },
        )]);
        let (resolved, _) = resolved_pins(compact(), &profile);
        assert_eq!(
            unsatisfiable(compact(), &resolved, &variants).map(|(_, reason)| reason),
            Some(PinFailure::BoundaryOpening)
        );
    }

    #[test]
    fn forbidding_every_archetype_is_unsatisfiable() {
        let variants = super::super::variants::catalogue();
        let all = vec![
            HexArchetype::Void,
            HexArchetype::Room,
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
            HexArchetype::Expanse,
        ];
        let profile = profile_with(vec![pin(3, 3, PinIntent::Forbid(all))]);
        let (resolved, _) = resolved_pins(compact(), &profile);
        assert_eq!(
            unsatisfiable(compact(), &resolved, &variants).map(|(_, reason)| reason),
            Some(PinFailure::EmptyForbidResidue)
        );
    }

    #[test]
    fn a_reasonable_pin_is_satisfiable() {
        let variants = super::super::variants::catalogue();
        let profile = profile_with(vec![pin(
            4,
            4,
            PinIntent::Archetype(HexArchetype::Junction),
        )]);
        let (resolved, diagnostics) = resolved_pins(compact(), &profile);
        assert!(diagnostics.is_empty());
        assert_eq!(unsatisfiable(compact(), &resolved, &variants), None);
    }

    /// The point of the whole slice: what an author pinned is what gets built.
    #[test]
    fn pins_survive_the_solve() {
        let config = compact();
        for &(q, r, archetype) in &[
            (4_u16, 4_u16, HexArchetype::Junction),
            (6, 3, HexArchetype::Straight),
            (5, 5, HexArchetype::Corner),
        ] {
            let profile = profile_with(vec![pin(q, r, PinIntent::Archetype(archetype))]);
            assert_eq!(profile.validate(), Ok(()));

            for seed in [7_u64, 42, 0xC047_0000_0000_0000] {
                let world =
                    super::super::HexWfcWorld::generate_with_profile(seed, config, None, &profile)
                        .expect("a single reasonable pin must still solve");
                let coord = HexCoord { q, r, level: 0 };
                let placed = world.placements.get(&coord).expect("the cell is placed");
                // A stamped room may land on the cell; blueprints are a frozen
                // contract and win. That is one legal way a pin does not
                // appear, and `diagnose_pins` reports it rather than leaving
                // the author guessing.
                if world.room_id_at(coord).is_some() {
                    continue;
                }
                // And the cell may not be part of the building. `route_corridors`
                // is the default now, so the live facility is the skeleton plus
                // whatever the collapse joined onto it, and `prune_disconnected`
                // voids the rest. On this small lattice that is a third to a
                // half of it - two of the three seeds here - where the weighted
                // blob it replaced reached nearly everywhere.
                //
                // The pin is not overruled in that case, it is *stranded*: the
                // collapse placed the Junction and then the facility decided
                // that corner is not part of itself. Worth knowing that unlike
                // the blueprint case this one is currently silent - the author
                // gets no diagnostic - which is a gap in `diagnose_pins` rather
                // than in the solver.
                let live = super::super::topology::active_component(
                    config,
                    &world.placements,
                    config.spawn(),
                );
                if !live.contains(&coord) {
                    assert_eq!(
                        placed.space,
                        HexSpace::Void,
                        "seed {seed:#x}: pin at ({q},{r}) is outside the live \
                         component but was not pruned"
                    );
                    continue;
                }
                // Or the corridor router claimed it, which is the third legal
                // override and the only one that is a *decision* rather than an
                // accident. A route is fixed before the collapse runs, exactly
                // as a blueprint is, so it wins for the same reason - and like
                // the blueprint case it is reported, as
                // `PinDiagnostic::CorridorCollision`.
                //
                // The exact mask is asserted rather than waved through: what the
                // route promised is what has to be built, or this exception
                // would excuse any outcome at all on a routed cell.
                let skeleton =
                    super::super::constraints::corridor_skeleton(config, &world.blueprints, false)
                        .map(|(doors, _, _)| doors)
                        .unwrap_or_default();
                if let Some(&mask) = skeleton.get(&coord) {
                    assert_eq!(
                        placed.doors, mask,
                        "seed {seed:#x}: the router claimed ({q},{r}) and the \
                         facility did not build the mask it decided"
                    );
                    continue;
                }
                assert_eq!(
                    placed.archetype, archetype,
                    "seed {seed:#x}: pin at ({q},{r}) did not survive"
                );
            }
        }
    }

    /// Blueprints outrank pins, and the override must be *reported* rather than
    /// silent. Finds a seed where a room actually lands on a pinned cell, then
    /// asserts both halves: the room wins, and the author is told.
    #[test]
    fn a_blueprint_wins_over_a_conflicting_pin_and_says_so() {
        let config = compact();
        let mut found = false;
        for q in 3..9_u16 {
            for r in 2..7_u16 {
                let profile = profile_with(vec![pin(
                    q,
                    r,
                    PinIntent::Archetype(HexArchetype::Straight),
                )]);
                let Ok(world) = super::super::HexWfcWorld::generate_with_profile(
                    PROBE_SEED, config, None, &profile,
                ) else {
                    continue;
                };
                let coord = HexCoord { q, r, level: 0 };
                if world.room_id_at(coord).is_none() {
                    continue;
                }
                found = true;
                assert_eq!(
                    world.placements[&coord].space,
                    HexSpace::Room,
                    "the blueprint must win the cell"
                );
                assert!(
                    diagnose_pins(config, &profile)
                        .iter()
                        .any(|d| matches!(d, PinDiagnostic::BlueprintCollision { .. })),
                    "the override must be reported, not silent"
                );
                break;
            }
            if found {
                break;
            }
        }
        assert!(
            found,
            "no pin in the scanned range landed inside a stamped room; \
             widen the range rather than deleting this gate"
        );
    }

    /// A `Forbid` pin is the checked form of "never build this here" — the
    /// thing a bias of zero would only make unlikely.
    #[test]
    fn a_forbid_pin_is_honoured() {
        let config = compact();
        let profile = profile_with(vec![pin(
            4,
            4,
            PinIntent::Forbid(vec![HexArchetype::Shaft, HexArchetype::RampUp]),
        )]);
        for seed in [7_u64, 42] {
            let world =
                super::super::HexWfcWorld::generate_with_profile(seed, config, None, &profile)
                    .expect("solves");
            let placed = world.placements[&HexCoord {
                q: 4,
                r: 4,
                level: 0,
            }];
            assert!(
                !matches!(placed.archetype, HexArchetype::Shaft | HexArchetype::RampUp),
                "seed {seed:#x}: a forbidden archetype was placed"
            );
        }
    }

    /// The Slice 0 guarantee has to survive Slice 3: with no pins authored, the
    /// solver must still reproduce its old output exactly.
    #[test]
    fn an_empty_pin_set_is_byte_identical_to_baseline() {
        let config = compact();
        let mut profile = HexCompositionProfile::baseline();
        profile.pin_sets.push(PinSet {
            id: String::from("empty"),
            note: String::new(),
            cols: config.cols,
            rows: config.rows,
            levels: config.levels,
            pins: Vec::new(),
        });
        for seed in [7_u64, 42, 0xC047_0000_0000_0000] {
            let plain = super::super::HexWfcWorld::generate(seed, config).expect("solves");
            let pinned =
                super::super::HexWfcWorld::generate_with_profile(seed, config, None, &profile)
                    .expect("solves");
            assert_eq!(plain.placements, pinned.placements, "seed {seed:#x}");
        }
    }

    /// An impossible pin must be named before the retry budget is touched.
    #[test]
    fn an_unsatisfiable_pin_reports_before_the_retry_budget() {
        let config = compact();
        let profile = profile_with(vec![pin(2, 2, PinIntent::Space(HexSpace::Room))]);
        let error = super::super::HexWfcWorld::generate_with_profile(7, config, None, &profile)
            .expect_err("a room pin on fabric cannot be satisfied");
        match error {
            super::super::HexWfcError::PinContradiction { coord, reason } => {
                assert_eq!(
                    coord,
                    HexCoord {
                        q: 2,
                        r: 2,
                        level: 0
                    }
                );
                assert_eq!(reason, PinFailure::RoomOutsideBlueprint);
            }
            other => panic!("expected PinContradiction, got {other:?}"),
        }
    }

    /// Attribution: the author needs to know *which* pin to change.
    #[test]
    fn diagnose_pins_names_a_culprit_when_the_set_is_over_constrained() {
        let config = compact();
        // Every lateral neighbour of (4,4) forced to seal against it while
        // (4,4) itself is forced to open through all six.
        let mut pins = vec![pin(
            4,
            4,
            PinIntent::Ports {
                doors: 0b0011_1111,
                up: PinPortClass::Sealed,
                down: PinPortClass::Sealed,
            },
        )];
        for (q, r) in [(5_u16, 4_u16), (3, 4), (4, 5), (4, 3), (5, 3), (3, 5)] {
            pins.push(pin(
                q,
                r,
                PinIntent::Ports {
                    doors: 0,
                    up: PinPortClass::Sealed,
                    down: PinPortClass::Sealed,
                },
            ));
        }
        let mut profile = profile_with(pins);
        // Routing off for this fixture, and only this one. The subject here is
        // pin against *pin*: six neighbours sealing while the cell between them
        // opens through all six. With `route_corridors` on - the shipped default
        // - the skeleton claims three of those seven cells and its exact mask
        // outranks a pin, so the contradiction never forms and the report comes
        // back as three `CorridorCollision`s instead. That is the right
        // behaviour and it is asserted next door in
        // `diagnose_pins_is_silent_on_a_workable_set`; here it would mean the
        // fixture no longer builds the thing the test is named for.
        profile.route_corridors = false;
        let diagnostics = diagnose_pins(config, &profile);
        assert!(
            diagnostics
                .iter()
                .any(|d| matches!(d, PinDiagnostic::Conflict { .. })),
            "an over-constrained set must name a culprit: {diagnostics:?}"
        );
    }

    /// Relayout rewires cells at runtime. If it were allowed to pick a pinned
    /// cell, authored intent would evaporate mid-match with nothing reported —
    /// so the world carries its pins and relayout treats them as protected.
    #[test]
    fn a_pinned_cell_is_never_a_relayout_mutation_site() {
        let config = compact();
        let coord = HexCoord {
            q: 4,
            r: 4,
            level: 0,
        };
        let profile = profile_with(vec![pin(
            coord.q,
            coord.r,
            PinIntent::Archetype(HexArchetype::Junction),
        )]);
        let world = super::super::HexWfcWorld::generate_with_profile(7, config, None, &profile)
            .expect("solves");

        assert!(
            world.authored_pins.contains(&coord),
            "the world must carry the pins it was solved under"
        );

        let observation = super::super::HexObservationFrame::default();
        let protected = super::super::relayout::protected_for_test(&world, &observation);
        assert!(
            protected.contains(&coord),
            "a pinned cell must be protected from relayout"
        );
    }

    /// A satisfiable set reports nothing, so the panel is quiet when it should
    /// be. A diagnostics view that always complains is a view nobody reads.
    #[test]
    fn diagnose_pins_is_silent_on_a_workable_set() {
        let profile = profile_with(vec![
            pin(4, 4, PinIntent::Archetype(HexArchetype::Junction)),
            pin(6, 3, PinIntent::Archetype(HexArchetype::Straight)),
        ]);
        assert_eq!(diagnose_pins(compact(), &profile), Vec::new());
    }

    #[test]
    fn pin_admits_matches_each_intent_shape() {
        let variants = super::super::variants::catalogue();
        let shaft = variants
            .iter()
            .find(|variant| variant.archetype == HexArchetype::Shaft)
            .expect("the catalogue has shafts");

        assert!(pin_admits(
            &PinIntent::Archetype(HexArchetype::Shaft),
            shaft
        ));
        assert!(!pin_admits(
            &PinIntent::Archetype(HexArchetype::Junction),
            shaft
        ));
        assert!(pin_admits(&PinIntent::Space(shaft.space), shaft));
        assert!(!pin_admits(
            &PinIntent::Forbid(vec![HexArchetype::Shaft]),
            shaft
        ));
        assert!(pin_admits(
            &PinIntent::Forbid(vec![HexArchetype::Junction]),
            shaft
        ));
        assert!(pin_admits(
            &PinIntent::Ports {
                doors: shaft.doors,
                up: shaft.up.into(),
                down: shaft.down.into(),
            },
            shaft
        ));
    }
}
