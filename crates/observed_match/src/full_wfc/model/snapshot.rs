use super::{FullWfcMatch, MatchSnapshot, PlayerSnapshot};
use crate::full_wfc::DeployableKind;

impl FullWfcMatch {
    pub fn snapshot(&self) -> MatchSnapshot {
        let players = self
            .players
            .values()
            .map(|player| PlayerSnapshot {
                id: player.id,
                team: player.team,
                cell: player.cell,
                millimeters: [
                    (player.position.x * 1_000.0).round() as i32,
                    (player.position.y * 1_000.0).round() as i32,
                    (player.position.z * 1_000.0).round() as i32,
                ],
                escaped: player.escaped,
            })
            .collect::<Vec<_>>();
        let teams = self
            .teams
            .values()
            .map(|team| {
                (
                    team.id,
                    team.keystones,
                    team.dual_station_complete,
                    team.escaped,
                    team.eliminated,
                )
            })
            .collect::<Vec<_>>();
        let remaining_keystones = self.available_keystones.iter().copied().collect::<Vec<_>>();
        let deployed = self
            .equipment
            .deployed
            .values()
            .map(|item| (item.id, item.team, item.kind, item.cell))
            .collect::<Vec<_>>();
        let mut snapshot = MatchSnapshot {
            input_version: super::FULL_WFC_INPUT_VERSION,
            tick: self.tick,
            generation: self.facility.generation,
            players,
            teams,
            remaining_keystones,
            deployed,
            guardian_cell: self.guardian.cell,
            status: self.status,
            digest: 0,
        };
        snapshot.digest = snapshot_digest(&snapshot);
        snapshot
    }
}

fn snapshot_digest(snapshot: &MatchSnapshot) -> u64 {
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
        mix(u64::from(player.team.0));
        mix(u64::from(player.cell.x)
            | (u64::from(player.cell.z) << 16)
            | (u64::from(player.cell.level) << 32));
        for axis in player.millimeters {
            mix(axis as u64);
        }
        mix(u64::from(player.escaped));
    }
    for (team, keys, station, escaped, eliminated) in &snapshot.teams {
        mix(u64::from(team.0));
        mix(u64::from(*keys));
        mix(u64::from(*station));
        mix(u64::from(*escaped));
        mix(u64::from(*eliminated));
    }
    for room in &snapshot.remaining_keystones {
        mix(u64::from(room.0));
    }
    for (id, team, kind, cell) in &snapshot.deployed {
        mix(u64::from(id.0));
        mix(u64::from(team.0));
        mix(match kind {
            DeployableKind::Anchor => 1,
            DeployableKind::TeleportPad => 2,
        });
        mix(u64::from(cell.x) | (u64::from(cell.z) << 16) | (u64::from(cell.level) << 32));
    }
    mix(u64::from(snapshot.guardian_cell.x));
    hash
}
