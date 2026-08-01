//! Which floors the viewport draws.
//!
//! Split from `lib.rs` for the 600-line review budget.

/// Which levels are drawn. The cycle runs `0, 1, … N-1, All` and wraps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layer {
    Single(u8),
    #[default]
    All,
}

impl Layer {
    /// The single level being drawn, or `None` for all of them.
    #[must_use]
    pub fn level(self) -> Option<u8> {
        match self {
            Self::Single(level) => Some(level),
            Self::All => None,
        }
    }

    /// Whether `level` is drawn at all.
    ///
    /// A single-floor view still draws the floor above and below, dimmed, so
    /// you can see how they relate — a plan with no context above or below
    /// tells you nothing about vertical connection. Everything further away is
    /// dropped, which is what keeps a ten-level facility from stacking into an
    /// unreadable thicket.
    #[must_use]
    pub fn draws(self, level: u8) -> bool {
        match self {
            Self::All => true,
            Self::Single(focus) => level.abs_diff(focus) <= 1,
        }
    }

    /// Whether `level` is the floor under inspection, as opposed to context.
    /// Only the focus floor gets solid detail and full-strength line work.
    #[must_use]
    pub fn is_focus(self, level: u8) -> bool {
        match self {
            Self::All => true,
            Self::Single(focus) => level == focus,
        }
    }

    #[must_use]
    pub fn next(self, levels: u8) -> Self {
        match self {
            Self::Single(level) if level + 1 < levels => Self::Single(level + 1),
            Self::Single(_) => Self::All,
            Self::All => Self::Single(0),
        }
    }

    #[must_use]
    pub fn previous(self, levels: u8) -> Self {
        match self {
            Self::Single(0) => Self::All,
            Self::Single(level) => Self::Single(level - 1),
            Self::All => Self::Single(levels.saturating_sub(1)),
        }
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Single(level) => format!("layer {level}"),
            Self::All => "all layers".to_string(),
        }
    }
}
