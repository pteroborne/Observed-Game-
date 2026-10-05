//! Completion facts, replay isolation and rematch choices at the assembled-app boundary.
use crate::flow::{AscentResult, AscentResultRole, Career};
use crate::play_setup::{LaunchedPlaySetup, PlayPreset, PlayRules, PlaySeat, PlaySetupDraft};
use crate::screens::{
    replay::ReplayAction,
    results::{ResultsAction, build_ascent_story},
};
use crate::sim::replay::ReplayTape;
use crate::tests::{all_texts, go, test_app};
use crate::{
    GameState,
    hex_wfc::{architect::ArchitectDesk, sim::HexWfcRuntime},
};
use bevy::prelude::*;
use observed_core::TeamId;
use observed_match::ascent::sim::MatchOutcome;

fn facts(role: AscentResultRole, outcome: MatchOutcome, winner: Option<TeamId>) -> AscentResult {
    AscentResult {
        role,
        outcome,
        winner,
        local_team: TeamId(0),
        loyal: 3,
        jailed: 3,
        corrupted: 1,
        rogue_by_capture: true,
    }
}

#[test]
fn ascent_completion_explains_each_faction_role_and_spectator_outcome() {
    for role in [
        AscentResultRole::Observer,
        AscentResultRole::Architect,
        AscentResultRole::Rogue,
        AscentResultRole::Spectator,
    ] {
        for (outcome, winner) in [
            (MatchOutcome::LoyalVictory, Some(TeamId(0))),
            (MatchOutcome::LoyalVictory, Some(TeamId(1))),
            (MatchOutcome::RogueVictory, None),
        ] {
            let facts = facts(role, outcome, winner);
            let story = build_ascent_story(facts, None);
            let won = if role == AscentResultRole::Rogue {
                outcome == MatchOutcome::RogueVictory
            } else {
                outcome == MatchOutcome::LoyalVictory && winner == Some(TeamId(0))
            };
            assert_eq!(
                story.headline,
                if role == AscentResultRole::Spectator {
                    "OBSERVATION COMPLETE"
                } else if won {
                    "VICTORY"
                } else {
                    "RUN ENDED"
                }
            );
            let words = story.lines.join("\n");
            assert!(
                !words.contains("series")
                    && !words.contains("keystone")
                    && !words.contains("absorbed")
            );
            assert!(words.contains(if outcome == MatchOutcome::RogueVictory {
                "jailed or corrupted"
            } else {
                "remaining loyal Observer"
            }));
            assert!(words.contains("No replay moments"));
            if role == AscentResultRole::Spectator {
                assert!(!words.contains("your team") && !words.contains("your faction"));
            }
        }
    }
}

#[test]
fn darkness_completion_does_not_claim_every_observer_was_jailed() {
    let mut facts = facts(AscentResultRole::Rogue, MatchOutcome::RogueVictory, None);
    facts.rogue_by_capture = false;
    facts.jailed = 0;
    let story = build_ascent_story(facts, None);
    assert!(story.lines[0].contains("Darkness"));
    assert!(story.lines[1].contains("unwitnessed"));
    assert!(!story.lines[1].contains("jailed"));
}

#[test]
fn incomplete_ascent_facts_do_not_infer_a_win() {
    let story = build_ascent_story(
        facts(AscentResultRole::Observer, MatchOutcome::Running, None),
        None,
    );
    assert_eq!(story.headline, "RESULTS");
    assert!(story.lines[2].contains("outcome unavailable"));
}

fn result_action(app: &mut App, action: ResultsAction) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &ResultsAction)>()
        .iter(world)
        .find(|(_, a)| **a == action)
        .unwrap()
        .0
}

#[test]
fn absent_and_empty_tapes_disable_replay_and_reject_activation() {
    for empty in [false, true] {
        let mut app = test_app();
        if empty {
            let spec = crate::map_catalog::default_map_spec(1);
            app.insert_resource(ReplayTape::new(1, &spec));
        }
        go(&mut app, GameState::Results);
        let button = result_action(&mut app, ResultsAction::WatchReplay);
        assert!(
            app.world()
                .get::<bevy::ui::InteractionDisabled>(button)
                .is_some()
        );
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: button });
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Results
        );
    }
}

#[test]
fn local_rematch_restores_launched_rules_seat_and_roster() {
    for (preset, seat) in [
        (PlayPreset::CoOp, PlaySeat::Architect),
        (PlayPreset::Spectate, PlaySeat::Observer),
    ] {
        let mut app = test_app();
        let launched = PlaySetupDraft {
            rules: PlayRules::Ascent,
            seat,
            guardian: false,
            ..PlaySetupDraft::for_preset(preset)
        };
        app.insert_resource(LaunchedPlaySetup(launched.clone()));
        app.insert_resource(PlaySetupDraft::for_preset(PlayPreset::Solo));
        app.insert_resource(crate::flow::ActiveMatchSeed(99));
        go(&mut app, GameState::Results);
        let entity = result_action(&mut app, ResultsAction::Rematch);
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Loading
        );
        assert_eq!(*app.world().resource::<PlaySetupDraft>(), launched);
        let request = app
            .world()
            .resource::<crate::hex_wfc::loading::HexLaunchRequest>();
        assert_eq!(request.spectator, preset == PlayPreset::Spectate);
        assert_eq!(request.spec.config.teams, launched.teams);
        assert_eq!(
            request.spec.config.members_per_team,
            launched.members_per_team
        );
        assert!(!request.spec.config.guardian);
        assert_ne!(request.spec.requested_seed, 99);
    }
}

#[test]
fn real_rogue_completion_survives_cleanup_and_replay_never_changes_the_tape() {
    for (seat, spectator, corrupted) in [
        (PlaySeat::Observer, false, false),
        (PlaySeat::Architect, false, false),
        (PlaySeat::Observer, true, false),
        (PlaySeat::Observer, false, true),
        (PlaySeat::Architect, false, true),
    ] {
        let mut app = test_app();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::ZERO,
        ));
        app.insert_resource(PlaySetupDraft {
            rules: PlayRules::Ascent,
            seat,
            guardian: false,
            ..PlaySetupDraft::for_preset(PlayPreset::TeamRace)
        });
        if spectator {
            app.insert_resource(crate::sim::state::SpectatorBot::for_seed(1));
        }
        go(&mut app, GameState::HexWfc);
        {
            let mut runtime = app.world_mut().resource_mut::<HexWfcRuntime>();
            let local = runtime.local_player;
            let players: Vec<_> = runtime.match_state.players.keys().copied().collect();
            for player in players {
                if corrupted && player == local {
                    runtime.match_state.drop_into_void(player);
                } else {
                    runtime.match_state.jail(player);
                }
            }
            for _ in 0..120 {
                let frame = observed_match::hex_wfc::HexInputFrame {
                    tick: runtime.match_state.tick + 1,
                    ..default()
                };
                super::ascent::step(&mut runtime, &frame, None, None);
                if runtime.match_state.status == observed_match::hex_wfc::HexMatchStatus::Finished {
                    break;
                }
            }
            assert_eq!(
                runtime.ascent.as_ref().unwrap().rules().outcome,
                MatchOutcome::RogueVictory
            );
            runtime.results_delay_frames = 89;
        }
        let without_tape = seat == PlaySeat::Architect && corrupted;
        if without_tape {
            app.world_mut().remove_resource::<ReplayTape>();
        }
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Results
        );
        assert!(!app.world().contains_resource::<HexWfcRuntime>());
        assert!(!app.world().contains_resource::<ArchitectDesk>());
        let role = if spectator {
            AscentResultRole::Spectator
        } else if seat == PlaySeat::Architect {
            AscentResultRole::Architect
        } else if corrupted {
            AscentResultRole::Rogue
        } else {
            AscentResultRole::Observer
        };
        let career = app.world().resource::<Career>();
        assert_eq!(career.last_ascent_result.unwrap().role, role);
        let rogue_won = corrupted && seat == PlaySeat::Observer;
        assert_eq!(career.last_result.as_ref().unwrap().local_won, rogue_won);
        assert_eq!(
            career.last_result.as_ref().unwrap().placement,
            rogue_won.then_some(1)
        );
        if without_tape {
            assert!(
                all_texts(&mut app)
                    .join("\n")
                    .contains("You played Architect")
            );
            assert!(!app.world().contains_resource::<ReplayTape>());
            continue;
        }
        let before = app.world().resource::<ReplayTape>().clone();
        assert_eq!(before.ascent_result, career.last_ascent_result);
        assert!(
            before
                .samples
                .last()
                .unwrap()
                .actors
                .iter()
                .all(|pose| pose.status == "jailed" || pose.status == "Rogue")
        );
        assert!(
            before
                .samples
                .last()
                .unwrap()
                .actors
                .iter()
                .all(|pose| pose.room.is_none())
        );
        let entity = result_action(&mut app, ResultsAction::WatchReplay);
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Replay
        );
        assert!(all_texts(&mut app).join("\n").contains("Architect Ascent"));
        for action in [
            ReplayAction::StepForward,
            ReplayAction::NextActor,
            ReplayAction::JumpBack,
            ReplayAction::Back,
        ] {
            let world = app.world_mut();
            let entity = world
                .query::<(Entity, &ReplayAction)>()
                .iter(world)
                .find(|(_, a)| **a == action)
                .unwrap()
                .0;
            world.trigger(bevy::ui_widgets::Activate { entity });
            app.update();
        }
        assert_eq!(*app.world().resource::<ReplayTape>(), before);
        assert_eq!(app.world().resource::<Career>().matches_completed, 1);
    }
}

#[test]
fn lan_results_and_replay_continue_return_to_the_lobby() {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    for replay in [false, true] {
        let mut app = test_app();
        let client = observed_net::lan::LanClient::connect(
            socket.local_addr().unwrap(),
            1,
            None,
            None,
            [0; 32],
        )
        .unwrap();
        app.world_mut()
            .resource_mut::<crate::lan::LanRuntime>()
            .client = Some(client);
        if replay {
            go(&mut app, GameState::Replay);
            assert!(all_texts(&mut app).join("\n").contains("Return to lobby"));
            let world = app.world_mut();
            let entity = world
                .query::<(Entity, &ReplayAction)>()
                .iter(world)
                .find(|(_, a)| **a == ReplayAction::Continue)
                .unwrap()
                .0;
            world.trigger(bevy::ui_widgets::Activate { entity });
        } else {
            go(&mut app, GameState::Results);
            let entity = result_action(&mut app, ResultsAction::Rematch);
            app.world_mut()
                .trigger(bevy::ui_widgets::Activate { entity });
        }
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Lobby
        );
        assert!(
            !app.world()
                .contains_resource::<crate::hex_wfc::loading::HexLaunchRequest>()
        );
    }
}

#[test]
fn summit_completion_reports_the_team_from_observer_architect_and_spectator_seats() {
    for (seat, spectator, corrupted) in [
        (PlaySeat::Observer, false, false),
        (PlaySeat::Architect, false, false),
        (PlaySeat::Observer, true, false),
        (PlaySeat::Observer, false, true),
        (PlaySeat::Architect, false, true),
    ] {
        let mut app = test_app();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::ZERO,
        ));
        app.insert_resource(PlaySetupDraft {
            rules: PlayRules::Ascent,
            seat,
            guardian: false,
            ..PlaySetupDraft::for_preset(PlayPreset::TeamRace)
        });
        if spectator {
            app.insert_resource(crate::sim::state::SpectatorBot::for_seed(1));
        }
        go(&mut app, GameState::HexWfc);
        let team;
        {
            let mut runtime = app.world_mut().resource_mut::<HexWfcRuntime>();
            let local = runtime.local_player;
            team = runtime.local().team;
            let summit = runtime.ascent.as_ref().unwrap().rules().world.config.exit();
            let feet = runtime
                .match_state
                .standing_point(summit)
                .expect("a standable summit");
            let players: Vec<_> = runtime
                .match_state
                .players
                .values()
                .filter(|p| p.team == team)
                .map(|p| p.id)
                .collect();
            for player in players {
                if corrupted && player == local {
                    runtime.match_state.drop_into_void(player);
                } else {
                    assert!(runtime.match_state.stage_body_facing(
                        player,
                        summit,
                        feet,
                        feet + Vec3::Y * 1.5 + Vec3::Z * 0.1
                    ));
                }
            }
            for _ in 0..120 {
                let frame = observed_match::hex_wfc::HexInputFrame {
                    tick: runtime.match_state.tick + 1,
                    ..default()
                };
                super::ascent::step(&mut runtime, &frame, None, None);
                if runtime.match_state.status == observed_match::hex_wfc::HexMatchStatus::Finished {
                    break;
                }
            }
            assert_eq!(
                runtime.ascent.as_ref().unwrap().rules().outcome,
                MatchOutcome::LoyalVictory
            );
            runtime.results_delay_frames = 89;
        }
        app.update();
        app.update();
        let career = app.world().resource::<Career>();
        assert_eq!(career.last_ascent_result.unwrap().winner, Some(team));
        let loyal_won = !corrupted || seat == PlaySeat::Architect;
        assert_eq!(career.last_result.as_ref().unwrap().local_won, loyal_won);
        assert_eq!(
            career.last_result.as_ref().unwrap().placement,
            loyal_won.then_some(1)
        );
        let words = all_texts(&mut app).join("\n");
        assert!(words.contains("remaining loyal Observer"));
        if corrupted && seat == PlaySeat::Observer {
            assert!(words.contains("your faction lost"));
        }
        if spectator {
            assert!(words.contains("OBSERVATION COMPLETE"));
        }
    }
}

#[test]
fn four_seat_coop_replay_keeps_every_observer_identity_distinct() {
    let mut app = test_app();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::ZERO,
    ));
    app.insert_resource(PlaySetupDraft {
        rules: PlayRules::Ascent,
        guardian: false,
        ..PlaySetupDraft::for_preset(PlayPreset::CoOp)
    });
    go(&mut app, GameState::HexWfc);
    let tape = app.world().resource::<ReplayTape>();
    let actors: std::collections::BTreeSet<_> = tape.actors.iter().map(|actor| actor.id).collect();
    let poses: std::collections::BTreeSet<_> = tape.samples[0]
        .actors
        .iter()
        .map(|pose| pose.actor)
        .collect();
    assert_eq!(actors.len(), 4);
    assert_eq!(poses.len(), 4);
    assert_eq!(actors, poses);
    assert!(actors.contains(&crate::sim::replay::ReplayActorId::Member {
        team: TeamId(0),
        member: 3
    }));
}
