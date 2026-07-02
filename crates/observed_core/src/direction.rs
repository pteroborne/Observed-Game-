//! Unified Direction representation (Side / Cardinal).

use glam::Vec2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub fn index(self) -> usize {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::North => "N",
            Self::East => "E",
            Self::South => "S",
            Self::West => "W",
        }
    }

    pub fn vector(self) -> Vec2 {
        match self {
            Self::North => Vec2::Y,
            Self::East => Vec2::X,
            Self::South => Vec2::NEG_Y,
            Self::West => Vec2::NEG_X,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}
