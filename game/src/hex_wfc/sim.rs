//! Bevy adapter around the pure authoritative hex-facility match.
//!
//! Pure simulation lives in `observed_match::hex_wfc`; this module owns only the Bevy
//! resource wrapper, match construction (tile-prototype corpus + nearby-solvable seed
//! search), and the fixed-step command threading. It reads simulation crates only and
//! never imports presentation (enforced by `arch_check::hex_sim_never_imports_presentation`).

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_authoring::{Manifest, TilePrototype};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexWfcConfig};
use observed_match::hex_wfc::{HexInputFrame, HexMatchConfig, HexMatchStatus, HexWfcMatch};
use player_input::PlayerIntent;

use crate::flow::ActiveMatchSeed;

pub(super) const LOCAL_PLAYER: PlayerId = PlayerId(0);
/// Camera eye rise above the simulation body centre (metres). Matches the full-WFC
/// adapter's first-person eye placement.
pub(super) const EYE_OFFSET: f32 = 0.70;

/// One-shot + held local input for the current tick, sanitized into a [`PlayerIntent`].
#[derive(Resource, Default)]
pub(super) struct HexWfcIntent {
    pub intent: PlayerIntent,
    pub toggle_map: bool,
}

#[derive(Resource)]
pub struct HexWfcRuntime {
    pub match_state: HexWfcMatch,
    pub local_player: PlayerId,
    /// Cells whose visuals must be (re)spawned — everything on entry, then the whole
    /// facility again after any relayout generation change. Mirrors the full-WFC
    /// `pending_visual_changes` streaming trigger.
    pub pending_full_rebuild: bool,
    /// Presentation-derived survivor knowledge: every cell the local player has ever
    /// occupied or seen. Never feeds simulation; drives only the tac-map sketch.
    pub discovered: BTreeSet<HexCoord>,
    pub status: String,
    pub map_open: bool,
    pub results_delay_frames: u16,
}

impl HexWfcRuntime {
    pub fn local(&self) -> &observed_match::hex_wfc::HexPlayerState {
        &self.match_state.players[&self.local_player]
    }
}

/// The workspace `assets/tiles` directory. Resolved without touching presentation: the
/// runtime cwd when it holds the tiles, otherwise the compile-time crate location. Same
/// resolution the hex labs use.
fn tile_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("assets/tiles");
    if cwd_relative.exists() {
        return cwd_relative;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles")
}

/// Load the committed authored hex-tile corpus from the workspace `assets/tiles`
/// manifest. This is the same corpus the hex labs and geometry tests project.
pub(crate) fn load_prototypes() -> Vec<TilePrototype> {
    let base = tile_dir();
    Manifest::load(&base.join("manifest.ron"))
        .expect("committed hex tile manifest loads")
        .load_tiles(&base)
        .expect("committed hex tile prototypes validate")
}

/// True when running under the test harness, where the production 28×20×10 solve is
/// swapped for the compact showcase fixture. Mirrors the full-WFC detection.
fn is_test_binary() -> bool {
    std::env::current_exe()
        .map(|path| {
            let s = path.to_string_lossy().to_lowercase();
            s.contains("deps") || s.contains("test")
        })
        .unwrap_or(false)
}

/// The match configuration for the current run. Production uses the real
/// `arc_default()` 28×20×10 facility (now ~0.8 s to solve, fine on the setup path);
/// tests use the compact showcase fixture from [`HexMatchConfig::default`]. The relayout
/// evidence capture also forces the showcase fixture so the pinned deterministic
/// warning@1620 / commit@1800 timeline (12×9×4, seed `0xcb85_21b1_f77d_d0fc`) reproduces
/// on the capture path — the production facility has a different mutation schedule.
fn runtime_config() -> HexMatchConfig {
    let mut config = HexMatchConfig::default();
    let relayout_capture = std::env::var("OBSERVED2_CAPTURE_HEX_WFC_RELAYOUT").is_ok();
    let traversal_capture = std::env::var("OBSERVED2_CAPTURE_HEX_WFC_TRAVERSAL").is_ok();
    let playtest = std::env::var("OBSERVED2_HEX_PLAYTEST")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase());
    if traversal_capture || playtest.as_deref() == Some("gate") {
        // Phase 94's pinned five-level route fixture: two ramp transitions plus two
        // shaft transitions. `OBSERVED2_HEX_PLAYTEST=gate` exposes the same compact
        // fixture for the required hands-on traversal/complete-match gate.
        config.wfc.levels = 5;
    } else if !is_test_binary() && !relayout_capture && playtest.as_deref() != Some("relayout") {
        config.wfc = HexWfcConfig::arc_default();
    }
    config
}

/// Build a hex match near `requested_seed`, advancing the seed over a small window if a
/// particular seed hits an unsolvable contradiction. Mirrors the full-WFC offset search.
pub(super) fn solve_nearby(
    requested_seed: u64,
    prototypes: &[TilePrototype],
) -> (HexWfcMatch, u64) {
    let config = runtime_config();
    (0..64u64)
        .find_map(|offset| {
            HexWfcMatch::new(requested_seed.wrapping_add(offset), config, prototypes)
                .ok()
                .map(|game| (game, offset))
        })
        .expect("the hex tile corpus must contain a solvable nearby seed")
}

pub(super) fn setup_runtime(
    mut commands: Commands,
    seed: Option<Res<ActiveMatchSeed>>,
    mut career: ResMut<crate::flow::Career>,
) {
    career.begin_match();
    let requested_seed = seed.as_deref().map_or(0xF011_FAC1_1177, |seed| seed.0);
    let prototypes = load_prototypes();
    let (match_state, seed_offset) = solve_nearby(requested_seed, &prototypes);
    let replay = crate::sim::replay::ReplayTape::new_hex_wfc(&match_state);
    let mut discovered = BTreeSet::new();
    discovered.insert(match_state.players[&LOCAL_PLAYER].cell);
    commands.insert_resource(HexWfcRuntime {
        match_state,
        local_player: LOCAL_PLAYER,
        pending_full_rebuild: true,
        discovered,
        status: if seed_offset == 0 {
            "authoritative hex facility ready".to_string()
        } else {
            format!("seed advanced by {seed_offset} after solve contradictions")
        },
        map_open: false,
        results_delay_frames: 0,
    });
    commands.insert_resource(HexWfcIntent::default());
    commands.insert_resource(replay);
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
    let result = crate::flow::resolve_hex_wfc(&runtime.match_state);
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

pub(super) fn step_runtime(
    mut intent: ResMut<HexWfcIntent>,
    mut runtime: ResMut<HexWfcRuntime>,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    if intent.toggle_map {
        runtime.map_open = !runtime.map_open;
        intent.toggle_map = false;
    }
    if runtime.match_state.status == HexMatchStatus::Finished {
        clear_one_shot_input(&mut intent.intent);
        return;
    }
    let local_intent = if spectator_bot.is_some() {
        runtime.match_state.bot_command(runtime.local_player)
    } else {
        intent.intent
    };
    let mut frame = HexInputFrame {
        tick: runtime.match_state.tick + 1,
        ..Default::default()
    };
    frame.commands.insert(runtime.local_player, local_intent);
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
        replay.record_hex_wfc(&runtime.match_state);
    }
    if runtime.match_state.facility.generation != previous_generation {
        runtime.pending_full_rebuild = true;
    }
    // The authoritative observation frame aggregates every runner for relayout safety.
    // Survivor-map knowledge is deliberately local: rival occupancy must not leak their
    // private route into the local player's sketch.
    let local_cell = runtime.local().cell;
    runtime.discovered.insert(local_cell);
    if let Some(event) = runtime.match_state.recent_events.last() {
        runtime.status = super::cues::cue_for(event.kind).label.to_string();
    }
    clear_one_shot_input(&mut intent.intent);
}

fn clear_one_shot_input(intent: &mut PlayerIntent) {
    intent.look = Vec2::ZERO;
    intent.jump_pressed = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn showcase_match(seed: u64) -> HexWfcMatch {
        let prototypes = load_prototypes();
        HexWfcMatch::new(seed, HexMatchConfig::default(), &prototypes).expect("showcase solves")
    }

    #[test]
    fn all_non_local_actors_cross_the_same_command_boundary() {
        let game = showcase_match(44);
        assert!(game.players.len() >= 2);
        assert!(
            game.players
                .keys()
                .filter(|&&id| id != LOCAL_PLAYER)
                .all(|&id| {
                    let _intent = game.bot_command(id);
                    true
                })
        );
    }

    #[test]
    fn hex_replay_records_the_versioned_simulation() {
        let mut game = showcase_match(44);
        let mut replay = crate::sim::replay::ReplayTape::new_hex_wfc(&game);
        let commands = game
            .players
            .keys()
            .copied()
            .map(|id| (id, game.bot_command(id)))
            .collect();
        game.step(&HexInputFrame {
            tick: 1,
            commands,
            ..Default::default()
        });
        replay.record_hex_wfc(&game);
        assert_eq!(
            replay.input_version,
            observed_match::hex_wfc::HEX_INPUT_VERSION
        );
        assert_eq!(replay.actors.len(), game.players.len());
        assert_eq!(replay.samples[0].actors.len(), game.players.len());
    }
}
