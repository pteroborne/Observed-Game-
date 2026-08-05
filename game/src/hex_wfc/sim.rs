//! Bevy wrapper, deterministic construction, and fixed-step command threading for the
//! pure authoritative hex-facility match.

use std::collections::{BTreeMap, BTreeSet};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_authoring::TilePrototype;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexWfcConfig};
use observed_match::hex_wfc::{
    HexActionButtons, HexInputFrame, HexMatchConfig, HexMatchStatus, HexPlayerCommand, HexWfcMatch,
};
use player_input::PlayerIntent;

use crate::flow::ActiveMatchSeed;

use super::HexOnboardingGate;
use super::launch::{HexLaunchError, HexLaunchSpec, HexSeedPolicy, prepare};
use super::loading::{HexLaunchRequest, PreparedHexLaunchSlot};
use super::overlay::{MatchOverlayState, SimulationPolicy, simulation_policy};

pub(super) const LOCAL_PLAYER: PlayerId = PlayerId(0);
/// Camera eye rise above the simulation body centre, in metres.
pub(super) const EYE_OFFSET: f32 = 0.70;

/// One-shot + held local input for the current tick, sanitized into a [`PlayerIntent`].
#[derive(Resource, Default)]
pub(super) struct HexWfcIntent {
    pub intent: PlayerIntent,
    pub actions: HexActionButtons,
    /// One-shot survivor-map floor browse request (`1` up, `-1` down).
    pub browse_map_level: i8,
}

#[derive(Resource)]
pub struct HexWfcRuntime {
    pub match_state: HexWfcMatch,
    pub local_player: PlayerId,
    /// Cells whose visuals must be (re)spawned after entry or relayout.
    pub pending_visual_cells: BTreeSet<HexCoord>,
    pub presented_revisions: BTreeMap<HexCoord, u32>,
    pub status: String,
    pub map_open: bool,
    /// Floor currently shown by the active-level survivor sketch.
    pub map_level: u8,
    pub results_delay_frames: u16,
    pub networked: bool,
    /// One history replay is allowed before a repeated desync disconnects.
    pub resync_attempts: u8,
}

impl HexWfcRuntime {
    pub fn local(&self) -> &observed_match::hex_wfc::HexPlayerState {
        &self.match_state.players[&self.local_player]
    }
}

/// Resolve the workspace tile directory without involving presentation.
#[cfg(test)]
fn tile_dir() -> std::path::PathBuf {
    // `launch::tile_dir` is `assets_root()/tiles` with a workspace fallback, so
    // it covers the packaged layout origin/main added *and* still resolves when
    // the tests run from the source tree.
    super::launch::tile_dir()
}

/// Load the same authored-plus-compatibility corpus used by tests and evidence.
pub(crate) fn load_prototypes() -> Vec<TilePrototype> {
    load_authoring_corpus().cells().to_vec()
}

pub(crate) fn simulation_content_hash() -> [u8; 32] {
    load_authoring_corpus().simulation_content_hash()
}

pub(crate) fn match_from_launch(
    seed: u64,
    config: HexMatchConfig,
    expected_hash: [u8; 32],
) -> Result<HexWfcMatch, HexLaunchError> {
    prepare(HexLaunchSpec {
        requested_seed: seed,
        config,
        seed_policy: HexSeedPolicy::Exact {
            expected_content_hash: expected_hash,
        },
    })
    .map(|prepared| prepared.match_state)
}

fn load_authoring_corpus() -> std::sync::Arc<observed_match::hex_wfc::HexMatchContent> {
    super::launch::load_current_content().expect("committed runtime hex catalog loads")
}

/// Tests swap the production 28×20×10 solve for the compact showcase fixture.
fn is_test_binary() -> bool {
    std::env::current_exe()
        .map(|path| {
            let s = path.to_string_lossy().to_lowercase();
            s.contains("deps") || s.contains("test")
        })
        .unwrap_or(false)
}

/// Production uses `arc_default`; tests and relayout evidence use the compact fixture
/// so its pinned warning@546 / commit@666 mutation timeline remains reproducible.
/// The match configuration a given set of settings asks for.
///
/// Bot fill is a *roster* decision, not a per-tick one. Turning it off shrinks
/// the match to the seats a human occupies rather than leaving bot-shaped bodies
/// standing in the facility, which is what filling a seat with no driver would
/// mean.
pub(crate) fn runtime_config_for(play_setup: &crate::play_setup::PlaySetupDraft) -> HexMatchConfig {
    let validated = play_setup
        .validate()
        .expect("persisted play setup is validated before launch");
    let mut config = validated.local_match_config(HexMatchConfig::default().wfc);
    let relayout_capture = std::env::var("OBSERVED2_CAPTURE_HEX_WFC_RELAYOUT").is_ok();
    let traversal_capture = std::env::var("OBSERVED2_CAPTURE_HEX_WFC_TRAVERSAL").is_ok();
    let playtest = std::env::var("OBSERVED2_HEX_PLAYTEST")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase());
    if traversal_capture || playtest.as_deref() == Some("gate") {
        // Pinned five-level route fixture: two ramp transitions plus two physical
        // stair transitions. `OBSERVED2_HEX_PLAYTEST=gate` exposes the same compact
        // fixture for the required hands-on traversal/complete-match gate.
        config.wfc.levels = 5;
    } else if !is_test_binary() && !relayout_capture && playtest.as_deref() != Some("relayout") {
        config.wfc = HexWfcConfig::arc_default();
    }
    config
}

pub(super) fn setup_runtime(
    mut commands: Commands,
    mut seed: Option<ResMut<ActiveMatchSeed>>,
    mut career: ResMut<crate::flow::Career>,
    play_setup: Res<crate::play_setup::PlaySetupDraft>,
    request: Option<Res<HexLaunchRequest>>,
    mut prepared_slot: Option<ResMut<PreparedHexLaunchSlot>>,
    direct_driver: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    career.begin_match();
    let handed_off = prepared_slot
        .as_deref_mut()
        .and_then(PreparedHexLaunchSlot::take);
    let (prepared, local_player, networked, spectator, context, launch_config) = if let Some(
        prepared,
    ) = handed_off
    {
        let request = request
            .as_deref()
            .expect("a prepared hex launch retains its finalized request metadata");
        (
            prepared,
            request.local_player,
            request.networked,
            request.spectator,
            Some(request.context),
            request.spec.config,
        )
    } else {
        assert!(
            request.is_none(),
            "a finalized hex launch request must pass through Loading before HexWfc"
        );
        assert!(
            is_test_binary() || direct_driver.is_some(),
            "HexWfc entered without a prepared launch; production launches must pass through Loading"
        );
        // Regression tests and autonomous evidence captures intentionally enter
        // HexWfc directly. Keep that private harness path deterministic while all
        // player-facing launches consume the asynchronous prepared handoff above.
        let requested_seed = seed.as_deref().map_or(0xF011_FAC1_1177, |seed| seed.0);
        let config = runtime_config_for(&play_setup);
        let prepared = prepare(HexLaunchSpec {
            requested_seed,
            config,
            seed_policy: HexSeedPolicy::Nearby,
        })
        .expect("the hex authoring catalog must contain a solvable nearby seed");
        debug_assert_eq!(prepared.requested_seed, requested_seed);
        debug_assert_eq!(prepared.selected_seed, prepared.match_state.seed);
        debug_assert_eq!(
            prepared.simulation_content_hash,
            prepared.match_state.simulation_content_hash
        );
        (
            prepared,
            LOCAL_PLAYER,
            false,
            direct_driver.is_some(),
            None,
            config,
        )
    };
    debug_assert_eq!(prepared.selected_seed, prepared.match_state.seed);
    debug_assert_eq!(
        prepared.simulation_content_hash,
        prepared.match_state.simulation_content_hash
    );
    if let Some(seed) = seed.as_deref_mut() {
        seed.0 = prepared.selected_seed;
    } else {
        commands.insert_resource(ActiveMatchSeed(prepared.selected_seed));
    }
    if spectator && direct_driver.is_none() {
        commands.insert_resource(crate::sim::state::SpectatorBot::for_seed(
            prepared.selected_seed,
        ));
    }
    commands.insert_resource(crate::play_setup::ActivePlaySession::from_launch(
        launch_config,
        spectator,
        networked,
    ));
    let seed_offset = prepared.seed_offset;
    let match_state = prepared.match_state;
    let replay = crate::sim::replay::ReplayTape::new_hex_wfc_for_player(&match_state, local_player);
    let map_level = match_state.players[&local_player].cell.level;
    let presented_revisions = match_state.facility.cell_revisions.clone();
    commands.insert_resource(HexWfcRuntime {
        match_state,
        local_player,
        pending_visual_cells: BTreeSet::new(),
        presented_revisions,
        status: if seed_offset == 0 {
            match context {
                Some(crate::play_setup::LaunchContext::Local) => {
                    "local hex facility ready".to_string()
                }
                Some(crate::play_setup::LaunchContext::Rematch) => {
                    "rematch hex facility ready".to_string()
                }
                Some(crate::play_setup::LaunchContext::Lan) => "LAN hex facility ready".to_string(),
                None => "authoritative hex facility ready".to_string(),
            }
        } else {
            format!("seed advanced by {seed_offset} after solve contradictions")
        },
        map_open: false,
        map_level,
        results_delay_frames: 0,
        networked,
        resync_attempts: 0,
    });
    commands.insert_resource(HexWfcIntent::default());
    commands.insert_resource(replay);
    commands.remove_resource::<PreparedHexLaunchSlot>();
    commands.remove_resource::<HexLaunchRequest>();
}

pub(super) fn finish_runtime(
    mut runtime: ResMut<HexWfcRuntime>,
    mut career: ResMut<crate::flow::Career>,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    mut next: ResMut<NextState<crate::GameState>>,
) {
    if runtime.match_state.status != HexMatchStatus::Finished {
        return;
    }
    runtime.results_delay_frames = runtime.results_delay_frames.saturating_add(1);
    if runtime.results_delay_frames < 90 {
        return;
    }
    let result =
        crate::flow::resolve_hex_wfc_for_player(&runtime.match_state, runtime.local_player);
    if let Some(replay) = replay.as_deref_mut() {
        replay.result = Some(result.clone());
    }
    career.record(result);
    next.set(crate::GameState::Results);
}

pub(super) fn cleanup_runtime(mut commands: Commands) {
    commands.remove_resource::<HexWfcRuntime>();
    commands.remove_resource::<HexWfcIntent>();
    commands.remove_resource::<crate::sim::state::SpectatorBot>();
}

#[derive(SystemParam)]
pub(super) struct SimulationControl<'w> {
    overlay: Res<'w, MatchOverlayState>,
    onboarding: Res<'w, HexOnboardingGate>,
}

pub(super) fn step_runtime(
    mut intent: ResMut<HexWfcIntent>,
    mut runtime: ResMut<HexWfcRuntime>,
    control: SimulationControl,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
    mut lan: ResMut<crate::lan::LanRuntime>,
    mut next: ResMut<NextState<crate::GameState>>,
) {
    if runtime.map_open && intent.browse_map_level != 0 {
        let discovered = runtime
            .match_state
            .player_map(runtime.local_player)
            .map(|knowledge| knowledge.cells.keys().copied().collect())
            .unwrap_or_default();
        runtime.map_level = browsed_level(&discovered, runtime.map_level, intent.browse_map_level);
    }
    intent.browse_map_level = 0;
    let policy = simulation_policy(
        *control.overlay,
        runtime.networked,
        control.onboarding.active,
    );
    if policy == SimulationPolicy::Stop {
        neutralize_input(&mut intent);
        return;
    }
    if runtime.match_state.status == HexMatchStatus::Finished {
        finish_input_tick(&mut intent, policy);
        return;
    }
    let local_player = runtime.local_player;
    let local_command = if policy.sends_neutral_input() {
        HexPlayerCommand::default()
    } else if spectator_bot.is_some() {
        runtime.match_state.bot_player_command(local_player)
    } else {
        HexPlayerCommand {
            intent: intent.intent,
            actions: intent.actions,
        }
    };
    if runtime.networked {
        let Some(client) = lan.client.as_mut() else {
            runtime.status = "LAN server disconnected".to_string();
            finish_input_tick(&mut intent, policy);
            return;
        };
        client.poll();
        let target_tick = runtime
            .match_state
            .tick
            .saturating_add(observed_net::lan::INPUT_LEAD_TICKS);
        if let Err(error) = client.queue_input(target_tick, local_command) {
            runtime.status = format!("LAN input error: {error}");
        }
        let frames = client.take_ready_frames(observed_net::lan::FRAME_WINDOW);
        let mut request_resync = false;
        let mut repeated_desync = false;
        for frame in frames {
            let previous_generation = runtime.match_state.facility.generation;
            runtime.match_state.step(&frame.to_input_frame());
            let digest = runtime.match_state.snapshot().digest;
            if digest != frame.digest {
                if runtime.resync_attempts == 0 {
                    runtime.status = format!(
                        "DESYNC at tick {}; replaying authoritative history",
                        frame.tick
                    );
                    request_resync = true;
                } else {
                    runtime.status = format!(
                        "Repeated DESYNC at tick {}: local {digest:016x}, server {:016x}",
                        frame.tick, frame.digest
                    );
                    repeated_desync = true;
                }
                break;
            }
            if let Some(replay) = replay.as_deref_mut() {
                replay.record_hex_wfc(&runtime.match_state);
            }
            record_generation_changes(&mut runtime, previous_generation);
        }
        if request_resync {
            let launch = client.launch;
            match launch.and_then(|launch| {
                match_from_launch(launch.seed, launch.config, launch.simulation_content_hash).ok()
            }) {
                Some(match_state) => {
                    runtime.match_state = match_state;
                    runtime.presented_revisions =
                        runtime.match_state.facility.cell_revisions.clone();
                    runtime.pending_visual_cells = runtime
                        .match_state
                        .facility
                        .placements
                        .keys()
                        .copied()
                        .collect();
                    runtime.map_level = runtime.local().cell.level;
                    runtime.resync_attempts = runtime.resync_attempts.saturating_add(1);
                    if let Some(replay) = replay.as_deref_mut() {
                        *replay = crate::sim::replay::ReplayTape::new_hex_wfc_for_player(
                            &runtime.match_state,
                            runtime.local_player,
                        );
                    }
                    if let Err(error) = client.request_resync() {
                        runtime.status = format!("LAN resync request failed: {error}");
                        repeated_desync = true;
                    }
                }
                None => {
                    runtime.status = "LAN resync could not reconstruct the launch".to_string();
                    repeated_desync = true;
                }
            }
        }
        if repeated_desync {
            client.goodbye();
        }
        if let Some(event) = runtime.match_state.recent_events.last() {
            runtime.status = super::cues::cue_for(event.kind).label.to_string();
        }
        finish_input_tick(&mut intent, policy);
        if repeated_desync {
            lan.leave();
            next.set(crate::GameState::MainMenu);
        }
        return;
    }
    let mut frame = HexInputFrame {
        tick: runtime.match_state.tick + 1,
        ..Default::default()
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
                .insert(id, runtime.match_state.bot_player_command(id));
        }
    }
    let previous_generation = runtime.match_state.facility.generation;
    runtime.match_state.step(&frame);
    if let Some(replay) = replay.as_deref_mut() {
        replay.record_hex_wfc(&runtime.match_state);
    }
    record_generation_changes(&mut runtime, previous_generation);
    // Survivor-map knowledge is simulation-owned and player-local. Presentation
    // reads it directly; rival occupancy never enters the local ledger.
    if let Some(event) = runtime.match_state.recent_events.last() {
        runtime.status = super::cues::cue_for(event.kind).label.to_string();
    }
    finish_input_tick(&mut intent, policy);
}

fn record_generation_changes(runtime: &mut HexWfcRuntime, previous_generation: u32) {
    if runtime.match_state.facility.generation == previous_generation {
        return;
    }
    let changed = changed_revisions(
        &runtime.match_state.facility.cell_revisions,
        &runtime.presented_revisions,
    );
    for (cell, revision) in changed {
        runtime.pending_visual_cells.insert(cell);
        runtime.presented_revisions.insert(cell, revision);
    }
}

fn clear_one_shot_input(intent: &mut PlayerIntent) {
    intent.look = Vec2::ZERO;
    intent.jump_pressed = false;
}

fn neutralize_input(intent: &mut HexWfcIntent) {
    intent.intent = PlayerIntent::default();
    intent.actions = HexActionButtons::default();
    intent.browse_map_level = 0;
}

fn finish_input_tick(intent: &mut HexWfcIntent, policy: SimulationPolicy) {
    if policy.sends_neutral_input() {
        intent.intent = PlayerIntent::default();
    } else {
        clear_one_shot_input(&mut intent.intent);
    }
    intent.actions = HexActionButtons::default();
}

fn browsed_level(discovered: &BTreeSet<HexCoord>, current: u8, direction: i8) -> u8 {
    let levels = discovered
        .iter()
        .map(|cell| cell.level)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if direction > 0 {
        levels
            .into_iter()
            .find(|&level| level > current)
            .unwrap_or(current)
    } else {
        levels
            .into_iter()
            .rev()
            .find(|&level| level < current)
            .unwrap_or(current)
    }
}

fn changed_revisions(
    live: &BTreeMap<HexCoord, u32>,
    presented: &BTreeMap<HexCoord, u32>,
) -> Vec<(HexCoord, u32)> {
    live.iter()
        .filter_map(|(&cell, &revision)| {
            (presented.get(&cell).copied().unwrap_or(0) != revision).then_some((cell, revision))
        })
        .collect()
}

#[cfg(test)]
#[path = "sim_catalog_tests.rs"]
mod catalog_tests;

#[cfg(test)]
#[path = "sim_tests.rs"]
mod tests;
