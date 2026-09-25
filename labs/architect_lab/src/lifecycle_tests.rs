//! Lifecycle checks stay with the lab shell after rules promotion.
use crate::sim::{
    ArchitectCommand, ArchitectLab, ArchitectMode, HAND_SIZE, MatchOutcome, ObserverId,
    ObserverState, PowerPolicy,
};
#[cfg(feature = "desktop")]
use observed_match::ascent::sim::DARKNESS_BEATS;
#[test]
fn power_policy_enforcement_and_reset_invariance() {
    // 1. AlwaysOn policy: cut_floor_power is ignored and toggle_generator refuses to cut power
    let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
        .expect("pocket solves")
        .with_power_policy(PowerPolicy::AlwaysOn);
    let obs_id = *lab.observers.keys().next().expect("observer exists");
    let gen_cell = lab.economy.generators[&0];
    lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
    assert!(!lab.cut_floor_power(0));
    assert!(lab.economy.is_powered(0));
    assert!(lab.toggle_generator(obs_id).is_err());
    assert!(lab.economy.is_powered(0));

    // 2. OneWay policy: power can be cut, but toggle_generator cannot restore it
    let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
        .expect("pocket solves")
        .with_power_policy(PowerPolicy::OneWay);
    lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
    assert!(lab.cut_floor_power(0));
    assert!(!lab.economy.is_powered(0));
    assert!(lab.toggle_generator(obs_id).is_err());
    assert!(!lab.economy.is_powered(0));
    let (intent, trace) = lab.observer_intent(obs_id);
    assert_ne!(intent, crate::sim::ObserverIntent::ToggleGenerator);
    assert_ne!(trace.selected, Some("restore floor power at generator"));

    // 3. Restorable policy: power can be cut and restored
    let mut lab = ArchitectLab::for_mode(ArchitectMode::Pocket)
        .expect("pocket solves")
        .with_power_policy(PowerPolicy::Restorable);
    lab.observers.get_mut(&obs_id).unwrap().cell = gen_cell;
    assert!(lab.cut_floor_power(0));
    assert!(!lab.economy.is_powered(0));
    lab.guardians.clear();
    let (intent, trace) = lab.observer_intent(obs_id);
    assert_eq!(intent, crate::sim::ObserverIntent::ToggleGenerator);
    assert_eq!(trace.selected, Some("restore floor power at generator"));
    assert!(lab.toggle_generator(obs_id).is_ok());
    assert!(lab.economy.is_powered(0));

    // Reset Path 1: desktop.rs (LabSession::reset and cycle_mode)
    #[cfg(feature = "desktop")]
    {
        let mut session = crate::desktop::LabSession::default();
        session.set_power_policy(PowerPolicy::OneWay);
        session.reset();
        assert_eq!(session.sim.power_policy, PowerPolicy::OneWay);
    }

    // Reset Path 2: view.rs (MapCameraState::reset_for_mode)
    #[cfg(feature = "desktop")]
    {
        let mut camera = crate::view::MapCameraState::default();
        camera.zoom = 2.5;
        camera.reset_for_mode(ArchitectMode::Pocket);
        assert!((camera.zoom - crate::view::DEFAULT_ZOOM).abs() < f32::EPSILON);
    }

    // Reset Path 3: web.rs (RogueGame::reset)
    #[cfg(feature = "web")]
    {
        let mut game = crate::web::RogueGame::new(0).unwrap();
        game.set_power_policy("one_way").unwrap();
        game.reset(0).unwrap();
        assert_eq!(game.power_policy(), "One-Way");
    }
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
        assert!((camera.zoom - crate::view::DEFAULT_ZOOM).abs() < f32::EPSILON);
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
#[cfg(feature = "desktop")]
fn all_three_reset_paths_clear_and_recreate_prison_state_without_leaking() {
    use crate::desktop::LabSession;
    use crate::sim::{ArchitectLab, ObserverId, ObserverState};

    // 1. Sim direct reset / recreation
    let mut sim = ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("sim boots");
    sim.jail(ObserverId(0));
    assert_eq!(sim.observers[&ObserverId(0)].state, ObserverState::Jailed);

    sim = ArchitectLab::for_mode(ArchitectMode::FullAscent).expect("sim resets");
    assert_eq!(
        sim.observers[&ObserverId(0)].state,
        ObserverState::Active,
        "reset restored fresh ObserverState"
    );
    assert_eq!(sim.prison_core.len(), 11);

    // 2. Desktop session reset
    let mut desktop = LabSession::default();
    desktop.sim.jail(ObserverId(0));
    assert_eq!(
        desktop.sim.observers[&ObserverId(0)].state,
        ObserverState::Jailed
    );

    desktop.reset();
    assert_eq!(
        desktop.sim.observers[&ObserverId(0)].state,
        ObserverState::Active,
        "desktop reset restored active observers"
    );
    assert!(!desktop.sim.prison_core.is_empty());

    // 3. Web session reset (gated by web feature if present)
    #[cfg(feature = "web")]
    {
        let mut web = crate::web::RogueGame::new(2).expect("web boots");
        web.sim.jail(ObserverId(0));
        assert_eq!(
            web.sim.observers[&ObserverId(0)].state,
            ObserverState::Jailed
        );

        web.reset(2).expect("web reset");
        assert_eq!(
            web.sim.observers[&ObserverId(0)].state,
            ObserverState::Active,
            "web reset restored active observers"
        );
        assert_eq!(web.sim.prison_core.len(), 11);
    }
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

#[test]
fn reset_paths_clear_the_darkness_hold_without_leaks() {
    // Path 1: desktop.rs (LabSession::reset)
    #[cfg(feature = "desktop")]
    {
        use crate::desktop::{ArchitectAction, LabSession};
        let mut session = LabSession::default();
        session.sim.darkness.streak = 9;
        session.sim.darkness.longest = 9;
        session.sim.darkness.total_held = 40;
        session.sim.darkness.completed_at = Some(600);
        session.sim.lit_sightlines = 7;
        session.apply_action(ArchitectAction::Reset);
        assert_eq!(session.sim.darkness.streak, 0);
        assert_eq!(session.sim.darkness.longest, 0);
        assert_eq!(session.sim.darkness.total_held, 0);
        assert_eq!(session.sim.darkness.completed_at, None);
        assert_eq!(session.sim.darkness.required, DARKNESS_BEATS);
    }

    // Path 2: view.rs (MapCameraState::reset_for_mode) holds no simulation state; the
    // lab it draws is rebuilt through path 1 or 3.

    // Path 3: web.rs (RogueGame::reset)
    #[cfg(feature = "web")]
    {
        use crate::web::RogueGame;
        let mut game = RogueGame::new(0).unwrap();
        game.sim.darkness.streak = 4;
        game.sim.darkness.completed_at = Some(120);
        game.reset(0).expect("web reset succeeds");
        assert_eq!(game.sim.darkness.streak, 0);
        assert_eq!(game.sim.darkness.completed_at, None);
    }
}

#[cfg(feature = "web")]
#[test]
fn web_reset_restores_prison_and_jailed_observers() {
    let mut game = crate::web::RogueGame::new(2).unwrap();
    game.sim.jail(ObserverId(0));
    assert_eq!(
        game.sim.observers[&ObserverId(0)].state,
        ObserverState::Jailed
    );
    game.reset(2).unwrap();
    assert_eq!(
        game.sim.observers[&ObserverId(0)].state,
        ObserverState::Active
    );
    assert_eq!(game.sim.prison_core.len(), 11);
}
