use super::*;

fn menu_app() -> App {
    let mut app = App::new();
    app.insert_resource(PlaySetupDraft::default())
        .init_resource::<HexLaunchRequestSequence>()
        .init_resource::<NextState<GameState>>()
        .add_observer(activate_hub)
        .add_systems(Startup, setup_hub)
        .add_systems(Update, refresh_hub);
    app.update();
    app
}

fn choose(app: &mut App, wanted: PlayAction) {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &PlayAction)>();
    let entity = query
        .iter(world)
        .find_map(|(entity, action)| (*action == wanted).then_some(entity))
        .unwrap();
    world.trigger(Activate { entity });
    app.update();
}

#[test]
fn spectate_and_race_disable_roles_without_forgetting_the_playable_role() {
    let mut app = menu_app();
    choose(&mut app, PlayAction::SelectSeat(PlaySeat::Architect));
    choose(&mut app, PlayAction::SelectPreset(PlayPreset::Spectate));
    choose(&mut app, PlayAction::SelectSeat(PlaySeat::Observer));
    assert_eq!(
        app.world().resource::<PlaySetupDraft>().seat,
        PlaySeat::Architect
    );
    let world = app.world_mut();
    let mut roles = world.query::<(&PlayAction, Option<&InteractionDisabled>, Option<&TabIndex>)>();
    for (action, disabled, tab) in roles.iter(world) {
        if matches!(action, PlayAction::SelectSeat(_)) {
            assert!(disabled.is_some());
            assert!(tab.is_none());
        }
    }
    choose(&mut app, PlayAction::SelectPreset(PlayPreset::CoOp));
    assert!(roles_available(app.world().resource::<PlaySetupDraft>()));
    choose(&mut app, PlayAction::SelectRules(PlayRules::Race));
    choose(&mut app, PlayAction::SelectSeat(PlaySeat::Observer));
    choose(&mut app, PlayAction::SelectRules(PlayRules::Ascent));
    assert_eq!(
        app.world().resource::<PlaySetupDraft>().seat,
        PlaySeat::Architect
    );
}

#[test]
fn summary_uses_final_local_roster_and_distinguishes_architect_from_bodies() {
    let mut setup = PlaySetupDraft {
        teams: 4,
        members_per_team: 4,
        preset: PlayPreset::Custom,
        seat: PlaySeat::Architect,
        ..PlaySetupDraft::default()
    };
    let summary = play_summary(&setup);
    assert!(summary.contains("1 team x 1 Observer body"));
    assert!(summary.contains("You: Architect desk"));
    assert!(summary.contains("1 bot Observer body"));
    setup.fill_empty_seats = true;
    assert!(play_summary(&setup).contains("4 teams x 4 Observer bodies"));
    setup.preset = PlayPreset::Spectate;
    let summary = play_summary(&setup);
    assert!(summary.contains("You: bot view"));
    assert!(summary.contains("16 bot Observer bodies"));
    assert!(!summary.contains("LAN still"));
}

#[test]
fn every_rules_role_preset_combination_has_an_explicit_launch_and_summary() {
    for rules in [PlayRules::Race, PlayRules::Ascent] {
        for seat in [PlaySeat::Observer, PlaySeat::Architect] {
            for preset in PlayPreset::ALL {
                let setup = PlaySetupDraft {
                    rules,
                    seat,
                    ..PlaySetupDraft::for_preset(preset)
                };
                let summary = play_summary(&setup);
                assert!(summary.contains(rules.label()));
                assert!(summary.contains("Guardian on"));
                if preset == PlayPreset::Spectate {
                    assert_eq!(launch_label(&setup), "Watch bot match");
                } else if rules == PlayRules::Race {
                    assert_eq!(launch_label(&setup), "Start facility race");
                } else {
                    assert!(launch_label(&setup).contains(match seat {
                        PlaySeat::Observer => "Observer",
                        PlaySeat::Architect => "Architect",
                    }));
                }
            }
        }
    }
}

#[test]
fn advanced_cycles_never_exceed_sixteen_seats() {
    for teams in 1..=16 {
        for size in 1..=16 {
            if u16::from(teams) * u16::from(size) <= 16 {
                let draft = PlaySetupDraft {
                    preset: PlayPreset::Custom,
                    teams,
                    members_per_team: size,
                    fill_empty_seats: true,
                    guardian: true,
                    rules: crate::play_setup::PlayRules::Race,
                    seat: crate::play_setup::PlaySeat::Observer,
                };
                assert!(draft.validate().is_ok());
            }
        }
    }
}

#[test]
fn launch_finalizes_the_visible_no_fill_roster_and_spectator_perspective() {
    let mut app = menu_app();
    app.insert_resource(PlaySetupDraft {
        preset: PlayPreset::Custom,
        teams: 4,
        members_per_team: 4,
        guardian: false,
        seat: PlaySeat::Architect,
        ..PlaySetupDraft::default()
    });
    choose(&mut app, PlayAction::Launch);
    let request = app
        .world()
        .resource::<crate::hex_wfc::loading::HexLaunchRequest>();
    assert_eq!(
        (
            request.spec.config.teams,
            request.spec.config.members_per_team
        ),
        (1, 1)
    );
    assert!(!request.spec.config.guardian);
    assert!(!request.spectator);
    choose(&mut app, PlayAction::SelectPreset(PlayPreset::Spectate));
    choose(&mut app, PlayAction::Launch);
    let request = app
        .world()
        .resource::<crate::hex_wfc::loading::HexLaunchRequest>();
    assert_eq!(
        (
            request.spec.config.teams,
            request.spec.config.members_per_team
        ),
        (2, 2)
    );
    assert!(request.spectator);
}
