//! The studio's type scale.
//!
//! Everything in this tool used to be one monospace size, which is most of why
//! a panel reads as a wall: a heading, a value, a hint and a warning all arrive
//! with identical weight, so the eye has nothing to sort them by.
//!
//! `feathers` embeds Fira Sans and Fira Mono, so the fonts cost nothing to ship
//! and need no asset directory.
//!
//! Mono is not the fallback here, it is a decision. Data whose columns line up -
//! signatures, counts, hex seeds - stays monospaced because the alignment *is*
//! the information. Prose, labels and headings go proportional. Panels that
//! still align their columns with padding spaces therefore stay on [`Role::Data`]
//! until they are rebuilt as real rows.

use bevy::feathers::constants::fonts;
use bevy::prelude::*;
use bevy::text::FontWeight;
use observed_ui::theme::{ChromeRole, chrome};

/// What a piece of text is for, rather than what it looks like.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Panel and section titles.
    Heading,
    /// A section title one level down.
    Subheading,
    /// Ordinary prose and labels.
    Body,
    /// Secondary text: units, hints, inactive tabs, explanatory lines.
    Dim,
    /// Column-aligned data. Monospaced on purpose.
    Data,
    /// A tab that is not the active one.
    TabIdle,
    /// The active tab.
    TabActive,
}

impl Role {
    fn font_path(self) -> &'static str {
        match self {
            Role::Heading | Role::Subheading | Role::TabActive => fonts::BOLD,
            Role::Body | Role::Dim | Role::TabIdle => fonts::REGULAR,
            Role::Data => fonts::MONO,
        }
    }

    fn size(self) -> f32 {
        match self {
            Role::Heading => 15.0,
            Role::Subheading => 13.0,
            Role::Body | Role::Dim => 13.0,
            Role::Data => 12.5,
            Role::TabIdle | Role::TabActive => 12.0,
        }
    }

    fn weight(self) -> FontWeight {
        match self {
            Role::Heading | Role::Subheading | Role::TabActive => FontWeight::BOLD,
            _ => FontWeight::NORMAL,
        }
    }

    /// Colour is a chrome decision, so it comes from `observed_ui`.
    pub fn colour(self) -> Color {
        match self {
            Role::Heading | Role::Subheading | Role::Body | Role::TabActive => {
                chrome(ChromeRole::TextMain)
            }
            Role::Dim | Role::TabIdle => chrome(ChromeRole::TextDim),
            Role::Data => chrome(ChromeRole::TextMain),
        }
    }
}

/// The `TextFont` for a role. Pair with [`Role::colour`].
pub fn font(assets: &AssetServer, role: Role) -> TextFont {
    TextFont {
        font: assets.load(role.font_path()).into(),
        font_size: FontSize::Px(role.size()),
        weight: role.weight(),
        ..default()
    }
}
