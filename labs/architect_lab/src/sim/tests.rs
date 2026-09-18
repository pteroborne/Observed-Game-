use super::behavior::ObserverIntent;
use super::*;

fn lab() -> ArchitectLab {
    ArchitectLab::new(7).expect("the pinned compact facility solves")
}

#[test]
fn real_two_floor_world_and_five_card_hand_boot() {
    let lab = lab();
    assert_eq!(lab.world.config.levels, 2);
    assert_eq!(lab.deck.hand.len(), HAND_SIZE);
    assert_eq!(lab.prison_core.len(), 2);
    assert_eq!(lab.observers.len(), 2);
    assert_eq!(lab.guardians.len(), 1);
}

#[test]
fn every_mode_is_small_deterministic_and_playable() {
    let expected = [
        (ArchitectMode::Pocket, (6, 5, 1)),
        (ArchitectMode::QuickClimb, (8, 6, 2)),
        (ArchitectMode::FullAscent, (10, 8, 2)),
    ];
    for (mode, dimensions) in expected {
        let first = ArchitectLab::for_mode(mode).expect("the pinned mode solves");
        let second = ArchitectLab::for_mode(mode).expect("the pinned mode solves twice");
        assert_eq!(
            (
                first.world.config.cols,
                first.world.config.rows,
                first.world.config.levels,
            ),
            dimensions
        );
        assert_eq!(first.world, second.world);
        assert_eq!(first.deck, second.deck);
        assert!(!first.mutable_targets().is_empty());
        assert!(!first.legal_commands().is_empty());
    }
}

#[test]
fn human_and_bot_use_the_same_authoritative_command_path() {
    let original = lab();
    let command = original
        .legal_commands()
        .into_iter()
        .next()
        .expect("opening hand has a legal command");
    let mut human = original.clone();
    let mut bot = original;
    human.submit(command).expect("human command accepted");
    bot.submit(command).expect("bot command accepted");
    assert_eq!(human.world, bot.world);
    assert_eq!(human.deck, bot.deck);
    assert_eq!(human.cooldown, bot.cooldown);
    assert_eq!(human.doors, bot.doors);
    assert_eq!(human.command_log, bot.command_log);
}

#[test]
fn observed_occupied_and_prison_cells_are_refused() {
    let lab = lab();
    let card = lab.deck.hand[0].id;
    let observed = *lab.observed.iter().next().expect("Observer sees a cell");
    assert_eq!(
        lab.refusal(ArchitectCommand::Play {
            card,
            target: observed,
            rotation: 0,
        }),
        Some(CommandRefusal::Observed)
    );
    let prison = *lab.prison_core.iter().next().expect("prison exists");
    assert_eq!(
        lab.refusal(ArchitectCommand::Play {
            card,
            target: prison,
            rotation: 0,
        }),
        Some(CommandRefusal::PrisonCore)
    );
}

#[test]
fn successful_play_spends_refills_and_starts_cooldown() {
    let mut lab = lab();
    let before: BTreeSet<_> = lab.deck.hand.iter().map(|card| card.id).collect();
    let command = lab.legal_commands()[0];
    let ArchitectCommand::Play { card, .. } = command else {
        panic!("expected play command");
    };
    lab.submit(command).expect("legal command applies");
    assert_eq!(lab.deck.hand.len(), HAND_SIZE);
    assert!(!lab.deck.hand.iter().any(|held| held.id == card));
    assert_eq!(lab.cooldown, ARCHITECT_COOLDOWN_TICKS);
    assert_ne!(before, lab.deck.hand.iter().map(|card| card.id).collect());
}

#[test]
fn same_snapshot_produces_same_tree_intent_and_trace() {
    let lab = lab();
    assert_eq!(
        lab.observer_intent(ObserverId(0)),
        lab.observer_intent(ObserverId(0))
    );
    assert_eq!(
        lab.guardian_intent(GuardianId(0)),
        lab.guardian_intent(GuardianId(0))
    );
    assert_eq!(lab.architect_intent(), lab.architect_intent());
}

#[test]
fn capture_uses_the_protected_prison_and_all_jailed_wins() {
    let mut lab = lab();
    let guardian = lab.guardians[&GuardianId(0)].cell;
    for observer in lab.observers.values_mut() {
        observer.cell = guardian;
        observer.state = ObserverState::Active;
    }
    let ids: Vec<_> = lab.observers.keys().copied().collect();
    for id in ids {
        lab.jail(id);
    }
    lab.refresh_observation();
    lab.step_beat();
    assert_eq!(lab.outcome, MatchOutcome::RogueVictory);
    assert!(
        lab.observers
            .values()
            .all(|observer| lab.prison_core.contains(&observer.cell))
    );
}

#[test]
fn bot_architect_soak_is_deterministic() {
    let mut a = lab();
    let mut b = lab();
    a.bot_architect = true;
    b.bot_architect = true;
    for _ in 0..20 {
        a.step_beat();
        b.step_beat();
    }
    assert_eq!(a.world, b.world);
    assert_eq!(a.observers, b.observers);
    assert_eq!(a.guardians, b.guardians);
    assert_eq!(a.command_log, b.command_log);
    assert_eq!(a.traces, b.traces);
}

#[test]
fn observer_tree_opens_a_deployed_door_that_blocks_its_route() {
    let mut lab = lab();
    // Isolate door behavior from the scenario's deliberately missing tiles.
    lab.world = HexWfcWorld::generate(7, lab.mode.config()).unwrap();
    lab.guardians.clear();
    let id = ObserverId(0);
    let cell = lab.world.config.spawn();
    lab.observers.get_mut(&id).unwrap().cell = cell;
    lab.observers.get_mut(&ObserverId(1)).unwrap().state = ObserverState::Jailed;
    for next in lab.exits(cell) {
        if let Some(key) = lab.threshold_between(cell, next) {
            lab.doors.insert(key, DoorState::Closed);
        }
    }
    let (intent, trace) = lab.observer_intent(id);
    let ObserverIntent::SetDoor(key, DoorState::Open) = intent else {
        panic!("expected an open-door intent, got {intent:?} with {trace:?}");
    };
    lab.apply_observer_intent(id, intent);
    assert_eq!(lab.doors.get(&key), Some(&DoorState::Open));
    assert_eq!(trace.selected, Some("open door toward summit"));
}
