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
    assert_eq!(lab.prison_core.len(), 11);
    assert_eq!(lab.prison_core.len(), lab.prison.cells.len());
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

#[test]
fn unreachable_minor_guardian_does_not_softlock_observer() {
    let mut lab = lab();
    lab.guardians.clear();
    let obs_id = ObserverId(0);
    let obs_cell = lab.world.config.spawn();
    lab.observers.get_mut(&obs_id).unwrap().cell = obs_cell;
    lab.observers.get_mut(&obs_id).unwrap().state = ObserverState::Active;
    lab.economy.set_charge(obs_id, 100);

    let adj = HexFace::LATERAL
        .into_iter()
        .find_map(|face| lab.world.config.grid().neighbor(obs_cell, face))
        .expect("adjacent neighbor exists");

    let key = lab
        .threshold_between(obs_cell, adj)
        .expect("threshold exists");
    lab.doors.insert(key, DoorState::Closed);

    let minor_id = GuardianId(1001);
    lab.guardians.insert(
        minor_id,
        Guardian {
            id: minor_id,
            cell: adj,
            last_detection: None,
            kind: GuardianKind::Minor,
        },
    );
    lab.observed.remove(&adj);

    assert!(!lab.can_shove(obs_id, minor_id));

    let (intent, trace) = lab.observer_intent(obs_id);
    assert!(!matches!(intent, ObserverIntent::Shove(_)));
    assert_ne!(trace.selected, Some("shove adjacent Minor Guardian"));

    lab.apply_observer_intent(obs_id, ObserverIntent::Shove(minor_id));
    assert_eq!(
        lab.economy.charge(obs_id),
        100,
        "Refused shove spent no charge"
    );
    let (next_intent, _) = lab.observer_intent(obs_id);
    assert!(!matches!(next_intent, ObserverIntent::Shove(_)));
}

#[test]
fn shove_resolves_against_minor_guardian_and_displaces_or_destroys() {
    use crate::economy::{SHOVE_COST, ShoveOutcome};

    let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).expect("pocket solves");
    lab.guardians.clear();
    let obs_id = ObserverId(0);
    let obs_cell = lab.observers[&obs_id].cell;
    lab.observers.get_mut(&obs_id).unwrap().state = ObserverState::Active;
    lab.economy.set_charge(obs_id, 100);

    let exit_cell = lab.exits(obs_cell).into_iter().next().expect("exit exists");
    let minor_id = GuardianId(1002);
    lab.guardians.insert(
        minor_id,
        Guardian {
            id: minor_id,
            cell: exit_cell,
            last_detection: None,
            kind: GuardianKind::Minor,
        },
    );
    lab.observed.insert(exit_cell);

    assert!(lab.can_shove(obs_id, minor_id));
    let (intent, trace) = lab.observer_intent(obs_id);
    assert_eq!(intent, ObserverIntent::Shove(minor_id));
    assert_eq!(trace.selected, Some("shove adjacent Minor Guardian"));

    lab.apply_observer_intent(obs_id, intent);
    assert_eq!(lab.economy.charge(obs_id), 100 - SHOVE_COST);

    // Shove toward grid boundary / void destroys Minor Guardian
    let edge_cell = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let obs_cell2 = HexCoord {
        q: 1,
        r: 0,
        level: 0,
    };
    if let Some(p) = lab.world.placements.get_mut(&edge_cell) {
        p.space = HexSpace::Hall;
        p.doors = 0b111111;
    }
    if let Some(p) = lab.world.placements.get_mut(&obs_cell2) {
        p.space = HexSpace::Hall;
        p.doors = 0b111111;
    }
    lab.observers.get_mut(&obs_id).unwrap().cell = obs_cell2;
    lab.economy.set_charge(obs_id, 100);
    let minor_id2 = GuardianId(1003);
    lab.guardians.insert(
        minor_id2,
        Guardian {
            id: minor_id2,
            cell: edge_cell,
            last_detection: None,
            kind: GuardianKind::Minor,
        },
    );
    let outcome = lab
        .shove(obs_id, minor_id2)
        .expect("shove into void succeeds");
    assert!(
        matches!(outcome, ShoveOutcome::CommittedToVoid { .. }),
        "Expected CommittedToVoid, got {:?}",
        outcome
    );
    assert!(
        !lab.guardians.contains_key(&minor_id2),
        "Minor guardian destroyed by shove into void"
    );
}

#[test]
fn emergency_requisition_is_reachable_under_duress() {
    let mut lab = lab();
    assert!(
        lab.legal_commands()
            .contains(&ArchitectCommand::Requisition)
    );

    lab.deck.hand.clear();
    lab.cooldown = 0;

    let (intent, trace) = lab.architect_intent();
    assert_eq!(intent, Some(ArchitectCommand::Requisition));
    assert_eq!(trace.selected, Some("emergency requisition"));

    let target_floor = crate::requisition::target_floor(&lab.observers);
    let majors_before = lab
        .guardians
        .values()
        .filter(|g| g.kind == GuardianKind::Major && g.cell.level == target_floor)
        .count();

    lab.submit(ArchitectCommand::Requisition)
        .expect("requisition succeeds");
    assert_eq!(lab.deck.hand.len(), HAND_SIZE);
    assert_eq!(lab.requisition.count, 1);
    let majors_after = lab
        .guardians
        .values()
        .filter(|g| g.kind == GuardianKind::Major && g.cell.level == target_floor)
        .count();
    assert_eq!(majors_after, majors_before + 1);
}

#[test]
fn floor_power_can_be_cut_and_exercises_power_gating() {
    let mut lab = lab();
    lab.guardians.clear();
    let floor = 0;
    assert!(lab.economy.is_powered(floor));

    lab.cut_floor_power(floor);
    assert!(!lab.economy.is_powered(floor));

    let door_key = ThresholdKey {
        cell: HexCoord {
            q: 0,
            r: 0,
            level: floor,
        },
        face: HexFace::East,
    };
    assert!(!lab.operate_door(door_key, DoorState::Closed));

    let obs_id = ObserverId(0);
    let gen_cell = lab.economy.generators[&floor];
    lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
    lab.observers.get_mut(&obs_id).unwrap().state = ObserverState::Active;

    let (intent, trace) = lab.observer_intent(obs_id);
    assert_eq!(intent, ObserverIntent::ToggleGenerator);
    assert_eq!(trace.selected, Some("restore floor power at generator"));

    lab.apply_observer_intent(obs_id, intent);
    assert!(lab.economy.is_powered(floor), "Power successfully restored");
}

#[test]
fn variable_loyal_team_size_1_2_3() {
    for size in 1..=3 {
        let lab = ArchitectLab::for_mode_with_team_size(ArchitectMode::Pocket, size)
            .expect("solves for team size");
        assert_eq!(lab.loyal_team_size, size);
        assert_eq!(lab.observers.len(), size);
        for id in 0..size {
            let obs = &lab.observers[&ObserverId(id as u16)];
            assert_eq!(obs.team, TeamId(0));
            assert_eq!(obs.state, ObserverState::Active);
        }
        let knowledge = lab.team_knowledge(TeamId(0));
        assert_eq!(knowledge.known_observers.len(), size);
    }
}

#[test]
fn corruption_adjusted_summit_quorum() {
    let mut lab = ArchitectLab::for_mode_with_team_size(ArchitectMode::Pocket, 2).unwrap();
    let exit = lab.world.config.exit();
    let obs0 = ObserverId(0);
    let obs1 = ObserverId(1);

    // Place obs0 at summit, obs1 away.
    lab.observers.get_mut(&obs0).unwrap().cell = exit;
    let non_summit = lab
        .world
        .placements
        .keys()
        .copied()
        .find(|&c| c != exit)
        .unwrap();
    lab.observers.get_mut(&obs1).unwrap().cell = non_summit;

    lab.step_beat();
    assert_eq!(
        lab.outcome,
        MatchOutcome::Running,
        "quorum is 2, only 1 at summit"
    );

    // Corrupt obs1 into Rogue faction.
    lab.observers.get_mut(&obs1).unwrap().state = ObserverState::Corrupted;
    assert_eq!(lab.observers[&obs1].state, ObserverState::Corrupted);

    // Now remaining loyal count is 1 (obs0). Obs0 is at summit -> quorum met!
    lab.step_beat();
    assert_eq!(
        lab.outcome,
        MatchOutcome::LoyalVictory,
        "corrupted observer leaves quorum, remaining loyal at summit wins"
    );
}

#[test]
fn jailed_observer_remains_loyal_and_counts_in_quorum() {
    let mut lab = ArchitectLab::for_mode_with_team_size(ArchitectMode::Pocket, 2).unwrap();
    let exit = lab.world.config.exit();
    let obs0 = ObserverId(0);
    let obs1 = ObserverId(1);

    // Place obs0 at summit.
    lab.observers.get_mut(&obs0).unwrap().cell = exit;

    // Jail obs1. Jailed observer is still loyal and must be rescued / escape.
    lab.jail(obs1);
    assert_eq!(lab.observers[&obs1].state, ObserverState::Jailed);

    lab.step_beat();
    assert_eq!(
        lab.outcome,
        MatchOutcome::Running,
        "jailed observer still counts in quorum; 1 of 2 loyal is insufficient"
    );

    // Free/escape obs1 and bring to summit.
    lab.observers.get_mut(&obs1).unwrap().state = ObserverState::Active;
    lab.observers.get_mut(&obs1).unwrap().cell = exit;

    lab.step_beat();
    assert_eq!(
        lab.outcome,
        MatchOutcome::LoyalVictory,
        "both loyal observers now at summit"
    );
}

#[test]
fn same_tick_capture_precedence_resolves_before_summit_quorum() {
    let mut lab = ArchitectLab::for_mode_with_team_size(ArchitectMode::Pocket, 1).unwrap();
    let exit = lab.world.config.exit();
    let obs0 = ObserverId(0);
    let guardian_id = *lab.guardians.keys().next().unwrap();

    // Place observer and guardian at the summit on the exact same tick.
    lab.observers.get_mut(&obs0).unwrap().cell = exit;
    let guardian = lab.guardians.get_mut(&guardian_id).unwrap();
    guardian.cell = exit;
    guardian.kind = GuardianKind::Minor;

    // Advance one beat: guardian capture resolves BEFORE summit quorum.
    lab.step_beat();

    // Guardian captures observer -> observer is jailed.
    assert_eq!(lab.observers[&obs0].state, ObserverState::Jailed);
    // Since the sole observer was jailed, Rogue wins (all jailed), NOT LoyalVictory.
    assert_eq!(
        lab.outcome,
        MatchOutcome::RogueVictory,
        "capture precedence executes before summit quorum, jailing the observer"
    );
}

#[test]
fn team_scoped_knowledge_filtering() {
    let lab = ArchitectLab::for_mode_with_team_size(ArchitectMode::Pocket, 2).unwrap();
    let team_k = lab.team_knowledge(TeamId(0));
    let rogue_k = lab.rogue_knowledge();

    // Loyal knowledge only contains what team 0 has observed:
    for &cell in &team_k.discovered_cells {
        assert!(lab.world.placements.contains_key(&cell));
    }
    // Facility truth has unobserved cells in Pocket:
    assert!(lab.world.placements.len() > team_k.discovered_cells.len());

    // Rogue knowledge knows full facility structure:
    assert_eq!(rogue_k.cells.len(), lab.world.placements.len());
    // But Rogue knowledge ONLY knows detected observers:
    assert_eq!(
        rogue_k
            .known_observers
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        lab.detected_observers()
    );
}

#[test]
fn bot_played_loyal_victory_is_reachable() {
    let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    // In pocket mode, bot observers advance toward summit.
    for _ in 0..90 {
        if lab.outcome != MatchOutcome::Running {
            break;
        }
        lab.step_beat();
    }
    assert_eq!(
        lab.outcome,
        MatchOutcome::LoyalVictory,
        "bot observers can and do achieve LoyalVictory in match play"
    );
}
