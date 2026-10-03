use super::*;

fn spawn_app(game_state: GameState, settings: Settings) -> App {
    let mut app = App::new();
    app.insert_resource(State::new(game_state))
        .insert_resource(settings)
        .insert_resource(PlaySetupDraft::for_preset(PlayPreset::Solo))
        .insert_resource(crate::lan::LanRuntime::new())
        .insert_resource(HexOnboardingGate::default())
        .init_resource::<UiInputCapture>()
        .add_observer(activate)
        .add_systems(Startup, spawn);
    app.update();
    app
}

fn action_entity(app: &mut App, wanted: OnboardingAction) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &OnboardingAction)>();
    query
        .iter(world)
        .find_map(|(entity, action)| (*action == wanted).then_some(entity))
        .expect("onboarding action exists")
}

#[test]
fn ascent_help_is_specific_to_the_selected_role_and_actual_bindings() {
    let mut settings = Settings::default();
    settings.bindings.ask = KeyCode::KeyY;
    let observer = onboarding_beats(&settings, OnboardingKind::Observer);
    let architect = onboarding_beats(&settings, OnboardingKind::Architect);
    assert!(observer[1].body.starts_with("Y or D-pad left asks"));
    assert!(observer[3].body.contains("corruption"));
    assert!(architect[0].body.contains("no body"));
    assert!(architect[1].body.contains("1-5"));
    assert!(!architect.iter().any(|beat| beat.body.contains("keystones")));
    assert!(!observer.iter().any(|beat| beat.body.contains("keystones")));
    for kind in [
        OnboardingKind::Race,
        OnboardingKind::Observer,
        OnboardingKind::Architect,
    ] {
        assert_eq!(onboarding_beats(&settings, kind).len(), 4);
        assert!(
            onboarding_beats(&settings, kind)
                .iter()
                .all(|beat| beat.body.len() < 360)
        );
    }
}

#[test]
fn requested_help_can_reopen_after_completion_and_cleanup_releases_everything() {
    let mut settings = Settings::default();
    settings.complete_onboarding();
    let mut app = spawn_app(GameState::HexWfc, settings);
    assert!(!app.world().contains_resource::<OnboardingState>());
    app.insert_resource(RoleHelpRequest)
        .add_systems(Update, spawn);
    app.update();
    assert!(app.world().contains_resource::<OnboardingState>());
    assert!(!app.world().contains_resource::<RoleHelpRequest>());
    app.add_systems(Update, cleanup);
    app.update();
    assert!(!app.world().contains_resource::<OnboardingState>());
    assert!(!app.world().resource::<HexOnboardingGate>().active);
    assert!(!app.world().resource::<UiInputCapture>().is_active());
}

#[test]
fn beats_are_short_and_reflect_rebindings() {
    let mut settings = Settings::default();
    crate::settings::BindingSlot::MoveForward.set(&mut settings.bindings, KeyCode::KeyJ);
    crate::settings::BindingSlot::Jump.set(&mut settings.bindings, KeyCode::KeyK);
    crate::settings::BindingSlot::Interact.set(&mut settings.bindings, KeyCode::KeyI);
    crate::settings::BindingSlot::Torch.set(&mut settings.bindings, KeyCode::KeyG);
    crate::settings::BindingSlot::TacMap.set(&mut settings.bindings, KeyCode::KeyM);
    crate::settings::BindingSlot::Pause.set(&mut settings.bindings, KeyCode::F10);

    let beats = onboarding_beats(&settings, OnboardingKind::Race);
    assert_eq!(beats.len(), 4);
    assert!(beats[0].body.starts_with("J/"));
    assert!(beats[1].body.contains("K jumps"));
    assert!(beats[2].body.contains("I uses"));
    assert!(beats[2].body.contains("G or left bumper"));
    assert!(beats[3].body.contains("M opens"));
    assert!(beats[3].body.contains("F10 or Start"));
    assert!(beats.iter().all(|beat| beat.body.len() < 360));
}

#[test]
fn spawn_is_hard_gated_to_canonical_hex_state() {
    let mut deprecated = spawn_app(GameState::Match, Settings::default());
    let mut canonical = spawn_app(GameState::HexWfc, Settings::default());
    let mut completed = Settings::default();
    completed.complete_onboarding();
    let mut completed = spawn_app(GameState::HexWfc, completed);

    let deprecated_count = {
        let world = deprecated.world_mut();
        let mut query = world.query::<&OnboardingPanel>();
        query.iter(world).count()
    };
    let canonical_count = {
        let world = canonical.world_mut();
        let mut query = world.query::<&OnboardingPanel>();
        query.iter(world).count()
    };
    let completed_count = {
        let world = completed.world_mut();
        let mut query = world.query::<&OnboardingPanel>();
        query.iter(world).count()
    };
    assert_eq!(deprecated_count, 0);
    assert_eq!(canonical_count, 1);
    assert_eq!(completed_count, 0);
}

#[test]
fn spectator_runs_do_not_receive_participant_onboarding() {
    let mut app = App::new();
    app.insert_resource(State::new(GameState::HexWfc))
        .insert_resource(Settings::default())
        .insert_resource(PlaySetupDraft::for_preset(PlayPreset::Spectate))
        .insert_resource(crate::lan::LanRuntime::new())
        .insert_resource(HexOnboardingGate::default())
        .init_resource::<UiInputCapture>()
        .add_systems(Startup, spawn);
    app.update();

    assert!(!app.world().contains_resource::<OnboardingState>());
    assert!(!app.world().resource::<HexOnboardingGate>().active);
}

#[test]
fn semantic_next_completes_and_persists_the_current_version() {
    let mut app = spawn_app(GameState::HexWfc, Settings::default());
    let next = action_entity(&mut app, OnboardingAction::Next);

    for expected_step in 1..4 {
        app.world_mut().trigger(Activate { entity: next });
        app.update();
        assert_eq!(
            app.world().resource::<OnboardingState>().step,
            expected_step
        );
    }
    app.world_mut().trigger(Activate { entity: next });
    app.update();

    let settings = app.world().resource::<Settings>();
    assert_eq!(
        settings.completed_onboarding_version,
        crate::settings::CURRENT_ONBOARDING_VERSION
    );
    assert!(!settings.needs_onboarding());
    assert!(!app.world().contains_resource::<OnboardingState>());
}

#[test]
fn semantic_skip_completes_immediately() {
    let mut app = spawn_app(GameState::HexWfc, Settings::default());
    let skip = action_entity(&mut app, OnboardingAction::Skip);

    app.world_mut().trigger(Activate { entity: skip });
    app.update();

    assert!(!app.world().resource::<Settings>().needs_onboarding());
    assert!(!app.world().contains_resource::<OnboardingState>());
    assert!(
        app.world().resource::<UiInputCapture>().is_active(),
        "dismissal retains input capture until the triggering edge has passed"
    );
}
