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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::net::SocketAddr;

    use observed_net::lan::LanClient;
    use observed_server::{AuthoritativeServer, ServerConfig};

    use super::*;
    use crate::lan::LanRuntime;

    /// A game client: its LAN transport, and the match it builds from the launch.
    struct Peer {
        lan: LanRuntime,
        runtime: Option<HexWfcRuntime>,
    }

    impl Peer {
        fn join(address: SocketAddr, account: u16, hash: [u8; 32]) -> Self {
            let mut lan = LanRuntime::new();
            lan.client =
                Some(LanClient::connect(address, account, None, None, hash).expect("client binds"));
            Self { lan, runtime: None }
        }

        fn client(&mut self) -> &mut LanClient {
            self.lan.client.as_mut().expect("connected")
        }

        /// Build the match - and its rules - from the launch alone, as the game's loading
        /// screen and `ascent::seat` do, and say it is prepared.
        fn prepare(&mut self) {
            let client = self.client();
            let launch = client.launch.expect("a launch");
            let local = client.player.expect("a seat");
            let mut match_state =
                match_from_launch(launch.seed, launch.config, launch.simulation_content_hash)
                    .expect("the launch reconstructs");
            let ascent = launch
                .ascent
                .then(|| crate::hex_wfc::ascent::lan_rules(&mut match_state))
                .flatten();
            assert_eq!(ascent.is_some(), launch.ascent);
            client
                .mark_launch_ready(launch.match_number)
                .expect("prepared");
            self.runtime = Some(HexWfcRuntime {
                presented_revisions: match_state.facility.cell_revisions.clone(),
                match_state,
                bot_driver: observed_match::hex_wfc::HexBotDriver::new(),
                local_player: local,
                pending_visual_cells: BTreeSet::new(),
                status: String::new(),
                map_open: false,
                map_level: 0,
                results_delay_frames: 0,
                networked: true,
                resync_attempts: 0,
                ascent,
                viewed_player: None,
            });
        }

        /// One tick of the game's own networked step, the body walked by the game's bot
        /// as a player would walk it.
        fn tick(&mut self) {
            let Some(runtime) = self.runtime.as_mut() else {
                self.client().poll();
                return;
            };
            let local = runtime.local_player;
            let command = runtime.bot_driver.command(&runtime.match_state, local);
            let leave = step(runtime, &mut self.lan, None, command);
            assert!(!leave, "a client was dropped: {}", runtime.status);
            assert_eq!(
                runtime.resync_attempts, 0,
                "a client parted from the server: {}",
                runtime.status
            );
        }
    }

    /// Two players and a late joiner play Architect Ascent against a real server over
    /// loopback UDP, each through the game's own networked step, and stay in step with
    /// the server throughout: the whole path a LAN playtest takes, headless.
    #[test]
    fn an_ascent_lan_match_stays_in_step_for_every_client_and_a_late_joiner() {
        // The dedicated server as a host would start it, on loopback.
        let config = ServerConfig::from_args(
            [
                "observed_server",
                "--bind",
                "127.0.0.1:0",
                "--no-discovery",
                "--ascent",
            ]
            .map(str::to_owned),
        )
        .expect("server arguments");
        let mut server = AuthoritativeServer::bind(config).expect("server binds");
        let address = server.local_addr().expect("server address");
        let hash = crate::hex_wfc::sim::simulation_content_hash();
        let mut peers = vec![
            Peer::join(address, 901, hash),
            Peer::join(address, 902, hash),
        ];
        let drive = |server: &mut AuthoritativeServer, peers: &mut Vec<Peer>| {
            server.fixed_tick().expect("server tick");
            for peer in peers.iter_mut() {
                peer.tick();
            }
        };

        for _ in 0..600 {
            drive(&mut server, &mut peers);
            if peers.iter_mut().all(|peer| peer.client().token.is_some()) {
                break;
            }
        }
        for peer in &mut peers {
            peer.client().set_ready(true).expect("ready");
        }
        for _ in 0..2_000 {
            drive(&mut server, &mut peers);
            for peer in &mut peers {
                if peer.runtime.is_none() && peer.client().launch.is_some() {
                    peer.prepare();
                }
            }
            if peers.iter().all(|peer| peer.runtime.is_some()) {
                break;
            }
        }
        assert!(
            peers.iter().all(|peer| peer.runtime.is_some()),
            "both launched"
        );

        let mut joined_late = false;
        let mut played = 0;
        for _ in 0..8_000 {
            drive(&mut server, &mut peers);
            played += 1;
            // A third player joins a match already under way, and replays its history.
            if played == 1_500 {
                peers.push(Peer::join(address, 903, hash));
                joined_late = true;
            }
            if let Some(late) = peers.get_mut(2)
                && late.runtime.is_none()
                && late.client().launch.is_some()
            {
                late.prepare();
            }
            let finished = server.match_state().is_none_or(|game| {
                game.status == observed_match::hex_wfc::HexMatchStatus::Finished
            });
            if finished {
                break;
            }
        }
        // Let the stragglers take the last frames.
        for _ in 0..240 {
            drive(&mut server, &mut peers);
        }
        assert!(joined_late);
        let (server_tick, server_digest) = {
            let game = server.match_state().expect("the match");
            (game.tick, game.snapshot().digest)
        };
        assert!(
            server_tick > 1_000,
            "a real stretch of play: {server_tick} ticks"
        );
        for (index, peer) in peers.iter().enumerate() {
            let runtime = peer.runtime.as_ref().expect("every peer is playing");
            assert_eq!(
                (
                    runtime.match_state.tick,
                    runtime.match_state.snapshot().digest
                ),
                (server_tick, server_digest),
                "peer {index} ends where the server does"
            );
            assert!(runtime.ascent.is_some(), "peer {index} plays the rules");
        }
        eprintln!(
            "LAN soak: {server_tick} ticks, {} peers in step, catches {}",
            peers.len(),
            server.match_state().map_or(0, |game| game
                .prison
                .as_ref()
                .map_or(0, |prison| prison.mazes.len()))
        );
    }
}
