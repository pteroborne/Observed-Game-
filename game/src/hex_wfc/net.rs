//! A LAN match's tick on this machine: send the local command ahead, then replay the
//! server's authoritative frames, checking each against the server's digest.
//!
//! The server steps the canonical match; every client steps the same frames through the
//! same simulation - and, when the launch says the match plays Architect Ascent, through
//! the same rules, built identically from the launch with a bot Architect for every team
//! (`ascent::seat`, `ascent::lan_rules`). A digest mismatch replays the whole history
//! once from the launch, rules and all; a second disconnects.

use observed_match::hex_wfc::HexPlayerCommand;

use super::sim::{HexWfcRuntime, match_from_launch, record_generation_changes};

/// One networked tick. Returns whether the player must leave: the server is gone for
/// good, or this machine cannot stay in step with it.
pub(super) fn step(
    runtime: &mut HexWfcRuntime,
    lan: &mut crate::lan::LanRuntime,
    mut replay: Option<&mut crate::sim::replay::ReplayTape>,
    local_command: HexPlayerCommand,
) -> bool {
    let Some(client) = lan.client.as_mut() else {
        runtime.status = "LAN server disconnected".to_string();
        return false;
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
        let input = frame.to_input_frame();
        // Through the rules when the match plays them, as the server steps it.
        if !super::ascent::step(runtime, &input, None, None) {
            runtime.match_state.step(&input);
        }
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
        record_generation_changes(runtime, previous_generation);
    }
    if request_resync {
        let launch = client.launch;
        match launch.and_then(|launch| {
            match_from_launch(launch.seed, launch.config, launch.simulation_content_hash)
                .ok()
                .map(|game| (game, launch.ascent))
        }) {
            Some((match_state, ascent)) => {
                runtime.match_state = match_state;
                // The rules start again from the launch too, or the replay diverges.
                runtime.ascent = ascent
                    .then(|| super::ascent::lan_rules(&mut runtime.match_state))
                    .flatten();
                runtime.bot_driver.reset();
                runtime.presented_revisions = runtime.match_state.facility.cell_revisions.clone();
                runtime.pending_visual_cells = runtime
                    .match_state
                    .facility
                    .placements
                    .keys()
                    .copied()
                    .collect();
                runtime.map_level = runtime.local().cell.level;
                runtime.resync_attempts = runtime.resync_attempts.saturating_add(1);
                if let Some(replay) = replay {
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
    repeated_desync
}
