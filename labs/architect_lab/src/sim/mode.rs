//! Small, deterministic scenario presets for the Architect proof.

use observed_facility::hex_wfc::HexWfcConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchitectMode {
    Pocket,
    QuickClimb,
    FullAscent,
}

impl ArchitectMode {
    pub const ALL: [Self; 3] = [Self::Pocket, Self::QuickClimb, Self::FullAscent];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pocket => "Pocket Pursuit",
            Self::QuickClimb => "Quick Climb",
            Self::FullAscent => "Full Ascent",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Pocket => "POCKET",
            Self::QuickClimb => "QUICK CLIMB",
            Self::FullAscent => "FULL ASCENT",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Pocket => "1 FLOOR / 30 CELLS",
            Self::QuickClimb => "2 FLOORS / 96 CELLS",
            Self::FullAscent => "2 FLOORS / 160 CELLS",
        }
    }

    #[must_use]
    pub const fn seed(self) -> u64 {
        match self {
            Self::Pocket => 19,
            Self::QuickClimb => 11,
            Self::FullAscent => 7,
        }
    }

    #[must_use]
    pub const fn config(self) -> HexWfcConfig {
        match self {
            Self::Pocket => HexWfcConfig {
                cols: 6,
                rows: 5,
                levels: 1,
                min_rooms: 2,
                max_rooms: 3,
                retry_budget: 100,
                min_room_distance: 1,
            },
            Self::QuickClimb => HexWfcConfig {
                cols: 8,
                rows: 6,
                levels: 2,
                min_rooms: 3,
                max_rooms: 5,
                retry_budget: 100,
                min_room_distance: 1,
            },
            Self::FullAscent => HexWfcConfig {
                cols: 10,
                rows: 8,
                levels: 2,
                min_rooms: 5,
                max_rooms: 7,
                retry_budget: 100,
                min_room_distance: 2,
            },
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Pocket => Self::QuickClimb,
            Self::QuickClimb => Self::FullAscent,
            Self::FullAscent => Self::Pocket,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Pocket => Self::FullAscent,
            Self::QuickClimb => Self::Pocket,
            Self::FullAscent => Self::QuickClimb,
        }
    }
}
