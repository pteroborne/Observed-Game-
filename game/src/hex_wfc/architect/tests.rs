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
