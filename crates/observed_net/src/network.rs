//! Deterministic peer-to-peer **lockstep** over a hostile datagram transport: each
//! peer owns one `PlayerIntent` but both simulate both bodies; a frame commits only
//! once both inputs arrive; packets carry cumulative ACKs and resend until
//! acknowledged, so loss/delay/duplication/reordering stall but never diverge. Every
//! committed frame is the replay tape. Promoted out of `network_lab` in refactor R9.

//! Phase 16 feasibility model: deterministic peer-to-peer lockstep over a
//! hostile datagram transport.
//!
//! Each peer owns one first-person `PlayerIntent`, but both peers simulate both
//! bodies. A frame commits only after both inputs arrive. Packets carry cumulative
//! acknowledgements and are resent until acknowledged, so deterministic loss,
//! delay, duplication, and reordering cause visible stalls but not divergence.
//! Every committed frame is also the replay tape.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Vec2, Vec3};
use observed_traversal::{
    FIXED_DT, FpsArena, FpsBody, FpsConfig, PhysicsBackend,
};
use player_input::PlayerIntent;

use crate::protocol::{PEER_COUNT, PeerId, StatusPacket, WireIntent};

pub const SESSION_ID: u32 = 0x1600_2026;
pub const TARGET_FRAMES: u32 = 240;
const INPUT_LEAD: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockstepFrame {
    pub index: u32,
    pub inputs: [WireIntent; PEER_COUNT],
}

#[derive(Clone, Debug)]
pub struct LockstepTape {
    pub frames: Vec<LockstepFrame>,
    pub physics_backend: PhysicsBackend,
    pub simulation_content_hash: [u8; 32],
}

impl LockstepTape {
    pub fn replay_to(&self, frame: usize) -> LockstepWorld {
        let mut world = LockstepWorld::authored_with_backend(self.physics_backend);
        for committed in &self.frames[..frame.min(self.frames.len())] {
            world.step(*committed);
        }
        world
    }

    pub fn final_hash(&self) -> u64 {
        self.replay_to(self.frames.len()).state_hash()
    }
}

#[derive(Clone, Debug)]
pub struct LockstepWorld {
    pub bodies: [FpsBody; PEER_COUNT],
    pub tick: u32,
    config: FpsConfig,
    rapier: observed_traversal::rapier_controller::RapierTraversalScene,
}

impl LockstepWorld {
    pub fn authored() -> Self {
        Self::authored_with_backend(PhysicsBackend::LegacyAabb)
    }

    pub fn authored_with_backend(backend: PhysicsBackend) -> Self {
        let arena = FpsArena::authored();
        let config = match backend {
            PhysicsBackend::LegacyAabb => FpsConfig::default(),
            PhysicsBackend::Rapier => FpsConfig::deliberate_rapier(),
        };
        let rapier = observed_traversal::rapier_controller::RapierTraversalScene::from_arena(&arena);
        Self {
            bodies: [
                FpsBody::spawned(Vec3::new(-7.5, 0.9, 13.5), 0.0),
                FpsBody::spawned(Vec3::new(7.5, 0.9, 13.5), 0.0),
            ],
            tick: 0,
            config,
            rapier,
        }
    }

    pub fn step(&mut self, frame: LockstepFrame) {
        assert_eq!(frame.index, self.tick, "frames must commit in order");
        for (body, input) in self.bodies.iter_mut().zip(frame.inputs) {
            observed_traversal::rapier_controller::step_character(
                &self.rapier,
                body,
                input.to_player_intent(),
                &self.config,
                FIXED_DT,
            );
        }
        self.tick += 1;
    }

    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        hash_u32(&mut hash, self.tick);
        for body in &self.bodies {
            for value in [
                body.position.x,
                body.position.y,
                body.position.z,
                body.velocity.x,
                body.velocity.y,
                body.velocity.z,
                body.yaw,
                body.pitch,
                body.jump_cd,
                body.spawn.x,
                body.spawn.y,
                body.spawn.z,
                body.spawn_yaw,
            ] {
                hash_u32(&mut hash, value.to_bits());
            }
            hash_byte(&mut hash, body.grounded as u8);
        }
        hash
    }
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= byte as u64;
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Desync {
    pub frame: u32,
    pub local_hash: u64,
    pub remote_hash: u64,
}

#[derive(Clone, Debug)]
pub struct PeerSession {
    pub id: PeerId,
    pub world: LockstepWorld,
    pub next_frame: u32,
    pub tape: LockstepTape,
    pub path: Vec<[Vec3; PEER_COUNT]>,
    pub sent_packets: u32,
    pub resent_packets: u32,
    pub received_packets: u32,
    pub duplicate_inputs: u32,
    pub rejected_packets: u32,
    pub wait_ticks: u32,
    pub desync: Option<Desync>,
    pub simulation_content_hash: [u8; 32],
    local_inputs: BTreeMap<u32, WireIntent>,
    remote_inputs: BTreeMap<u32, WireIntent>,
    outbox: BTreeMap<u32, WireIntent>,
    send_counts: BTreeMap<u32, u32>,
    received_remote_frames: BTreeSet<u32>,
    ack_through: Option<u32>,
    remote_ack_through: Option<u32>,
    local_hashes: BTreeMap<u32, u64>,
    remote_hashes: BTreeMap<u32, u64>,
    send_cursor: usize,
}

impl PeerSession {
    pub fn new(id: PeerId) -> Self {
        Self::new_with_backend_content(id, PhysicsBackend::LegacyAabb, [0; 32])
    }

    pub fn new_with_backend_content(
        id: PeerId,
        physics_backend: PhysicsBackend,
        simulation_content_hash: [u8; 32],
    ) -> Self {
        let world = LockstepWorld::authored_with_backend(physics_backend);
        let initial_hash = world.state_hash();
        Self {
            id,
            world,
            next_frame: 0,
            tape: LockstepTape {
                frames: Vec::new(),
                physics_backend,
                simulation_content_hash,
            },
            path: Vec::new(),
            sent_packets: 0,
            resent_packets: 0,
            received_packets: 0,
            duplicate_inputs: 0,
            rejected_packets: 0,
            wait_ticks: 0,
            desync: None,
            simulation_content_hash,
            local_inputs: BTreeMap::new(),
            remote_inputs: BTreeMap::new(),
            outbox: BTreeMap::new(),
            send_counts: BTreeMap::new(),
            received_remote_frames: BTreeSet::new(),
            ack_through: None,
            remote_ack_through: None,
            local_hashes: BTreeMap::from([(0, initial_hash)]),
            remote_hashes: BTreeMap::new(),
            send_cursor: 0,
        }
    }

    pub fn queue_input_lead(&mut self, target_frames: u32) {
        let end = (self.next_frame + INPUT_LEAD).min(target_frames);
        for frame in self.next_frame..end {
            if self.local_inputs.contains_key(&frame) {
                continue;
            }
            let input = scripted_input(self.id, frame);
            self.local_inputs.insert(frame, input);
            self.outbox.insert(frame, input);
        }
    }

    pub fn status_packet(&mut self) -> StatusPacket {
        let pending: Vec<(u32, WireIntent)> = self
            .outbox
            .iter()
            .map(|(frame, input)| (*frame, *input))
            .collect();
        let input = if pending.is_empty() {
            None
        } else {
            let selected = pending[self.send_cursor % pending.len()];
            self.send_cursor = self.send_cursor.wrapping_add(1);
            let count = self.send_counts.entry(selected.0).or_default();
            if *count > 0 {
                self.resent_packets += 1;
            }
            *count += 1;
            Some(selected)
        };
        self.sent_packets += 1;
        StatusPacket {
            session_id: SESSION_ID,
            sender: self.id,
            input,
            ack_through: self.ack_through,
            state_frame: self.next_frame,
            state_hash: self.world.state_hash(),
            simulation_content_hash: self.simulation_content_hash,
            simulation_backend: match self.tape.physics_backend {
                PhysicsBackend::LegacyAabb => 0,
                PhysicsBackend::Rapier => 1,
            },
        }
    }

    pub fn receive(&mut self, bytes: &[u8]) {
        let Ok(packet) = StatusPacket::decode(bytes) else {
            self.rejected_packets += 1;
            return;
        };
        if packet.session_id != SESSION_ID
            || packet.sender != self.id.other()
            || packet.simulation_content_hash != self.simulation_content_hash
            || packet.simulation_backend
                != match self.tape.physics_backend {
                    PhysicsBackend::LegacyAabb => 0,
                    PhysicsBackend::Rapier => 1,
                }
        {
            self.rejected_packets += 1;
            return;
        }
        self.received_packets += 1;

        if let Some(acked) = packet.ack_through {
            self.remote_ack_through =
                Some(self.remote_ack_through.map_or(acked, |old| old.max(acked)));
            self.outbox.retain(|frame, _| *frame > acked);
        }

        if let Some((frame, input)) = packet.input {
            if !self.received_remote_frames.insert(frame) {
                self.duplicate_inputs += 1;
            } else {
                self.remote_inputs.insert(frame, input);
                self.advance_ack();
            }
        }

        self.remote_hashes
            .insert(packet.state_frame, packet.state_hash);
        self.compare_hash(packet.state_frame);
    }

    fn advance_ack(&mut self) {
        let mut next = self.ack_through.map_or(0, |frame| frame + 1);
        while self.received_remote_frames.contains(&next) {
            self.ack_through = Some(next);
            next += 1;
        }
    }

    pub fn commit_ready_frames(&mut self, target_frames: u32) {
        let before = self.next_frame;
        while self.next_frame < target_frames {
            let Some(local) = self.local_inputs.remove(&self.next_frame) else {
                break;
            };
            let Some(remote) = self.remote_inputs.remove(&self.next_frame) else {
                self.local_inputs.insert(self.next_frame, local);
                break;
            };
            let mut inputs = [WireIntent::default(); PEER_COUNT];
            inputs[self.id.index()] = local;
            inputs[self.id.other().index()] = remote;
            let frame = LockstepFrame {
                index: self.next_frame,
                inputs,
            };
            self.world.step(frame);
            self.tape.frames.push(frame);
            self.path.push(self.world.bodies.map(|body| body.position));
            self.next_frame += 1;
            let hash = self.world.state_hash();
            self.local_hashes.insert(self.next_frame, hash);
            self.compare_hash(self.next_frame);
        }
        if before == self.next_frame && self.next_frame < target_frames {
            self.wait_ticks += 1;
        }
    }

    fn compare_hash(&mut self, frame: u32) {
        if self.desync.is_some() {
            return;
        }
        let (Some(local_hash), Some(remote_hash)) = (
            self.local_hashes.get(&frame).copied(),
            self.remote_hashes.get(&frame).copied(),
        ) else {
            return;
        };
        if local_hash != remote_hash {
            self.desync = Some(Desync {
                frame,
                local_hash,
                remote_hash,
            });
        }
    }

    pub fn inject_divergence(&mut self) {
        self.world.bodies[0].position.x += 0.25;
        let hash = self.world.state_hash();
        self.local_hashes.insert(self.next_frame, hash);
        self.compare_hash(self.next_frame);
    }

    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    pub fn remote_hash(&self, frame: u32) -> Option<u64> {
        self.remote_hashes.get(&frame).copied()
    }
}

fn scripted_input(peer: PeerId, frame: u32) -> WireIntent {
    let intent = match peer.0 {
        0 if frame < 70 => PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            sprint_held: true,
            ..Default::default()
        },
        0 if frame < 125 => PlayerIntent {
            movement: Vec2::new(0.0, 0.8),
            look: Vec2::new(1.0, 0.0),
            ..Default::default()
        },
        0 => PlayerIntent {
            movement: Vec2::new(0.8, 0.5),
            jump_pressed: frame == 150,
            ..Default::default()
        },
        1 if frame < 55 => PlayerIntent {
            movement: Vec2::new(-0.4, 1.0),
            ..Default::default()
        },
        1 if frame < 115 => PlayerIntent {
            movement: Vec2::new(0.0, 0.7),
            look: Vec2::new(-1.0, 0.0),
            sprint_held: true,
            ..Default::default()
        },
        _ => PlayerIntent {
            movement: Vec2::new(-0.7, 0.55),
            jump_pressed: frame == 145,
            ..Default::default()
        },
    };
    WireIntent::from_player_intent(intent)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkProfile {
    Clean,
    Hostile,
}

impl NetworkProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "CLEAN",
            Self::Hostile => "HOSTILE",
        }
    }
}

#[derive(Clone, Debug)]
struct Datagram {
    deliver_tick: u32,
    ordinal: u64,
    from: PeerId,
    to: PeerId,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SimulatedNetwork {
    pub profile: NetworkProfile,
    pub tick: u32,
    pub sent: u32,
    pub dropped: u32,
    pub duplicated: u32,
    pub delivered: u32,
    pub reordered: u32,
    queue: Vec<Datagram>,
    ordinal: u64,
    last_delivered_ordinal: Option<u64>,
}

impl SimulatedNetwork {
    pub fn new(profile: NetworkProfile) -> Self {
        Self {
            profile,
            tick: 0,
            sent: 0,
            dropped: 0,
            duplicated: 0,
            delivered: 0,
            reordered: 0,
            queue: Vec::new(),
            ordinal: 0,
            last_delivered_ordinal: None,
        }
    }

    pub fn send(&mut self, from: PeerId, to: PeerId, bytes: Vec<u8>) {
        self.sent += 1;
        self.ordinal += 1;
        let ordinal = self.ordinal;

        if self.profile == NetworkProfile::Hostile && ordinal.is_multiple_of(5) {
            self.dropped += 1;
            return;
        }

        let (base, jitter) = match self.profile {
            NetworkProfile::Clean => (1, 0),
            NetworkProfile::Hostile => (2, ((ordinal * 7 + from.0 as u64 * 3) % 6) as u32),
        };
        self.queue.push(Datagram {
            deliver_tick: self.tick + base + jitter,
            ordinal,
            from,
            to,
            bytes: bytes.clone(),
        });

        if self.profile == NetworkProfile::Hostile && ordinal.is_multiple_of(7) {
            self.duplicated += 1;
            self.queue.push(Datagram {
                deliver_tick: self.tick + base + ((jitter + 3) % 6),
                ordinal,
                from,
                to,
                bytes,
            });
        }
    }

    fn advance(&mut self) -> Vec<Datagram> {
        self.tick += 1;
        let mut due = Vec::new();
        let mut pending = Vec::new();
        for datagram in self.queue.drain(..) {
            if datagram.deliver_tick <= self.tick {
                due.push(datagram);
            } else {
                pending.push(datagram);
            }
        }
        self.queue = pending;
        due.sort_by_key(|packet| (packet.deliver_tick, std::cmp::Reverse(packet.ordinal)));
        for packet in &due {
            if self
                .last_delivered_ordinal
                .is_some_and(|last| packet.ordinal < last)
            {
                self.reordered += 1;
            }
            self.last_delivered_ordinal = Some(packet.ordinal);
        }
        self.delivered += due.len() as u32;
        due
    }

    /// Advance one transport tick and return the datagrams delivered this tick as
    /// `(recipient, bytes)` pairs. This is the public, byte-generic surface a second
    /// consumer drives directly (the in-lab `advance_transport_tick` keeps using the
    /// private `advance`); the deterministic loss / delay / duplication / reordering
    /// behaviour is identical either way.
    pub fn step(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        self.advance()
            .into_iter()
            .map(|datagram| (datagram.to, datagram.bytes))
            .collect()
    }

    pub fn in_flight(&self) -> usize {
        self.queue.len()
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct LockstepDemo {
    pub peers: [PeerSession; PEER_COUNT],
    pub network: SimulatedNetwork,
    pub target_frames: u32,
    pub transport_ticks: u32,
}

impl LockstepDemo {
    pub fn authored(profile: NetworkProfile) -> Self {
        Self::authored_with_backend_content(profile, PhysicsBackend::LegacyAabb, [0; 32])
    }

    pub fn authored_with_backend_content(
        profile: NetworkProfile,
        physics_backend: PhysicsBackend,
        simulation_content_hash: [u8; 32],
    ) -> Self {
        Self {
            peers: [
                PeerSession::new_with_backend_content(
                    PeerId(0),
                    physics_backend,
                    simulation_content_hash,
                ),
                PeerSession::new_with_backend_content(
                    PeerId(1),
                    physics_backend,
                    simulation_content_hash,
                ),
            ],
            network: SimulatedNetwork::new(profile),
            target_frames: TARGET_FRAMES,
            transport_ticks: 0,
        }
    }

    pub fn reset(&mut self, profile: NetworkProfile) {
        *self = Self::authored(profile);
    }

    pub fn advance_transport_tick(&mut self) {
        for peer in &mut self.peers {
            peer.queue_input_lead(self.target_frames);
        }
        for index in 0..PEER_COUNT {
            let packet = self.peers[index].status_packet().encode();
            let from = PeerId(index as u8);
            self.network.send(from, from.other(), packet);
        }

        for datagram in self.network.advance() {
            debug_assert_eq!(datagram.from.other(), datagram.to);
            self.peers[datagram.to.index()].receive(&datagram.bytes);
        }
        for peer in &mut self.peers {
            peer.commit_ready_frames(self.target_frames);
        }
        self.transport_ticks += 1;
    }

    pub fn run_until_synchronized(&mut self, max_ticks: u32) {
        for _ in 0..max_ticks {
            if self.synchronized() {
                return;
            }
            self.advance_transport_tick();
        }
    }

    pub fn frames_match(&self) -> bool {
        self.peers[0].next_frame == self.peers[1].next_frame
    }

    pub fn hashes_match(&self) -> bool {
        self.peers[0].world.state_hash() == self.peers[1].world.state_hash()
    }

    pub fn states_match_exactly(&self) -> bool {
        self.peers[0].world.tick == self.peers[1].world.tick
            && self.peers[0].world.bodies == self.peers[1].world.bodies
    }

    pub fn tapes_match(&self) -> bool {
        self.peers[0].tape.frames == self.peers[1].tape.frames
    }

    pub fn replay_matches(&self) -> bool {
        self.peers.iter().all(|peer| {
            let replay = peer.tape.replay_to(peer.tape.frames.len());
            peer.tape.frames.len() == peer.next_frame as usize
                && replay.tick == peer.world.tick
                && replay.bodies == peer.world.bodies
                && replay.state_hash() == peer.world.state_hash()
        })
    }

    pub fn synchronized(&self) -> bool {
        self.peers
            .iter()
            .all(|peer| peer.next_frame == self.target_frames)
            && self.peers.iter().all(|peer| peer.outbox_len() == 0)
            && self.frames_match()
            && self.hashes_match()
            && self.states_match_exactly()
            && self.tapes_match()
            && self.replay_matches()
            && self.peers.iter().all(|peer| {
                peer.remote_hash(self.target_frames) == Some(peer.world.state_hash())
                    && peer.desync.is_none()
            })
    }

    pub fn has_desync(&self) -> bool {
        self.peers.iter().any(|peer| peer.desync.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_never_advances_without_the_complete_frame() {
        let mut peer = PeerSession::new(PeerId(0));
        peer.queue_input_lead(4);
        peer.commit_ready_frames(4);
        assert_eq!(peer.next_frame, 0);
        assert_eq!(peer.wait_ticks, 1);
    }

    #[test]
    fn duplicate_input_packets_are_idempotent() {
        let mut sender = PeerSession::new(PeerId(0));
        let mut receiver = PeerSession::new(PeerId(1));
        sender.queue_input_lead(2);
        let bytes = sender.status_packet().encode();
        receiver.receive(&bytes);
        receiver.receive(&bytes);
        assert_eq!(receiver.duplicate_inputs, 1);
        receiver.queue_input_lead(2);
        receiver.commit_ready_frames(2);
        assert_eq!(receiver.next_frame, 1);
    }

    #[test]
    fn hostile_loss_delay_duplication_and_reordering_still_converge() {
        let mut demo = LockstepDemo::authored(NetworkProfile::Hostile);
        demo.run_until_synchronized(20_000);
        assert!(
            demo.synchronized(),
            "reliable lockstep must eventually converge"
        );
        assert!(demo.network.dropped > 0);
        assert!(demo.network.duplicated > 0);
        assert!(demo.network.reordered > 0);
        assert!(demo.peers.iter().any(|peer| peer.resent_packets > 0));
        assert!(demo.peers.iter().any(|peer| peer.wait_ticks > 0));
        assert!(demo.states_match_exactly());
    }

    #[test]
    fn both_peers_commit_the_identical_tape_and_replay_it_exactly() {
        let mut demo = LockstepDemo::authored(NetworkProfile::Hostile);
        demo.run_until_synchronized(20_000);
        assert_eq!(demo.peers[0].tape.frames, demo.peers[1].tape.frames);
        assert!(demo.states_match_exactly());
        assert!(demo.replay_matches());
    }

    #[test]
    fn per_frame_hash_exchange_detects_a_deliberate_divergence() {
        let mut demo = LockstepDemo::authored(NetworkProfile::Clean);
        demo.target_frames = 40;
        demo.run_until_synchronized(2_000);
        assert!(demo.synchronized());

        demo.peers[0].inject_divergence();
        for _ in 0..20 {
            demo.advance_transport_tick();
            if demo.has_desync() {
                break;
            }
        }
        assert!(demo.has_desync(), "a changed state hash must report desync");
    }

    #[test]
    fn the_public_transport_step_delivers_bytes_to_the_recipient() {
        let mut network = SimulatedNetwork::new(NetworkProfile::Clean);
        network.send(PeerId(0), PeerId(1), vec![1, 2, 3]);
        // Clean profile delivers after a fixed delay; drive ticks until it arrives.
        let mut delivered = Vec::new();
        for _ in 0..8 {
            delivered.extend(network.step());
            if !delivered.is_empty() {
                break;
            }
        }
        assert_eq!(delivered, vec![(PeerId(1), vec![1, 2, 3])]);
        assert_eq!(network.delivered, 1);
    }

    #[test]
    fn reset_restores_a_fresh_session() {
        let mut demo = LockstepDemo::authored(NetworkProfile::Hostile);
        for _ in 0..100 {
            demo.advance_transport_tick();
        }
        demo.reset(NetworkProfile::Clean);
        assert_eq!(demo.transport_ticks, 0);
        assert_eq!(demo.network.profile, NetworkProfile::Clean);
        assert!(demo.peers.iter().all(|peer| peer.next_frame == 0));
        assert!(demo.peers.iter().all(|peer| peer.tape.frames.is_empty()));
        assert!(demo.hashes_match());
    }

    #[test]
    fn peers_reject_different_simulation_content_before_committing() {
        let mut sender =
            PeerSession::new_with_backend_content(PeerId(0), PhysicsBackend::Rapier, [1; 32]);
        let mut receiver =
            PeerSession::new_with_backend_content(PeerId(1), PhysicsBackend::Rapier, [2; 32]);
        sender.queue_input_lead(4);
        receiver.receive(&sender.status_packet().encode());
        assert_eq!(receiver.rejected_packets, 1);
        assert_eq!(receiver.next_frame, 0);
    }

    #[test]
    fn peers_reject_different_physics_backends_before_committing() {
        let mut sender =
            PeerSession::new_with_backend_content(PeerId(0), PhysicsBackend::Rapier, [3; 32]);
        let mut receiver =
            PeerSession::new_with_backend_content(PeerId(1), PhysicsBackend::LegacyAabb, [3; 32]);
        sender.queue_input_lead(4);
        receiver.receive(&sender.status_packet().encode());
        assert_eq!(receiver.rejected_packets, 1);
        assert_eq!(receiver.next_frame, 0);
    }

    #[test]
    fn rapier_lockstep_worlds_remain_bit_identical() {
        let mut a = LockstepWorld::authored_with_backend(PhysicsBackend::Rapier);
        let mut b = a.clone();
        for index in 0..TARGET_FRAMES {
            let frame = LockstepFrame {
                index,
                inputs: [
                    scripted_input(PeerId(0), index),
                    scripted_input(PeerId(1), index),
                ],
            };
            a.step(frame);
            b.step(frame);
            assert_eq!(a.bodies, b.bodies, "Rapier diverged at frame {index}");
            assert_eq!(a.state_hash(), b.state_hash());
        }
    }
}
