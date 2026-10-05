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
    let launch = client.launch;
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
            let seats = frame
                .seat_commands()
                .into_iter()
                .map(|(player, command)| {
                    let at_desk = launch.is_some_and(|launch| launch.is_architect(player));
                    (
                        observed_match::ascent::facility::seat_for(
                            &runtime.match_state,
                            player,
                            at_desk,
                        ),
                        command,
                    )
                })
                .collect();
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
        /// The Architect's desk, when this peer's human sits at it.
        desk: Option<crate::hex_wfc::architect::ArchitectDesk>,
        ask: crate::hex_wfc::ask::AskTheArchitect,
    }

    impl Peer {
        fn join(address: SocketAddr, account: u16, hash: [u8; 32]) -> Self {
            let mut lan = LanRuntime::new();
            lan.client =
                Some(LanClient::connect(address, account, None, None, hash).expect("client binds"));
            Self {
                lan,
                runtime: None,
                desk: None,
                ask: crate::hex_wfc::ask::AskTheArchitect::default(),
            }
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
                .then(|| crate::hex_wfc::ascent::lan_rules(&mut match_state, &launch))
                .flatten();
            assert_eq!(ascent.is_some(), launch.ascent);
            client
                .mark_launch_ready(launch.match_number)
                .expect("prepared");
            // At the desk if the launch says so, as `ascent::seat` would put them.
            let team = match_state.players[&local].team;
            self.desk = launch.is_architect(local).then(|| {
                crate::hex_wfc::architect::ArchitectDesk::new(
                    crate::hex_wfc::ascent::architect_seat(team),
                    observed_match::ascent::sim::TeamId(team.0),
                    0,
                )
            });
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
            let command = runtime.bot_command(local);
            let seats = (self.desk.as_mut(), Some(&mut self.ask));
            let leave = step(runtime, &mut self.lan, None, command, seats);
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
        let mut caught_up: Option<usize> = None;
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
            // How long the joiner takes to come into step: instrumentation, timed, so
            // printed rather than asserted (the server streams a lagging client several
            // bundles a tick, which `observed_server` tests deterministically).
            if let (Some(late), Some(game)) = (peers.get(2), server.match_state())
                && let Some(runtime) = late.runtime.as_ref()
                && caught_up.is_none()
                && runtime.match_state.tick + observed_net::lan::INPUT_LEAD_TICKS >= game.tick
            {
                caught_up = Some(played - 1_500);
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
            "LAN soak: {server_tick} ticks, {} peers in step, catches {}, the late joiner in \
             step {caught_up:?} ticks after joining 1500 behind",
            peers.len(),
            server.match_state().map_or(0, |game| game
                .prison
                .as_ref()
                .map_or(0, |prison| prison.mazes.len()))
        );
    }

    /// One player claims the team's Architect desk in the lobby and plays a card from it,
    /// and another body asks for help, all through the real wire: every peer applies both
    /// on the same tick, digest for digest with the server.
    #[test]
    fn a_human_architect_s_play_and_a_body_s_ask_travel_over_lan() {
        use observed_match::ascent::sim::ArchitectCommand;

        let config = ServerConfig::from_args(
            [
                "observed_server",
                "--bind",
                "127.0.0.1:0",
                "--no-discovery",
                "--ascent",
                "--teams",
                "1",
                "--team-size",
                "3",
            ]
            .map(str::to_owned),
        )
        .expect("server arguments");
        let mut server = AuthoritativeServer::bind(config).expect("server binds");
        let address = server.local_addr().expect("server address");
        let hash = crate::hex_wfc::sim::simulation_content_hash();
        let mut peers = vec![
            Peer::join(address, 911, hash),
            Peer::join(address, 912, hash),
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
        // The first takes the desk, and the roster says so before anyone is ready.
        peers[0].client().claim_architect(true).expect("claim");
        for _ in 0..120 {
            drive(&mut server, &mut peers);
        }
        assert!(
            server.session.seats.iter().any(|seat| seat.architect),
            "the server honoured the claim"
        );
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
        assert!(peers[0].desk.is_some(), "the claimant sits at the desk");
        assert!(peers[1].desk.is_none(), "the other walks");

        // Once the team has mapped somewhere, the Architect plays and the body asks.
        let mut played = false;
        let mut asked = false;
        for _ in 0..6_000 {
            drive(&mut server, &mut peers);
            let runtime = peers[0].runtime.as_ref().expect("launched");
            let desk = peers[0].desk.as_ref().expect("the desk");
            if !played
                && let Some(ascent) = runtime.ascent.as_ref()
                && let Some(play) = ascent.session().hands.get(&desk.team).and_then(|hand| {
                    let known = &ascent.rules().team_knowledge.get(&desk.team)?.cells;
                    hand.deck.hand.iter().find_map(|card| {
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
                })
            {
                peers[0].desk.as_mut().expect("the desk").pending = Some(play);
                played = true;
            }
            if !asked && played {
                peers[1].ask.pending = true;
                asked = true;
            }
            let logged = |peer: &Peer| {
                peer.runtime
                    .as_ref()
                    .and_then(|runtime| runtime.ascent.as_ref())
                    .map_or(0, |ascent| ascent.rules().command_log.len())
            };
            if played && asked && peers.iter().all(|peer| logged(peer) > 0) {
                break;
            }
        }
        assert!(played && asked, "a play and an ask were made");
        for _ in 0..120 {
            drive(&mut server, &mut peers);
        }
        let game = server.match_state().expect("the match");
        for (index, peer) in peers.iter().enumerate() {
            let runtime = peer.runtime.as_ref().expect("playing");
            let ascent = runtime.ascent.as_ref().expect("the rules");
            assert_eq!(
                (
                    runtime.match_state.tick,
                    runtime.match_state.snapshot().digest
                ),
                (game.tick, game.snapshot().digest),
                "peer {index} in step"
            );
            assert!(
                !ascent.rules().command_log.is_empty(),
                "peer {index} applied the Architect's play"
            );
        }
        let asker = peers[1].runtime.as_ref().expect("playing").local_player;
        let request = peers[0]
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.ascent.as_ref())
            .map(|ascent| ascent.session().requests.get(&asker).copied());
        assert!(
            request.flatten().is_some() || peers[1].ask.refused.is_some(),
            "the ask reached the Architect's rules, or came back refused"
        );
    }
}
