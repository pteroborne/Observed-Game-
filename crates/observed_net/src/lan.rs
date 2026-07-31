//! Versioned LAN datagrams for the authoritative hex-WFC server.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::HexWfcConfig;
use observed_match::hex_wfc::{
    HEX_INPUT_VERSION, HexActionButtons, HexInputFrame, HexMatchConfig, HexPlayerCommand,
};

use crate::protocol::WireIntent;

/// Version 3 adds the generation-scoped preparation barrier and an explicit empty
/// seat. Older peers must reject it rather than simulate as soon as `Launch` lands.
pub const LAN_PROTOCOL_VERSION: u16 = 3;
pub const DEFAULT_LAN_PORT: u16 = 47_624;
pub const MAX_DATAGRAM: usize = 1_200;
pub const INPUT_LEAD_TICKS: u64 = 3;
pub const FRAME_WINDOW: usize = 16;
/// Hard ceiling on seats in one match, and so on commands in one frame.
///
/// A wire-format limit, not a gameplay one: the count is a `u8` and the
/// per-frame cost is what drives [`frames_per_bundle`]. Raising it means
/// re-checking that budget, not just this constant.
pub const MAX_SEATS: usize = 16;
const MAGIC: [u8; 4] = *b"O2LN";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WirePhase {
    Lobby,
    Countdown,
    InMatch,
    PostMatch,
}

impl WirePhase {
    fn encode(self) -> u8 {
        match self {
            Self::Lobby => 0,
            Self::Countdown => 1,
            Self::InMatch => 2,
            Self::PostMatch => 3,
        }
    }

    fn decode(value: u8) -> Result<Self, LanCodecError> {
        match value {
            0 => Ok(Self::Lobby),
            1 => Ok(Self::Countdown),
            2 => Ok(Self::InMatch),
            3 => Ok(Self::PostMatch),
            _ => Err(LanCodecError::InvalidValue),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WireHexCommand {
    pub intent: WireIntent,
    pub actions: u8,
}

impl WireHexCommand {
    const INTERACT: u8 = 1;
    const DEPLOY: u8 = 1 << 1;
    const RECOVER: u8 = 1 << 2;

    #[must_use]
    pub fn from_command(command: HexPlayerCommand) -> Self {
        let mut actions = 0;
        if command.actions.interact {
            actions |= Self::INTERACT;
        }
        if command.actions.deploy_lantern {
            actions |= Self::DEPLOY;
        }
        if command.actions.recover_lantern {
            actions |= Self::RECOVER;
        }
        Self {
            intent: WireIntent::from_player_intent(command.intent.sanitized()),
            actions,
        }
    }

    #[must_use]
    pub fn to_command(self) -> HexPlayerCommand {
        HexPlayerCommand {
            intent: self.intent.to_player_intent(),
            actions: HexActionButtons {
                interact: self.actions & Self::INTERACT != 0,
                deploy_lantern: self.actions & Self::DEPLOY != 0,
                recover_lantern: self.actions & Self::RECOVER != 0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireSeatOccupant {
    Bot,
    Human,
    ReservedHuman,
    SynchronizingHuman,
    Empty,
}

impl WireSeatOccupant {
    const fn encode(self) -> u8 {
        match self {
            Self::Bot => 0,
            Self::Human => 1,
            Self::ReservedHuman => 2,
            Self::SynchronizingHuman => 3,
            Self::Empty => 4,
        }
    }

    fn decode(value: u8) -> Result<Self, LanCodecError> {
        match value {
            0 => Ok(Self::Bot),
            1 => Ok(Self::Human),
            2 => Ok(Self::ReservedHuman),
            3 => Ok(Self::SynchronizingHuman),
            4 => Ok(Self::Empty),
            _ => Err(LanCodecError::InvalidValue),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireSeat {
    pub player: PlayerId,
    pub team: TeamId,
    pub occupant: WireSeatOccupant,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireFrame {
    pub tick: u64,
    /// One command per seat, in seat order. Length-prefixed on the wire and
    /// capped at [`MAX_SEATS`]; it was a fixed `[WireHexCommand; 4]`, which is
    /// what pinned the whole stack to 2v2.
    pub commands: Vec<WireHexCommand>,
    pub digest: u64,
}

impl WireFrame {
    #[must_use]
    pub fn to_input_frame(&self) -> HexInputFrame {
        HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick: self.tick,
            commands: self
                .commands
                .iter()
                .enumerate()
                .map(|(index, command)| (PlayerId(index as u16), command.to_command()))
                .collect(),
        }
    }

    /// Bytes this frame occupies inside a [`LanPacket::FrameBundle`].
    #[must_use]
    pub fn wire_len(&self) -> usize {
        // tick + seat count + commands + digest.
        8 + 1 + self.commands.len() * WIRE_COMMAND_BYTES + 8
    }
}

/// Encoded size of one [`WireHexCommand`]: a [`WireIntent`] plus its action bits.
const WIRE_COMMAND_BYTES: usize = 5 + 1;

/// How many frames of `seats` commands fit in one datagram alongside the header.
///
/// The old code sent [`FRAME_WINDOW`] frames unconditionally, which was fine at
/// four seats and impossible at sixteen: sixteen frames of sixteen commands is
/// 1 808 bytes against a 1 200-byte [`MAX_DATAGRAM`], so `encode` did not
/// degrade — it returned `Oversized` and the bundle never went out. Budgeting by
/// bytes means the window shrinks as the roster grows instead of the send
/// failing.
#[must_use]
pub fn frames_per_bundle(seats: usize) -> usize {
    // Envelope, packet tag, and the bundle's own frame count.
    const OVERHEAD: usize = 12 + 1 + 1;
    let per_frame = 8 + 1 + seats.max(1) * WIRE_COMMAND_BYTES + 8;
    ((MAX_DATAGRAM - OVERHEAD) / per_frame).clamp(1, FRAME_WINDOW)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LobbyAction {
    Ready(bool),
    RequestTeam(TeamId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LanPacket {
    DiscoveryProbe,
    DiscoveryReply {
        name: String,
        phase: WirePhase,
        humans: u8,
        joinable: bool,
    },
    Hello {
        account: u16,
        requested_team: Option<TeamId>,
        resume_token: Option<u64>,
        input_version: u16,
        simulation_content_hash: [u8; 32],
    },
    Welcome {
        session: u32,
        resume_token: u64,
        player: PlayerId,
        team: TeamId,
        phase: WirePhase,
        server_tick: u64,
    },
    Reject {
        reason: String,
    },
    LobbyCommand {
        token: u64,
        action: LobbyAction,
    },
    LobbySnapshot {
        session: u32,
        match_number: u32,
        server_tick: u64,
        phase: WirePhase,
        countdown_ticks: u16,
        seats: Vec<WireSeat>,
    },
    Launch {
        seed: u64,
        match_number: u32,
        config: HexMatchConfig,
        simulation_content_hash: [u8; 32],
    },
    InputBundle {
        token: u64,
        commands: Vec<(u64, WireHexCommand)>,
    },
    FrameBundle {
        frames: Vec<WireFrame>,
    },
    Ack {
        token: u64,
        through_tick: u64,
    },
    MatchEnded {
        escape_order: Vec<TeamId>,
    },
    Goodbye {
        token: u64,
    },
    /// Ask the server to replay the authoritative frame history from tick one.
    /// The client reconstructs the deterministic launch state before applying it.
    Resync {
        token: u64,
    },
    /// The client finished deterministic preparation for this exact generation.
    /// The packet is idempotent and may be retried until `LaunchStart` arrives.
    LaunchReady {
        token: u64,
        match_number: u32,
    },
    /// Authoritative permission to enter the match and consume its frames.
    /// A `Launch` descriptor by itself is never permission to simulate.
    LaunchStart {
        match_number: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanCodecError {
    WrongLength,
    WrongMagic,
    UnsupportedVersion,
    InvalidKind,
    InvalidValue,
    BadChecksum,
    Oversized,
    Utf8,
}

impl LanPacket {
    pub fn encode(&self) -> Result<Vec<u8>, LanCodecError> {
        let (kind, payload) = encode_payload(self)?;
        if payload.len() + 12 > MAX_DATAGRAM || payload.len() > usize::from(u16::MAX) {
            return Err(LanCodecError::Oversized);
        }
        let mut bytes = Vec::with_capacity(payload.len() + 12);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&LAN_PROTOCOL_VERSION.to_le_bytes());
        bytes.push(kind);
        bytes.push(0);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&payload);
        let checksum = checksum32(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LanCodecError> {
        if bytes.len() < 14 || bytes.len() > MAX_DATAGRAM {
            return Err(LanCodecError::WrongLength);
        }
        if bytes[0..4] != MAGIC {
            return Err(LanCodecError::WrongMagic);
        }
        if u16::from_le_bytes([bytes[4], bytes[5]]) != LAN_PROTOCOL_VERSION {
            return Err(LanCodecError::UnsupportedVersion);
        }
        let payload_len = usize::from(u16::from_le_bytes([bytes[8], bytes[9]]));
        if bytes.len() != 10 + payload_len + 4 {
            return Err(LanCodecError::WrongLength);
        }
        let checksum_at = bytes.len() - 4;
        let expected = u32::from_le_bytes(bytes[checksum_at..].try_into().expect("four bytes"));
        if checksum32(&bytes[..checksum_at]) != expected {
            return Err(LanCodecError::BadChecksum);
        }
        decode_payload(bytes[6], &bytes[10..checksum_at])
    }
}

fn encode_payload(packet: &LanPacket) -> Result<(u8, Vec<u8>), LanCodecError> {
    let mut out = Vec::new();
    let kind = match packet {
        LanPacket::DiscoveryProbe => 0,
        LanPacket::DiscoveryReply {
            name,
            phase,
            humans,
            joinable,
        } => {
            put_string(&mut out, name)?;
            out.push(phase.encode());
            out.push(*humans);
            out.push(u8::from(*joinable));
            1
        }
        LanPacket::Hello {
            account,
            requested_team,
            resume_token,
            input_version,
            simulation_content_hash,
        } => {
            put_u16(&mut out, *account);
            out.push(requested_team.map_or(u8::MAX, |team| team.0));
            put_u64(&mut out, resume_token.unwrap_or(0));
            put_u16(&mut out, *input_version);
            out.extend_from_slice(simulation_content_hash);
            2
        }
        LanPacket::Welcome {
            session,
            resume_token,
            player,
            team,
            phase,
            server_tick,
        } => {
            put_u32(&mut out, *session);
            put_u64(&mut out, *resume_token);
            put_u16(&mut out, player.0);
            out.push(team.0);
            out.push(phase.encode());
            put_u64(&mut out, *server_tick);
            3
        }
        LanPacket::Reject { reason } => {
            put_string(&mut out, reason)?;
            4
        }
        LanPacket::LobbyCommand { token, action } => {
            put_u64(&mut out, *token);
            match action {
                LobbyAction::Ready(ready) => {
                    out.push(0);
                    out.push(u8::from(*ready));
                }
                LobbyAction::RequestTeam(team) => {
                    out.push(1);
                    out.push(team.0);
                }
            }
            5
        }
        LanPacket::LobbySnapshot {
            session,
            match_number,
            server_tick,
            phase,
            countdown_ticks,
            seats,
        } => {
            put_u32(&mut out, *session);
            put_u32(&mut out, *match_number);
            put_u64(&mut out, *server_tick);
            out.push(phase.encode());
            put_u16(&mut out, *countdown_ticks);
            out.push(seats.len() as u8);
            for seat in seats {
                put_u16(&mut out, seat.player.0);
                out.push(seat.team.0);
                out.push(seat.occupant.encode());
                out.push(u8::from(seat.ready));
            }
            6
        }
        LanPacket::Launch {
            seed,
            match_number,
            config,
            simulation_content_hash,
        } => {
            put_u64(&mut out, *seed);
            put_u32(&mut out, *match_number);
            encode_config(&mut out, *config);
            out.extend_from_slice(simulation_content_hash);
            7
        }
        LanPacket::InputBundle { token, commands } => {
            put_u64(&mut out, *token);
            out.push(commands.len() as u8);
            for (tick, command) in commands {
                put_u64(&mut out, *tick);
                encode_command(&mut out, *command);
            }
            8
        }
        LanPacket::FrameBundle { frames } => {
            out.push(frames.len() as u8);
            for frame in frames {
                put_u64(&mut out, frame.tick);
                out.push(frame.commands.len() as u8);
                for command in &frame.commands {
                    encode_command(&mut out, *command);
                }
                put_u64(&mut out, frame.digest);
            }
            9
        }
        LanPacket::Ack {
            token,
            through_tick,
        } => {
            put_u64(&mut out, *token);
            put_u64(&mut out, *through_tick);
            10
        }
        LanPacket::MatchEnded { escape_order } => {
            out.push(escape_order.len() as u8);
            out.extend(escape_order.iter().map(|team| team.0));
            11
        }
        LanPacket::Goodbye { token } => {
            put_u64(&mut out, *token);
            12
        }
        LanPacket::Resync { token } => {
            put_u64(&mut out, *token);
            13
        }
        LanPacket::LaunchReady {
            token,
            match_number,
        } => {
            put_u64(&mut out, *token);
            put_u32(&mut out, *match_number);
            14
        }
        LanPacket::LaunchStart { match_number } => {
            put_u32(&mut out, *match_number);
            15
        }
    };
    Ok((kind, out))
}

fn decode_payload(kind: u8, bytes: &[u8]) -> Result<LanPacket, LanCodecError> {
    let mut cursor = Cursor::new(bytes);
    let packet = match kind {
        0 => LanPacket::DiscoveryProbe,
        1 => LanPacket::DiscoveryReply {
            name: cursor.string()?,
            phase: WirePhase::decode(cursor.u8()?)?,
            humans: cursor.u8()?,
            joinable: cursor.bool()?,
        },
        2 => {
            let account = cursor.u16()?;
            let requested = cursor.u8()?;
            let resume = cursor.u64()?;
            LanPacket::Hello {
                account,
                requested_team: (requested != u8::MAX).then_some(TeamId(requested)),
                resume_token: (resume != 0).then_some(resume),
                input_version: cursor.u16()?,
                simulation_content_hash: cursor.array32()?,
            }
        }
        3 => LanPacket::Welcome {
            session: cursor.u32()?,
            resume_token: cursor.u64()?,
            player: PlayerId(cursor.u16()?),
            team: TeamId(cursor.u8()?),
            phase: WirePhase::decode(cursor.u8()?)?,
            server_tick: cursor.u64()?,
        },
        4 => LanPacket::Reject {
            reason: cursor.string()?,
        },
        5 => {
            let token = cursor.u64()?;
            let action = match cursor.u8()? {
                0 => LobbyAction::Ready(cursor.bool()?),
                1 => LobbyAction::RequestTeam(TeamId(cursor.u8()?)),
                _ => return Err(LanCodecError::InvalidValue),
            };
            LanPacket::LobbyCommand { token, action }
        }
        6 => {
            let session = cursor.u32()?;
            let match_number = cursor.u32()?;
            let server_tick = cursor.u64()?;
            let phase = WirePhase::decode(cursor.u8()?)?;
            let countdown_ticks = cursor.u16()?;
            let count = usize::from(cursor.u8()?);
            let mut seats = Vec::with_capacity(count);
            for _ in 0..count {
                seats.push(WireSeat {
                    player: PlayerId(cursor.u16()?),
                    team: TeamId(cursor.u8()?),
                    occupant: WireSeatOccupant::decode(cursor.u8()?)?,
                    ready: cursor.bool()?,
                });
            }
            LanPacket::LobbySnapshot {
                session,
                match_number,
                server_tick,
                phase,
                countdown_ticks,
                seats,
            }
        }
        7 => LanPacket::Launch {
            seed: cursor.u64()?,
            match_number: cursor.u32()?,
            config: decode_config(&mut cursor)?,
            simulation_content_hash: cursor.array32()?,
        },
        8 => {
            let token = cursor.u64()?;
            let count = usize::from(cursor.u8()?);
            let mut commands = Vec::with_capacity(count);
            for _ in 0..count {
                commands.push((cursor.u64()?, decode_command(&mut cursor)?));
            }
            LanPacket::InputBundle { token, commands }
        }
        9 => {
            let count = usize::from(cursor.u8()?);
            let mut frames = Vec::with_capacity(count);
            for _ in 0..count {
                let tick = cursor.u64()?;
                let seats = usize::from(cursor.u8()?);
                if seats > MAX_SEATS {
                    // A count past the cap is a malformed or hostile datagram,
                    // not a bigger match: refuse rather than allocate for it.
                    return Err(LanCodecError::InvalidValue);
                }
                let mut commands = Vec::with_capacity(seats);
                for _ in 0..seats {
                    commands.push(decode_command(&mut cursor)?);
                }
                frames.push(WireFrame {
                    tick,
                    commands,
                    digest: cursor.u64()?,
                });
            }
            LanPacket::FrameBundle { frames }
        }
        10 => LanPacket::Ack {
            token: cursor.u64()?,
            through_tick: cursor.u64()?,
        },
        11 => {
            let count = usize::from(cursor.u8()?);
            LanPacket::MatchEnded {
                escape_order: (0..count)
                    .map(|_| cursor.u8().map(TeamId))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
        12 => LanPacket::Goodbye {
            token: cursor.u64()?,
        },
        13 => LanPacket::Resync {
            token: cursor.u64()?,
        },
        14 => LanPacket::LaunchReady {
            token: cursor.u64()?,
            match_number: cursor.u32()?,
        },
        15 => LanPacket::LaunchStart {
            match_number: cursor.u32()?,
        },
        _ => return Err(LanCodecError::InvalidKind),
    };
    if cursor.remaining() != 0 {
        return Err(LanCodecError::WrongLength);
    }
    Ok(packet)
}

fn encode_command(out: &mut Vec<u8>, command: WireHexCommand) {
    out.push(command.intent.movement_x as u8);
    out.push(command.intent.movement_y as u8);
    out.push(command.intent.look_x as u8);
    out.push(command.intent.look_y as u8);
    out.push(command.intent.flags);
    out.push(command.actions);
}

fn decode_command(cursor: &mut Cursor<'_>) -> Result<WireHexCommand, LanCodecError> {
    let command = WireHexCommand {
        intent: WireIntent {
            movement_x: cursor.u8()? as i8,
            movement_y: cursor.u8()? as i8,
            look_x: cursor.u8()? as i8,
            look_y: cursor.u8()? as i8,
            flags: cursor.u8()?,
        },
        actions: cursor.u8()?,
    };
    if command.actions & !0b111 != 0 {
        return Err(LanCodecError::InvalidValue);
    }
    Ok(command)
}

fn encode_config(out: &mut Vec<u8>, config: HexMatchConfig) {
    out.push(config.teams);
    out.push(config.members_per_team);
    // The Guardian flag changes the simulation, so it has to cross the wire.
    // A host running co-op without a Guardian and a client assuming one would
    // diverge on the first tick the Guardian would have moved.
    out.push(u8::from(config.guardian));
    put_u16(out, config.wfc.cols);
    put_u16(out, config.wfc.rows);
    out.push(config.wfc.levels);
    put_u32(out, config.wfc.min_rooms as u32);
    put_u32(out, config.wfc.max_rooms as u32);
    put_u32(out, config.wfc.retry_budget);
    put_u32(out, config.wfc.min_room_distance);
}

fn decode_config(cursor: &mut Cursor<'_>) -> Result<HexMatchConfig, LanCodecError> {
    Ok(HexMatchConfig {
        teams: cursor.u8()?,
        members_per_team: cursor.u8()?,
        guardian: cursor.u8()? != 0,
        wfc: HexWfcConfig {
            cols: cursor.u16()?,
            rows: cursor.u16()?,
            levels: cursor.u8()?,
            min_rooms: cursor.u32()? as usize,
            max_rooms: cursor.u32()? as usize,
            retry_budget: cursor.u32()?,
            min_room_distance: cursor.u32()?,
        },
    })
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), LanCodecError> {
    if value.len() > 96 {
        return Err(LanCodecError::Oversized);
    }
    out.push(value.len() as u8);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn checksum32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |mut hash, byte| {
        hash ^= u32::from(*byte);
        hash.wrapping_mul(0x0100_0193)
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], LanCodecError> {
        let end = self.at.checked_add(len).ok_or(LanCodecError::WrongLength)?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or(LanCodecError::WrongLength)?;
        self.at = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, LanCodecError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, LanCodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(LanCodecError::InvalidValue),
        }
    }
    fn u16(&mut self) -> Result<u16, LanCodecError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }
    fn u32(&mut self) -> Result<u32, LanCodecError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }
    fn u64(&mut self) -> Result<u64, LanCodecError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }
    fn array32(&mut self) -> Result<[u8; 32], LanCodecError> {
        Ok(self.take(32)?.try_into().expect("32 bytes"))
    }
    fn string(&mut self) -> Result<String, LanCodecError> {
        let len = usize::from(self.u8()?);
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| LanCodecError::Utf8)
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveredServer {
    pub address: SocketAddr,
    pub name: String,
    pub phase: WirePhase,
    pub humans: u8,
    pub joinable: bool,
    pub last_seen: Instant,
}

pub struct DiscoveryBrowser {
    socket: UdpSocket,
    port: u16,
    servers: BTreeMap<SocketAddr, DiscoveredServer>,
}

impl DiscoveryBrowser {
    pub fn bind(port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            port,
            servers: BTreeMap::new(),
        })
    }

    pub fn probe(&self) -> io::Result<()> {
        let bytes = LanPacket::DiscoveryProbe.encode().expect("probe encodes");
        self.socket
            .send_to(&bytes, (Ipv4Addr::BROADCAST, self.port))?;
        Ok(())
    }

    pub fn poll(&mut self) {
        let mut buffer = [0_u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, address)) => {
                    if let Ok(LanPacket::DiscoveryReply {
                        name,
                        phase,
                        humans,
                        joinable,
                    }) = LanPacket::decode(&buffer[..len])
                    {
                        self.servers.insert(
                            address,
                            DiscoveredServer {
                                address,
                                name,
                                phase,
                                humans,
                                joinable,
                                last_seen: Instant::now(),
                            },
                        );
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        self.servers
            .retain(|_, server| server.last_seen.elapsed() < Duration::from_secs(3));
    }

    #[must_use]
    pub fn servers(&self) -> Vec<DiscoveredServer> {
        self.servers.values().cloned().collect()
    }
}

/// Deterministic descriptor for one authoritative LAN match.
///
/// The server may repeat this descriptor until it observes client progress.
/// [`LanClient`] therefore treats `match_number` as a monotonic generation and
/// only resets its transport state when a genuinely newer generation arrives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanLaunch {
    pub seed: u64,
    pub match_number: u32,
    pub config: HexMatchConfig,
    pub simulation_content_hash: [u8; 32],
}

/// Latest authoritative lobby projection. The generation travels with the phase
/// so a delayed `Launch` datagram cannot resurrect a launch the server withdrew.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanLobby {
    pub session: u32,
    pub match_number: u32,
    pub server_tick: u64,
    pub phase: WirePhase,
    pub countdown_ticks: u16,
    pub seats: Vec<WireSeat>,
}

/// Nonblocking client endpoint. The game owns simulation construction and applies
/// authoritative frames returned by [`Self::take_ready_frames`].
pub struct LanClient {
    socket: UdpSocket,
    server: SocketAddr,
    account: u16,
    requested_team: Option<TeamId>,
    resume_token: Option<u64>,
    simulation_content_hash: [u8; 32],
    pub token: Option<u64>,
    pub player: Option<PlayerId>,
    pub team: Option<TeamId>,
    pub phase: Option<WirePhase>,
    pub lobby: Option<LanLobby>,
    pub launch: Option<LanLaunch>,
    /// The generation for which the server has explicitly opened the barrier.
    pub launch_started: Option<u32>,
    pub rejection: Option<String>,
    launch_ready: Option<u32>,
    input_outbox: VecDeque<(u64, WireHexCommand)>,
    frames: BTreeMap<u64, WireFrame>,
    next_frame: u64,
    last_heartbeat: Instant,
}

impl LanClient {
    pub fn connect(
        server: SocketAddr,
        account: u16,
        requested_team: Option<TeamId>,
        resume_token: Option<u64>,
        simulation_content_hash: [u8; 32],
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        let client = Self {
            socket,
            server,
            account,
            requested_team,
            resume_token,
            simulation_content_hash,
            token: None,
            player: None,
            team: None,
            phase: None,
            lobby: None,
            launch: None,
            launch_started: None,
            rejection: None,
            launch_ready: None,
            input_outbox: VecDeque::new(),
            frames: BTreeMap::new(),
            next_frame: 1,
            last_heartbeat: Instant::now(),
        };
        client.send(&LanPacket::Hello {
            account,
            requested_team,
            resume_token,
            input_version: HEX_INPUT_VERSION,
            simulation_content_hash,
        })?;
        Ok(client)
    }

    pub fn poll(&mut self) {
        let mut buffer = [0_u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, address)) if address == self.server => {
                    if let Ok(packet) = LanPacket::decode(&buffer[..len]) {
                        self.receive(packet);
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        if self.last_heartbeat.elapsed() >= Duration::from_millis(500) {
            if let Some(token) = self.token {
                if let Some(match_number) = self.launch_ready
                    && self.launch_started != Some(match_number)
                {
                    let _ = self.send(&LanPacket::LaunchReady {
                        token,
                        match_number,
                    });
                }
                let _ = self.send(&LanPacket::Ack {
                    token,
                    through_tick: self.next_frame.saturating_sub(1),
                });
            } else if self.rejection.is_none() {
                // The first UDP hello may be lost. Retry it until a welcome or
                // explicit rejection establishes the connection outcome.
                let _ = self.send(&LanPacket::Hello {
                    account: self.account,
                    requested_team: self.requested_team,
                    resume_token: self.resume_token,
                    input_version: HEX_INPUT_VERSION,
                    simulation_content_hash: self.simulation_content_hash,
                });
            }
            self.last_heartbeat = Instant::now();
        }
    }

    fn receive(&mut self, packet: LanPacket) {
        match packet {
            LanPacket::Welcome {
                resume_token,
                player,
                team,
                phase,
                server_tick,
                ..
            } => {
                self.token = Some(resume_token);
                self.resume_token = Some(resume_token);
                self.player = Some(player);
                self.team = Some(team);
                self.phase = Some(phase);
                self.next_frame = server_tick.saturating_add(1).min(self.next_frame);
            }
            LanPacket::Reject { reason } => self.rejection = Some(reason),
            LanPacket::LobbySnapshot {
                session,
                match_number,
                server_tick,
                phase,
                countdown_ticks,
                seats,
            } => {
                self.receive_lobby(LanLobby {
                    session,
                    match_number,
                    server_tick,
                    phase,
                    countdown_ticks,
                    seats,
                });
            }
            LanPacket::Launch {
                seed,
                match_number,
                config,
                simulation_content_hash,
            } => {
                self.receive_launch(LanLaunch {
                    seed,
                    match_number,
                    config,
                    simulation_content_hash,
                });
            }
            LanPacket::LaunchStart { match_number } => {
                self.receive_launch_start(match_number);
            }
            LanPacket::FrameBundle { frames } => {
                for frame in frames {
                    self.frames.entry(frame.tick).or_insert(frame);
                }
            }
            LanPacket::MatchEnded { .. } => self.phase = Some(WirePhase::PostMatch),
            _ => {}
        }
    }

    fn receive_lobby(&mut self, incoming: LanLobby) {
        if self.lobby.as_ref().is_some_and(|current| {
            current.session == incoming.session && current.server_tick > incoming.server_tick
        }) {
            return;
        }
        self.phase = Some(incoming.phase);
        if incoming.phase != WirePhase::InMatch
            && self
                .launch
                .is_some_and(|launch| launch.match_number == incoming.match_number)
        {
            self.launch_ready = None;
            self.launch_started = None;
        }
        self.lobby = Some(incoming);
    }

    fn receive_launch(&mut self, incoming: LanLaunch) {
        match self.launch {
            Some(current) if incoming.match_number < current.match_number => {
                // UDP can deliver a delayed launch after a newer match has begun.
                // Ignoring it keeps both the active descriptor and buffered frames.
            }
            Some(current) if incoming.match_number == current.match_number => {
                if incoming == current {
                    // Repeated launch packets acknowledge the same match; transport
                    // progress made while the game prepares must remain intact.
                    if !self.lobby_withdrew(incoming.match_number) {
                        self.phase = Some(WirePhase::InMatch);
                    }
                } else {
                    // A generation identifies exactly one deterministic launch. Two
                    // descriptors for it would construct divergent simulations, so
                    // reject the contradiction without mutating the accepted state.
                    self.rejection = Some(format!(
                        "conflicting launch descriptor for match {}",
                        incoming.match_number
                    ));
                }
            }
            None | Some(_) => {
                if self.lobby_withdrew(incoming.match_number) {
                    return;
                }
                self.launch = Some(incoming);
                self.launch_ready = None;
                self.launch_started = None;
                self.next_frame = 1;
                self.frames.clear();
                self.phase = Some(WirePhase::InMatch);
            }
        }
    }

    fn receive_launch_start(&mut self, match_number: u32) {
        if self.lobby_withdrew(match_number) {
            return;
        }
        let Some(launch) = self.launch else {
            return;
        };
        if match_number == launch.match_number {
            self.launch_started = Some(match_number);
        }
        // An older start is a delayed UDP packet. A newer start cannot be
        // interpreted safely until its descriptor arrives, so both are ignored.
    }

    fn lobby_withdrew(&self, match_number: u32) -> bool {
        self.lobby.as_ref().is_some_and(|lobby| {
            lobby.match_number >= match_number && lobby.phase != WirePhase::InMatch
        })
    }

    /// Publish local preparation for the accepted descriptor. Repeating this call
    /// is safe; the client also retries it on its transport heartbeat until start.
    pub fn mark_launch_ready(&mut self, match_number: u32) -> io::Result<()> {
        if self.lobby_withdrew(match_number) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "launch generation was withdrawn by the server",
            ));
        }
        let launch = self
            .launch
            .filter(|launch| launch.match_number == match_number)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "launch generation is not the accepted descriptor",
                )
            })?;
        let token = self
            .token
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "not welcomed"))?;
        self.launch_ready = Some(launch.match_number);
        self.send(&LanPacket::LaunchReady {
            token,
            match_number: launch.match_number,
        })
    }

    #[must_use]
    pub fn launch_has_started(&self, match_number: u32) -> bool {
        self.launch_started == Some(match_number)
            && self
                .launch
                .is_some_and(|launch| launch.match_number == match_number)
    }

    pub fn set_ready(&self, ready: bool) -> io::Result<()> {
        self.command(LobbyAction::Ready(ready))
    }
    pub fn request_team(&self, team: TeamId) -> io::Result<()> {
        self.command(LobbyAction::RequestTeam(team))
    }
    fn command(&self, action: LobbyAction) -> io::Result<()> {
        let token = self
            .token
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "not welcomed"))?;
        self.send(&LanPacket::LobbyCommand { token, action })
    }

    pub fn queue_input(&mut self, tick: u64, command: HexPlayerCommand) -> io::Result<()> {
        self.input_outbox
            .push_back((tick, WireHexCommand::from_command(command)));
        while self.input_outbox.len() > 8 {
            self.input_outbox.pop_front();
        }
        let token = self
            .token
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "not welcomed"))?;
        self.send(&LanPacket::InputBundle {
            token,
            commands: self.input_outbox.iter().copied().collect(),
        })
    }

    pub fn take_ready_frames(&mut self, max: usize) -> Vec<WireFrame> {
        let mut ready = Vec::new();
        while ready.len() < max {
            let Some(frame) = self.frames.remove(&self.next_frame) else {
                break;
            };
            ready.push(frame);
            self.next_frame = self.next_frame.saturating_add(1);
        }
        if let (Some(token), Some(last)) = (self.token, ready.last()) {
            let _ = self.send(&LanPacket::Ack {
                token,
                through_tick: last.tick,
            });
        }
        ready
    }

    /// Reset local transport cursors and request the server's retained authoritative
    /// history. Simulation reconstruction remains the game adapter's responsibility.
    pub fn request_resync(&mut self) -> io::Result<()> {
        let token = self
            .token
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "not welcomed"))?;
        self.frames.clear();
        self.input_outbox.clear();
        self.next_frame = 1;
        self.send(&LanPacket::Resync { token })
    }

    pub fn goodbye(&self) {
        if let Some(token) = self.token {
            let _ = self.send(&LanPacket::Goodbye { token });
        }
    }

    fn send(&self, packet: &LanPacket) -> io::Result<()> {
        let bytes = packet
            .encode()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?;
        self.socket.send_to(&bytes, self.server)?;
        Ok(())
    }

    #[must_use]
    pub fn account(&self) -> u16 {
        self.account
    }
    #[must_use]
    pub fn requested_team(&self) -> Option<TeamId> {
        self.requested_team
    }
    #[must_use]
    pub fn simulation_content_hash(&self) -> [u8; 32] {
        self.simulation_content_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use player_input::PlayerIntent;

    fn packets() -> Vec<LanPacket> {
        let command = WireHexCommand::from_command(HexPlayerCommand {
            intent: PlayerIntent {
                movement: Vec2::new(0.5, -1.0),
                jump_pressed: true,
                ..Default::default()
            },
            actions: HexActionButtons {
                interact: true,
                deploy_lantern: false,
                recover_lantern: true,
            },
        });
        vec![
            LanPacket::DiscoveryProbe,
            LanPacket::DiscoveryReply {
                name: "Workshop".into(),
                phase: WirePhase::Lobby,
                humans: 2,
                joinable: true,
            },
            LanPacket::Hello {
                account: 7,
                requested_team: Some(TeamId(1)),
                resume_token: Some(9),
                input_version: HEX_INPUT_VERSION,
                simulation_content_hash: [3; 32],
            },
            LanPacket::Welcome {
                session: 4,
                resume_token: 9,
                player: PlayerId(2),
                team: TeamId(1),
                phase: WirePhase::InMatch,
                server_tick: 44,
            },
            LanPacket::LobbyCommand {
                token: 9,
                action: LobbyAction::Ready(true),
            },
            LanPacket::LobbySnapshot {
                session: 4,
                match_number: 2,
                server_tick: 120,
                phase: WirePhase::Lobby,
                countdown_ticks: 0,
                seats: vec![
                    WireSeat {
                        player: PlayerId(0),
                        team: TeamId(0),
                        occupant: WireSeatOccupant::Human,
                        ready: true,
                    },
                    WireSeat {
                        player: PlayerId(1),
                        team: TeamId(0),
                        occupant: WireSeatOccupant::Empty,
                        ready: false,
                    },
                ],
            },
            LanPacket::Launch {
                seed: 55,
                match_number: 3,
                config: HexMatchConfig::default(),
                simulation_content_hash: [4; 32],
            },
            LanPacket::InputBundle {
                token: 9,
                commands: vec![(4, command)],
            },
            LanPacket::FrameBundle {
                frames: vec![WireFrame {
                    tick: 4,
                    commands: vec![command; 4],
                    digest: 88,
                }],
            },
            LanPacket::Ack {
                token: 9,
                through_tick: 4,
            },
            LanPacket::MatchEnded {
                escape_order: vec![TeamId(1), TeamId(0)],
            },
            LanPacket::Goodbye { token: 9 },
            LanPacket::Resync { token: 9 },
            LanPacket::LaunchReady {
                token: 9,
                match_number: 3,
            },
            LanPacket::LaunchStart { match_number: 3 },
        ]
    }

    #[test]
    fn every_lan_packet_round_trips() {
        for packet in packets() {
            assert_eq!(
                LanPacket::decode(&packet.encode().expect("encode")),
                Ok(packet)
            );
        }
    }

    #[test]
    fn corruption_and_version_mismatch_are_rejected() {
        let mut bytes = LanPacket::DiscoveryProbe.encode().expect("encode");
        bytes[6] ^= 0x40;
        assert_eq!(LanPacket::decode(&bytes), Err(LanCodecError::BadChecksum));
        let mut bytes = LanPacket::DiscoveryProbe.encode().expect("encode");
        bytes[4..6].copy_from_slice(&(LAN_PROTOCOL_VERSION + 1).to_le_bytes());
        assert_eq!(
            LanPacket::decode(&bytes),
            Err(LanCodecError::UnsupportedVersion)
        );
    }
    /// The sixteen-seat worst case fits a datagram, which it did not before.
    ///
    /// A frame of sixteen commands is 113 bytes, so the old fixed window of
    /// `FRAME_WINDOW` frames came to 1 808 against a 1 200-byte limit — `encode`
    /// returned `Oversized` and the bundle simply never went out. This is the
    /// exact cliff the phase existed to remove, so it is pinned at the boundary
    /// rather than at a comfortable size.
    #[test]
    fn a_full_sixteen_seat_bundle_fits_one_datagram() {
        let seats = MAX_SEATS;
        let per_bundle = frames_per_bundle(seats);
        assert!(per_bundle >= 1, "a bundle must carry at least one frame");
        assert!(
            per_bundle < FRAME_WINDOW,
            "sixteen seats should shrink the window below the four-seat one"
        );

        let frame = |tick: u64| WireFrame {
            tick,
            commands: vec![
                WireHexCommand {
                    intent: WireIntent {
                        movement_x: i8::MAX,
                        movement_y: i8::MIN,
                        look_x: i8::MAX,
                        look_y: i8::MIN,
                        // Every defined flag bit; `u8::MAX` would set
                        // undefined ones and the decoder rightly refuses those.
                        flags: 0b0001_1111,
                    },
                    actions: 0b0000_0111,
                };
                seats
            ],
            digest: u64::MAX,
        };
        let frames: Vec<_> = (0..per_bundle as u64).map(frame).collect();
        let encoded = LanPacket::FrameBundle {
            frames: frames.clone(),
        }
        .encode()
        .expect("a budgeted bundle encodes");
        assert!(
            encoded.len() <= MAX_DATAGRAM,
            "{} bytes exceeds the {MAX_DATAGRAM}-byte datagram",
            encoded.len()
        );
        assert_eq!(
            LanPacket::decode(&encoded).expect("round trip"),
            LanPacket::FrameBundle { frames }
        );

        // One frame more than the budget must not fit, or the budget is slack
        // and this test would keep passing while the cliff crept back.
        let overflowing: Vec<_> = (0..=per_bundle as u64).map(frame).collect();
        assert!(
            LanPacket::FrameBundle {
                frames: overflowing
            }
            .encode()
            .is_err(),
            "the budget should be the largest bundle that fits"
        );
    }

    /// The wire cap and the simulation's roster guard must be the same number.
    ///
    /// They live in different crates and `observed_match` cannot see this one —
    /// the dependency runs the other way — so the agreement is asserted here,
    /// where both are visible. Disagreement is invisible until a lobby fills to
    /// the wire's cap and the match then refuses to start.
    ///
    /// `observed_progression`'s `LAN_MAX_SEATS` is the third copy and is not
    /// reachable from any crate that can also see these two; it pins itself to
    /// the same literal.
    #[test]
    fn the_wire_cap_and_the_roster_guard_agree() {
        assert_eq!(
            MAX_SEATS,
            usize::from(observed_match::hex_wfc::MAX_ROSTER),
            "observed_net::lan::MAX_SEATS and observed_match MAX_ROSTER disagree"
        );
    }

    /// A seat count past the cap is refused rather than allocated for.
    #[test]
    fn a_frame_claiming_more_than_the_cap_is_rejected() {
        let mut bytes = LanPacket::FrameBundle {
            frames: vec![WireFrame {
                tick: 1,
                commands: vec![WireHexCommand::default(); 2],
                digest: 7,
            }],
        }
        .encode()
        .expect("encodes");
        // The seat count sits after the envelope, the frame count and the tick.
        let seat_count = 12 + 1 + 8;
        bytes[seat_count] = (MAX_SEATS + 1) as u8;
        assert!(LanPacket::decode(&bytes).is_err());
    }

    fn test_client() -> LanClient {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 47_625));
        LanClient::connect(address, 7, None, None, [9; 32]).expect("loopback UDP client binds")
    }

    fn launch(match_number: u32, seed: u64) -> LanLaunch {
        LanLaunch {
            seed,
            match_number,
            config: HexMatchConfig::default(),
            simulation_content_hash: [4; 32],
        }
    }

    fn launch_packet(launch: LanLaunch) -> LanPacket {
        LanPacket::Launch {
            seed: launch.seed,
            match_number: launch.match_number,
            config: launch.config,
            simulation_content_hash: launch.simulation_content_hash,
        }
    }

    fn preserve_candidate_transport(client: &mut LanClient, next_frame: u64) -> WireFrame {
        client.next_frame = next_frame;
        let frame = WireFrame {
            tick: next_frame,
            commands: vec![WireHexCommand::default(); 4],
            digest: 88,
        };
        client.frames.insert(next_frame, frame.clone());
        frame
    }

    #[test]
    fn repeated_identical_launch_preserves_frame_progress() {
        let mut client = test_client();
        let launch = launch(3, 55);
        client.receive(launch_packet(launch));
        let frame = preserve_candidate_transport(&mut client, 8);

        client.receive(launch_packet(launch));

        assert_eq!(client.launch, Some(launch));
        assert_eq!(client.next_frame, 8);
        assert_eq!(client.frames.get(&8), Some(&frame));
        assert_eq!(client.phase, Some(WirePhase::InMatch));
        assert_eq!(client.launch_started, None);
        assert!(client.rejection.is_none());
    }

    #[test]
    fn older_launch_is_ignored_without_rewinding_transport() {
        let mut client = test_client();
        let current = launch(4, 56);
        client.receive(launch_packet(current));
        let frame = preserve_candidate_transport(&mut client, 12);

        client.receive(launch_packet(launch(3, 55)));

        assert_eq!(client.launch, Some(current));
        assert_eq!(client.next_frame, 12);
        assert_eq!(client.frames.get(&12), Some(&frame));
        assert_eq!(client.phase, Some(WirePhase::InMatch));
        assert!(client.rejection.is_none());
    }

    #[test]
    fn conflicting_descriptor_for_current_match_is_rejected_without_mutation() {
        let mut client = test_client();
        let current = launch(4, 56);
        client.receive(launch_packet(current));
        let frame = preserve_candidate_transport(&mut client, 12);

        client.receive(launch_packet(launch(4, 99)));

        assert_eq!(client.launch, Some(current));
        assert_eq!(client.next_frame, 12);
        assert_eq!(client.frames.get(&12), Some(&frame));
        assert_eq!(client.phase, Some(WirePhase::InMatch));
        assert_eq!(
            client.rejection.as_deref(),
            Some("conflicting launch descriptor for match 4")
        );
    }

    #[test]
    fn newer_launch_resets_transport_exactly_once() {
        let mut client = test_client();
        client.receive(launch_packet(launch(3, 55)));
        preserve_candidate_transport(&mut client, 8);
        let newer = launch(4, 56);

        client.receive(launch_packet(newer));

        assert_eq!(client.launch, Some(newer));
        assert_eq!(client.next_frame, 1);
        assert!(client.frames.is_empty());
        let frame = preserve_candidate_transport(&mut client, 9);

        client.receive(launch_packet(newer));

        assert_eq!(client.next_frame, 9);
        assert_eq!(client.frames.get(&9), Some(&frame));
        assert!(client.rejection.is_none());
    }

    #[test]
    fn launch_start_is_accepted_only_for_the_current_descriptor() {
        let mut client = test_client();
        client.token = Some(9);
        client.receive(launch_packet(launch(4, 56)));

        client
            .mark_launch_ready(4)
            .expect("accepted generation can publish readiness");
        assert_eq!(client.launch_ready, Some(4));
        assert!(!client.launch_has_started(4));

        client.receive(LanPacket::LaunchStart { match_number: 3 });
        client.receive(LanPacket::LaunchStart { match_number: 5 });
        assert_eq!(
            client.launch_started, None,
            "stale/future starts are ignored"
        );

        client.receive(LanPacket::LaunchStart { match_number: 4 });
        assert!(client.launch_has_started(4));

        client.receive(launch_packet(launch(4, 56)));
        assert!(
            client.launch_has_started(4),
            "an idempotent descriptor retry must preserve start"
        );
        client.receive(launch_packet(launch(5, 57)));
        assert_eq!(client.launch_ready, None);
        assert_eq!(client.launch_started, None);
    }

    #[test]
    fn readiness_rejects_a_generation_without_its_descriptor() {
        let mut client = test_client();
        client.token = Some(9);
        client.receive(launch_packet(launch(4, 56)));

        assert_eq!(
            client
                .mark_launch_ready(3)
                .expect_err("stale generation must not be published")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(client.launch_ready, None);
    }

    #[test]
    fn newer_lobby_snapshot_withdraws_launch_and_stale_packets_cannot_resurrect_it() {
        let mut client = test_client();
        client.token = Some(9);
        let descriptor = launch(4, 56);
        client.receive(launch_packet(descriptor));
        client.mark_launch_ready(4).expect("prepared");
        client.receive(LanPacket::LaunchStart { match_number: 4 });
        assert!(client.launch_has_started(4));

        let snapshot = |server_tick, phase| LanPacket::LobbySnapshot {
            session: 2,
            match_number: 4,
            server_tick,
            phase,
            countdown_ticks: 0,
            seats: Vec::new(),
        };
        client.receive(snapshot(20, WirePhase::Lobby));
        assert_eq!(client.phase, Some(WirePhase::Lobby));
        assert!(!client.launch_has_started(4));

        client.receive(snapshot(19, WirePhase::InMatch));
        client.receive(launch_packet(descriptor));
        client.receive(LanPacket::LaunchStart { match_number: 4 });
        assert_eq!(client.phase, Some(WirePhase::Lobby));
        assert_eq!(
            client.lobby.as_ref().map(|lobby| lobby.server_tick),
            Some(20)
        );
        assert!(!client.launch_has_started(4));
    }
}
