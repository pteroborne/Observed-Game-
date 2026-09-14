//! Stable Architect command shape and named authority refusals.

use observed_hex::{HexCoord, HexFace};

use super::CardId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThresholdKey {
    pub cell: HexCoord,
    pub face: HexFace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoorState {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchitectCommand {
    Play {
        card: CardId,
        target: HexCoord,
        rotation: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRefusal {
    MatchFinished,
    Cooldown,
    CardNotInHand,
    UnknownTarget,
    VoidTarget,
    CollapsedFloor,
    NoChange,
    WrongDistrict,
    Observed,
    Occupied,
    Anchored,
    PrisonCore,
    NoLocalAttachment,
    InvalidThreshold,
    DoorAlreadyPresent,
}

impl CommandRefusal {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MatchFinished => "the match has ended",
            Self::Cooldown => "Architect cooldown is active",
            Self::CardNotInHand => "that card is not in hand",
            Self::UnknownTarget => "the Rogue AI has not mapped that cell",
            Self::CollapsedFloor => "this floor has permanently collapsed",
            Self::NoChange => {
                "this would leave the same connections; rotate or choose another tile"
            }
            Self::VoidTarget => "there is no tile at that target",
            Self::WrongDistrict => "the card belongs to the other district",
            Self::Observed => "an Observer is holding that tile in view",
            Self::Occupied => "an actor occupies that tile",
            Self::Anchored => "an anchor or open door holds that tile",
            Self::PrisonCore => "the prison core cannot be rewritten",
            Self::NoLocalAttachment => "the tile does not fit any selected boundary",
            Self::InvalidThreshold => "that face is not an open threshold",
            Self::DoorAlreadyPresent => "a deployable door already owns that threshold",
        }
    }
}
