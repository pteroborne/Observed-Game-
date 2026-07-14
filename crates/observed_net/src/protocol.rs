//! Fixed, checksummed wire representation for the lockstep feasibility lab.
//! The simulation consumes the same `PlayerIntent` boundary as local play, but
//! the network transmits quantized values so every peer reconstructs identical
//! inputs without depending on platform-specific serialization.

use crate::PacketError;
use glam::Vec2;
use player_input::PlayerIntent;

pub const PEER_COUNT: usize = 2;
const MAGIC: [u8; 4] = *b"O2LK";
const VERSION: u8 = 2;
const HAS_INPUT: u8 = 1;
const NONE_FRAME: u32 = u32::MAX;
const PAYLOAD_LEN: usize = 70;
pub const PACKET_LEN: usize = PAYLOAD_LEN + 4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PeerId(pub u8);

impl PeerId {
    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn other(self) -> Self {
        Self(1 - self.0)
    }

    pub fn label(self) -> &'static str {
        match self.0 {
            0 => "PEER A",
            1 => "PEER B",
            _ => "PEER ?",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WireIntent {
    pub movement_x: i8,
    pub movement_y: i8,
    pub look_x: i8,
    pub look_y: i8,
    pub flags: u8,
}

impl WireIntent {
    const JUMP: u8 = 1 << 0;
    const SPRINT: u8 = 1 << 1;
    const INTERACT: u8 = 1 << 2;
    const INTERACT_HELD: u8 = 1 << 3;
    const CLIMB: u8 = 1 << 4;

    pub fn from_player_intent(intent: PlayerIntent) -> Self {
        let mut flags = 0;
        if intent.jump_pressed {
            flags |= Self::JUMP;
        }
        if intent.sprint_held {
            flags |= Self::SPRINT;
        }
        if intent.interact_pressed {
            flags |= Self::INTERACT;
        }
        if intent.interact_held {
            flags |= Self::INTERACT_HELD;
        }
        if intent.climb_pressed {
            flags |= Self::CLIMB;
        }
        Self {
            movement_x: quantize(intent.movement.x),
            movement_y: quantize(intent.movement.y),
            look_x: quantize(intent.look.x),
            look_y: quantize(intent.look.y),
            flags,
        }
    }

    pub fn to_player_intent(self) -> PlayerIntent {
        PlayerIntent {
            movement: Vec2::new(dequantize(self.movement_x), dequantize(self.movement_y)),
            look: Vec2::new(dequantize(self.look_x), dequantize(self.look_y)),
            jump_pressed: self.flags & Self::JUMP != 0,
            sprint_held: self.flags & Self::SPRINT != 0,
            interact_pressed: self.flags & Self::INTERACT != 0,
            interact_held: self.flags & Self::INTERACT_HELD != 0,
            climb_pressed: self.flags & Self::CLIMB != 0,
        }
    }
}

fn quantize(value: f32) -> i8 {
    (value.clamp(-1.0, 1.0) * 127.0).round() as i8
}

fn dequantize(value: i8) -> f32 {
    value as f32 / 127.0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusPacket {
    pub session_id: u32,
    pub sender: PeerId,
    pub input: Option<(u32, WireIntent)>,
    pub ack_through: Option<u32>,
    /// State after this many committed lockstep frames.
    pub state_frame: u32,
    pub state_hash: u64,
    /// Hash of every traversal-affecting content input (movement profile, ports,
    /// collision bakes, and authored layout data). Presentation assets are excluded.
    pub simulation_content_hash: [u8; 32],
    /// Stable traversal implementation identifier (`0` legacy AABB, `1` Rapier).
    pub simulation_backend: u8,
}

impl StatusPacket {
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = vec![0u8; PACKET_LEN];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4] = VERSION;
        bytes[5] = self.sender.0;
        if self.input.is_some() {
            bytes[6] |= HAS_INPUT;
        }
        bytes[8..12].copy_from_slice(&self.session_id.to_le_bytes());

        let (input_frame, intent) = self.input.unwrap_or((NONE_FRAME, WireIntent::default()));
        bytes[12..16].copy_from_slice(&input_frame.to_le_bytes());
        bytes[16] = intent.movement_x as u8;
        bytes[17] = intent.movement_y as u8;
        bytes[18] = intent.look_x as u8;
        bytes[19] = intent.look_y as u8;
        bytes[20] = intent.flags;
        bytes[21..25].copy_from_slice(&self.ack_through.unwrap_or(NONE_FRAME).to_le_bytes());
        bytes[25..29].copy_from_slice(&self.state_frame.to_le_bytes());
        bytes[29..37].copy_from_slice(&self.state_hash.to_le_bytes());
        bytes[37..69].copy_from_slice(&self.simulation_content_hash);
        bytes[69] = self.simulation_backend;
        let checksum = checksum32(&bytes[..PAYLOAD_LEN]);
        bytes[PAYLOAD_LEN..PACKET_LEN].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() != PACKET_LEN {
            return Err(PacketError::WrongLength);
        }
        if bytes[0..4] != MAGIC {
            return Err(PacketError::WrongMagic);
        }
        if bytes[4] != VERSION {
            return Err(PacketError::UnsupportedVersion);
        }
        if bytes[5] as usize >= PEER_COUNT {
            return Err(PacketError::InvalidPeer);
        }
        if bytes[6] & !HAS_INPUT != 0 {
            return Err(PacketError::InvalidFlags);
        }
        if bytes[69] > 1 {
            return Err(PacketError::InvalidFlags);
        }
        let expected = u32::from_le_bytes(bytes[PAYLOAD_LEN..PACKET_LEN].try_into().unwrap());
        if checksum32(&bytes[..PAYLOAD_LEN]) != expected {
            return Err(PacketError::BadChecksum);
        }

        let input_frame = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let input = (bytes[6] & HAS_INPUT != 0).then_some((
            input_frame,
            WireIntent {
                movement_x: bytes[16] as i8,
                movement_y: bytes[17] as i8,
                look_x: bytes[18] as i8,
                look_y: bytes[19] as i8,
                flags: bytes[20],
            },
        ));
        let ack = u32::from_le_bytes(bytes[21..25].try_into().unwrap());
        Ok(Self {
            session_id: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            sender: PeerId(bytes[5]),
            input,
            ack_through: (ack != NONE_FRAME).then_some(ack),
            state_frame: u32::from_le_bytes(bytes[25..29].try_into().unwrap()),
            state_hash: u64::from_le_bytes(bytes[29..37].try_into().unwrap()),
            simulation_content_hash: bytes[37..69].try_into().unwrap(),
            simulation_backend: bytes[69],
        })
    }
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for byte in bytes {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::time::Duration;

    use super::*;

    fn example_packet() -> StatusPacket {
        StatusPacket {
            session_id: 0x0b5e_2e16,
            sender: PeerId(1),
            input: Some((
                42,
                WireIntent::from_player_intent(PlayerIntent {
                    movement: Vec2::new(1.0, -0.5),
                    look: Vec2::new(-1.0, 0.25),
                    jump_pressed: true,
                    sprint_held: true,
                    ..Default::default()
                }),
            )),
            ack_through: Some(39),
            state_frame: 40,
            state_hash: 0x1234_5678_9abc_def0,
            simulation_content_hash: [0x5a; 32],
            simulation_backend: 1,
        }
    }

    #[test]
    fn packet_codec_round_trips_every_field() {
        let packet = example_packet();
        assert_eq!(StatusPacket::decode(&packet.encode()), Ok(packet));
    }

    #[test]
    fn corrupted_and_incompatible_packets_are_rejected() {
        let mut corrupt = example_packet().encode();
        corrupt[18] ^= 0x80;
        assert_eq!(
            StatusPacket::decode(&corrupt),
            Err(PacketError::BadChecksum)
        );

        let mut wrong_version = example_packet().encode();
        wrong_version[4] = VERSION + 1;
        let checksum = checksum32(&wrong_version[..PAYLOAD_LEN]);
        wrong_version[PAYLOAD_LEN..].copy_from_slice(&checksum.to_le_bytes());
        assert_eq!(
            StatusPacket::decode(&wrong_version),
            Err(PacketError::UnsupportedVersion)
        );
    }

    #[test]
    fn quantized_intents_are_stable_on_the_wire() {
        let original = PlayerIntent {
            movement: Vec2::new(0.25, -0.75),
            look: Vec2::new(-0.5, 1.0),
            jump_pressed: true,
            sprint_held: true,
            interact_pressed: true,
            interact_held: true,
            climb_pressed: true,
        };
        let wire = WireIntent::from_player_intent(original);
        assert_eq!(
            WireIntent::from_player_intent(wire.to_player_intent()),
            wire
        );
    }

    #[test]
    fn encoded_packets_cross_real_udp_loopback() {
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set timeout");
        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        let packet = example_packet();
        sender
            .send_to(&packet.encode(), receiver.local_addr().unwrap())
            .expect("send datagram");

        let mut buffer = [0u8; PACKET_LEN];
        let (len, _) = receiver.recv_from(&mut buffer).expect("receive datagram");
        assert_eq!(len, PACKET_LEN);
        assert_eq!(StatusPacket::decode(&buffer), Ok(packet));
    }
}
