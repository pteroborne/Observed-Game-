//! Architect Ascent in the game: the rules riding beside the first-person match.
//!
//! The runtime keeps its `HexWfcMatch` where every presentation system reads it, and
//! holds the Ascent rules ([`AscentRules`]) beside it. A local launch with the Ascent
//! rules builds them; each tick steps both together; the rules' outcome ends the match.
//! Every team's Architect seat is held by the rules' own loyal bot for now.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::{PlayerId, TeamId};
use observed_match::ascent::facility::AscentRules;
use observed_match::ascent::session::{ASCENT_INPUT_VERSION, InputFrame, SeatCommand};
use observed_match::ascent::sim::{ArchitectCommand, MatchOutcome, TeamId as AscentTeam};
use observed_match::hex_wfc::{HexBodyPlace, HexInputFrame, HexPlayerState, HexWfcMatch};

use super::architect::ArchitectDesk;
use super::sim::HexWfcRuntime;
use crate::flow::MatchResult;

/// How far below the facility the prison dimension lies, and how far apart each team's
/// maze is from the next. Only presentation reads these: in the simulation each maze is a
/// space of its own, and a jailed body's position is in its maze's own frame.
pub(super) const PRISON_DEPTH: f32 = 1_000.0;
const PRISON_SPACING: f32 = 2_000.0;

/// The seat team `team`'s Architect sits in.
#[must_use]
pub(super) fn architect_seat(team: TeamId) -> PlayerId {
    observed_match::ascent::facility::architect_seat(team)
}

/// The rules for a fresh local match, with an Architect for every team: the local
/// player in `human`'s seat, and a bot in every other. Every body a bot drives - all of
/// them when the local player is an Architect, every other when they are a body
/// (`local`) - asks its Architect for help as a bot. `None` when the facility cannot host
/// them, which a solved facility always can.
pub(super) fn rules_for(
    match_state: &mut HexWfcMatch,
    local: PlayerId,
    human: Option<TeamId>,
) -> Option<AscentRules> {
    let seats = observed_match::ascent::facility::architect_seats(match_state, human);
    let seed = match_state.seed;
    let driven: Vec<PlayerId> = match_state
        .players
        .keys()
        .copied()
        .filter(|&player| human.is_some() || player != local)
        .collect();
    let mut rules = AscentRules::new(match_state, seed, seats)
        .inspect_err(|refusal| warn!("Architect Ascent could not start: {refusal:?}"))
        .ok()?;
    rules.voice(driven);
    Some(rules)
}

/// Step the match through its rules, if it has them, with the local Architect's play
/// or the local body's ask when there is one. Returns whether it did.
pub(super) fn step(
    runtime: &mut HexWfcRuntime,
    bodies: &HexInputFrame,
    mut desk: Option<&mut ArchitectDesk>,
    mut ask: Option<&mut super::ask::AskTheArchitect>,
) -> bool {
    if runtime.ascent.is_none() {
        return false;
    }
    let commands = local_seat_command(runtime, desk.as_deref_mut(), ask.as_deref_mut())
        .map(|(seat, command)| BTreeMap::from([(seat, command)]))
        .unwrap_or_default();
    apply(runtime, bodies, commands, desk, ask)
}

/// What the local player says to the rules this tick, taken from what is waiting: the
/// desk's play or answer (one command a seat a tick, so an answer waits behind a play), or
/// the local body's ask, for what the rules say it needs. Keyed by the rules seat it is
/// from: the desk's Architect seat, or the body's own.
pub(super) fn local_seat_command(
    runtime: &HexWfcRuntime,
    desk: Option<&mut ArchitectDesk>,
    ask: Option<&mut super::ask::AskTheArchitect>,
) -> Option<(PlayerId, SeatCommand)> {
    if let Some(desk) = desk {
        if let Some(play) = desk.pending.take() {
            return Some((desk.seat, SeatCommand::Architect(play)));
        }
        if let Some((author, created_at)) = desk.pending_answer.take() {
            return Some((desk.seat, SeatCommand::Acknowledge { author, created_at }));
        }
    }
    let ask = ask?;
    if !std::mem::take(&mut ask.pending) {
        return None;
    }
    let local = runtime.local_player;
    let (kind, target) = runtime.ascent.as_ref()?.session().ask_for_help(local)?;
    Some((local, SeatCommand::Request { kind, target }))
}

/// Step the match through its rules with `commands`, every seat's say this tick, and tell
/// the desk and the local body how theirs went: a play's refusal, or the building the
/// Architect now knows stands; an ask the rules refused. Returns whether there were rules.
pub(super) fn apply(
    runtime: &mut HexWfcRuntime,
    bodies: &HexInputFrame,
    commands: BTreeMap<PlayerId, SeatCommand>,
    desk: Option<&mut ArchitectDesk>,
    ask: Option<&mut super::ask::AskTheArchitect>,
) -> bool {
    let local = runtime.local_player;
    let runtime = &mut *runtime;
    let Some(rules) = runtime.ascent.as_mut() else {
        return false;
    };
    let played = desk
        .as_ref()
        .and_then(|desk| match commands.get(&desk.seat) {
            Some(SeatCommand::Architect(play)) => Some(*play),
            _ => None,
        });
    let asked = matches!(commands.get(&local), Some(SeatCommand::Request { .. }));
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick: rules.rules().tick + 1,
        commands,
    };
    // A refused frame is one the rules would refuse whole: the match has already been
    // decided, and the physical match has stopped with it.
    let stepped = rules.step(&mut runtime.match_state, bodies, &seats);
    if let (Ok(refusals), Some(ask)) = (&stepped, ask)
        && asked
    {
        let tick = rules.rules().tick;
        ask.refused = refusals.get(&local).map(|&refusal| (refusal, tick));
    }
    if let Ok(refusals) = stepped
        && let Some(desk) = desk
        // A play's refusal is the desk's to say; an answer the rules find expired is not.
        && played.is_some()
    {
        desk.last_refusal = refusals.get(&desk.seat).copied();
        // What the rules took is what the Architect knows stands, seen or not: a tile's
        // cell, or every cell of a stair's climb composition, its landing a floor up.
        if desk.last_refusal.is_none()
            && let Some(ArchitectCommand::Play { target, .. }) = played
        {
            let world = &rules.rules().world;
            let cells = world
                .placements
                .get(&target)
                .map_or_else(Vec::new, |placement| {
                    observed_facility::hex_wfc::composition_cells(
                        world.config.grid(),
                        target,
                        placement.archetype,
                    )
                    .map_or_else(|| vec![target], Vec::from)
                });
            for cell in cells {
                let Some(&placement) = rules.rules().world.placements.get(&cell) else {
                    continue;
                };
                let known = desk
                    .knowledge(rules.rules())
                    .and_then(|knowledge| knowledge.cells.get(&cell))
                    .map(|known| known.placement);
                if known != Some(placement) {
                    desk.built.insert(cell, (placement, rules.rules().tick));
                }
            }
        }
    }
    true
}

/// The rules for a LAN match that plays Architect Ascent (`launch`): an Architect for
/// every team, a human's where the launch names one at the desk and a bot's everywhere
/// else, and nobody's requests voiced, since which bodies bots drive changes with who is
/// connected. Built identically on the server and every client from the launch alone.
pub(super) fn lan_rules(
    match_state: &mut HexWfcMatch,
    launch: &observed_net::lan::LanLaunch,
) -> Option<AscentRules> {
    let at_desk: Vec<TeamId> = match_state
        .players
        .iter()
        .filter(|(player, _)| launch.is_architect(**player))
        .map(|(_, state)| state.team)
        .collect();
    let seats = observed_match::ascent::facility::architect_seats_where(match_state, |team| {
        at_desk.contains(&team)
    });
    let seed = match_state.seed;
    AscentRules::new(match_state, seed, seats)
        .inspect_err(|refusal| warn!("Architect Ascent could not start: {refusal:?}"))
        .ok()
}

/// Seat a fresh local match for `play_setup`: the Ascent rules when it asks for them, the
/// local player at their team's Architect desk when that is their seat, and a way for a
/// local body to ask its Architect for help when it is not. `None` for a race. A LAN
/// match (`lan`: `Some`, with its launch once there is one) takes the launch's rules
/// instead, and the local player sits at the desk if the launch says so. Clears the last
/// match's desk.
pub(super) fn seat(
    commands: &mut Commands,
    match_state: &mut HexWfcMatch,
    local: PlayerId,
    play_setup: &crate::play_setup::PlaySetupDraft,
    lan: Option<Option<observed_net::lan::LanLaunch>>,
) -> Option<AscentRules> {
    commands.remove_resource::<ArchitectDesk>();
    commands.remove_resource::<super::ask::AskTheArchitect>();
    let team = match_state.players[&local].team;
    let (rules, human) = match lan {
        Some(launch) => {
            let launch = launch.filter(|launch| launch.ascent)?;
            let human = launch.is_architect(local).then_some(team);
            (lan_rules(match_state, &launch)?, human)
        }
        None => {
            if play_setup.rules != crate::play_setup::PlayRules::Ascent {
                return None;
            }
            let human = (play_setup.seat == crate::play_setup::PlaySeat::Architect).then_some(team);
            (rules_for(match_state, local, human)?, human)
        }
    };
    if human.is_some() {
        commands.insert_resource(ArchitectDesk::new(
            architect_seat(team),
            AscentTeam(team.0),
            match_state.players[&local].cell.level,
        ));
    } else {
        commands.insert_resource(super::ask::AskTheArchitect::default());
    }
    Some(rules)
}

/// A body that fell into true void has corrupted into the Rogue AI (design section 7):
/// its player leaves first-person play and takes a seat at the Rogue board, beside the bot
/// Rogue and every player corrupted before them. Their seat was a body's and is the
/// Rogue's now, so the plays they make there are the Rogue's (`AscentSession::advance`).
pub(super) fn join_rogue_board(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    desk: Option<Res<ArchitectDesk>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    if desk.is_some() || !corrupted(ascent, runtime.local_player) {
        return;
    }
    let body = runtime.local();
    commands.insert_resource(ArchitectDesk::rogue(
        runtime.local_player,
        AscentTeam(body.team.0),
        body.cell.level,
    ));
}

/// Whether `player`'s body has corrupted: fallen into true void, for good.
fn corrupted(rules: &AscentRules, player: PlayerId) -> bool {
    rules.observer_for(player).is_some_and(|id| {
        rules.rules().observers.get(&id).is_some_and(|observer| {
            observer.state == observed_match::ascent::sim::ObserverState::Corrupted
        })
    })
}

/// The local player's result from the rules' outcome. A player who corrupted plays for the
/// Rogue, and wins with it.
pub(crate) fn result_for(rules: &AscentRules, game: &HexWfcMatch, local: PlayerId) -> MatchResult {
    let local_team = game
        .players
        .get(&local)
        .map_or(TeamId(0), |player| player.team);
    let winner = match rules.rules().outcome {
        MatchOutcome::LoyalVictory => rules.rules().summit_team.map(|team| TeamId(team.0)),
        MatchOutcome::RogueVictory | MatchOutcome::Running => None,
    };
    MatchResult {
        local_team,
        placement: (winner == Some(local_team)).then_some(1),
        escaped: usize::from(winner.is_some()),
        absorbed: game.teams.len() - usize::from(winner.is_some()),
        winner,
        local_won: if corrupted(rules, local) {
            rules.rules().outcome == MatchOutcome::RogueVictory
        } else {
            winner == Some(local_team)
        },
    }
}

/// Where presentation draws a body: in the facility where it is, and in its team's maze,
/// far below and apart from every other team's, when it is jailed.
#[must_use]
pub(super) fn presented_position(player: &HexPlayerState) -> Vec3 {
    match player.place {
        HexBodyPlace::Prison => player.position + prison_offset(player.team),
        HexBodyPlace::Facility | HexBodyPlace::Void => player.position,
    }
}

/// Where team `team`'s maze is drawn.
#[must_use]
pub(super) fn prison_offset(team: TeamId) -> Vec3 {
    Vec3::new(f32::from(team.0) * PRISON_SPACING, -PRISON_DEPTH, 0.0)
}
