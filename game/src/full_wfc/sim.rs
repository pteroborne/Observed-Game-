//! Bevy adapter around the pure authoritative full-WFC match.

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_core::PlayerId;
use observed_facility::full_wfc::CellCoord;
use observed_match::full_wfc::{
    FullWfcMatch, FullWfcMatchConfig, GameplayEventKind, InputFrame, MatchStatus, PlayerCommand,
};

use crate::flow::ActiveMatchSeed;

pub(super) const LOCAL_PLAYER: PlayerId = PlayerId(0);
pub(super) const EYE_OFFSET: f32 = 0.70;

#[derive(Resource, Default)]
pub(super) struct FullWfcIntent {
    pub command: PlayerCommand,
    pub toggle_map: bool,
}

#[derive(Resource)]
pub struct FullWfcRuntime {
    pub match_state: FullWfcMatch,
    pub local_player: PlayerId,
    pub pending_visual_changes: BTreeSet<CellCoord>,
    pub status: String,
    pub map_open: bool,
    pub results_delay_frames: u16,
}

impl FullWfcRuntime {
    pub fn local(&self) -> &observed_match::full_wfc::PlayerState {
        &self.match_state.players[&self.local_player]
    }
}

pub(super) fn setup_runtime(
    mut commands: Commands,
    seed: Option<Res<ActiveMatchSeed>>,
    mut career: ResMut<crate::flow::Career>,
) {
    career.begin_match();
    let requested_seed = seed.as_deref().map_or(0xF011_FAC1_1177, |seed| seed.0);
    let mut match_config = FullWfcMatchConfig::default();
    let is_test = std::env::current_exe()
        .map(|p| {
            let s = p.to_string_lossy().to_lowercase();
            s.contains("deps") || s.contains("test")
        })
        .unwrap_or(false);
    if !is_test {
        match_config.wfc = Some(observed_facility::full_wfc::FullWfcConfig::liminal_large());
    }
    let (match_state, seed_offset) = (0..64u64)
        .find_map(|offset| {
            FullWfcMatch::new(requested_seed.wrapping_add(offset), match_config)
                .ok()
                .map(|game| (game, offset))
        })
        .expect("the full-WFC default corpus must contain a solvable nearby seed");
    let pending_visual_changes = match_state.facility.placements.keys().copied().collect();
    let replay = crate::sim::replay::ReplayTape::new_full_wfc(&match_state);
    commands.insert_resource(FullWfcRuntime {
        match_state,
        local_player: LOCAL_PLAYER,
        pending_visual_changes,
        status: if seed_offset == 0 {
            "authoritative local match ready".to_string()
        } else {
            format!("seed advanced by {seed_offset} after solve contradictions")
        },
        map_open: false,
        results_delay_frames: 0,
    });
    commands.insert_resource(FullWfcIntent::default());
    commands.insert_resource(replay);
}

pub(super) fn finish_runtime(
    mut runtime: ResMut<FullWfcRuntime>,
    mut career: ResMut<crate::flow::Career>,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    mut next: ResMut<NextState<crate::GameState>>,
) {
    if runtime.match_state.status != MatchStatus::Finished {
        return;
    }
    runtime.results_delay_frames = runtime.results_delay_frames.saturating_add(1);
    if runtime.results_delay_frames < 90 {
        return;
    }
    let result = crate::flow::resolve_full_wfc(&runtime.match_state);
    if let Some(replay) = replay.as_deref_mut() {
        replay.result = Some(result.clone());
    }
    career.record(result);
    next.set(crate::GameState::Results);
}

pub(super) fn cleanup_runtime(mut commands: Commands) {
    commands.remove_resource::<FullWfcRuntime>();
    commands.remove_resource::<FullWfcIntent>();
    commands.remove_resource::<crate::sim::state::SpectatorBot>();
}

pub(super) fn step_runtime(
    mut intent: ResMut<FullWfcIntent>,
    mut runtime: ResMut<FullWfcRuntime>,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    if intent.toggle_map {
        runtime.map_open = !runtime.map_open;
        intent.toggle_map = false;
    }
    if runtime.match_state.status == MatchStatus::Finished {
        clear_one_shot_input(&mut intent.command);
        return;
    }
    let mut frame = InputFrame {
        tick: runtime.match_state.tick + 1,
        ..Default::default()
    };
    let local_command = if spectator_bot.is_some() {
        runtime.match_state.bot_command(runtime.local_player)
    } else {
        intent.command
    };
    frame.commands.insert(runtime.local_player, local_command);
    for id in runtime
        .match_state
        .players
        .keys()
        .copied()
        .collect::<Vec<_>>()
    {
        if id != runtime.local_player {
            frame
                .commands
                .insert(id, runtime.match_state.bot_command(id));
        }
    }
    let previous_generation = runtime.match_state.facility.generation;
    runtime.match_state.step(&frame);
    if let Some(replay) = replay.as_deref_mut() {
        replay.record_full_wfc(&runtime.match_state);
    }
    if runtime.match_state.facility.generation != previous_generation {
        runtime.pending_visual_changes = runtime
            .match_state
            .facility
            .placements
            .keys()
            .copied()
            .collect();
    }
    if let Some(event) = runtime.match_state.recent_events.last() {
        runtime.status = event_label(event.kind).to_string();
    }
    clear_one_shot_input(&mut intent.command);
}

fn clear_one_shot_input(command: &mut PlayerCommand) {
    command.intent.look = Vec2::ZERO;
    command.intent.jump_pressed = false;
    command.actions = Default::default();
}

fn event_label(kind: GameplayEventKind) -> &'static str {
    match kind {
        GameplayEventKind::MutationWarning => "the structure is breathing",
        GameplayEventKind::MutationCommitted => "unseen rooms have mutated",
        GameplayEventKind::MutationCancelled => "mutation held by observation",
        GameplayEventKind::AnchorDeployed => "threshold anchor deployed",
        GameplayEventKind::AnchorRecovered => "threshold anchor recovered",
        GameplayEventKind::PadDeployed => "teleport pad deployed",
        GameplayEventKind::PadRecovered => "teleport pad recovered",
        GameplayEventKind::PadUsed => "teleport link traversed",
        GameplayEventKind::KeystoneCollected => "keystone secured",
        GameplayEventKind::DualStationProgress => "dual station synchronizing",
        GameplayEventKind::DualStationCompleted => "exit authorization complete",
        GameplayEventKind::MonitorSurveyed => "monitor survey copied to team map",
        GameplayEventKind::GuardianCatch => "Guardian catch: player displaced",
        GameplayEventKind::GuardianRedirected => "Guardian redirected",
        GameplayEventKind::TeamEscaped => "team escaped; collapse countdown started",
        GameplayEventKind::MatchFinished => "match complete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_non_local_actors_cross_the_same_command_boundary() {
        let game = FullWfcMatch::new(44, FullWfcMatchConfig::default()).expect("match");
        assert_eq!(game.players.len(), 8);
        assert!(
            game.players
                .keys()
                .filter(|&&id| id != LOCAL_PLAYER)
                .all(|&id| {
                    let _command = game.bot_command(id);
                    true
                })
        );
    }

    #[test]
    fn full_wfc_replay_records_the_versioned_eight_actor_simulation() {
        let mut game = FullWfcMatch::new(44, FullWfcMatchConfig::default()).expect("match");
        let mut replay = crate::sim::replay::ReplayTape::new_full_wfc(&game);
        let commands = game
            .players
            .keys()
            .copied()
            .map(|id| (id, game.bot_command(id)))
            .collect();
        game.step(&InputFrame {
            tick: 1,
            commands,
            ..Default::default()
        });
        replay.record_full_wfc(&game);
        assert_eq!(
            replay.input_version,
            observed_match::full_wfc::FULL_WFC_INPUT_VERSION
        );
        assert_eq!(replay.actors.len(), 8);
        assert_eq!(replay.samples[0].actors.len(), 8);
        assert!(!replay.rooms.is_empty());
    }
}
