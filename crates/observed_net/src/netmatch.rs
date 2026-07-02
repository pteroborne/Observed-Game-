//! Host-authoritative networked first-person match: the deterministic hybrid match
//! (observed_match) replicated to a remote peer over the lockstep transport, so two peers
//! reconstruct the identical match/maze/pose. Promoted out of net_match_lab in refactor R9.

//! Phase 28 feasibility model: the **networked first-person hybrid match**.
//!
//! This is the integration of two proven results:
//!
//! - `network_lab` proved deterministic peer-to-peer **lockstep over a hostile
//!   datagram transport** — loss, delay, duplication, and reordering cause stalls
//!   but never divergence — using its byte-generic [`SimulatedNetwork`].
//! - `fps_hybrid_match_lab` proved the **first-person hybrid match is
//!   deterministic and replayable** from a stream of round [`LocalAction`]s.
//!
//! Here both peers run the *same* `HybridMatch` and exchange the local team's
//! per-round action over the hostile transport. The local team has two members
//! (the proven `MEMBERS_PER_TEAM = 2`); we model each member as a peer that **owns
//! alternate rounds'** action. A peer commits a round only once it holds the
//! authoritative action for it — its own for owned rounds, or the *received* one
//! for the teammate's rounds (it never shortcuts to its locally-known script). So
//! advancing genuinely requires the network, and reliable resend/ack guarantees
//! both peers converge on the **identical match, maze, and first-person pose,
//! round-for-round**, equal to the single-player tape.
//!
//! Only the per-round action wire format and the reliable action peer are new; the
//! transport and the match brain are reused wholesale.

use std::collections::{BTreeMap, BTreeSet};

use crate::PacketError;
use crate::network::{NetworkProfile, SimulatedNetwork};
use crate::protocol::{PEER_COUNT, PeerId};
use observed_facility::map_spec::MapSpec;
use observed_match::hybrid::{HybridMatch, HybridSnapshot, HybridTape, LOCAL_TEAM, LocalAction};
use player_input::PlayerIntent;

pub const SESSION_ID: u32 = 0x2800_2026;
/// How far ahead of the committed round a peer produces its owned actions.
const INPUT_LEAD: u32 = 6;

/// Which peer owns a given round's action (the two teammates alternate).
fn owner(round: u32) -> PeerId {
    PeerId((round % PEER_COUNT as u32) as u8)
}

// --- wire format -----------------------------------------------------------
const MAGIC: [u8; 4] = *b"O2NM";
const VERSION: u8 = 1;
const HAS_ACTION: u8 = 1;
const NONE_ROUND: u32 = u32::MAX;
const PAYLOAD_LEN: usize = 20;
pub const PACKET_LEN: usize = PAYLOAD_LEN + 4;

fn action_code(action: LocalAction) -> u8 {
    match action {
        LocalAction::Advance => 0,
        LocalAction::Seize => 1,
        LocalAction::Wait => 2,
    }
}

fn action_from_code(code: u8) -> Option<LocalAction> {
    match code {
        0 => Some(LocalAction::Advance),
        1 => Some(LocalAction::Seize),
        2 => Some(LocalAction::Wait),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionPacket {
    pub session_id: u32,
    pub sender: PeerId,
    pub action: Option<(u32, LocalAction)>,
    pub ack_through: Option<u32>,
}

impl ActionPacket {
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = vec![0u8; PACKET_LEN];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4] = VERSION;
        bytes[5] = self.sender.0;
        let (round, action) = match self.action {
            Some((round, action)) => {
                bytes[6] |= HAS_ACTION;
                (round, action_code(action))
            }
            None => (NONE_ROUND, 0),
        };
        bytes[7] = action;
        bytes[8..12].copy_from_slice(&self.session_id.to_le_bytes());
        bytes[12..16].copy_from_slice(&round.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.ack_through.unwrap_or(NONE_ROUND).to_le_bytes());
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
        let expected = u32::from_le_bytes(bytes[PAYLOAD_LEN..PACKET_LEN].try_into().unwrap());
        if checksum32(&bytes[..PAYLOAD_LEN]) != expected {
            return Err(PacketError::BadChecksum);
        }
        let round = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let action = if bytes[6] & HAS_ACTION != 0 {
            let decoded = action_from_code(bytes[7]).ok_or(PacketError::InvalidAction)?;
            Some((round, decoded))
        } else {
            None
        };
        let ack = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        Ok(Self {
            session_id: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            sender: PeerId(bytes[5]),
            action,
            ack_through: (ack != NONE_ROUND).then_some(ack),
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

// --- the reliable action peer ----------------------------------------------
#[derive(Clone, Debug)]
pub struct NetPeer {
    pub id: PeerId,
    pub match_state: HybridMatch,
    pub committed_round: u32,
    pub snapshots: Vec<HybridSnapshot>,
    pub sent_packets: u32,
    pub resent_packets: u32,
    pub received_packets: u32,
    pub duplicate_actions: u32,
    pub rejected_packets: u32,
    pub wait_ticks: u32,
    /// Every action this peer holds (owned-and-produced or received): the source
    /// for both committing and acknowledging.
    actions: BTreeMap<u32, LocalAction>,
    /// Owned actions not yet acknowledged by the teammate.
    outbox: BTreeMap<u32, LocalAction>,
    send_counts: BTreeMap<u32, u32>,
    received_rounds: BTreeSet<u32>,
    ack_through: Option<u32>,
    remote_ack_through: Option<u32>,
    next_produce: u32,
    send_cursor: usize,
}

impl NetPeer {
    pub fn new(id: PeerId, seed: u64) -> Self {
        let match_state = HybridMatch::authored(seed);
        let snapshots = vec![match_state.snapshot()];
        Self {
            id,
            match_state,
            committed_round: 0,
            snapshots,
            sent_packets: 0,
            resent_packets: 0,
            received_packets: 0,
            duplicate_actions: 0,
            rejected_packets: 0,
            wait_ticks: 0,
            actions: BTreeMap::new(),
            outbox: BTreeMap::new(),
            send_counts: BTreeMap::new(),
            received_rounds: BTreeSet::new(),
            ack_through: None,
            remote_ack_through: None,
            next_produce: 0,
            send_cursor: 0,
        }
    }

    pub fn new_for_map_spec(id: PeerId, seed: u64, spec: MapSpec) -> Self {
        let match_state = HybridMatch::for_map_spec(seed, spec);
        let snapshots = vec![match_state.snapshot()];
        Self {
            id,
            match_state,
            committed_round: 0,
            snapshots,
            sent_packets: 0,
            resent_packets: 0,
            received_packets: 0,
            duplicate_actions: 0,
            rejected_packets: 0,
            wait_ticks: 0,
            actions: BTreeMap::new(),
            outbox: BTreeMap::new(),
            send_counts: BTreeMap::new(),
            received_rounds: BTreeSet::new(),
            ack_through: None,
            remote_ack_through: None,
            next_produce: 0,
            send_cursor: 0,
        }
    }

    /// Produce this peer's owned actions a few rounds ahead of the commit cursor.
    pub fn produce(&mut self, total: u32, script: &[LocalAction]) {
        let end = (self.committed_round + INPUT_LEAD).min(total);
        while self.next_produce < end {
            let round = self.next_produce;
            if owner(round) == self.id {
                let action = script[round as usize];
                self.actions.insert(round, action);
                self.outbox.insert(round, action);
            }
            self.next_produce += 1;
        }
        self.advance_ack();
    }

    fn advance_ack(&mut self) {
        let mut next = self.ack_through.map_or(0, |frame| frame + 1);
        while self.actions.contains_key(&next) {
            self.ack_through = Some(next);
            next += 1;
        }
    }

    /// Build this tick's outgoing packet: one (re)sent owned action round-robin,
    /// plus the cumulative acknowledgement. Sends an ack-only packet when the
    /// outbox is empty so the teammate's outbox still drains.
    pub fn outgoing(&mut self) -> ActionPacket {
        let pending: Vec<(u32, LocalAction)> =
            self.outbox.iter().map(|(round, a)| (*round, *a)).collect();
        let action = if pending.is_empty() {
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
        ActionPacket {
            session_id: SESSION_ID,
            sender: self.id,
            action,
            ack_through: self.ack_through,
        }
    }

    pub fn receive(&mut self, bytes: &[u8]) {
        let Ok(packet) = ActionPacket::decode(bytes) else {
            self.rejected_packets += 1;
            return;
        };
        if packet.session_id != SESSION_ID || packet.sender != self.id.other() {
            self.rejected_packets += 1;
            return;
        }
        self.received_packets += 1;

        if let Some(acked) = packet.ack_through {
            self.remote_ack_through =
                Some(self.remote_ack_through.map_or(acked, |old| old.max(acked)));
            self.outbox.retain(|round, _| *round > acked);
        }

        if let Some((round, action)) = packet.action {
            if !self.received_rounds.insert(round) {
                self.duplicate_actions += 1;
            } else {
                self.actions.insert(round, action);
                self.advance_ack();
            }
        }
    }

    /// Commit and apply every round for which the authoritative action is in hand.
    pub fn commit(&mut self, total: u32) {
        let before = self.committed_round;
        while self.committed_round < total {
            let Some(&action) = self.actions.get(&self.committed_round) else {
                break;
            };
            assert!(
                self.match_state.apply_action(action),
                "a committed action must apply to the deterministic match"
            );
            self.committed_round += 1;
            self.snapshots.push(self.match_state.snapshot());
        }
        if before == self.committed_round && self.committed_round < total {
            self.wait_ticks += 1;
        }
    }

    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    /// Inject a locally-resolved action for sending (host-authoritative live play):
    /// the action is recorded and queued to the teammate, who replicates it.
    pub fn push_owned(&mut self, round: u32, action: LocalAction) {
        self.actions.insert(round, action);
        self.outbox.insert(round, action);
        self.advance_ack();
    }
}

// --- the networked match orchestrator --------------------------------------
#[derive(Clone, Debug)]
pub struct NetMatch {
    pub peers: [NetPeer; PEER_COUNT],
    pub network: SimulatedNetwork,
    pub seed: u64,
    pub script: Vec<LocalAction>,
    pub total: u32,
    pub transport_ticks: u32,
    /// The single-player tape's snapshots — the ground truth both peers must match.
    pub reference: Vec<HybridSnapshot>,
}

impl NetMatch {
    pub fn authored(seed: u64, profile: NetworkProfile) -> Self {
        let tape = HybridTape::record_demo(seed);
        let script: Vec<LocalAction> = tape.frames.iter().map(|frame| frame.local).collect();
        let total = script.len() as u32;
        Self {
            peers: [NetPeer::new(PeerId(0), seed), NetPeer::new(PeerId(1), seed)],
            network: SimulatedNetwork::new(profile),
            seed,
            script,
            total,
            transport_ticks: 0,
            reference: tape.snapshots,
        }
    }

    pub fn reset(&mut self, profile: NetworkProfile) {
        *self = Self::authored(self.seed, profile);
    }

    pub fn advance_transport_tick(&mut self) {
        for peer in &mut self.peers {
            peer.produce(self.total, &self.script);
        }
        for index in 0..PEER_COUNT {
            let packet = self.peers[index].outgoing().encode();
            let from = PeerId(index as u8);
            self.network.send(from, from.other(), packet);
        }
        for (to, bytes) in self.network.step() {
            self.peers[to.index()].receive(&bytes);
        }
        for peer in &mut self.peers {
            peer.commit(self.total);
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

    pub fn committed(&self) -> bool {
        self.peers
            .iter()
            .all(|peer| peer.committed_round == self.total)
    }

    pub fn both_finished(&self) -> bool {
        self.peers
            .iter()
            .all(|peer| peer.match_state.competitive.finished)
    }

    /// The two peers reconstructed bit-identical match state at every round.
    pub fn peers_agree(&self) -> bool {
        self.peers[0].snapshots == self.peers[1].snapshots
    }

    /// Both peers reproduced the single-player tape exactly (match, maze, pose).
    pub fn matches_reference(&self) -> bool {
        self.peers
            .iter()
            .all(|peer| peer.snapshots == self.reference)
    }

    pub fn outboxes_drained(&self) -> bool {
        self.peers.iter().all(|peer| peer.outbox_len() == 0)
    }

    pub fn synchronized(&self) -> bool {
        self.committed()
            && self.outboxes_drained()
            && self.both_finished()
            && self.peers_agree()
            && self.matches_reference()
    }

    pub fn total_resent(&self) -> u32 {
        self.peers.iter().map(|peer| peer.resent_packets).sum()
    }

    pub fn total_waits(&self) -> u32 {
        self.peers.iter().map(|peer| peer.wait_ticks).sum()
    }
}

/// A **live, host-authoritative networked match**: the host plays the hybrid match
/// in first person (driving its body with the controller); each round it resolves is
/// pushed over the hostile transport to a remote replica that reconstructs the
/// identical match. Because every resolved round ends in a canonical pose
/// (`HybridMatch` places the body in the room centre), the replica's per-round
/// snapshots equal the host's regardless of how the player physically walked there —
/// so live first-person play is bit-exactly replicable over the network.
///
/// This is the match the assembled game runs.
#[derive(Clone, Debug)]
pub struct LiveNetMatch {
    /// The host peer; `host.match_state` is the locally-played, authoritative match.
    pub host: NetPeer,
    /// The remote peer; `remote.match_state` is rebuilt purely from the network.
    pub remote: NetPeer,
    pub network: SimulatedNetwork,
    pub seed: u64,
    pub resolved: u32,
    pub transport_ticks: u32,
    pub map_spec: Option<MapSpec>,
}

impl LiveNetMatch {
    pub fn new(seed: u64, profile: NetworkProfile) -> Self {
        Self {
            host: NetPeer::new(PeerId(0), seed),
            remote: NetPeer::new(PeerId(1), seed),
            network: SimulatedNetwork::new(profile),
            seed,
            resolved: 0,
            transport_ticks: 0,
            map_spec: None,
        }
    }

    pub fn new_for_map_spec(seed: u64, profile: NetworkProfile, spec: MapSpec) -> Self {
        Self {
            host: NetPeer::new_for_map_spec(PeerId(0), seed, spec.clone()),
            remote: NetPeer::new_for_map_spec(PeerId(1), seed, spec.clone()),
            network: SimulatedNetwork::new(profile),
            seed,
            resolved: 0,
            transport_ticks: 0,
            map_spec: Some(spec),
        }
    }

    pub fn reset(&mut self, profile: NetworkProfile) {
        if let Some(spec) = self.map_spec.clone() {
            *self = Self::new_for_map_spec(self.seed, profile, spec);
        } else {
            *self = Self::new(self.seed, profile);
        }
    }

    pub fn host_match(&self) -> &HybridMatch {
        &self.host.match_state
    }

    pub fn finished(&self) -> bool {
        self.host.match_state.competitive.finished
    }

    pub fn local_active(&self) -> bool {
        self.host
            .match_state
            .competitive
            .team(LOCAL_TEAM)
            .is_some_and(|team| team.active_runner())
    }

    fn record_resolved(&mut self, action: LocalAction) {
        self.host.push_owned(self.resolved, action);
        self.host.snapshots.push(self.host.match_state.snapshot());
        self.resolved += 1;
    }

    /// Step the host's first-person controller; if crossing into the next spine room
    /// (or seizing) resolves a round, queue that action for the remote.
    pub fn step_host(&mut self, intent: PlayerIntent, top_down: bool) -> Option<LocalAction> {
        if self.finished() {
            return None;
        }
        let action = self.host.match_state.step_player(intent, top_down)?;
        self.record_resolved(action);
        Some(action)
    }

    /// Resolve a round directly (used to keep the match moving when the local team is
    /// no longer an active runner, and for headless play/tests).
    pub fn force_round(&mut self, action: LocalAction) -> bool {
        if self.finished() || !self.host.match_state.apply_action(action) {
            return false;
        }
        self.record_resolved(action);
        true
    }

    /// One transport tick: the host sends its pending actions reliably, the remote
    /// acknowledges, and the remote commits every round it now holds.
    pub fn pump(&mut self) {
        let host_packet = self.host.outgoing().encode();
        self.network.send(PeerId(0), PeerId(1), host_packet);
        let remote_packet = self.remote.outgoing().encode();
        self.network.send(PeerId(1), PeerId(0), remote_packet);
        for (to, bytes) in self.network.step() {
            if to == PeerId(0) {
                self.host.receive(&bytes);
            } else {
                self.remote.receive(&bytes);
            }
        }
        self.remote.commit(self.resolved);
        self.transport_ticks += 1;
    }

    /// The remote has replicated every resolved round and agrees with the host.
    pub fn in_sync(&self) -> bool {
        self.remote.committed_round == self.resolved && self.host.snapshots == self.remote.snapshots
    }

    pub fn synchronized(&self) -> bool {
        self.finished() && self.in_sync() && self.host.outbox_len() == 0
    }

    /// Play the whole match headless (advance while the local team is active, else
    /// wait), pumping the network so the remote stays in sync. For tests and a
    /// headless "simulate" path.
    pub fn run_to_completion_headless(&mut self, max_ticks: u32) {
        let mut guard = 0;
        while !self.finished() && guard < 256 {
            let action = if self.local_active() {
                LocalAction::Advance
            } else {
                LocalAction::Wait
            };
            self.force_round(action);
            self.pump();
            guard += 1;
        }
        for _ in 0..max_ticks {
            if self.synchronized() {
                break;
            }
            self.pump();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_match::facility::TEAM_COUNT;
    use observed_match::hybrid::LOCAL_TEAM;

    #[test]
    fn the_action_packet_round_trips_and_rejects_corruption() {
        let packet = ActionPacket {
            session_id: SESSION_ID,
            sender: PeerId(1),
            action: Some((7, LocalAction::Seize)),
            ack_through: Some(6),
        };
        assert_eq!(ActionPacket::decode(&packet.encode()), Ok(packet));

        let mut corrupt = packet.encode();
        corrupt[7] ^= 0x40;
        assert_eq!(
            ActionPacket::decode(&corrupt),
            Err(PacketError::BadChecksum)
        );

        let ack_only = ActionPacket {
            session_id: SESSION_ID,
            sender: PeerId(0),
            action: None,
            ack_through: None,
        };
        assert_eq!(ActionPacket::decode(&ack_only.encode()), Ok(ack_only));
    }

    #[test]
    fn a_peer_will_not_commit_a_teammate_round_until_it_arrives() {
        // Peer 0 owns even rounds, peer 1 owns odd rounds. Peer 0 alone cannot get
        // past round 1 (round 1 is the teammate's, never received) — it must wait.
        let net = NetMatch::authored(1, NetworkProfile::Clean);
        let mut peer = net.peers[0].clone();
        peer.produce(net.total, &net.script);
        peer.commit(net.total);
        assert_eq!(
            peer.committed_round, 1,
            "owns round 0 only, blocks on round 1"
        );
        // A second commit with no new actions stalls — it genuinely waits for the
        // teammate's round-1 action to arrive over the network.
        peer.commit(net.total);
        assert_eq!(peer.committed_round, 1);
        assert!(peer.wait_ticks > 0);
    }

    #[test]
    fn a_clean_network_converges_to_the_single_player_tape() {
        let mut net = NetMatch::authored(1, NetworkProfile::Clean);
        net.run_until_synchronized(20_000);
        assert!(net.synchronized());
        assert!(net.peers_agree());
        assert!(net.matches_reference());
    }

    #[test]
    fn hostile_loss_delay_duplication_and_reordering_still_converge() {
        let mut net = NetMatch::authored(2, NetworkProfile::Hostile);
        net.run_until_synchronized(50_000);
        assert!(
            net.synchronized(),
            "reliable lockstep must carry the match to convergence"
        );
        // The adversity was real and was overcome.
        assert!(net.network.dropped > 0);
        assert!(net.network.duplicated > 0);
        assert!(net.total_resent() > 0);
        assert!(net.total_waits() > 0);
        assert!(net.peers.iter().any(|peer| peer.duplicate_actions > 0));
    }

    #[test]
    fn both_peers_reconstruct_the_identical_match_maze_and_pose() {
        let mut net = NetMatch::authored(3, NetworkProfile::Hostile);
        net.run_until_synchronized(50_000);
        assert!(net.both_finished());
        // Identical first-person pose and rendered maze at every round.
        for (a, b) in net.peers[0].snapshots.iter().zip(&net.peers[1].snapshots) {
            assert_eq!(a.body_position, b.body_position);
            assert_eq!(a.body_yaw, b.body_yaw);
            assert_eq!(a.maze_tiles, b.maze_tiles);
            assert_eq!(a.elevation_steps, b.elevation_steps);
            assert_eq!(a.safe_tiles, b.safe_tiles);
            assert_eq!(a.trap_tiles, b.trap_tiles);
            assert_eq!(a.rendered_routes, b.rendered_routes);
        }
        assert!(
            net.peers[0]
                .snapshots
                .iter()
                .any(|snapshot| snapshot.body_position.y > 1.4),
            "the replicated match reaches the elevated room bands"
        );
    }

    #[test]
    fn the_networked_match_resolves_to_the_competitive_result() {
        let mut net = NetMatch::authored(1, NetworkProfile::Hostile);
        net.run_until_synchronized(50_000);
        let competitive = &net.peers[0].match_state.competitive;
        assert!(competitive.finished);
        assert_eq!(competitive.winner, Some(LOCAL_TEAM));
        assert_eq!(
            competitive.escaped_count() + competitive.absorbed_count(),
            TEAM_COUNT
        );
    }

    #[test]
    fn the_transport_does_not_change_the_outcome() {
        // A clean and a hostile network both land on the same final match state.
        let mut clean = NetMatch::authored(5, NetworkProfile::Clean);
        clean.run_until_synchronized(50_000);
        let mut hostile = NetMatch::authored(5, NetworkProfile::Hostile);
        hostile.run_until_synchronized(50_000);
        assert_eq!(clean.peers[0].snapshots, hostile.peers[0].snapshots);
    }

    #[test]
    fn reset_restores_a_fresh_session() {
        let mut net = NetMatch::authored(1, NetworkProfile::Hostile);
        net.run_until_synchronized(50_000);
        net.reset(NetworkProfile::Clean);
        assert_eq!(net.transport_ticks, 0);
        assert_eq!(net.network.profile, NetworkProfile::Clean);
        assert!(net.peers.iter().all(|peer| peer.committed_round == 0));
        assert!(!net.synchronized());
    }

    #[test]
    fn live_host_play_replicates_to_the_remote_over_a_hostile_network() {
        let mut live = LiveNetMatch::new(1, NetworkProfile::Hostile);
        live.run_to_completion_headless(50_000);
        assert!(live.synchronized());
        assert!(live.finished());
        // The remote rebuilt the host's match exactly from the network alone.
        assert_eq!(live.host.snapshots, live.remote.snapshots);
        assert_eq!(live.host.match_state.competitive.winner, Some(LOCAL_TEAM));
    }

    #[test]
    fn the_remote_replica_tracks_each_resolved_round() {
        let mut live = LiveNetMatch::new(3, NetworkProfile::Hostile);
        while !live.finished() {
            let action = if live.local_active() {
                LocalAction::Advance
            } else {
                LocalAction::Wait
            };
            assert!(live.force_round(action));
            for _ in 0..5_000 {
                if live.in_sync() {
                    break;
                }
                live.pump();
            }
            assert!(live.in_sync(), "remote replicates round {}", live.resolved);
            assert_eq!(live.remote.committed_round, live.resolved);
        }
        assert_eq!(live.host.match_state.competitive.escaped_count(), 2);
    }

    #[test]
    fn a_seize_replicates_over_the_network_without_panicking() {
        // Regression: a committed Seize must apply on the remote replica (whose body
        // is canonical, not the host's live position) instead of being rejected and
        // tripping the "committed action must apply" assertion.
        use observed_match::hybrid::CONTROL_ROOM;
        let mut live = LiveNetMatch::new(10, NetworkProfile::Hostile);
        let mut guard = 0;
        while live.host_match().local_room() != CONTROL_ROOM && guard < 64 {
            assert!(live.force_round(LocalAction::Advance));
            guard += 1;
        }
        assert_eq!(live.host_match().local_room(), CONTROL_ROOM);
        assert!(live.force_round(LocalAction::Seize));
        for _ in 0..5_000 {
            if live.in_sync() {
                break;
            }
            live.pump();
        }
        assert!(live.in_sync(), "the seize replicates to the remote exactly");
        assert_eq!(
            live.host_match().competitive.control_holder,
            Some(LOCAL_TEAM)
        );
    }

    #[test]
    fn live_net_reset_restores_a_fresh_session() {
        let mut live = LiveNetMatch::new(1, NetworkProfile::Hostile);
        live.run_to_completion_headless(50_000);
        live.reset(NetworkProfile::Clean);
        assert_eq!(live.resolved, 0);
        assert_eq!(live.transport_ticks, 0);
        assert_eq!(live.network.profile, NetworkProfile::Clean);
        assert!(!live.finished());
    }
}
