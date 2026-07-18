//! Deterministic per-tick digest of the hex match. The digest folds every
//! player's logical cell and quantized world pose so headless and interactive
//! runs can be proven bit-identical.

use observed_core::PlayerId;
use observed_hex::HexCoord;

use super::{HEX_INPUT_VERSION, HexMatchStatus, HexWfcMatch};

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
    pub tick: u64,
    pub generation: u32,
    pub players: Vec<HexPlayerSnapshot>,
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
            tick: self.tick,
            generation: self.facility.generation,
            players,
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
    hash
}
