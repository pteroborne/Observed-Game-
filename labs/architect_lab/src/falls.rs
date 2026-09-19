//! Unsafe falls, landing on lower surviving structure, and true-void corruption.
//!
//! Ordinary falls land on lower surviving structure when geometry permits.
//! Only a fall through the whole surviving stack into true void corrupts.
//! Corruption is immediate, irreversible, and public: the former Observer leaves
//! first-person play and joins the Rogue faction. If every loyal Observer corrupts,
//! that resolves as `MatchOutcome::RogueVictory`.

use observed_facility::hex_wfc::HexSpace;
use observed_hex::{HexCoord, HexFace};

use crate::sim::{ArchitectLab, LabEventKind, MatchOutcome, ObserverId, ObserverState};

/// The outcome of an Observer falling when their tile ceases to support them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FallOutcome {
    /// The Observer safely landed on a lower surviving structure.
    Landed { from: HexCoord, to: HexCoord },
    /// The Observer fell through the entire surviving stack into true void and corrupted.
    Corrupted { from: HexCoord },
}

/// A fall event recording the Observer and the outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallEvent {
    pub observer: ObserverId,
    pub outcome: FallOutcome,
}

/// Returns true if `cell` currently supports an actor standing on it.
///
/// A cell does not support an actor if:
/// - It has been retracted.
/// - It is on a collapsed floor (unless it is part of the immortal prison core).
/// - It has no placement in the world, or its space is `HexSpace::Void`.
#[must_use]
pub fn is_supporting(lab: &ArchitectLab, cell: HexCoord) -> bool {
    if lab.retracted.contains(&cell) {
        return false;
    }
    if lab.collapsed_floors.contains(&cell.level) && !lab.prison_core.contains(&cell) {
        return false;
    }
    match lab.world.placements.get(&cell) {
        Some(placement) => placement.space != HexSpace::Void,
        None => false,
    }
}

/// Searches downward through the surviving stack for the first solid cell beneath `from`.
///
/// In hex grid coordinates, vertical columns have identical `(q, r)` across levels.
/// Searching from `from.level - 1` down to `0`:
/// 1. First checks the direct column `(from.q, from.r, level)`.
/// 2. If the direct column has retracted into void, searches the 1-step lateral
///    neighborhood on that level (canonical face order), representing an actor
///    tumbling onto or catching the edge of surviving floor structure.
///
/// If no level beneath `from` has surviving structure within 1 step, returns `None` (true void).
#[must_use]
pub fn find_lower_surviving_structure(lab: &ArchitectLab, from: HexCoord) -> Option<HexCoord> {
    for level in (0..from.level).rev() {
        let candidate = HexCoord {
            q: from.q,
            r: from.r,
            level,
        };
        if is_supporting(lab, candidate) {
            return Some(candidate);
        }
        for face in HexFace::LATERAL {
            if let Some(neighbor) = lab.world.config.grid().neighbor(candidate, face)
                && is_supporting(lab, neighbor)
            {
                return Some(neighbor);
            }
        }
    }
    None
}

/// Resolves falls for all active Observers whose current cell does not support them.
///
/// Observers are processed deterministically in `ObserverId` order.
///
/// - If a lower surviving structure exists, the Observer lands on it, resets hold beats,
///   and records a `LabEventKind::Fell` event.
/// - If no lower surviving structure exists (true void), the Observer immediately corrupts:
///   `state` transitions to `ObserverState::Corrupted`, records `LabEventKind::Corrupted`,
///   and leaves first-person play.
/// - If every loyal Observer is now eliminated (all Observers are `Jailed` or `Corrupted`),
///   resolves match outcome to `MatchOutcome::RogueVictory`.
pub fn resolve_falls(lab: &mut ArchitectLab) -> Vec<FallEvent> {
    let mut events = Vec::new();

    // Identify active observers that have lost support.
    // Iterating keys from BTreeMap ensures deterministic ObserverId ordering.
    let unsupported: Vec<(ObserverId, HexCoord)> = lab
        .observers
        .iter()
        .filter(|(_, observer)| {
            observer.state == ObserverState::Active && !is_supporting(lab, observer.cell)
        })
        .map(|(&id, observer)| (id, observer.cell))
        .collect();

    for (id, from) in unsupported {
        if let Some(to) = find_lower_surviving_structure(lab, from) {
            if let Some(observer) = lab.observers.get_mut(&id) {
                observer.cell = to;
                observer.hold_beats = 0;
            }
            lab.record_event(
                LabEventKind::Fell,
                Some(to),
                &format!(
                    "Observer {} fell from floor {} to surviving structure on floor {}.",
                    id.0,
                    from.level + 1,
                    to.level + 1
                ),
            );
            events.push(FallEvent {
                observer: id,
                outcome: FallOutcome::Landed { from, to },
            });
        } else {
            if let Some(observer) = lab.observers.get_mut(&id) {
                observer.state = ObserverState::Corrupted;
                observer.hold_beats = 0;
            }
            lab.record_event(
                LabEventKind::Corrupted,
                Some(from),
                &format!(
                    "Observer {} fell into true void and corrupted into Rogue AI.",
                    id.0
                ),
            );
            events.push(FallEvent {
                observer: id,
                outcome: FallOutcome::Corrupted { from },
            });
        }
    }

    // Check zero-loyal elimination: if all observers are eliminated (Jailed or Corrupted),
    // Rogue achieves victory.
    if !lab.observers.is_empty()
        && lab
            .observers
            .values()
            .all(|o| o.state == ObserverState::Jailed || o.state == ObserverState::Corrupted)
    {
        lab.outcome = MatchOutcome::RogueVictory;
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ArchitectMode;

    #[test]
    fn fall_lands_on_lower_surviving_structure_without_corrupting() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
        let obs_id = ObserverId(0);
        let (upper_cell, lower_cell) = sim
            .world
            .placements
            .keys()
            .filter(|c| c.level == 1)
            .find_map(|&upper| {
                let lower = HexCoord {
                    q: upper.q,
                    r: upper.r,
                    level: 0,
                };
                if sim.world.placements.contains_key(&lower) {
                    Some((upper, lower))
                } else {
                    None
                }
            })
            .expect("column spanning levels 0 and 1 must exist");

        // Place active observer at upper cell.
        let observer = sim.observers.get_mut(&obs_id).unwrap();
        observer.cell = upper_cell;
        observer.state = ObserverState::Active;

        // Invalidate support at upper cell (e.g. tile retracts).
        sim.retracted.insert(upper_cell);

        let events = sim.resolve_falls();
        assert_eq!(
            events,
            vec![FallEvent {
                observer: obs_id,
                outcome: FallOutcome::Landed {
                    from: upper_cell,
                    to: lower_cell,
                },
            }]
        );

        let obs = &sim.observers[&obs_id];
        assert_eq!(obs.cell, lower_cell);
        assert_eq!(obs.state, ObserverState::Active);
        assert_eq!(sim.outcome, MatchOutcome::Running);
        assert!(
            sim.events.iter().any(|e| e.kind == LabEventKind::Fell),
            "Fell event must be recorded publicly in the event stream"
        );
    }

    #[test]
    fn multi_level_gap_fall_skips_void_and_lands_on_lower_floor() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
        let obs_id = ObserverId(0);
        let (level1, level0) = sim
            .world
            .placements
            .keys()
            .filter(|c| c.level == 1)
            .find_map(|&upper| {
                let lower = HexCoord {
                    q: upper.q,
                    r: upper.r,
                    level: 0,
                };
                if sim.world.placements.contains_key(&lower) {
                    Some((upper, lower))
                } else {
                    None
                }
            })
            .expect("column spanning levels 0 and 1 must exist");

        // Construct level 2 directly above level 1
        let level2 = HexCoord {
            q: level1.q,
            r: level1.r,
            level: 2,
        };
        let sample_placement = sim.world.placements[&level1];
        sim.world.placements.insert(level2, sample_placement);

        // Place observer at top level (level 2)
        let observer = sim.observers.get_mut(&obs_id).unwrap();
        observer.cell = level2;
        observer.state = ObserverState::Active;

        // Level 2 retracts; level 1 also retracts (void/empty gap); level 0 remains solid
        sim.retracted.insert(level2);
        sim.retracted.insert(level1);

        let events = sim.resolve_falls();
        assert_eq!(
            events,
            vec![FallEvent {
                observer: obs_id,
                outcome: FallOutcome::Landed {
                    from: level2,
                    to: level0,
                },
            }]
        );

        let obs = &sim.observers[&obs_id];
        assert_eq!(obs.cell, level0);
        assert_eq!(obs.state, ObserverState::Active);
    }

    #[test]
    fn fall_through_whole_surviving_stack_into_true_void_corrupts() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let obs_id = ObserverId(0);
        let ground_cell = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };

        let observer = sim.observers.get_mut(&obs_id).unwrap();
        observer.cell = ground_cell;
        observer.state = ObserverState::Active;

        // Ground floor ceases to support (stack beneath is empty).
        sim.retracted.insert(ground_cell);

        let events = sim.resolve_falls();
        assert_eq!(
            events,
            vec![FallEvent {
                observer: obs_id,
                outcome: FallOutcome::Corrupted { from: ground_cell },
            }]
        );

        let obs = &sim.observers[&obs_id];
        assert_eq!(obs.state, ObserverState::Corrupted);
        assert!(
            sim.events.iter().any(|e| e.kind == LabEventKind::Corrupted),
            "Corrupted event must be recorded publicly in the event stream"
        );
    }

    #[test]
    fn corruption_is_immediate_irreversible_and_public() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let obs_id = ObserverId(0);
        let ground_cell = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };

        sim.observers.get_mut(&obs_id).unwrap().cell = ground_cell;
        sim.retracted.insert(ground_cell);

        sim.resolve_falls();
        assert_eq!(sim.observers[&obs_id].state, ObserverState::Corrupted);

        let event = sim
            .events
            .iter()
            .find(|e| e.kind == LabEventKind::Corrupted)
            .expect("public corruption event must exist");
        assert_eq!(event.cell, Some(ground_cell));

        // Advance several beats and simulate repairs / placements:
        // Even if the ground cell is un-retracted or repaired, corrupted state is irreversible.
        sim.retracted.remove(&ground_cell);
        for _ in 0..10 {
            sim.tick();
        }

        assert_eq!(
            sim.observers[&obs_id].state,
            ObserverState::Corrupted,
            "corrupted observer must never return to loyal"
        );
    }

    #[test]
    fn corrupted_observer_leaves_first_person_play() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let obs_id = ObserverId(0);
        let ground_cell = sim.observers[&obs_id].cell;
        sim.guardians.clear();

        sim.retracted.insert(ground_cell);
        sim.resolve_falls();

        assert_eq!(sim.observers[&obs_id].state, ObserverState::Corrupted);

        // Not in occupied set
        assert!(
            !sim.occupied().contains(&ground_cell),
            "corrupted observer must be excluded from occupied set"
        );

        // Not in detected observers
        assert!(
            !sim.detected_observers().contains(&obs_id),
            "corrupted observer must be excluded from detected observers"
        );

        // Does not act in behavior tree
        let initial_facing = sim.observers[&obs_id].facing;
        let initial_hold = sim.observers[&obs_id].hold_beats;
        sim.step_beat();
        assert_eq!(sim.observers[&obs_id].cell, ground_cell);
        assert_eq!(sim.observers[&obs_id].facing, initial_facing);
        assert_eq!(sim.observers[&obs_id].hold_beats, initial_hold);
    }

    #[test]
    fn all_loyal_observers_corrupted_resolves_as_rogue_victory() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let obs0 = ObserverId(0);
        let obs1 = ObserverId(1);
        let cell0 = sim.observers[&obs0].cell;
        let cell1 = sim.observers[&obs1].cell;

        // Corrupt first observer
        sim.retracted.insert(cell0);
        sim.resolve_falls();
        assert_eq!(sim.observers[&obs0].state, ObserverState::Corrupted);
        assert_eq!(sim.outcome, MatchOutcome::Running);

        // Corrupt second observer: zero loyal remain
        sim.retracted.insert(cell1);
        sim.resolve_falls();
        assert_eq!(sim.observers[&obs1].state, ObserverState::Corrupted);
        assert_eq!(
            sim.outcome,
            MatchOutcome::RogueVictory,
            "zero-loyal elimination must resolve as RogueVictory"
        );
    }

    #[test]
    fn mixed_jailed_and_corrupted_resolves_as_rogue_victory() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let obs0 = ObserverId(0);
        let obs1 = ObserverId(1);
        let cell0 = sim.observers[&obs0].cell;

        sim.observers.get_mut(&obs1).unwrap().state = ObserverState::Jailed;

        // Corrupt the remaining active observer
        sim.retracted.insert(cell0);
        sim.resolve_falls();
        assert_eq!(sim.observers[&obs0].state, ObserverState::Corrupted);
        assert_eq!(
            sim.outcome,
            MatchOutcome::RogueVictory,
            "when all observers are jailed or corrupted, Rogue wins"
        );
    }

    #[test]
    fn prison_core_never_collapses_and_never_produces_a_fall() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let core_cell = *sim.prison_core.iter().next().expect("prison core exists");

        // Put all active observers inside prison core
        for observer in sim.observers.values_mut() {
            observer.cell = core_cell;
            observer.state = ObserverState::Active;
        }

        // Even if floor collapses, prison core remains immortal
        sim.collapsed_floors.insert(core_cell.level);
        assert!(is_supporting(&sim, core_cell));

        let events = sim.resolve_falls();
        assert!(
            events.is_empty(),
            "prison core cell must never produce a fall"
        );
        for observer in sim.observers.values() {
            assert_eq!(observer.cell, core_cell);
            assert_eq!(observer.state, ObserverState::Active);
        }
    }

    #[test]
    fn falls_and_corruptions_reproduce_from_seed_and_ordered_commands() {
        let mut sim_a = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let mut sim_b = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();

        let target_cell = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        sim_a.observers.get_mut(&ObserverId(0)).unwrap().cell = target_cell;
        sim_b.observers.get_mut(&ObserverId(0)).unwrap().cell = target_cell;

        sim_a.retracted.insert(target_cell);
        sim_b.retracted.insert(target_cell);

        let events_a = sim_a.resolve_falls();
        let events_b = sim_b.resolve_falls();

        assert_eq!(events_a, events_b);
        assert_eq!(sim_a.observers, sim_b.observers);
        assert_eq!(sim_a.outcome, sim_b.outcome);
        assert_eq!(sim_a.events, sim_b.events);
    }

    #[test]
    fn reset_paths_clear_corruption_without_leaks() {
        // Path 1: desktop.rs:143 (LabSession::reset)
        #[cfg(feature = "desktop")]
        {
            use crate::desktop::{ArchitectAction, LabSession};
            let mut session = LabSession::default();
            session.sim.observers.get_mut(&ObserverId(0)).unwrap().state = ObserverState::Corrupted;
            session.sim.outcome = MatchOutcome::RogueVictory;

            session.apply_action(ArchitectAction::Reset);
            assert!(
                session
                    .sim
                    .observers
                    .values()
                    .all(|o| o.state == ObserverState::Active),
                "desktop reset must un-corrupt all observers"
            );
            assert_eq!(session.sim.outcome, MatchOutcome::Running);
        }

        // Path 2: view.rs:65 (MapCameraState::reset_for_mode)
        #[cfg(feature = "desktop")]
        {
            use crate::view::MapCameraState;
            let mut camera = MapCameraState::default();
            camera.zoom = 2.5;
            camera.reset_for_mode(ArchitectMode::Pocket);
            assert!((camera.zoom - 0.62).abs() < f32::EPSILON);
        }

        // Path 3: web.rs:24 (RogueGame::reset)
        #[cfg(feature = "web")]
        {
            use crate::web::RogueGame;
            let mut game = RogueGame::new(0).unwrap();
            game.sim.observers.get_mut(&ObserverId(0)).unwrap().state = ObserverState::Corrupted;
            game.sim.outcome = MatchOutcome::RogueVictory;

            game.reset(0).expect("web reset succeeds");
            assert!(
                game.sim
                    .observers
                    .values()
                    .all(|o| o.state == ObserverState::Active),
                "web reset must un-corrupt all observers"
            );
        }
    }

    #[test]
    fn fall_lands_on_adjacent_surviving_structure_when_column_is_void() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
        let obs_id = ObserverId(0);

        // Find an upper cell on level 1 with a supporting neighbor on level 0,
        // but whose direct column on level 0 is retracted into void.
        let (upper, _lower_neighbor) = sim
            .world
            .placements
            .keys()
            .filter(|c| c.level == 1)
            .find_map(|&upper| {
                let direct_lower = HexCoord {
                    q: upper.q,
                    r: upper.r,
                    level: 0,
                };
                for face in HexFace::LATERAL {
                    if let Some(neighbor) = sim.world.config.grid().neighbor(direct_lower, face)
                        && is_supporting(&sim, neighbor)
                    {
                        return Some((upper, neighbor));
                    }
                }
                None
            })
            .expect("upper cell with neighboring lower support must exist");

        let direct_lower = HexCoord {
            q: upper.q,
            r: upper.r,
            level: 0,
        };

        // Retract upper and retract direct lower:
        sim.retracted.insert(upper);
        sim.retracted.insert(direct_lower);

        let observer = sim.observers.get_mut(&obs_id).unwrap();
        observer.cell = upper;
        observer.state = ObserverState::Active;

        let events = sim.resolve_falls();
        assert_eq!(events.len(), 1);
        match events[0].outcome {
            FallOutcome::Landed { from, to } => {
                assert_eq!(from, upper);
                assert_eq!(to.level, 0);
                assert!(is_supporting(&sim, to));
                // Verifies it landed on an adjacent supporting neighbor rather than corrupting
                assert_ne!(to, direct_lower);
            }
            FallOutcome::Corrupted { .. } => {
                panic!("fall should have landed on adjacent surviving structure");
            }
        }
    }
}
