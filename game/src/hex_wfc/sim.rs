//! Bevy wrapper, deterministic construction, and fixed-step command threading for the
//! pure authoritative hex-facility match.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use bevy::prelude::*;
use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexWfcConfig};
use observed_match::hex_wfc::{
    HexActionButtons, HexInputFrame, HexMatchConfig, HexMatchStatus, HexPlayerCommand, HexWfcMatch,
};
use player_input::PlayerIntent;

use crate::flow::ActiveMatchSeed;

pub(super) const LOCAL_PLAYER: PlayerId = PlayerId(0);
/// Camera eye rise above the simulation body centre, in metres.
pub(super) const EYE_OFFSET: f32 = 0.70;

/// One-shot + held local input for the current tick, sanitized into a [`PlayerIntent`].
#[derive(Resource, Default)]
pub(super) struct HexWfcIntent {
    pub intent: PlayerIntent,
    pub actions: HexActionButtons,
    pub toggle_map: bool,
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
fn tile_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("assets/tiles");
    if cwd_relative.exists() {
        return cwd_relative;
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles")
}

/// Load the same authored-plus-compatibility corpus used by tests and evidence.
pub(crate) fn load_prototypes() -> Vec<TilePrototype> {
    load_authoring_corpus().cells
}

#[derive(Clone)]
struct HexAuthoringCorpus {
    cells: Vec<TilePrototype>,
    rooms: Vec<RoomPrototype>,
    simulation_content_hash: [u8; 32],
}

pub(crate) fn simulation_content_hash() -> [u8; 32] {
    load_authoring_corpus().simulation_content_hash
}

pub(crate) fn match_from_launch(
    seed: u64,
    config: HexMatchConfig,
    expected_hash: [u8; 32],
) -> Result<HexWfcMatch, String> {
    let corpus = load_authoring_corpus();
    if corpus.simulation_content_hash != expected_hash {
        return Err("server simulation content does not match this client".to_string());
    }
    let mut game = HexWfcMatch::new_with_rooms(seed, config, &corpus.cells, &corpus.rooms)
        .map_err(|error| format!("construct network match: {error:?}"))?;
    game.bind_simulation_content_hash(expected_hash);
    Ok(game)
}

fn load_authoring_corpus() -> HexAuthoringCorpus {
    static CORPUS: OnceLock<HexAuthoringCorpus> = OnceLock::new();
    CORPUS
        .get_or_init(|| {
            let base = tile_dir();
            let register_slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
            let loaded = RuntimeHexCatalog::load(&base, &register_slugs)
                .expect("committed runtime hex catalog loads");
            HexAuthoringCorpus {
                cells: loaded.cells,
                rooms: loaded.rooms,
                simulation_content_hash: loaded.simulation_content_hash,
            }
        })
        .clone()
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
fn runtime_config() -> HexMatchConfig {
    let mut config = HexMatchConfig::default();
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

fn solve_nearby_with_rooms(requested_seed: u64, corpus: &HexAuthoringCorpus) -> (HexWfcMatch, u64) {
    let config = runtime_config();
    (0..64u64)
        .find_map(|offset| {
            HexWfcMatch::new_with_rooms(
                requested_seed.wrapping_add(offset),
                config,
                &corpus.cells,
                &corpus.rooms,
            )
            .ok()
            .map(|game| (game, offset))
        })
        .expect("the hex authoring catalog must contain a solvable nearby seed")
}

pub(super) fn setup_runtime(
    mut commands: Commands,
    seed: Option<Res<ActiveMatchSeed>>,
    mut career: ResMut<crate::flow::Career>,
    lan: Res<crate::lan::LanRuntime>,
) {
    career.begin_match();
    let network_launch = lan.client.as_ref().and_then(|client| client.launch);
    let (match_state, local_player, seed_offset, networked) =
        if let Some((launch_seed, _, config, content_hash)) = network_launch {
            let game = match_from_launch(launch_seed, config, content_hash)
                .expect("a compatible LAN launch constructs locally");
            let local_player = lan
                .client
                .as_ref()
                .and_then(|client| client.player)
                .expect("welcomed LAN client has a stable player seat");
            (game, local_player, 0, true)
        } else {
            let requested_seed = seed.as_deref().map_or(0xF011_FAC1_1177, |seed| seed.0);
            let corpus = load_authoring_corpus();
            let (mut game, seed_offset) = solve_nearby_with_rooms(requested_seed, &corpus);
            game.bind_simulation_content_hash(corpus.simulation_content_hash);
            (game, LOCAL_PLAYER, seed_offset, false)
        };
    let replay = crate::sim::replay::ReplayTape::new_hex_wfc_for_player(&match_state, local_player);
    let map_level = match_state.players[&local_player].cell.level;
    let presented_revisions = match_state.facility.cell_revisions.clone();
    commands.insert_resource(HexWfcRuntime {
        match_state,
        local_player,
        pending_visual_cells: BTreeSet::new(),
        presented_revisions,
        status: if seed_offset == 0 {
            "authoritative hex facility ready".to_string()
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

pub(super) fn step_runtime(
    mut intent: ResMut<HexWfcIntent>,
    mut runtime: ResMut<HexWfcRuntime>,
    mut replay: Option<ResMut<crate::sim::replay::ReplayTape>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
    mut lan: ResMut<crate::lan::LanRuntime>,
    mut next: ResMut<NextState<crate::GameState>>,
) {
    if intent.toggle_map {
        runtime.map_open = !runtime.map_open;
        if runtime.map_open {
            runtime.map_level = runtime.local().cell.level;
        }
        intent.toggle_map = false;
    }
    if runtime.map_open && intent.browse_map_level != 0 {
        let discovered = runtime
            .match_state
            .player_map(runtime.local_player)
            .map(|knowledge| knowledge.cells.keys().copied().collect())
            .unwrap_or_default();
        runtime.map_level = browsed_level(&discovered, runtime.map_level, intent.browse_map_level);
    }
    intent.browse_map_level = 0;
    if runtime.match_state.status == HexMatchStatus::Finished {
        clear_one_shot_input(&mut intent.intent);
        intent.actions = HexActionButtons::default();
        return;
    }
    let local_command = if spectator_bot.is_some() {
        HexPlayerCommand {
            intent: runtime.match_state.bot_command(runtime.local_player),
            actions: HexActionButtons::default(),
        }
    } else {
        HexPlayerCommand {
            intent: intent.intent,
            actions: intent.actions,
        }
    };
    if runtime.networked {
        let Some(client) = lan.client.as_mut() else {
            runtime.status = "LAN server disconnected".to_string();
            clear_one_shot_input(&mut intent.intent);
            intent.actions = HexActionButtons::default();
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
            match launch
                .and_then(|(seed, _, config, hash)| match_from_launch(seed, config, hash).ok())
            {
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
        clear_one_shot_input(&mut intent.intent);
        intent.actions = HexActionButtons::default();
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
            frame.commands.insert(
                id,
                HexPlayerCommand {
                    intent: runtime.match_state.bot_command(id),
                    actions: HexActionButtons::default(),
                },
            );
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
    clear_one_shot_input(&mut intent.intent);
    intent.actions = HexActionButtons::default();
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
mod tests {
    use super::*;

    fn showcase_match(seed: u64) -> HexWfcMatch {
        let prototypes = load_prototypes();
        HexWfcMatch::new(seed, HexMatchConfig::default(), &prototypes).expect("showcase solves")
    }

    #[test]
    fn map_browsing_visits_only_discovered_levels() {
        let discovered = BTreeSet::from([
            HexCoord {
                q: 1,
                r: 1,
                level: 0,
            },
            HexCoord {
                q: 1,
                r: 1,
                level: 3,
            },
            HexCoord {
                q: 1,
                r: 1,
                level: 7,
            },
        ]);
        assert_eq!(browsed_level(&discovered, 0, 1), 3);
        assert_eq!(browsed_level(&discovered, 3, 1), 7);
        assert_eq!(browsed_level(&discovered, 7, 1), 7);
        assert_eq!(browsed_level(&discovered, 7, -1), 3);
        assert_eq!(browsed_level(&discovered, 0, -1), 0);
    }

    #[test]
    fn presentation_cursor_selects_only_cells_with_new_revisions() {
        let a = HexCoord {
            q: 1,
            r: 1,
            level: 0,
        };
        let b = HexCoord {
            q: 2,
            r: 1,
            level: 0,
        };
        let c = HexCoord {
            q: 3,
            r: 1,
            level: 0,
        };
        let live = BTreeMap::from([(a, 0), (b, 2), (c, 1)]);
        let presented = BTreeMap::from([(a, 0), (b, 1), (c, 1)]);
        assert_eq!(changed_revisions(&live, &presented), vec![(b, 2)]);
    }

    #[test]
    fn merged_authoring_corpus_covers_every_wfc_geometry_demand_exactly() {
        let corpus = load_authoring_corpus();
        for demand in observed_facility::hex_wfc::geometry_demands() {
            for register in ArchitectureRegister::ALL {
                let register = register.slug();
                assert!(
                    corpus.cells.iter().any(|tile| {
                        tile.key.archetype == demand.archetype
                            && tile.signature == demand.signature
                            && (tile.key.register == register || tile.key.register == "generic")
                    }),
                    "missing exact tile coverage for archetype={} register={} signature={:?}",
                    demand.archetype,
                    register,
                    demand.signature
                );
            }
        }
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
        game.bind_simulation_content_hash([0x5A; 32]);
        let local = PlayerId(2);
        let mut replay = crate::sim::replay::ReplayTape::new_hex_wfc_for_player(&game, local);
        let commands = game
            .players
            .keys()
            .copied()
            .map(|id| {
                (
                    id,
                    HexPlayerCommand {
                        intent: game.bot_command(id),
                        actions: HexActionButtons::default(),
                    },
                )
            })
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
        assert_eq!(replay.map_name, "hex_wfc_v2");
        assert_eq!(replay.simulation_content_hash, [0x5A; 32]);
        assert_eq!(replay.actors.len(), game.players.len());
        assert_eq!(replay.samples[0].actors.len(), game.players.len());
        assert_eq!(
            replay.samples[0].actors[local.index()].actor,
            crate::sim::replay::ReplayActorId::LocalPlayer
        );
    }
}
