//! Deterministic per-tick digest of the hex match. The digest folds every
//! player's logical cell and quantized world pose so headless and interactive
//! runs can be proven bit-identical.

use observed_core::{EquipmentId, PlayerId};
use observed_hex::HexCoord;

use super::{
    HEX_INPUT_VERSION, HexGuardianStatus, HexMapDiscovery, HexMatchStatus, HexThresholdKey,
    HexWfcMatch,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexPlayerSnapshot {
    pub id: PlayerId,
    pub cell: HexCoord,
    /// World position quantized to millimetres for exact cross-run comparison.
    pub millimeters: [i32; 3],
    pub climbing: bool,
    pub escaped: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexMatchSnapshot {
    pub input_version: u16,
    pub simulation_content_hash: [u8; 32],
    pub tick: u64,
    pub generation: u32,
    pub players: Vec<HexPlayerSnapshot>,
    pub lanterns: Vec<(PlayerId, u16)>,
    pub deployed: Vec<(EquipmentId, PlayerId, HexThresholdKey, HexCoord)>,
    pub guardian: (HexCoord, HexGuardianStatus),
    pub map_cells: Vec<(PlayerId, HexCoord, HexMapDiscovery, u32, u16, bool)>,
    pub status: HexMatchStatus,
    pub digest: u64,
}

impl HexWfcMatch {
    #[must_use]
    pub fn snapshot(&self) -> HexMatchSnapshot {
        let players = self
            .players
            .values()
            .map(|player| HexPlayerSnapshot {
                id: player.id,
                cell: player.cell,
                millimeters: [
                    (player.position.x * 1_000.0).round() as i32,
                    (player.position.y * 1_000.0).round() as i32,
                    (player.position.z * 1_000.0).round() as i32,
                ],
                climbing: player.climb_target.is_some() || player.transit_target.is_some(),
                escaped: player.escaped,
            })
            .collect::<Vec<_>>();
        let mut snapshot = HexMatchSnapshot {
            input_version: HEX_INPUT_VERSION,
            simulation_content_hash: self.simulation_content_hash,
            tick: self.tick,
            generation: self.facility.generation,
            players,
            lanterns: self
                .lanterns
                .carried
                .iter()
                .map(|(&player, &count)| (player, count))
                .collect(),
            deployed: self
                .lanterns
                .deployed
                .values()
                .map(|lantern| (lantern.id, lantern.owner, lantern.threshold, lantern.cell))
                .collect(),
            guardian: (self.guardian.cell, self.guardian.status),
            map_cells: self
                .map_knowledge
                .iter()
                .flat_map(|(&player, knowledge)| {
                    knowledge.cells.iter().map(move |(&cell, known)| {
                        (
                            player,
                            cell,
                            known.discovery,
                            known.last_confirmed_revision,
                            known.known_ports.0,
                            known.anchored,
                        )
                    })
                })
                .collect(),
            status: self.status,
            digest: 0,
        };
        snapshot.digest = snapshot_digest(&snapshot);
        snapshot
    }
}

fn snapshot_digest(snapshot: &HexMatchSnapshot) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x100_0000_01B3);
    };
    mix(snapshot.tick);
    mix(u64::from(snapshot.input_version));
    for chunk in snapshot.simulation_content_hash.chunks_exact(8) {
        mix(u64::from_le_bytes(
            chunk.try_into().expect("eight-byte hash chunk"),
        ));
    }
    mix(u64::from(snapshot.generation));
    for player in &snapshot.players {
        mix(u64::from(player.id.0));
        mix(u64::from(player.cell.q)
            | (u64::from(player.cell.r) << 16)
            | (u64::from(player.cell.level) << 32));
        for axis in player.millimeters {
            mix(axis as u64);
        }
        mix(u64::from(player.climbing));
        mix(u64::from(player.escaped));
    }
    for (player, count) in &snapshot.lanterns {
        mix(u64::from(player.0));
        mix(u64::from(*count));
    }
    for (id, owner, threshold, cell) in &snapshot.deployed {
        mix(u64::from(id.0));
        mix(u64::from(owner.0));
        mix(threshold.room_generation_key);
        for byte in threshold.port.bytes() {
            mix(u64::from(byte));
        }
        mix(pack_cell(*cell));
    }
    mix(pack_cell(snapshot.guardian.0));
    mix(match snapshot.guardian.1 {
        HexGuardianStatus::Active => 0,
        HexGuardianStatus::FrozenByPlayer => 1,
        HexGuardianStatus::FrozenByAnchor => 2,
    });
    for (player, cell, discovery, revision, ports, anchored) in &snapshot.map_cells {
        mix(u64::from(player.0));
        mix(pack_cell(*cell));
        mix(match discovery {
            HexMapDiscovery::Glimpsed => 0,
            HexMapDiscovery::Traversed => 1,
        });
        mix(u64::from(*revision));
        mix(u64::from(*ports));
        mix(u64::from(*anchored));
    }
    hash
}

fn pack_cell(cell: HexCoord) -> u64 {
    u64::from(cell.q) | (u64::from(cell.r) << 16) | (u64::from(cell.level) << 32)
}
