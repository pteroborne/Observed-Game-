use super::*;

fn unstable() -> ArchitectLab {
    let lab = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
    lab.legal_commands()
        .into_iter()
        .find_map(|command| {
            let mut candidate = lab.clone();
            candidate.submit(command).unwrap();
            (!candidate.contradictions.is_empty() && candidate.next_retraction().is_some())
                .then_some(candidate)
        })
        .expect("scenario admits a locally fitting, globally contradicted card")
}

#[test]
fn rogue_surge_raises_visible_pressure_and_hastens_only_an_exposed_warning() {
    let mut sim = unstable();
    let floor = sim.next_retraction().unwrap().level;
    let before_world = sim.world.clone();
    let before_contradictions = sim.contradictions.clone();
    let before_due = sim.next_retraction_tick.unwrap();
    sim.cooldown = 0;
    sim.deck = Deck::rogue(91, sim.world.config.levels, &TileShape::ALL);
    assert!(sim.deck.stage_kind(CardKind::Surge));
    sim.economy.set_disturbance(floor, 0);
    let card = sim
        .deck
        .hand
        .iter()
        .find(|card| card.kind == CardKind::Surge)
        .unwrap()
        .id;
    let target = sim
        .mutable_targets()
        .into_iter()
        .find(|target| {
            target.level == floor
                && sim
                    .refusal(ArchitectCommand::Play {
                        card,
                        target: *target,
                        rotation: 0,
                    })
                    .is_none()
        })
        .expect("surge has an unprotected built target on the unstable floor");
    sim.submit(ArchitectCommand::Play {
        card,
        target,
        rotation: 0,
    })
    .unwrap();
    assert_eq!(sim.world, before_world, "a surge changes no geometry");
    assert_eq!(sim.contradictions, before_contradictions);
    assert_eq!(sim.economy.disturbance(floor), 50);
    assert_eq!(sim.next_retraction_tick, Some(sim.tick + 90));
    assert!(before_due > sim.next_retraction_tick.unwrap());
    assert_eq!(sim.cooldown, ARCHITECT_COOLDOWN_TICKS);
    assert!(sim.deck.hand.iter().all(|held| held.id != card));
    assert!(sim.events.iter().any(|event| {
        event.cell == Some(target)
            && event.kind == LabEventKind::Warning
            && event.message.contains("Instability surge")
    }));
}

#[test]
fn surge_cannot_touch_warded_or_void_cells_and_does_not_bypass_protection() {
    let mut sim = unstable();
    sim.cooldown = 0;
    sim.deck = Deck::rogue(92, sim.world.config.levels, &TileShape::ALL);
    assert!(sim.deck.stage_kind(CardKind::Surge));
    let card = sim
        .deck
        .hand
        .iter()
        .find(|card| card.kind == CardKind::Surge)
        .unwrap()
        .id;
    let target = sim.next_retraction().unwrap();
    let play = ArchitectCommand::Play {
        card,
        target,
        rotation: 0,
    };
    sim.observed.insert(target);
    assert_eq!(sim.refusal(play), Some(CommandRefusal::Observed));
    sim.observed.remove(&target);
    sim.anchored.insert(target);
    assert_eq!(sim.refusal(play), Some(CommandRefusal::Anchored));
    sim.anchored.remove(&target);
    let void = *sim
        .mutable_targets()
        .iter()
        .find(|cell| sim.world.placements[cell].space.unbuilt())
        .expect("an unbuilt mutable target");
    assert_eq!(
        sim.refusal(ArchitectCommand::Play {
            card,
            target: void,
            rotation: 0,
        }),
        Some(CommandRefusal::VoidTarget)
    );
    sim.observed.extend(sim.contradictions.iter().copied());
    let before_due = sim.next_retraction_tick;
    let other = sim
        .mutable_targets()
        .into_iter()
        .find(|cell| {
            sim.refusal(ArchitectCommand::Play {
                card,
                target: *cell,
                rotation: 0,
            })
            .is_none()
        })
        .expect("another unprotected target");
    // Keep the pressure below wave threshold so no new Guardian refreshes the
    // synthetic observation mask used to exercise retraction protection.
    sim.economy.set_disturbance(other.level, 0);
    assert_eq!(sim.next_retraction(), None);
    sim.submit(ArchitectCommand::Play {
        card,
        target: other,
        rotation: 0,
    })
    .unwrap();
    assert_eq!(sim.next_retraction_tick, before_due);
    assert_eq!(sim.next_retraction(), None);
}

#[test]
fn rogue_bot_uses_a_surge_only_when_it_detects_prey() {
    let mut sim = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
    sim.guardians.clear();
    sim.deck = Deck::rogue(93, sim.world.config.levels, &TileShape::ALL);
    assert!(sim.deck.stage_kind(CardKind::Surge));
    sim.deck.hand.retain(|card| card.kind == CardKind::Surge);
    let (blind, _) = sim.rogue_intent();
    assert_eq!(blind, None);

    let prey = sim.observers.values().next().unwrap().cell;
    sim.sensors.insert(prey, sim.tick);
    let (sighted, _) = sim.rogue_intent();
    let ArchitectCommand::Play { card, target, .. } =
        sighted.expect("the Rogue can surge near prey")
    else {
        panic!("the Rogue must play its surge card");
    };
    assert_eq!(sim.deck.hand[0].id, card);
    assert_eq!(target.level, prey.level);
    assert!(observed_hex::travel_distance(target, prey) <= 6);
    assert_eq!(
        sim.refusal(ArchitectCommand::Play {
            card,
            target,
            rotation: 0,
        }),
        None
    );
}

#[test]
fn warning_precedes_retraction_and_retractions_are_spaced() {
    let mut sim = unstable();
    let first = sim.next_retraction().unwrap();
    assert_eq!(sim.next_retraction_tick, Some(RETRACTION_TICKS));
    sim.tick = RETRACTION_TICKS - 1;
    sim.advance_retraction();
    assert!(sim.retracted.is_empty());
    sim.tick += 1;
    sim.advance_retraction();
    assert!(sim.retracted.contains(&first));
    let count = sim.retracted.len();
    sim.advance_retraction();
    assert_eq!(
        sim.retracted.len(),
        count,
        "one tile per interval, never one per render frame"
    );
    if !sim.contradictions.is_empty() {
        assert_eq!(sim.next_retraction_tick, Some(RETRACTION_TICKS * 2));
    }
}

#[test]
fn every_protection_holds_against_retraction() {
    let original = unstable();
    let target = original.next_retraction().unwrap();
    for protection in 0..4 {
        let mut sim = original.clone();
        match protection {
            0 => {
                sim.observed.insert(target);
            }
            1 => {
                sim.anchored.insert(target);
            }
            2 => {
                sim.prison_core.insert(target);
            }
            _ => {
                let face = HexFace::LATERAL
                    .into_iter()
                    .find(|&face| sim.threshold_key(target, face).is_some())
                    .unwrap();
                sim.doors
                    .insert(sim.threshold_key(target, face).unwrap(), DoorState::Open);
            }
        }
        sim.tick = RETRACTION_TICKS;
        sim.advance_retraction();
        assert!(!sim.retracted.contains(&target), "protection {protection}");
        assert_ne!(sim.world.placements[&target].space, HexSpace::Void);
    }
}

#[test]
fn compatible_card_repairs_pending_collapse_and_rebuilds_removed_tiles() {
    let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    let opening = sim.events.front().unwrap().cell.unwrap();
    // Build the missing connection through the ordinary legal command boundary.
    let command = sim
        .legal_commands()
        .into_iter()
        .find(|command| {
            if let ArchitectCommand::Play { target, .. } = command {
                *target == opening && {
                    let mut next = sim.clone();
                    next.submit(*command).unwrap();
                    next.contradictions.is_empty()
                }
            } else {
                false
            }
        })
        .unwrap();
    sim.submit(command).unwrap();
    let original = sim.world.placements[&opening];
    // The same piece is offered from the finite deck after a collapse.
    sim.world.placements.get_mut(&opening).unwrap().space = HexSpace::Void;
    sim.world.placements.get_mut(&opening).unwrap().doors = 0;
    sim.retracted.insert(opening);
    sim.refresh_contradictions();
    sim.sync_retraction_clock();
    assert!(!sim.contradictions.is_empty());
    sim.cooldown = 0;
    let shape = TileShape::ALL
        .into_iter()
        .find(|shape| (0..6).any(|r| shape.doors(r) == original.doors))
        .unwrap();
    sim.deck.offer_tile(shape, District::GROUND);
    let rotation = (0..6).find(|&r| shape.doors(r) == original.doors).unwrap();
    let repair = sim.selected_command(0, opening, rotation).unwrap();
    sim.submit(repair).unwrap();
    assert!(!sim.retracted.contains(&opening));
    assert!(sim.contradictions.is_empty());
    assert_eq!(sim.next_retraction_tick, None);
    assert!(
        sim.events
            .iter()
            .any(|event| event.kind == LabEventKind::Repaired)
    );
}

#[test]
fn empty_floor_closes_permanently_but_prison_survives() {
    let mut sim = unstable();
    let target = sim.next_retraction().unwrap();
    for (&cell, tile) in &mut sim.world.placements {
        if cell.level == target.level && cell != target && !sim.prison_core.contains(&cell) {
            tile.space = HexSpace::Void;
            tile.doors = 0;
        }
    }
    sim.observed.clear();
    sim.anchored.clear();
    sim.guardians.clear();
    sim.observers.clear();
    sim.doors.clear();
    sim.contradictions = BTreeSet::from([target]);
    sim.tick = RETRACTION_TICKS;
    sim.advance_retraction();
    assert!(sim.collapsed_floors.contains(&target.level));
    assert!(
        sim.prison_core
            .iter()
            .all(|cell| sim.world.placements[cell].space.built())
    );
    sim.cooldown = 0;
    let command = sim.selected_command(0, target, 0).unwrap();
    assert_eq!(sim.refusal(command), Some(CommandRefusal::CollapsedFloor));
    assert!(
        !sim.deck
            .hand
            .iter()
            .any(|card| card.district == Some(sim.district(target.level)))
    );
}

#[test]
fn one_lost_floor_does_not_retire_cards_for_another_floor_of_its_district() {
    let mut sim = ArchitectLab::for_mode(ArchitectMode::DeepStack).unwrap();
    // Two floors sharing a district on the climb.
    let levels = sim.world.config.levels;
    let (lost, twin) = (0..levels)
        .flat_map(|a| ((a + 1)..levels).map(move |b| (a, b)))
        .find(|&(a, b)| sim.district(a) == sim.district(b))
        .expect("Deep Stack has two floors of one district");
    let district = sim.district(lost);
    let deck = sim.deck.clone();
    sim.collapsed_floors.insert(lost);
    sim.retire_closed_district(lost);
    assert_eq!(
        sim.deck,
        deck,
        "floor {twin} still uses {} cards",
        district.label()
    );

    sim.collapsed_floors.insert(twin);
    sim.retire_closed_district(twin);
    assert_ne!(sim.deck, deck);
    assert!(!sim.deck.offer_any_tile(district));
}

#[test]
fn rejected_commands_are_atomic_and_no_op_cards_are_refused() {
    let sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    let mut applied = sim.clone();
    applied.submit(sim.legal_commands()[0]).unwrap();
    let before = applied.clone();
    let command = applied
        .selected_command(0, *applied.known.first().unwrap(), 0)
        .unwrap();
    assert_eq!(applied.submit(command), Err(CommandRefusal::Cooldown));
    assert_eq!(applied.world, before.world);
    assert_eq!(applied.deck, before.deck);
    assert_eq!(applied.command_log, before.command_log);
    assert_eq!(applied.next_retraction_tick, before.next_retraction_tick);

    let mut sim = sim;
    let target = sim
        .mutable_targets()
        .into_iter()
        .find(|cell| !sim.retraction_protected(*cell) && !sim.occupied().contains(cell))
        .unwrap();
    sim.world.placements.get_mut(&target).unwrap().space = HexSpace::Hall;
    sim.world.placements.get_mut(&target).unwrap().doors = TileShape::Corridor.doors(0);
    sim.deck.offer_tile(TileShape::Corridor, District::GROUND);
    assert_eq!(
        sim.refusal(sim.selected_command(0, target, 0).unwrap()),
        Some(CommandRefusal::NoChange)
    );
}

#[test]
fn pocket_needs_a_card_and_the_bot_completes_the_same_loop() {
    let mut idle = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    assert!(
        idle.deck
            .hand
            .iter()
            .all(|card| card.district.is_none_or(|d| d == District::GROUND))
    );
    for _ in 0..90 {
        idle.step_beat();
    }
    assert_eq!(idle.outcome, MatchOutcome::LoyalVictory);
    let mut bot = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    bot.bot_architect = true;
    for _ in 0..90 {
        bot.step_beat();
    }
    assert_eq!(bot.outcome, MatchOutcome::LoyalVictory);
    assert!(!bot.command_log.is_empty());
    let mut replay = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    while replay.tick < bot.tick {
        // Bot commands are issued on the actor beat before actors move. Match that order.
        replay.bot_architect = true;
        replay.tick();
    }
    assert_eq!(replay.command_log, bot.command_log);
    assert_eq!(replay.world, bot.world);
    assert_eq!(replay.events, bot.events);
}

#[test]
fn sealed_vertical_faces_never_become_traversable_just_because_they_match() {
    let mut sim = ArchitectLab::for_mode(ArchitectMode::QuickClimb).unwrap();
    let lower = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let upper = HexCoord { level: 1, ..lower };
    for cell in [lower, upper] {
        let tile = sim.world.placements.get_mut(&cell).unwrap();
        tile.space = HexSpace::Hall;
        tile.up = observed_hex::PortClass::Sealed;
        tile.down = observed_hex::PortClass::Sealed;
    }
    assert!(!sim.exits(lower).contains(&upper));
    sim.world.placements.get_mut(&lower).unwrap().up = observed_hex::PortClass::RampOpen;
    sim.world.placements.get_mut(&upper).unwrap().down = observed_hex::PortClass::RampOpen;
    assert!(sim.exits(lower).contains(&upper));
    sim.world.placements.get_mut(&upper).unwrap().down = observed_hex::PortClass::Sealed;
    assert!(!sim.exits(lower).contains(&upper));
    sim.refresh_contradictions();
    assert!(sim.contradictions.contains(&lower));
    assert!(sim.contradictions.contains(&upper));
}

#[test]
fn every_shape_rotation_moves_each_port_to_the_next_clockwise_face() {
    for shape in TileShape::ALL {
        for rotation in 0..6 {
            let actual = shape.doors(rotation);
            let expected = HexFace::LATERAL
                .into_iter()
                .filter(|face| shape.base_doors() & (1 << face.index()) != 0)
                .fold(0, |mask, face| {
                    mask | (1 << ((face.index() + usize::from(rotation)) % 6))
                });
            assert_eq!(actual, expected, "{shape:?} rotation {rotation}");
            assert_eq!(shape.doors(rotation + 6), actual);
        }
    }
}
