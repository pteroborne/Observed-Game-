//! The desk's round trip: a play made at the desk reaches the rules, and a refusal
//! comes back to it.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::HexCoord;
use observed_match::ascent::session::Refusal;
use observed_match::ascent::sim::{ArchitectCommand, CommandRefusal, TeamId as AscentTeam};
use observed_match::hex_wfc::{
    HEX_INPUT_VERSION, HexBotDriver, HexInputFrame, HexMatchConfig, HexWfcMatch,
};

use super::ArchitectDesk;
use crate::hex_wfc::ascent;
use crate::hex_wfc::sim::HexWfcRuntime;

const TEAM: TeamId = TeamId(0);

fn runtime() -> HexWfcRuntime {
    let config = HexMatchConfig {
        teams: 2,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let prototypes = crate::hex_wfc::sim::load_prototypes();
    let mut match_state = HexWfcMatch::new(7, config, &prototypes).expect("solves");
    let ascent = ascent::rules_for(&mut match_state, Some(TEAM));
    HexWfcRuntime {
        presented_revisions: match_state.facility.cell_revisions.clone(),
        match_state,
        bot_driver: HexBotDriver::new(),
        local_player: PlayerId(0),
        pending_visual_cells: BTreeSet::new(),
        status: String::new(),
        map_open: false,
        map_level: 0,
        results_delay_frames: 0,
        networked: false,
        resync_attempts: 0,
        ascent,
    }
}

fn desk() -> ArchitectDesk {
    ArchitectDesk::new(ascent::architect_seat(TEAM), AscentTeam(TEAM.0), 0)
}

/// One tick, every body walked by the bot, and the desk's play, if any, sent.
fn tick(runtime: &mut HexWfcRuntime, desk: &mut ArchitectDesk) {
    let commands = runtime
        .match_state
        .players
        .keys()
        .map(|&id| (id, runtime.match_state.bot_player_command(id)))
        .collect::<BTreeMap<_, _>>();
    let frame = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick: runtime.match_state.tick + 1,
        commands,
    };
    assert!(ascent::step(runtime, &frame, Some(desk)));
}

/// A play the rules would take from this desk now, if there is one.
fn a_legal_play(runtime: &HexWfcRuntime, desk: &ArchitectDesk) -> Option<ArchitectCommand> {
    let ascent = runtime.ascent.as_ref()?;
    let known = &ascent.rules().team_knowledge.get(&desk.team)?.cells;
    let hand = &ascent.session().hands.get(&desk.team)?.deck.hand;
    hand.iter().find_map(|card| {
        known.keys().find_map(|&target| {
            (0..6).find_map(|rotation| {
                let play = ArchitectCommand::Play {
                    card: card.id,
                    target,
                    rotation,
                };
                ascent
                    .session()
                    .architect_refusal(desk.seat, play)
                    .is_none()
                    .then_some(play)
            })
        })
    })
}

#[test]
fn the_local_team_s_architect_is_the_player_and_every_other_is_a_bot() {
    let runtime = runtime();
    let seats = runtime.ascent.as_ref().expect("ascent").session().seats();
    assert!(
        !seats[&ascent::architect_seat(TEAM)].bot,
        "the player's seat"
    );
    assert!(seats[&ascent::architect_seat(TeamId(1))].bot, "the rival's");
}

#[test]
fn a_play_made_at_the_desk_is_played_by_the_rules() {
    let mut runtime = runtime();
    let mut desk = desk();
    let mut play = None;
    for _ in 0..4_000 {
        play = a_legal_play(&runtime, &desk);
        if play.is_some() {
            break;
        }
        tick(&mut runtime, &mut desk);
    }
    let play = play.expect("walking the facility maps somewhere to build");
    let before = runtime.ascent.as_ref().unwrap().rules().command_log.len();
    desk.pending = Some(play);
    tick(&mut runtime, &mut desk);
    let rules = runtime.ascent.as_ref().unwrap().rules();
    assert_eq!(desk.pending, None, "the desk handed the play over");
    assert_eq!(desk.last_refusal, None);
    assert_eq!(rules.command_log.len(), before + 1);
    assert_eq!(
        rules.command_log.last().map(|(_, command)| *command),
        Some(play)
    );
}

#[test]
fn a_refused_play_comes_back_to_the_desk_with_its_reason() {
    let mut runtime = runtime();
    let mut desk = desk();
    tick(&mut runtime, &mut desk);
    let card = runtime.ascent.as_ref().unwrap().session().hands[&desk.team]
        .deck
        .hand[0]
        .id;
    // A cell nobody on the team has seen.
    let far = HexCoord {
        q: 11,
        r: 8,
        level: 1,
    };
    desk.pending = Some(ArchitectCommand::Play {
        card,
        target: far,
        rotation: 0,
    });
    tick(&mut runtime, &mut desk);
    assert_eq!(
        desk.last_refusal,
        Some(Refusal::Architect(CommandRefusal::UnknownTarget))
    );
}

#[test]
fn the_architect_believes_what_they_built_until_the_team_sees_the_cell_again() {
    use observed_match::ascent::sim::KnownCell;
    let runtime = runtime();
    let mut placements = runtime.match_state.facility.placements.iter();
    let (&cell, &seen) = placements.next().expect("a cell");
    let (_, &built) = placements
        .find(|(_, other)| **other != seen)
        .expect("two different cells");
    let mut desk = desk();
    let known = |seen_at| KnownCell {
        placement: seen,
        seen_at,
    };
    assert_eq!(desk.believed(cell, Some(&known(5))), Some(seen));
    assert_eq!(desk.believed(cell, None), None);

    desk.built.insert(cell, (built, 10));
    assert_eq!(
        desk.believed(cell, Some(&known(5))),
        Some(built),
        "the team last saw it before the build"
    );
    assert_eq!(desk.believed(cell, None), Some(built));
    assert_eq!(
        desk.believed(cell, Some(&known(12))),
        Some(seen),
        "the team has seen it since, and what it saw wins"
    );
}

fn cell(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

#[test]
fn a_click_aims_and_a_second_click_on_the_aim_confirms() {
    let mut desk = desk();
    assert!(
        !desk.click_cell(cell(1, 1)),
        "no card in hand: nothing to aim"
    );
    assert_eq!(desk.aimed, None);

    desk.pick_up(2, 5);
    assert!(!desk.click_cell(cell(1, 1)), "the first click aims");
    assert_eq!(desk.aimed, Some(cell(1, 1)));
    assert!(!desk.click_cell(cell(2, 1)), "another cell moves the aim");
    assert_eq!(desk.aimed, Some(cell(2, 1)));
    assert!(desk.click_cell(cell(2, 1)), "the aim again confirms");

    // The aim is what the play is about, even with the cursor elsewhere.
    desk.hovered = Some(cell(4, 4));
    assert_eq!(desk.focus(), Some(cell(2, 1)));
}

#[test]
fn escape_steps_back_from_the_aim_then_from_the_card() {
    let mut desk = desk();
    desk.pick_up(0, 5);
    desk.click_cell(cell(1, 1));
    desk.cancel();
    assert_eq!((desk.selected, desk.aimed), (Some(0), None));
    desk.cancel();
    assert_eq!(desk.selected, None);

    // Another card keeps the aim; the same card again puts it down with the aim.
    desk.pick_up(0, 5);
    desk.click_cell(cell(1, 1));
    desk.pick_up(1, 5);
    assert_eq!((desk.selected, desk.aimed), (Some(1), Some(cell(1, 1))));
    desk.pick_up(1, 5);
    assert_eq!((desk.selected, desk.aimed), (None, None));
    desk.pick_up(7, 5);
    assert_eq!(desk.selected, None, "no such card");
}

#[test]
fn a_confirmed_play_is_sent_only_if_the_rules_would_take_it() {
    let mut desk = desk();
    desk.pick_up(0, 5);
    desk.click_cell(cell(1, 1));
    let play = ArchitectCommand::Requisition;
    let refusal = Refusal::Architect(CommandRefusal::UnknownTarget);
    desk.settle(play, Some(refusal));
    assert_eq!(desk.pending, None);
    assert_eq!(desk.last_refusal, Some(refusal));
    assert_eq!(desk.aimed, Some(cell(1, 1)), "a refused play stays aimed");

    desk.settle(play, None);
    assert_eq!(desk.pending, Some(play));
    assert_eq!(
        (desk.selected, desk.aimed, desk.last_refusal),
        (None, None, None)
    );
}

#[test]
fn changing_floor_drops_the_aim() {
    let mut desk = desk();
    desk.pick_up(0, 5);
    desk.click_cell(cell(1, 1));
    desk.look_at(0);
    assert_eq!(desk.aimed, Some(cell(1, 1)), "the same floor keeps it");
    desk.look_at(1);
    assert_eq!((desk.floor, desk.aimed, desk.hovered), (1, None, None));
    assert_eq!(desk.selected, Some(0), "the card stays in hand");
}

#[test]
fn a_controller_goes_along_the_hand_and_wraps() {
    let mut desk = desk();
    desk.cycle(1, 5);
    assert_eq!(
        desk.selected,
        Some(0),
        "forward from none is the first card"
    );
    desk.cycle(-1, 5);
    assert_eq!(desk.selected, Some(4), "and back from the first, the last");
    desk.cycle(1, 5);
    assert_eq!(desk.selected, Some(0));
    desk.put_down();
    desk.cycle(-1, 5);
    assert_eq!(desk.selected, Some(4), "back from none is the last card");
    desk.cycle(1, 0);
    assert_eq!(
        desk.selected,
        Some(4),
        "an empty hand leaves the desk as it is"
    );
}

#[test]
fn a_controller_drives_the_desk_through_the_same_steps() {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::ButtonInput;
    use bevy::input::gamepad::{Gamepad, GamepadButton};
    use bevy::prelude::*;

    use crate::hex_wfc::overlay::{MatchOverlayState, PausePage};
    use crate::screens::widgets::UiInputCapture;

    let mut app = App::new();
    app.insert_resource(runtime())
        .insert_resource(desk())
        .init_resource::<MatchOverlayState>()
        .init_resource::<UiInputCapture>()
        .init_resource::<Time>();
    let pad = app.world_mut().spawn(Gamepad::default()).id();
    let press = |app: &mut App, button: GamepadButton| {
        let mut gamepad = app.world_mut().get_mut::<Gamepad>(pad).expect("a pad");
        *gamepad.digital_mut() = ButtonInput::default();
        gamepad.digital_mut().press(button);
        app.world_mut()
            .run_system_once(super::pad::input)
            .expect("the pad runs");
    };
    let desk_now = |app: &App| {
        let desk = app.world().resource::<ArchitectDesk>();
        (desk.selected, desk.aimed, desk.rotation)
    };

    press(&mut app, GamepadButton::DPadRight);
    assert_eq!(desk_now(&app), (Some(0), None, 0), "along the hand");
    assert!(
        app.world().resource::<ArchitectDesk>().pad,
        "the prompts follow"
    );
    press(&mut app, GamepadButton::RightTrigger);
    assert_eq!(desk_now(&app).2, 1, "RB turns it");

    let target = HexCoord {
        q: 1,
        r: 1,
        level: 0,
    };
    app.world_mut().resource_mut::<ArchitectDesk>().hovered = Some(target);
    press(&mut app, GamepadButton::South);
    assert_eq!(desk_now(&app), (Some(0), Some(target), 1), "A aims");
    press(&mut app, GamepadButton::East);
    assert_eq!(
        desk_now(&app),
        (Some(0), None, 1),
        "B steps back from the aim"
    );
    press(&mut app, GamepadButton::East);
    assert_eq!(desk_now(&app).0, None, "and from the card");

    // A pause page has the pad until it closes.
    *app.world_mut().resource_mut::<MatchOverlayState>() =
        MatchOverlayState::Pause(PausePage::Root);
    press(&mut app, GamepadButton::DPadRight);
    assert_eq!(desk_now(&app).0, None, "the desk does not hear it");
}
