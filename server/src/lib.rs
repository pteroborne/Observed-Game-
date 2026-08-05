//! Bevy-free authoritative LAN server shared by the dedicated binary and listen host.

use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::HexWfcConfig;
use observed_match::hex_wfc::{
    HEX_INPUT_VERSION, HexBotDriver, HexInputFrame, HexMatchConfig, HexMatchContent,
    HexMatchStatus, HexPlayerCommand, HexWfcMatch,
};
use observed_net::lan::{
    LanPacket, LobbyAction, MAX_DATAGRAM, WireFrame, WireHexCommand, WirePhase, WireSeat,
    WireSeatOccupant, frames_per_bundle,
};
use observed_progression::session::lan::{
    LAN_DEFAULT_ROSTER, LAN_MAX_SEATS, LAN_RECONNECT_GRACE_TICKS, LanRoster,
};
use observed_progression::session::{AccountId, LanPhase, LanSeatOccupant, LanSession, SessionId};

pub const SERVER_HZ: u64 = 60;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub name: String,
    /// Teams and seats per team. One team is co-op.
    pub roster: LanRoster,
    pub min_humans: u8,
    /// When false, every configured seat must be occupied by a connected human
    /// before countdown and bot seats are presented as explicitly empty.
    pub fill_empty_seats: bool,
    pub base_seed: u64,
    pub discovery: bool,
    pub tile_dir: PathBuf,
    match_config: HexMatchConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from((Ipv4Addr::UNSPECIFIED, observed_net::lan::DEFAULT_LAN_PORT)),
            name: "Observed 2 LAN".to_string(),
            roster: LAN_DEFAULT_ROSTER,
            min_humans: 1,
            fill_empty_seats: true,
            base_seed: time_seed(),
            discovery: true,
            tile_dir: default_tile_dir(),
            match_config: HexMatchConfig {
                guardian: true,
                teams: 2,
                members_per_team: 2,
                wfc: HexWfcConfig::arc_default(),
            },
        }
    }
}

impl ServerConfig {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let mut args = args.into_iter();
        let _binary = args.next();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--bind" => {
                    config.bind = args
                        .next()
                        .ok_or("--bind requires an IP:port")?
                        .parse()
                        .map_err(|_| "--bind is not a valid IP:port")?;
                }
                "--name" => config.name = args.next().ok_or("--name requires text")?,
                "--min-humans" => {
                    config.min_humans = args
                        .next()
                        .ok_or("--min-humans requires 1..16")?
                        .parse::<u8>()
                        .map_err(|_| "--min-humans requires 1..16")?
                        .clamp(1, LAN_MAX_SEATS as u8);
                }
                "--teams" => {
                    config.roster.teams = args
                        .next()
                        .ok_or("--teams requires 1..16")?
                        .parse::<u8>()
                        .map_err(|_| "--teams requires 1..16")?;
                }
                "--team-size" => {
                    config.roster.members_per_team = args
                        .next()
                        .ok_or("--team-size requires 1..16")?
                        .parse::<u8>()
                        .map_err(|_| "--team-size requires 1..16")?;
                }
                "--co-op" => {
                    let members = args
                        .next()
                        .ok_or("--co-op requires a team size of 1..16")?
                        .parse::<u8>()
                        .map_err(|_| "--co-op requires a team size of 1..16")?;
                    config.roster = LanRoster::co_op(members);
                }
                "--seed" => {
                    let value = args.next().ok_or("--seed requires an integer")?;
                    config.base_seed = parse_seed(&value)?;
                }
                "--tiles" => {
                    config.tile_dir = PathBuf::from(args.next().ok_or("--tiles requires a path")?)
                }
                "--no-discovery" => config.discovery = false,
                "--require-full-roster" => config.fill_empty_seats = false,
                "--no-guardian" => config.match_config.guardian = false,
                "--help" | "-h" => return Err(help_text().to_string()),
                unknown => return Err(format!("unknown option {unknown}\n{}", help_text())),
            }
        }
        if config.name.is_empty() || config.name.len() > 96 {
            return Err("--name must contain 1..96 bytes".to_string());
        }
        Ok(config)
    }
}

pub fn help_text() -> &'static str {
    "observed_server [--bind 0.0.0.0:47624] [--name TEXT] [--min-humans 1..16] [--teams 1..16] [--team-size 1..16] [--require-full-roster] [--seed INTEGER] [--tiles PATH] [--no-discovery]"
}

fn parse_seed(value: &str) -> Result<u64, String> {
    value
        .strip_prefix("0x")
        .map_or_else(|| value.parse::<u64>(), |hex| u64::from_str_radix(hex, 16))
        .map_err(|_| "--seed must be a decimal integer or 0x-prefixed hex".to_string())
}

fn time_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0xF011_FAC1_1177, |duration| duration.as_nanos() as u64)
}

fn default_tile_dir() -> PathBuf {
    observed_assets::assets_root().join("tiles")
}

#[derive(Clone, Debug)]
struct ClientConnection {
    account: AccountId,
    token: u64,
    player: PlayerId,
    address: SocketAddr,
    last_seen: Instant,
    ack_through: u64,
    synchronizing: bool,
    launch_ready_for: Option<u32>,
}

pub struct AuthoritativeServer {
    socket: UdpSocket,
    pub config: ServerConfig,
    content: Arc<HexMatchContent>,
    pub session: LanSession,
    clients: BTreeMap<u64, ClientConnection>,
    inputs: BTreeMap<(PlayerId, u64), WireHexCommand>,
    match_state: Option<HexWfcMatch>,
    bot_driver: HexBotDriver,
    launch_config: Option<HexMatchConfig>,
    launch_started: Option<u32>,
    frames: Vec<WireFrame>,
    server_tick: u64,
    token_cursor: u64,
}

impl AuthoritativeServer {
    pub fn bind(mut config: ServerConfig) -> Result<Self, String> {
        // The wire format used to require exactly 2v2, because a frame carried a
        // fixed four commands. It now carries a counted list, so the only limit
        // left is the cap the count byte and the datagram budget imply.
        let roster = config.roster.clamped();
        if !roster.is_valid() {
            return Err(format!(
                "a roster of {} teams x {} is not seatable (1..={LAN_MAX_SEATS} seats)",
                roster.teams, roster.members_per_team
            ));
        }
        config.match_config.teams = roster.teams;
        config.match_config.members_per_team = roster.members_per_team;
        let register_slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        let content = Arc::new(
            HexMatchContent::load(&config.tile_dir, &register_slugs)
                .map_err(|error| format!("load runtime hex catalog: {error}"))?,
        );
        let socket = UdpSocket::bind(config.bind)
            .map_err(|error| format!("bind {}: {error}", config.bind))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("nonblocking socket: {error}"))?;
        socket
            .set_broadcast(true)
            .map_err(|error| format!("broadcast socket: {error}"))?;
        let local = socket
            .local_addr()
            .map_err(|error| format!("local address: {error}"))?;
        let session_id =
            SessionId((config.base_seed as u32).rotate_left(7) ^ u32::from(local.port()));
        let min_humans = if config.fill_empty_seats {
            config.min_humans
        } else {
            roster.seats() as u8
        };
        Ok(Self {
            socket,
            session: LanSession::with_roster(session_id, min_humans, roster),
            config,
            content,
            clients: BTreeMap::new(),
            inputs: BTreeMap::new(),
            match_state: None,
            bot_driver: HexBotDriver::new(),
            launch_config: None,
            launch_started: None,
            frames: Vec::new(),
            server_tick: 0,
            token_cursor: time_seed() | 1,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    pub fn match_state(&self) -> Option<&HexWfcMatch> {
        self.match_state.as_ref()
    }

    pub fn run(mut self, stop: Arc<AtomicBool>) -> Result<(), String> {
        let frame = Duration::from_nanos(1_000_000_000 / SERVER_HZ);
        let mut next = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            self.poll_network();
            let now = Instant::now();
            if now >= next {
                self.fixed_tick()?;
                next += frame;
                if now > next + frame * 4 {
                    next = now + frame;
                }
            } else {
                thread::sleep((next - now).min(Duration::from_millis(2)));
            }
        }
        Ok(())
    }

    /// One deterministic server tick, public for labs and loopback integration tests.
    pub fn fixed_tick(&mut self) -> Result<(), String> {
        self.poll_network();
        self.server_tick = self.server_tick.wrapping_add(1);
        self.disconnect_timed_out_clients();
        self.session.expire_reservations(self.server_tick);
        let previous_phase = self.session.phase;
        let seed = self
            .config
            .base_seed
            .wrapping_add(u64::from(self.session.match_number + 1));
        if let Some(launch) = self.session.tick(seed) {
            self.start_match(launch.seed)?;
        }
        if matches!(self.session.phase, LanPhase::InMatch) {
            if self.launch_started.is_none() && self.launch_barrier_ready() {
                self.open_launch_barrier();
            }
            if self.launch_started == Some(self.session.match_number) {
                self.step_match();
            }
        } else if matches!(previous_phase, LanPhase::PostMatch { .. })
            && matches!(self.session.phase, LanPhase::Lobby)
        {
            self.match_state = None;
            self.launch_config = None;
            self.launch_started = None;
            self.frames.clear();
            self.inputs.clear();
        }
        if self.server_tick.is_multiple_of(15) || self.session.phase != previous_phase {
            self.broadcast_lobby();
            self.broadcast_launch_progress();
        }
        self.broadcast_frame_windows();
        Ok(())
    }

    pub fn poll_network(&mut self) {
        let mut buffer = [0_u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, address)) => {
                    if let Ok(packet) = LanPacket::decode(&buffer[..len]) {
                        self.handle_packet(address, packet);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    fn handle_packet(&mut self, address: SocketAddr, packet: LanPacket) {
        match packet {
            LanPacket::DiscoveryProbe if self.config.discovery => {
                let _ = self.send_to(
                    address,
                    &LanPacket::DiscoveryReply {
                        name: self.config.name.clone(),
                        phase: wire_phase(self.session.phase),
                        humans: self.session.human_count() as u8,
                        joinable: self.session.joinable(),
                    },
                );
            }
            LanPacket::Hello {
                account,
                requested_team,
                resume_token,
                input_version,
                simulation_content_hash,
            } => self.handle_hello(
                address,
                AccountId(account),
                requested_team,
                resume_token,
                input_version,
                simulation_content_hash,
            ),
            LanPacket::LobbyCommand { token, action } => {
                if let Some((account, _)) = self.validate_client(address, token) {
                    match action {
                        LobbyAction::Ready(ready) => {
                            self.session.set_ready(account, ready);
                        }
                        LobbyAction::RequestTeam(team) => {
                            if let Some(player) = self.session.request_team(account, team) {
                                if let Some(client) = self.clients.get_mut(&token) {
                                    client.player = player;
                                }
                                let phase = wire_phase(self.session.phase);
                                let live_tick =
                                    self.match_state.as_ref().map_or(0, |game| game.tick);
                                self.welcome(address, token, player, phase, live_tick);
                            }
                        }
                    }
                    self.broadcast_lobby();
                }
            }
            LanPacket::InputBundle { token, commands } => {
                if let Some((_, player)) = self.validate_client(address, token) {
                    for (tick, command) in commands {
                        if tick > self.match_state.as_ref().map_or(0, |game| game.tick)
                            && tick <= self.match_state.as_ref().map_or(0, |game| game.tick) + 120
                        {
                            self.inputs.entry((player, tick)).or_insert(command);
                        }
                    }
                }
            }
            LanPacket::Ack {
                token,
                through_tick,
            } => {
                let live_tick = self.match_state.as_ref().map_or(0, |game| game.tick);
                if self.validate_client(address, token).is_some()
                    && let Some(client) = self.clients.get_mut(&token)
                {
                    client.ack_through = client.ack_through.max(through_tick.min(live_tick));
                    if self.launch_started == Some(self.session.match_number)
                        && live_tick > 0
                        && client.ack_through >= live_tick
                    {
                        client.synchronizing = false;
                    }
                }
            }
            LanPacket::Resync { token } => {
                if self.validate_client(address, token).is_some() {
                    if let Some(client) = self.clients.get_mut(&token) {
                        client.ack_through = 0;
                        client.synchronizing = true;
                    }
                    self.send_launch_to(token);
                    self.broadcast_lobby();
                }
            }
            LanPacket::LaunchReady {
                token,
                match_number,
            } => {
                if self.validate_client(address, token).is_none() {
                    return;
                }
                if matches!(self.session.phase, LanPhase::InMatch)
                    && match_number == self.session.match_number
                    && self.match_state.is_some()
                {
                    if let Some(client) = self.clients.get_mut(&token) {
                        client.launch_ready_for = Some(match_number);
                    }
                    if self.launch_started == Some(match_number) {
                        self.send_launch_start_to(token);
                    }
                    self.broadcast_lobby();
                } else {
                    self.send_launch_to(token);
                }
            }
            LanPacket::Goodbye { token } => self.disconnect_token(address, token),
            _ => {}
        }
    }

    fn handle_hello(
        &mut self,
        address: SocketAddr,
        account: AccountId,
        requested_team: Option<TeamId>,
        resume_token: Option<u64>,
        input_version: u16,
        simulation_content_hash: [u8; 32],
    ) {
        if input_version != HEX_INPUT_VERSION {
            self.reject(address, "hex input version mismatch");
            return;
        }
        if simulation_content_hash != self.content.simulation_content_hash() {
            self.reject(address, "simulation content mismatch");
            return;
        }
        if let Some(token) = resume_token
            && let Some(existing) = self.clients.get(&token).cloned()
            && existing.account == account
            && self.session.reconnect(account, self.server_tick).is_some()
        {
            let phase = wire_phase(self.session.phase);
            let live_tick = self.match_state.as_ref().map_or(0, |game| game.tick);
            if let Some(client) = self.clients.get_mut(&token) {
                client.address = address;
                client.last_seen = Instant::now();
                client.synchronizing = matches!(self.session.phase, LanPhase::InMatch);
                client.ack_through = 0;
                client.launch_ready_for = None;
            }
            self.welcome(address, token, existing.player, phase, live_tick);
            self.send_launch_to(token);
            self.broadcast_lobby();
            return;
        }
        // A retried initial Hello uses no resume token. If its Welcome was the
        // datagram that vanished, answer idempotently instead of rejecting the
        // already-created seat.
        if let Some(existing) = self
            .clients
            .values()
            .find(|client| client.account == account && client.address == address)
            .cloned()
        {
            if let Some(client) = self.clients.get_mut(&existing.token) {
                client.last_seen = Instant::now();
            }
            let phase = wire_phase(self.session.phase);
            let live_tick = self.match_state.as_ref().map_or(0, |game| game.tick);
            self.welcome(address, existing.token, existing.player, phase, live_tick);
            self.send_launch_to(existing.token);
            return;
        }
        let player = match self.session.join(account, requested_team) {
            Ok(player) => player,
            Err(error) => {
                self.reject(address, &format!("join rejected: {error:?}"));
                return;
            }
        };
        let token = self.allocate_token(account, player);
        let phase = wire_phase(self.session.phase);
        let live_tick = self.match_state.as_ref().map_or(0, |game| game.tick);
        self.clients.insert(
            token,
            ClientConnection {
                account,
                token,
                player,
                address,
                last_seen: Instant::now(),
                ack_through: 0,
                synchronizing: matches!(self.session.phase, LanPhase::InMatch),
                launch_ready_for: None,
            },
        );
        self.welcome(address, token, player, phase, live_tick);
        self.send_launch_to(token);
        self.broadcast_lobby();
    }

    fn start_match(&mut self, seed: u64) -> Result<(), String> {
        let config = self.config.match_config;
        let (game, selected_seed) = (0..64u64)
            .find_map(|offset| {
                let seed = seed.wrapping_add(offset);
                HexWfcMatch::new_with_content(seed, config, Arc::clone(&self.content))
                    .ok()
                    .map(|game| (game, seed))
            })
            .ok_or("no solvable nearby server seed")?;
        self.config.base_seed = selected_seed.wrapping_sub(u64::from(self.session.match_number));
        self.match_state = Some(game);
        self.bot_driver.reset();
        self.launch_config = Some(config);
        self.launch_started = None;
        self.frames.clear();
        self.inputs.clear();
        for client in self.clients.values_mut().filter(|client| {
            self.session.seats[client.player.index()]
                .connected_human()
                .is_some()
        }) {
            client.ack_through = 0;
            client.synchronizing = true;
            client.launch_ready_for = None;
        }
        self.broadcast(&LanPacket::Launch {
            seed: selected_seed,
            match_number: self.session.match_number,
            config,
            simulation_content_hash: self.content.simulation_content_hash(),
        });
        Ok(())
    }

    /// The barrier follows the session's connected humans rather than the set of
    /// clients present when countdown ended. A join during preparation must prepare;
    /// a disconnect immediately stops being a reason to hold everyone else.
    fn launch_barrier_ready(&self) -> bool {
        let generation = self.session.match_number;
        self.session
            .seats
            .iter()
            .filter_map(|seat| seat.connected_human())
            .all(|account| {
                self.clients.values().any(|client| {
                    client.account == account && client.launch_ready_for == Some(generation)
                })
            })
    }

    fn open_launch_barrier(&mut self) {
        let generation = self.session.match_number;
        if self.match_state.is_none() || self.launch_started == Some(generation) {
            return;
        }
        self.launch_started = Some(generation);
        for client in self.clients.values_mut() {
            if client.launch_ready_for == Some(generation) {
                client.synchronizing = false;
            }
        }
        self.broadcast(&LanPacket::LaunchStart {
            match_number: generation,
        });
    }

    fn step_match(&mut self) {
        let human_controls = (0..self.session.seats.len())
            .map(|index| self.human_controls(PlayerId(index as u16)))
            .collect::<Vec<_>>();
        let Some(game) = self.match_state.as_mut() else {
            return;
        };
        if game.status == HexMatchStatus::Finished {
            return;
        }
        let tick = game.tick + 1;
        let mut wire = vec![WireHexCommand::default(); self.session.seats.len()];
        let mut commands = BTreeMap::new();
        for seat in &self.session.seats {
            let command = if human_controls[seat.player.index()] {
                self.bot_driver.clear_player(seat.player);
                self.inputs
                    .remove(&(seat.player, tick))
                    .map_or_else(HexPlayerCommand::default, WireHexCommand::to_command)
            } else {
                self.bot_driver.command(game, seat.player)
            };
            wire[seat.player.index()] = WireHexCommand::from_command(command);
            commands.insert(seat.player, command);
        }
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        let digest = game.snapshot().digest;
        self.frames.push(WireFrame {
            tick: game.tick,
            commands: wire,
            digest,
        });
        self.inputs.retain(|(_, frame), _| *frame > game.tick);
        if game.status == HexMatchStatus::Finished {
            let escape_order = game.escape_order.clone();
            self.broadcast(&LanPacket::MatchEnded { escape_order });
            self.session.finish_match();
            self.broadcast_lobby();
        }
    }

    fn human_controls(&self, player: PlayerId) -> bool {
        let Some(seat) = self.session.seats.get(player.index()) else {
            return false;
        };
        let Some(account) = seat.connected_human() else {
            return false;
        };
        self.clients.values().any(|client| {
            client.account == account && client.player == player && !client.synchronizing
        })
    }

    fn send_launch_to(&self, token: u64) {
        let (Some(game), Some(config), Some(client)) = (
            self.match_state.as_ref(),
            self.launch_config,
            self.clients.get(&token),
        ) else {
            return;
        };
        let _ = self.send_to(
            client.address,
            &LanPacket::Launch {
                seed: game.seed,
                match_number: self.session.match_number,
                config,
                simulation_content_hash: self.content.simulation_content_hash(),
            },
        );
    }

    fn send_launch_start_to(&self, token: u64) {
        let generation = self.session.match_number;
        let Some(client) = self.clients.get(&token) else {
            return;
        };
        if self.launch_started == Some(generation) {
            let _ = self.send_to(
                client.address,
                &LanPacket::LaunchStart {
                    match_number: generation,
                },
            );
        }
    }

    fn broadcast_launch_progress(&self) {
        if !matches!(self.session.phase, LanPhase::InMatch) || self.match_state.is_none() {
            return;
        }
        let generation = self.session.match_number;
        for client in self.clients.values() {
            if client.launch_ready_for != Some(generation) {
                self.send_launch_to(client.token);
            }
            if self.launch_started == Some(generation) && client.ack_through == 0 {
                self.send_launch_start_to(client.token);
            }
        }
    }

    fn broadcast_frame_windows(&self) {
        if self.frames.is_empty() {
            return;
        }
        for client in self.clients.values() {
            // Launch is idempotent and repeated until the first frame ACK so one
            // lost datagram cannot strand a welcomed client in the lobby UI.
            if client.ack_through == 0 {
                self.send_launch_to(client.token);
                self.send_launch_start_to(client.token);
            }
            let start = client.ack_through.saturating_add(1);
            let frames = self
                .frames
                .iter()
                .filter(|frame| frame.tick >= start)
                .take(frames_per_bundle(self.session.seats.len()))
                .cloned()
                .collect::<Vec<_>>();
            if !frames.is_empty() {
                let _ = self.send_to(client.address, &LanPacket::FrameBundle { frames });
            }
        }
    }

    fn broadcast_lobby(&self) {
        let countdown_ticks = match self.session.phase {
            LanPhase::Countdown { remaining } => remaining,
            _ => 0,
        };
        let seats = self
            .session
            .seats
            .iter()
            .map(|seat| WireSeat {
                player: seat.player,
                team: seat.team,
                occupant: match seat.occupant {
                    LanSeatOccupant::Bot if self.config.fill_empty_seats => WireSeatOccupant::Bot,
                    LanSeatOccupant::Bot => WireSeatOccupant::Empty,
                    LanSeatOccupant::Human {
                        connected: false, ..
                    } => WireSeatOccupant::ReservedHuman,
                    LanSeatOccupant::Human {
                        account,
                        connected: true,
                        ..
                    } => {
                        if self
                            .clients
                            .values()
                            .any(|client| client.account == account && client.synchronizing)
                        {
                            WireSeatOccupant::SynchronizingHuman
                        } else {
                            WireSeatOccupant::Human
                        }
                    }
                },
                ready: seat.ready,
            })
            .collect();
        self.broadcast(&LanPacket::LobbySnapshot {
            session: self.session.id.0,
            match_number: self.session.match_number,
            server_tick: self.server_tick,
            phase: wire_phase(self.session.phase),
            countdown_ticks,
            seats,
        });
    }

    fn disconnect_timed_out_clients(&mut self) {
        let awaiting_launch = self.awaiting_launch_barrier();
        let timed_out = self
            .clients
            .values()
            .filter(|client| client.last_seen.elapsed() > CLIENT_TIMEOUT)
            .map(|client| (client.token, client.account))
            .collect::<Vec<_>>();
        for (token, account) in timed_out {
            self.session.disconnect(account, self.server_tick);
            if awaiting_launch && !self.config.fill_empty_seats {
                self.clients.remove(&token);
                self.release_account_seat(account);
            } else if let Some(client) = self.clients.get_mut(&token) {
                client.synchronizing = true;
                client.launch_ready_for = None;
                client.last_seen =
                    Instant::now() + Duration::from_secs(LAN_RECONNECT_GRACE_TICKS / SERVER_HZ);
            }
        }
        if awaiting_launch && !self.config.fill_empty_seats {
            self.abort_pending_launch();
        }
    }

    fn disconnect_token(&mut self, address: SocketAddr, token: u64) {
        let awaiting_launch = self.awaiting_launch_barrier();
        if self
            .clients
            .get(&token)
            .is_some_and(|client| client.address == address)
            && let Some(client) = self.clients.remove(&token)
        {
            self.session.disconnect(client.account, self.server_tick);
            self.release_account_seat(client.account);
            if awaiting_launch && !self.config.fill_empty_seats {
                self.abort_pending_launch();
            }
        }
        self.broadcast_lobby();
    }

    fn awaiting_launch_barrier(&self) -> bool {
        matches!(self.session.phase, LanPhase::InMatch)
            && self.match_state.is_some()
            && self.launch_started != Some(self.session.match_number)
    }

    fn release_account_seat(&mut self, account: AccountId) {
        if let Some(seat) = self.session.seats.iter_mut().find(|seat| {
            matches!(
                seat.occupant,
                LanSeatOccupant::Human {
                    account: found,
                    ..
                } if found == account
            )
        }) {
            seat.occupant = LanSeatOccupant::Bot;
            seat.ready = false;
        }
    }

    fn abort_pending_launch(&mut self) {
        if !self.awaiting_launch_barrier() {
            return;
        }
        self.session.phase = LanPhase::Lobby;
        for seat in &mut self.session.seats {
            seat.ready = false;
        }
        for client in self.clients.values_mut() {
            client.ack_through = 0;
            client.synchronizing = false;
            client.launch_ready_for = None;
        }
        self.match_state = None;
        self.launch_config = None;
        self.launch_started = None;
        self.frames.clear();
        self.inputs.clear();
    }

    /// Validate a token/address pair, touch its heartbeat, and transparently reclaim
    /// a reserved seat after a transient timeout. In-match control stays with the bot
    /// until the client's authoritative-history acknowledgement catches up.
    fn validate_client(
        &mut self,
        address: SocketAddr,
        token: u64,
    ) -> Option<(AccountId, PlayerId)> {
        let (account, player) = {
            let client = self.clients.get_mut(&token)?;
            if client.address != address {
                return None;
            }
            client.last_seen = Instant::now();
            (client.account, client.player)
        };
        if self.session.reconnect(account, self.server_tick).is_some()
            && let Some(client) = self.clients.get_mut(&token)
        {
            client.ack_through = 0;
            client.synchronizing = matches!(self.session.phase, LanPhase::InMatch);
            client.launch_ready_for = None;
        }
        Some((account, player))
    }

    fn allocate_token(&mut self, account: AccountId, player: PlayerId) -> u64 {
        self.token_cursor = self
            .token_cursor
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left(17)
            ^ u64::from(account.0)
            ^ u64::from(player.0);
        self.token_cursor | 1
    }

    fn welcome(
        &self,
        address: SocketAddr,
        token: u64,
        player: PlayerId,
        phase: WirePhase,
        server_tick: u64,
    ) {
        let team = self.session.seats[player.index()].team;
        let _ = self.send_to(
            address,
            &LanPacket::Welcome {
                session: self.session.id.0,
                resume_token: token,
                player,
                team,
                phase,
                server_tick,
            },
        );
    }
    fn reject(&self, address: SocketAddr, reason: &str) {
        let _ = self.send_to(
            address,
            &LanPacket::Reject {
                reason: reason.to_string(),
            },
        );
    }
    fn broadcast(&self, packet: &LanPacket) {
        for client in self.clients.values() {
            let _ = self.send_to(client.address, packet);
        }
    }
    fn send_to(&self, address: SocketAddr, packet: &LanPacket) -> io::Result<()> {
        let bytes = packet
            .encode()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?;
        self.socket.send_to(&bytes, address)?;
        Ok(())
    }
}

fn wire_phase(phase: LanPhase) -> WirePhase {
    match phase {
        LanPhase::Lobby => WirePhase::Lobby,
        LanPhase::Countdown { .. } => WirePhase::Countdown,
        LanPhase::InMatch => WirePhase::InMatch,
        LanPhase::PostMatch { .. } => WirePhase::PostMatch,
    }
}

pub struct ServerHandle {
    pub address: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), String>>>,
}

impl ServerHandle {
    pub fn spawn(config: ServerConfig) -> Result<Self, String> {
        let server = AuthoritativeServer::bind(config)?;
        let address = server.local_addr().map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("observed-lan-server".to_string())
            .spawn(move || server.run(thread_stop))
            .map_err(|error| format!("spawn server thread: {error}"))?;
        Ok(Self {
            address,
            stop,
            join: Some(join),
        })
    }

    pub fn shutdown(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| "server thread panicked".to_string())??;
        }
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_net::lan::{LanClient, WireSeatOccupant};

    #[test]
    fn cli_defaults_and_overrides_are_stable() {
        let config = ServerConfig::from_args([
            "observed_server".to_string(),
            "--bind".to_string(),
            "127.0.0.1:0".to_string(),
            "--name".to_string(),
            "Test Facility".to_string(),
            "--min-humans".to_string(),
            "3".to_string(),
            "--seed".to_string(),
            "0x2a".to_string(),
            "--no-discovery".to_string(),
        ])
        .expect("config");
        assert_eq!(config.bind, "127.0.0.1:0".parse().unwrap());
        assert_eq!(config.name, "Test Facility");
        assert_eq!(config.min_humans, 3);
        assert_eq!(config.base_seed, 42);
        assert!(!config.discovery);
    }

    #[test]
    fn real_udp_client_switches_team_and_receives_authoritative_frames() {
        let config = ServerConfig {
            bind: "127.0.0.1:0".parse().expect("loopback address"),
            discovery: false,
            match_config: HexMatchConfig::default(),
            ..ServerConfig::default()
        };
        let mut server = AuthoritativeServer::bind(config).expect("server binds");
        let hash = server.content.simulation_content_hash();
        let address = server.local_addr().expect("server address");
        let mut client = LanClient::connect(address, 77, None, None, hash).expect("client binds");

        drive_until(&mut server, &mut client, |client| client.token.is_some());
        assert_eq!(client.player, Some(PlayerId(0)));
        client.request_team(TeamId(1)).expect("request team");
        drive_until(&mut server, &mut client, |client| {
            client.team == Some(TeamId(1))
        });
        assert_eq!(client.player, Some(PlayerId(2)));

        client.set_ready(true).expect("ready");
        drive_until(&mut server, &mut client, |client| client.launch.is_some());
        assert_eq!(
            server.match_state().map(|game| game.tick),
            Some(0),
            "the descriptor must not authorize tick one"
        );
        let generation = client.launch.expect("launch descriptor").match_number;
        client
            .mark_launch_ready(generation)
            .expect("prepared client announces readiness");
        let mut received = Vec::new();
        for _ in 0..64 {
            server.fixed_tick().expect("server tick");
            client.poll();
            received.extend(client.take_ready_frames(observed_net::lan::FRAME_WINDOW));
            if !received.is_empty() {
                break;
            }
            thread::yield_now();
        }
        assert_eq!(received.first().map(|frame| frame.tick), Some(1));
        assert_eq!(server.match_state().map(|game| game.tick > 0), Some(true));

        let token = client.token.expect("resume token");
        server.poll_network();
        server
            .clients
            .get_mut(&token)
            .expect("connection")
            .last_seen = Instant::now() - CLIENT_TIMEOUT - Duration::from_millis(1);
        server.disconnect_timed_out_clients();
        assert!(server.session.seats[2].is_bot_controlled());
        let future_tick = server.match_state().expect("match").tick + 3;
        client
            .queue_input(future_tick, HexPlayerCommand::default())
            .expect("resume heartbeat");
        server.fixed_tick().expect("reconnect tick");
        assert_eq!(
            server.session.seats[2].connected_human(),
            Some(AccountId(77))
        );
        assert!(server.clients[&token].synchronizing);

        client.request_resync().expect("request history replay");
        let mut replayed = Vec::new();
        for _ in 0..64 {
            server.fixed_tick().expect("server tick");
            client.poll();
            replayed.extend(client.take_ready_frames(observed_net::lan::FRAME_WINDOW));
            if !replayed.is_empty() {
                break;
            }
            thread::yield_now();
        }
        assert_eq!(replayed.first().map(|frame| frame.tick), Some(1));
    }

    #[test]
    fn authoritative_tick_one_waits_for_every_connected_human() {
        let config = ServerConfig {
            bind: "127.0.0.1:0".parse().expect("loopback address"),
            discovery: false,
            min_humans: 2,
            ..ServerConfig::default()
        };
        let mut server = AuthoritativeServer::bind(config).expect("server binds");
        let hash = server.content.simulation_content_hash();
        let address = server.local_addr().expect("server address");
        let mut first = LanClient::connect(address, 71, None, None, hash).expect("first binds");
        let mut second = LanClient::connect(address, 72, None, None, hash).expect("second binds");

        drive_pair_until(&mut server, &mut first, &mut second, |first, second| {
            first.token.is_some() && second.token.is_some()
        });
        first.set_ready(true).expect("first ready");
        second.set_ready(true).expect("second ready");
        drive_pair_until(&mut server, &mut first, &mut second, |first, second| {
            first.launch.is_some() && second.launch.is_some()
        });
        let generation = first.launch.expect("first launch").match_number;
        assert_eq!(
            second.launch.expect("second launch").match_number,
            generation
        );
        assert_eq!(server.match_state().map(|game| game.tick), Some(0));

        first.mark_launch_ready(generation).expect("first prepared");
        first
            .mark_launch_ready(generation)
            .expect("duplicate preparation is idempotent");
        for _ in 0..16 {
            server.fixed_tick().expect("server tick");
            first.poll();
            second.poll();
        }
        assert_eq!(server.match_state().map(|game| game.tick), Some(0));
        assert!(!first.launch_has_started(generation));

        second
            .mark_launch_ready(generation)
            .expect("second prepared");
        drive_pair_until(&mut server, &mut first, &mut second, |first, second| {
            first.launch_has_started(generation) && second.launch_has_started(generation)
        });
        assert!(server.match_state().is_some_and(|game| game.tick >= 1));
    }

    #[test]
    fn humans_only_roster_exposes_empty_seats_and_aborts_if_a_loader_times_out() {
        let config = ServerConfig {
            bind: "127.0.0.1:0".parse().expect("loopback address"),
            discovery: false,
            roster: LanRoster::co_op(2),
            min_humans: 1,
            fill_empty_seats: false,
            ..ServerConfig::default()
        };
        let mut server = AuthoritativeServer::bind(config).expect("server binds");
        assert_eq!(server.session.min_humans, 2);
        let hash = server.content.simulation_content_hash();
        let address = server.local_addr().expect("server address");
        let mut first = LanClient::connect(address, 81, None, None, hash).expect("first binds");
        drive_until(&mut server, &mut first, |client| client.lobby.is_some());
        let seats = &first.lobby.as_ref().expect("roster").seats;
        assert_eq!(seats.len(), 2);
        assert_eq!(seats[1].occupant, WireSeatOccupant::Empty);

        first.set_ready(true).expect("first ready");
        for _ in 0..observed_progression::session::lan::LAN_COUNTDOWN_TICKS + 5 {
            server.fixed_tick().expect("server tick");
            first.poll();
        }
        assert!(matches!(server.session.phase, LanPhase::Lobby));

        let mut second = LanClient::connect(address, 82, None, None, hash).expect("second binds");
        drive_pair_until(&mut server, &mut first, &mut second, |first, second| {
            first.token.is_some() && second.token.is_some()
        });
        second.set_ready(true).expect("second ready");
        drive_pair_until(&mut server, &mut first, &mut second, |first, second| {
            first.launch.is_some() && second.launch.is_some()
        });
        let first_generation = first.launch.expect("launch").match_number;
        first
            .mark_launch_ready(first_generation)
            .expect("first prepared");

        let second_token = second.token.expect("second token");
        server
            .clients
            .get_mut(&second_token)
            .expect("second connection")
            .last_seen = Instant::now() - CLIENT_TIMEOUT - Duration::from_millis(1);
        server.disconnect_timed_out_clients();
        assert!(matches!(server.session.phase, LanPhase::Lobby));
        assert!(server.match_state().is_none());
        assert_eq!(
            server.session.seats[1].occupant,
            LanSeatOccupant::Bot,
            "the timed-out human-only seat is immediately available"
        );
    }

    fn drive_until(
        server: &mut AuthoritativeServer,
        client: &mut LanClient,
        condition: impl Fn(&LanClient) -> bool,
    ) {
        for _ in 0..512 {
            server.fixed_tick().expect("server tick");
            client.poll();
            if condition(client) {
                return;
            }
            thread::yield_now();
        }
        panic!("loopback condition was not reached");
    }

    fn drive_pair_until(
        server: &mut AuthoritativeServer,
        first: &mut LanClient,
        second: &mut LanClient,
        condition: impl Fn(&LanClient, &LanClient) -> bool,
    ) {
        for _ in 0..768 {
            server.fixed_tick().expect("server tick");
            first.poll();
            second.poll();
            if condition(first, second) {
                return;
            }
            thread::yield_now();
        }
        panic!("loopback pair condition was not reached");
    }
}
