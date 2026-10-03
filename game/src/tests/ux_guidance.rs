//! Assembled-app coverage for the next step, spectator input ownership and map guide.
use crate::hex_wfc::{
    architect::ArchitectDesk,
    ask::AskTheArchitect,
    overlay::MatchOverlayState,
    sim::{HexWfcIntent, HexWfcRuntime},
    view::map::{HexMapLegend, HexMapProjection, HexMapVisual},
};
use crate::play_setup::{PlayPreset, PlayRules, PlaySetupDraft};
use crate::screens::widgets::UiInputCapture;
use crate::sim::state::SpectatorBot;
use crate::tests::{all_texts, count, go, tap_update, test_app};
use crate::{GameState, flow::MATCH_SEED};
use bevy::prelude::*;

fn play_app(spectator: bool) -> App {
    let mut app = test_app();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::ZERO,
    ));
    app.insert_resource(crate::flow::ActiveMatchSeed(MATCH_SEED));
    app.world_mut()
        .resource_mut::<crate::settings::Settings>()
        .complete_help(crate::settings::OnboardingKind::Observer);
    app.insert_resource(PlaySetupDraft {
        rules: PlayRules::Ascent,
        guardian: false,
        ..PlaySetupDraft::for_preset(PlayPreset::TeamRace)
    });
    if spectator {
        app.insert_resource(SpectatorBot::for_seed(MATCH_SEED));
    }
    go(&mut app, GameState::HexWfc);
    assert!(app.world().resource::<HexWfcRuntime>().ascent.is_some());
    app
}

fn legend(app: &mut App) -> String {
    let world = app.world_mut();
    world
        .query_filtered::<&Text, With<HexMapLegend>>()
        .single(world)
        .expect("one map legend")
        .0
        .clone()
}

#[test]
fn spectator_camera_controls_follow_bodies_but_never_issue_observer_asks() {
    let mut app = play_app(true);
    let before = app.world().resource::<HexWfcRuntime>().local_player;
    let bodies: Vec<_> = app
        .world()
        .resource::<HexWfcRuntime>()
        .match_state
        .players
        .keys()
        .copied()
        .collect();
    tap_update(&mut app, KeyCode::KeyF);
    assert_ne!(app.world().resource::<HexWfcRuntime>().local_player, before);
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text.contains("WATCHING OBSERVER"))
    );
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text.contains("CHASE CAMERA"))
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let mut pad = Gamepad::default();
    pad.digital_mut().press(GamepadButton::DPadLeft);
    let pad_entity = app.world_mut().spawn(pad).id();
    app.world_mut().run_schedule(Update);
    assert_eq!(
        app.world().resource::<HexWfcRuntime>().local_player,
        bodies[2 % bodies.len()],
        "follows the next body in stable ID order"
    );
    assert!(
        !app.world().resource::<AskTheArchitect>().pending,
        "spectator D-pad left must not ask for a bot"
    );
    assert!(app.world().resource::<HexWfcIntent>().intent.is_neutral());
    *app.world_mut()
        .get_mut::<Gamepad>(pad_entity)
        .expect("pad")
        .digital_mut() = ButtonInput::default();
    app.world_mut()
        .get_mut::<Gamepad>(pad_entity)
        .expect("pad")
        .digital_mut()
        .press(GamepadButton::North);
    app.world_mut().run_schedule(Update);
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text == "SPECTATING / OVERVIEW")
    );
    *app.world_mut()
        .get_mut::<Gamepad>(pad_entity)
        .expect("pad")
        .digital_mut() = ButtonInput::default();
    app.world_mut()
        .get_mut::<Gamepad>(pad_entity)
        .expect("pad")
        .digital_mut()
        .press(GamepadButton::West);
    app.world_mut().run_schedule(Update);
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text == "SPECTATING / OBSERVER'S EYES")
    );
    for button in [GamepadButton::North, GamepadButton::West] {
        *app.world_mut()
            .get_mut::<Gamepad>(pad_entity)
            .expect("pad")
            .digital_mut() = ButtonInput::default();
        app.world_mut()
            .get_mut::<Gamepad>(pad_entity)
            .expect("pad")
            .digital_mut()
            .press(button);
        app.world_mut().run_schedule(Update);
    }
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text == "SPECTATING / OBSERVER'S EYES"),
        "selecting eyes from overview always returns to eyes"
    );
}

#[test]
fn spectator_controls_yield_to_pause_map_and_exclusive_capture() {
    let mut app = play_app(true);
    let before = app.world().resource::<HexWfcRuntime>().local_player;
    for overlay in [
        MatchOverlayState::Pause(crate::hex_wfc::overlay::PausePage::Root),
        MatchOverlayState::SurvivorMap,
    ] {
        *app.world_mut().resource_mut::<MatchOverlayState>() = overlay;
        tap_update(&mut app, KeyCode::KeyF);
        assert_eq!(app.world().resource::<HexWfcRuntime>().local_player, before);
    }
    *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::Playing;
    app.world_mut()
        .resource_mut::<UiInputCapture>()
        .capture("ux_test");
    tap_update(&mut app, KeyCode::KeyF);
    assert_eq!(app.world().resource::<HexWfcRuntime>().local_player, before);
}

#[test]
fn following_a_corrupted_bot_does_not_claim_a_human_rogue_desk() {
    let mut app = play_app(true);
    let local = app.world().resource::<HexWfcRuntime>().local_player;
    app.world_mut()
        .resource_mut::<HexWfcRuntime>()
        .match_state
        .drop_into_void(local);
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(Update);
    assert!(!app.world().contains_resource::<ArchitectDesk>());
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text == "Jail every remaining loyal Observer")
    );
}

#[test]
fn map_reading_guide_updates_without_rebuilding_geometry_and_cleans_up() {
    let mut app = play_app(true);
    *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::SurvivorMap;
    app.world_mut().run_schedule(Update);
    let before = legend(&mut app);
    assert!(before.starts_with("TEAM MAP / FLOOR 1"));
    assert!(before.contains("Known floors: [1]"));
    assert!(!before.contains("Stale cells"));
    let world = app.world_mut();
    let visuals: Vec<_> = world
        .query_filtered::<Entity, With<HexMapVisual>>()
        .iter(world)
        .collect();
    tap_update(&mut app, KeyCode::KeyH);
    let expanded = legend(&mut app);
    assert!(expanded.contains("Stale cells may have changed"));
    let world = app.world_mut();
    let after: Vec<_> = world
        .query_filtered::<Entity, With<HexMapVisual>>()
        .iter(world)
        .collect();
    assert_eq!(visuals, after, "reading guide must not rebuild map meshes");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let mut pad = Gamepad::default();
    pad.digital_mut().press(GamepadButton::West);
    let pad_entity = app.world_mut().spawn(pad).id();
    app.world_mut().run_schedule(Update);
    assert!(
        !legend(&mut app).contains("Stale cells"),
        "X closes the reading guide without changing the spectator view"
    );
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text == "SPECTATING / CHASE CAMERA")
    );
    *app.world_mut()
        .get_mut::<Gamepad>(pad_entity)
        .expect("pad")
        .digital_mut() = ButtonInput::default();
    app.world_mut()
        .resource_mut::<crate::settings::Settings>()
        .gameplay_text_scale = 1.25;
    app.world_mut().run_schedule(Update);
    let world = app.world_mut();
    let font = world
        .query_filtered::<&TextFont, With<HexMapLegend>>()
        .single(world)
        .expect("legend font");
    assert_eq!(font.font_size, FontSize::Px(17.5));
    *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::Playing;
    app.world_mut().run_schedule(Update);
    *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::SurvivorMap;
    app.world_mut().run_schedule(Update);
    assert!(
        !legend(&mut app).contains("Stale cells"),
        "reopening starts with the short guide"
    );
    go(&mut app, GameState::MainMenu);
    assert!(!app.world().contains_resource::<HexMapProjection>());
    assert_eq!(count::<HexMapVisual>(&mut app), 0);
    assert_eq!(count::<HexMapLegend>(&mut app), 0);
}

#[test]
fn a_spectator_can_browse_known_floors_without_moving_the_followed_body() {
    let mut app = play_app(true);
    *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::SurvivorMap;
    tap_update(&mut app, KeyCode::PageUp);
    let intent = app.world().resource::<HexWfcIntent>();
    assert_eq!(intent.browse_map_level, 1);
    assert!(intent.intent.is_neutral());
    app.world_mut()
        .resource_mut::<UiInputCapture>()
        .capture("ux_test");
    tap_update(&mut app, KeyCode::PageUp);
    assert_eq!(
        app.world().resource::<HexWfcIntent>().browse_map_level,
        0,
        "exclusive input capture blocks browsing too"
    );
}

fn review_from_pause(app: &mut App) {
    *app.world_mut().resource_mut::<MatchOverlayState>() =
        MatchOverlayState::Pause(crate::hex_wfc::overlay::PausePage::Root);
    app.world_mut().run_schedule(Update);
    let world = app.world_mut();
    let entity = world
        .query::<(Entity, &crate::screens::widgets::WidgetId)>()
        .iter(world)
        .find(|(_, id)| **id == crate::screens::widgets::WidgetId::named("hex.pause.role_help"))
        .expect("role-help button")
        .0;
    world.trigger(bevy::ui_widgets::Activate { entity });
    app.world_mut().run_schedule(Update);
}

#[test]
fn pause_help_describes_the_current_rogue_or_spectator_role() {
    let mut app = play_app(true);
    review_from_pause(&mut app);
    assert_eq!(
        app.world()
            .resource::<crate::screens::onboarding::OnboardingState>()
            .kind,
        crate::settings::OnboardingKind::Spectator
    );
    let mut app = play_app(false);
    let local = app.world().resource::<HexWfcRuntime>().local_player;
    app.world_mut()
        .resource_mut::<HexWfcRuntime>()
        .match_state
        .drop_into_void(local);
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(Update);
    assert!(app.world().contains_resource::<ArchitectDesk>());
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text.contains("Jail every remaining loyal Observer to win"))
    );
    review_from_pause(&mut app);
    assert_eq!(
        app.world()
            .resource::<crate::screens::onboarding::OnboardingState>()
            .kind,
        crate::settings::OnboardingKind::Rogue
    );
}

#[test]
fn a_spectator_map_keeps_the_followed_bot_on_its_normal_driver() {
    let mut viewing = play_app(true);
    let mut mapping = play_app(true);
    *mapping.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::SurvivorMap;
    mapping.world_mut().run_schedule(Update);
    let poses = |app: &App| {
        app.world()
            .resource::<HexWfcRuntime>()
            .match_state
            .players
            .values()
            .map(|player| {
                (
                    player.id,
                    player.cell,
                    player.position,
                    player.yaw,
                    player.place,
                )
            })
            .collect::<Vec<_>>()
    };
    let initial = poses(&viewing);
    assert_eq!(initial, poses(&mapping));
    for _ in 0..180 {
        viewing.world_mut().run_schedule(FixedUpdate);
        mapping.world_mut().run_schedule(FixedUpdate);
    }
    assert_ne!(
        initial,
        poses(&viewing),
        "the proof exercises a moving bot run"
    );
    assert_eq!(
        poses(&viewing),
        poses(&mapping),
        "opening the spectator map must not change any bot's physical run"
    );
}

#[test]
fn reopened_help_reads_online_continuity_from_the_active_match() {
    let mut app = play_app(false);
    app.world_mut().resource_mut::<HexWfcRuntime>().networked = true;
    review_from_pause(&mut app);
    assert!(
        all_texts(&mut app)
            .iter()
            .any(|text| text.contains("LAN MATCH CONTINUES"))
    );
}
