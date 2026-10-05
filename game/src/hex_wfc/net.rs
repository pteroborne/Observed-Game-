//! A LAN match's tick on this machine: send the local command ahead, then replay the
//! server's authoritative frames, checking each against the server's digest.
//!
//! The server steps the canonical match; every client steps the same frames through the
//! same simulation - and, when the launch says the match plays Architect Ascent, through
//! the same rules, built identically from the launch with a bot Architect for every team
//! (`ascent::seat`, `ascent::lan_rules`). A digest mismatch replays the whole history
//! once from the launch, rules and all; a second disconnects.

use observed_match::hex_wfc::HexPlayerCommand;
use observed_net::lan::WireSeatCommand;

use super::sim::{HexWfcRuntime, match_from_launch, record_generation_changes};

/// How long one networked tick may spend replaying frames when this machine is behind -
/// joining late, reconnecting or resyncing - before it lets the frame be drawn. It replays
/// about fifty ticks of match a second of this, so a joiner catches up at a dozen or more
/// ticks a tick, where a fixed window of sixteen frames let it gain only fifteen at best.
const CATCH_UP_BUDGET: std::time::Duration = std::time::Duration::from_millis(6);

/// One networked tick. Returns whether the player must leave: the server is gone for
/// good, or this machine cannot stay in step with it.
pub(super) fn step(
    runtime: &mut HexWfcRuntime,
    lan: &mut crate::lan::LanRuntime,
    mut replay: Option<&mut crate::sim::replay::ReplayTape>,
    local_command: HexPlayerCommand,
    (mut desk, mut ask): (
        Option<&mut super::architect::ArchitectDesk>,
        Option<&mut super::ask::AskTheArchitect>,
    ),
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
    // The desk's play or answer, or the body's ask, rides with the body's command; the
    // server puts it in the frame, and it is applied when the frame comes back.
    let said = super::ascent::local_seat_command(runtime, desk.as_deref_mut(), ask.as_deref_mut())
        .map_or(WireSeatCommand::None, |(_, command)| {
            WireSeatCommand::from_seat(command)
        });
    if let Err(error) = client.queue_input(target_tick, local_command, said) {
        runtime.status = format!("LAN input error: {error}");
    }
    let started = std::time::Instant::now();
    let mut request_resync = false;
    let mut repeated_desync = false;
    // A window of frames at a time, and another while there are more and the budget lasts:
    // in step there is a frame or two; behind, the history the server streams.
    'replay: loop {
        let frames = client.take_ready_frames(observed_net::lan::FRAME_WINDOW);
        let more = frames.len() == observed_net::lan::FRAME_WINDOW;
        for frame in frames {
            let previous_generation = runtime.match_state.facility.generation;
            let input = frame.to_input_frame();
            // Through the rules when the match plays them, as the server steps it, with every
            // seat's say mapped to its rules seat the way the server maps it.
            let mut seats: std::collections::BTreeMap<_, _> =
                frame.seat_commands().into_iter().collect();
            let human: Vec<_> = frame.architects.iter().map(|(team, _)| *team).collect();
            if let Some(rules) = runtime.ascent.as_mut() {
                rules.set_human_architects(&human);
            }
            for (team, command) in &frame.architects {
                if let Some(command) = command.to_seat() {
                    seats.insert(
                        observed_match::ascent::facility::architect_seat(*team),
                        command,
                    );
                }
            }
            if !super::ascent::apply(
                runtime,
                &input,
                seats,
                desk.as_deref_mut(),
                ask.as_deref_mut(),
            ) {
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
                break 'replay;
            }
            if let Some(replay) = replay.as_deref_mut() {
                if let Some(rules) = &runtime.ascent {
                    replay.record_ascent(&runtime.match_state, rules);
                } else {
                    replay.record_hex_wfc(&runtime.match_state);
                }
            }
            record_generation_changes(runtime, previous_generation);
        }
        if !more || started.elapsed() >= CATCH_UP_BUDGET {
            break;
        }
    }
    if request_resync {
        let launch = client.launch;
        match launch.and_then(|launch| {
            match_from_launch(launch.seed, launch.config, launch.simulation_content_hash)
                .ok()
                .map(|game| (game, launch))
        }) {
            Some((match_state, launch)) => {
                runtime.match_state = match_state;
                // The rules start again from the launch too, or the replay diverges.
                runtime.ascent = launch
                    .ascent
                    .then(|| super::ascent::lan_rules(&mut runtime.match_state, &launch))
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
                    let previous_role = replay.ascent_result.map(|f| f.role);
                    let cosmetics = replay.cosmetics.clone();
                    *replay = if client.is_architect() {
                        crate::sim::replay::ReplayTape::new_hex_wfc_for_architect(
                            &runtime.match_state,
                            runtime.local_player,
                        )
                    } else {
                        crate::sim::replay::ReplayTape::new_hex_wfc_for_player(
                            &runtime.match_state,
                            runtime.local_player,
                        )
                    };
                    replay.cosmetics = cosmetics;
                    if let Some(rules) = &runtime.ascent {
                        replay.ascent_result = Some(super::ascent::completion_for(
                            rules,
                            &runtime.match_state,
                            runtime.local_player,
                            client.is_architect(),
                            previous_role == Some(crate::flow::AscentResultRole::Spectator),
                        ));
                        replay.record_ascent(&runtime.match_state, rules);
                    } else {
                        replay.record_hex_wfc(&runtime.match_state);
                    }
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

#[cfg(test)]
mod tests;
