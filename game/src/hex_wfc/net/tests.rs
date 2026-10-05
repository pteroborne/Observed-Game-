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
    replay: Option<crate::sim::replay::ReplayTape>,
}

impl Peer {
    fn join(address: SocketAddr, account: u16, hash: [u8; 32]) -> Self {
        Self::join_role(
            address,
            account,
            hash,
            observed_core::lan::LanRole::Observer,
        )
    }
    fn join_role(
        address: SocketAddr,
        account: u16,
        hash: [u8; 32],
        role: observed_core::lan::LanRole,
    ) -> Self {
        let mut lan = LanRuntime::new();
        lan.client = Some(
            LanClient::connect_with_role(
                address,
                account,
                None,
                None,
                hash,
                observed_core::cosmetics::CosmeticLook::default(),
                role,
            )
            .expect("client binds"),
        );
        Self {
            lan,
            runtime: None,
            desk: None,
            replay: None,
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
        let local = client.view_player(&launch).expect("a view anchor");
        let architect = client.is_architect();
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
        self.desk = architect.then(|| {
            crate::hex_wfc::architect::ArchitectDesk::new(
                crate::hex_wfc::ascent::architect_seat(team),
                observed_match::ascent::sim::TeamId(team.0),
                0,
            )
        });
        let mut replay = if architect {
            crate::sim::replay::ReplayTape::new_hex_wfc_for_architect(&match_state, local)
        } else {
            crate::sim::replay::ReplayTape::new_hex_wfc_for_player(&match_state, local)
        };
        if let Some(rules) = &ascent {
            replay.ascent_result = Some(super::super::ascent::completion_for(
                rules,
                &match_state,
                local,
                architect,
                false,
            ));
            replay.record_ascent(&match_state, rules);
        } else {
            replay.record_hex_wfc(&match_state);
        }
        self.replay = Some(replay);
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
        let leave = step(runtime, &mut self.lan, self.replay.as_mut(), command, seats);
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
            "--seed",
            "42",
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
        let finished = server
            .match_state()
            .is_none_or(|game| game.status == observed_match::hex_wfc::HexMatchStatus::Finished);
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
        let tape = peer.replay.as_ref().expect("recorded LAN replay");
        assert!(tape.ascent_result.is_some());
        assert!(
            tape.scene_frames
                .windows(2)
                .all(|frames| frames[0].tick < frames[1].tick)
        );
        let frame = tape.scene_frames.last().expect("physical LAN frames");
        assert!(server_tick.saturating_sub(frame.tick) < 6);
        assert_eq!(
            frame.facility.generation,
            runtime.match_state.facility.generation
        );
        assert!(frame.bodies.iter().all(|body| body.actor
            == crate::sim::replay::ReplayActorId::LocalPlayer
            || body.player != runtime.local_player));
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
        Peer::join_role(address, 911, hash, observed_core::lan::LanRole::Architect),
        Peer::join(address, 912, hash),
        Peer::join(address, 913, hash),
        Peer::join(address, 914, hash),
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
    assert!(peers[0].client().is_architect());
    assert_eq!(peers[0].client().player, None);
    assert_eq!(server.session.human_count(), 4);
    assert_eq!(server.session.seats.len(), 3);
    for _ in 0..120 {
        drive(&mut server, &mut peers);
    }
    assert!(
        server
            .session
            .architects
            .iter()
            .any(|seat| seat.occupant.connected_human().is_some()),
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
    assert!(
        peers[1..].iter().all(|peer| peer.desk.is_none()),
        "all three Observers walk"
    );
    let ids: std::collections::BTreeSet<_> = peers[1..]
        .iter()
        .map(|peer| peer.runtime.as_ref().unwrap().local_player)
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(
        peers[0]
            .replay
            .as_ref()
            .unwrap()
            .scene_frames
            .iter()
            .flat_map(|frame| &frame.bodies)
            .all(|body| body.actor != crate::sim::replay::ReplayActorId::LocalPlayer)
    );

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
        assert!(
            peer.replay
                .as_ref()
                .unwrap()
                .markers
                .iter()
                .any(|m| m.label.contains("Architect played") && m.cell.is_some()),
            "peer {index} recorded the Architect's named card and target"
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
