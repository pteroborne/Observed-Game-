//! Emergency requisition mechanics: refills the Architect hand to exactly five,
//! releasing exactly one major Guardian on the floor active Observers occupy,
//! visible to every faction, leaving the placement cooldown untouched.

use std::collections::{BTreeMap, BTreeSet};

use observed_facility::hex_wfc::{HexCoord, HexSpace, HexWfcWorld};

use crate::sim::{
    ArchitectCommand, ArchitectLab, Guardian, GuardianId, GuardianKind, LabEventKind, Observer,
    ObserverId, ObserverState,
};

/// Deterministic PRNG following SplitMix discipline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequisitionPrng(pub u64);

impl RequisitionPrng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    #[must_use]
    pub fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }
}

/// Simulation-level tracking for emergency requisitions.
#[derive(Clone, Debug, PartialEq)]
pub struct RequisitionState {
    pub count: u32,
    pub next_guardian_id: u16,
    pub rng: RequisitionPrng,
}

impl RequisitionState {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            count: 0,
            next_guardian_id: 1, // GuardianId(0) is reserved for the initial guardian
            rng: RequisitionPrng(seed.wrapping_add(0x517C_C1B7_2722_0A95)),
        }
    }
}

/// Determines the floor active Observers occupy.
///
/// Priority is given to active Observers (ordered by stable ObserverId).
/// If all Observers are jailed or inactive, falls back to the first Observer's floor,
/// or floor 0 if none exist.
#[must_use]
pub fn target_floor(observers: &BTreeMap<ObserverId, Observer>) -> u8 {
    observers
        .values()
        .find(|o| o.state == ObserverState::Active)
        .or_else(|| observers.values().next())
        .map_or(0, |o| o.cell.level)
}

/// Deterministically chooses a candidate spawn cell on the target floor.
///
/// Candidates are solid, non-void, non-retracted, non-prison, and currently unoccupied.
/// Sorting candidate coordinates prior to PRNG selection guarantees that seed + ordered
/// commands strictly reproduce the chosen cell without iteration-order dependence.
#[must_use]
pub fn choose_spawn_cell(
    world: &HexWfcWorld,
    floor: u8,
    occupied: &BTreeSet<HexCoord>,
    prison_core: &BTreeSet<HexCoord>,
    retracted: &BTreeSet<HexCoord>,
    rng: &mut RequisitionPrng,
) -> HexCoord {
    let mut candidates: Vec<HexCoord> = world
        .placements
        .iter()
        .filter(|(cell, placement)| {
            cell.level == floor
                && placement.space != HexSpace::Void
                && !retracted.contains(cell)
                && !prison_core.contains(cell)
                && !occupied.contains(cell)
        })
        .map(|(&cell, _)| cell)
        .collect();

    candidates.sort();

    if !candidates.is_empty() {
        let index = rng.below(candidates.len());
        candidates[index]
    } else {
        // Fallback: any non-void solid cell on that floor outside prison core
        let mut fallbacks: Vec<HexCoord> = world
            .placements
            .iter()
            .filter(|(cell, placement)| {
                cell.level == floor
                    && placement.space != HexSpace::Void
                    && !prison_core.contains(cell)
            })
            .map(|(&cell, _)| cell)
            .collect();
        fallbacks.sort();
        if !fallbacks.is_empty() {
            let index = rng.below(fallbacks.len());
            fallbacks[index]
        } else {
            world.config.spawn()
        }
    }
}

/// Executes an emergency requisition on the simulation.
///
/// - Refills the Architect hand to exactly five.
/// - Releases exactly one major Guardian on the floor currently occupied by active Observers.
/// - Pushes the event to the public match event stream (visible to every faction).
/// - Leaves the placement cooldown completely untouched.
pub fn apply_requisition(lab: &mut ArchitectLab) {
    // 1. Refill hand to exactly five.
    lab.deck.emergency_refill();

    // 2. Select target floor and spawn cell for exactly one major Guardian.
    let floor = target_floor(&lab.observers);
    let occupied = lab.occupied();
    let spawn_cell = choose_spawn_cell(
        &lab.world,
        floor,
        &occupied,
        &lab.prison_core,
        &lab.retracted,
        &mut lab.requisition.rng,
    );

    let guardian_id = GuardianId(lab.requisition.next_guardian_id);
    lab.requisition.next_guardian_id += 1;
    lab.requisition.count += 1;

    // The requisition's price is one *major* Guardian: the looking problem, not
    // the doing problem. Minors belong to the facility and arrive on disturbance
    // waves; they are never something an Architect buys.
    let guardian = Guardian {
        id: guardian_id,
        cell: spawn_cell,
        last_detection: None,
        kind: GuardianKind::Major,
    };
    lab.guardians.insert(guardian_id, guardian);

    // 3. Pushes command to log.
    lab.command_log
        .push((lab.tick, ArchitectCommand::Requisition));

    // 4. Public announcement to every faction.
    lab.record_event(
        LabEventKind::Requisition,
        Some(spawn_cell),
        &format!(
            "Emergency requisition: major Guardian {} released on floor {}.",
            guardian_id.0, floor
        ),
    );

    // 5. Cooldown is untouched (lab.cooldown is preserved as-is).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{ArchitectMode, HAND_SIZE};

    fn lab() -> ArchitectLab {
        ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket mode solves")
    }

    fn multi_floor_lab() -> ArchitectLab {
        ArchitectLab::for_mode(ArchitectMode::QuickClimb).expect("quick climb solves")
    }

    #[test]
    fn test_clause_exactly_five_refills_depleted_and_dead_hands() {
        let mut sim = lab();

        // Sub-case A: hand depleted to fewer than five cards (e.g. 2 cards)
        sim.deck.hand.truncate(2);
        assert_eq!(sim.deck.hand.len(), 2);
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds on depleted hand");
        assert_eq!(sim.deck.hand.len(), HAND_SIZE);

        // Sub-case B: completely empty hand
        sim.deck.hand.clear();
        assert_eq!(sim.deck.hand.len(), 0);
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds on empty hand");
        assert_eq!(sim.deck.hand.len(), HAND_SIZE);

        // Sub-case C: full hand of 5 cards (dead hand out)
        let initial_hand: Vec<_> = sim.deck.hand.iter().map(|c| c.id).collect();
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds on full hand");
        assert_eq!(sim.deck.hand.len(), HAND_SIZE);
        let refilled_hand: Vec<_> = sim.deck.hand.iter().map(|c| c.id).collect();
        assert_ne!(
            initial_hand, refilled_hand,
            "emergency requisition must cycle dead cards into discard and draw fresh cards"
        );
    }

    #[test]
    fn test_clause_releases_exactly_one_major_guardian() {
        let mut sim = lab();
        let initial_count = sim.guardians.len();
        assert_eq!(initial_count, 1, "simulation begins with 1 major guardian");

        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds");

        // Assert delta is EXACTLY +1
        assert_eq!(
            sim.guardians.len(),
            initial_count + 1,
            "must release exactly one major Guardian"
        );

        let new_guardian_id = GuardianId(1);
        let new_guardian = sim
            .guardians
            .get(&new_guardian_id)
            .expect("spawned guardian is registered in guardians map");
        assert_eq!(new_guardian.id, new_guardian_id);
        assert_eq!(new_guardian.last_detection, None);
        assert!(
            sim.world.placements.contains_key(&new_guardian.cell),
            "spawn cell must be a valid facility placement"
        );
    }

    #[test]
    fn test_clause_releases_on_correct_floor() {
        let mut sim = multi_floor_lab();
        assert!(
            sim.world.config.levels > 1,
            "test requires a multi-floor facility"
        );

        // Scenario 1: Observers occupy floor 0
        for obs in sim.observers.values_mut() {
            obs.cell.level = 0;
            obs.state = ObserverState::Active;
        }
        let before_count = sim.guardians.len();
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition on floor 0 succeeds");
        let spawned_g1 = sim
            .guardians
            .get(&GuardianId(before_count as u16))
            .expect("new guardian exists");
        assert_eq!(
            spawned_g1.cell.level, 0,
            "guardian must be released on floor 0 when Observers occupy floor 0"
        );

        // Scenario 2: Observers occupy floor 1
        for obs in sim.observers.values_mut() {
            obs.cell.level = 1;
            obs.state = ObserverState::Active;
        }
        let before_count_2 = sim.guardians.len();
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition on floor 1 succeeds");
        let spawned_g2 = sim
            .guardians
            .get(&GuardianId(before_count_2 as u16))
            .expect("second new guardian exists");
        assert_eq!(
            spawned_g2.cell.level, 1,
            "guardian must be released on floor 1 when Observers occupy floor 1"
        );
    }

    #[test]
    fn test_clause_placement_cooldown_provably_untouched() {
        let mut sim = lab();

        // Sub-test 1: Cooldown is 0 -> remains 0
        sim.cooldown = 0;
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition at cooldown 0 succeeds");
        assert_eq!(
            sim.cooldown, 0,
            "cooldown at 0 must not be started by requisition"
        );

        // Sub-test 2: Cooldown is active at 150 -> remains 150
        sim.cooldown = 150;
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition at cooldown 150 succeeds");
        assert_eq!(
            sim.cooldown, 150,
            "cooldown at 150 must neither be reset nor consumed by requisition"
        );

        // Sub-test 3: Cooldown is max (300) -> remains 300
        sim.cooldown = 300;
        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition at cooldown 300 succeeds");
        assert_eq!(
            sim.cooldown, 300,
            "cooldown at 300 must remain untouched by requisition"
        );
    }

    #[test]
    fn test_clause_visible_to_every_faction() {
        let mut sim = lab();
        let initial_events = sim.events.len();

        sim.submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds");

        // Assert public match event stream contains Requisition event
        assert!(
            sim.events.len() > initial_events,
            "requisition must record to the public events stream"
        );
        let event = sim
            .events
            .iter()
            .find(|e| e.kind == LabEventKind::Requisition)
            .expect("LabEventKind::Requisition must be present in public event log");

        assert!(
            event.cell.is_some(),
            "requisition event must state where guardian was released"
        );
        assert!(
            event.message.contains("Emergency requisition"),
            "event message must announce emergency requisition publicly"
        );

        // Assert recorded in command log
        assert!(
            sim.command_log
                .iter()
                .any(|(_, cmd)| *cmd == ArchitectCommand::Requisition),
            "command log must record the requisition"
        );
    }

    #[test]
    fn test_determinism_seed_plus_commands_reproduces_outcome() {
        let run = |seed: u64| -> ArchitectLab {
            let mut sim = ArchitectLab::new(seed).expect("seed solves");
            // Execute a mix of plays and requisitions
            if let Some(cmd) = sim.legal_commands().into_iter().next() {
                let _ = sim.submit(cmd);
            }
            sim.submit(ArchitectCommand::Requisition).unwrap();
            for _ in 0..10 {
                sim.tick();
            }
            sim.submit(ArchitectCommand::Requisition).unwrap();
            sim
        };

        let sim_a = run(42);
        let sim_b = run(42);

        assert_eq!(sim_a.guardians, sim_b.guardians);
        assert_eq!(sim_a.deck, sim_b.deck);
        assert_eq!(sim_a.command_log, sim_b.command_log);
        assert_eq!(sim_a.cooldown, sim_b.cooldown);
        assert_eq!(sim_a.events, sim_b.events);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn test_reset_paths_clear_all_requisition_spawned_state() {
        use crate::desktop::{ArchitectAction, LabSession};
        use crate::view::MapCameraState;

        // Path 1: desktop.rs:143 (LabSession::reset)
        let mut session = LabSession::default();
        session
            .sim
            .submit(ArchitectCommand::Requisition)
            .expect("requisition in session succeeds");
        assert_eq!(session.sim.guardians.len(), 2);
        assert_eq!(session.sim.requisition.count, 1);

        session.apply_action(ArchitectAction::Reset);
        assert_eq!(
            session.sim.guardians.len(),
            1,
            "desktop reset must remove all requisition-spawned guardians"
        );
        assert_eq!(
            session.sim.requisition.count, 0,
            "desktop reset must reset requisition count"
        );
        assert_eq!(
            session.sim.deck.hand.len(),
            HAND_SIZE,
            "desktop reset restores pristine 5-card hand"
        );

        // Path 2: view.rs:65 (MapCameraState::reset_for_mode)
        let mut camera_state = MapCameraState::default();
        camera_state.zoom = 2.5;
        camera_state.reset_for_mode(ArchitectMode::Pocket);
        assert!((camera_state.zoom - crate::view::DEFAULT_ZOOM).abs() < f32::EPSILON);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_web_reset_clears_all_requisition_state() {
        use crate::web::RogueGame;

        // Path 3: web.rs:24 (RogueGame::reset)
        let mut game = RogueGame::new(0).expect("web pocket initializes");
        game.sim
            .submit(ArchitectCommand::Requisition)
            .expect("requisition succeeds");
        assert_eq!(game.sim.guardians.len(), 2);
        assert_eq!(game.sim.requisition.count, 1);

        game.reset(0).expect("web reset succeeds");
        assert_eq!(
            game.sim.guardians.len(),
            1,
            "web reset restores 1 initial guardian"
        );
        assert_eq!(
            game.sim.requisition.count, 0,
            "web reset resets requisition state"
        );
        assert_eq!(
            game.sim.deck.hand.len(),
            HAND_SIZE,
            "web reset restores 5-card hand"
        );
    }
}
