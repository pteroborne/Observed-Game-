//! Small deterministic helpers shared by cards, commands, and role trees.

use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, HexFace};

use super::{ArchitectCommand, ThresholdKey};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Prng(pub(super) u64);

impl Prng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub(super) fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }
}

pub(super) fn lateral_face(rotation: u8) -> HexFace {
    HexFace::LATERAL[(rotation % 6) as usize]
}

pub(super) fn face_between(config: HexWfcConfig, from: HexCoord, to: HexCoord) -> Option<HexFace> {
    HexFace::ALL
        .into_iter()
        .find(|&face| config.grid().neighbor(from, face) == Some(to))
}

pub(super) fn face_toward(world: &HexWfcWorld, from: HexCoord, to: HexCoord) -> Option<HexFace> {
    world
        .route_between(from, to)
        .and_then(|path| path.get(1).copied())
        .and_then(|next| face_between(world.config, from, next))
}

pub(super) fn key_face_from(key: ThresholdKey, cell: HexCoord) -> HexFace {
    if key.cell == cell {
        key.face
    } else {
        key.face.opposite()
    }
}

pub(super) fn threshold_touches(key: ThresholdKey, cell: HexCoord, world: &HexWfcWorld) -> bool {
    key.cell == cell || world.config.grid().neighbor(key.cell, key.face) == Some(cell)
}

pub(super) fn command_key(command: ArchitectCommand) -> (u32, u8, u16, u16, u8) {
    match command {
        ArchitectCommand::Play {
            card,
            target,
            rotation,
        } => (card.0, target.level, target.q, target.r, rotation % 6),
    }
}
