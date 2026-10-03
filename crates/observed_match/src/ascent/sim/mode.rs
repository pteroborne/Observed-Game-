//! Small, deterministic scenario presets for the Architect proof.

use observed_facility::hex_wfc::HexWfcConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchitectMode {
    Pocket,
    QuickClimb,
    FullAscent,
    DeepStack,
}

impl ArchitectMode {
    pub const ALL: [Self; 4] = [
        Self::Pocket,
        Self::QuickClimb,
        Self::FullAscent,
        Self::DeepStack,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pocket => "Pocket Pursuit",
            Self::QuickClimb => "Quick Climb",
            Self::FullAscent => "Full Ascent",
            Self::DeepStack => "Deep Stack",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Pocket => "POCKET",
            Self::QuickClimb => "QUICK CLIMB",
            Self::FullAscent => "FULL ASCENT",
            Self::DeepStack => "DEEP STACK",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Pocket => "1 FLOOR / 30 CELLS",
            Self::QuickClimb => "2 FLOORS / 96 CELLS",
            Self::FullAscent => "2 FLOORS / 160 CELLS",
            Self::DeepStack => "5 FLOORS / 240 CELLS",
        }
    }

    #[must_use]
    pub const fn seed(self) -> u64 {
        match self {
            // Re-chosen for the climb compositions, by two measured tests. The route offers
            // the scenario all its gaps (`wanted` in `generate_with_team_size`): a gap must
            // be a cell some card rebuilds exactly, and since routed halls carry a branch
            // many routes have too few - Pocket's old 19 had none. And the bot match is
            // not decided in its opening beats: with the second Observer two thirds along
            // the route, many seeds start it a short walk from the exit, and Quick Climb's
            // and Full Ascent's first full-gap seeds ended in 6 and 9 beats. These play
            // 233, 336 and 413 beats. Pocket's is a sprint by design.
            Self::Pocket => 20,
            Self::QuickClimb => 19,
            Self::FullAscent => 21,
            Self::DeepStack => 27,
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
            // Deep Stack exists to make vertical rules observable. A fall can only
            // land on lower surviving structure if there IS lower structure, and a
            // one- or two-level facility cannot express that — see backlog #42,
            // where every fall in the shallow modes corrupts by construction.
            Self::DeepStack => HexWfcConfig {
                cols: 8,
                rows: 6,
                levels: 5,
                min_rooms: 4,
                max_rooms: 7,
                retry_budget: 200,
                min_room_distance: 1,
            },
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Pocket => Self::QuickClimb,
            Self::QuickClimb => Self::FullAscent,
            Self::FullAscent => Self::DeepStack,
            Self::DeepStack => Self::Pocket,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Pocket => Self::DeepStack,
            Self::QuickClimb => Self::Pocket,
            Self::FullAscent => Self::QuickClimb,
            Self::DeepStack => Self::FullAscent,
        }
    }
}
