//! Keyboard-navigable tool menu for the hex tile lab.
//!
//! One modal panel (`F2`), four tabs, arrow-key navigation. While the menu is
//! open it owns the keyboard: lab hotkeys and character movement are gated off
//! so a key never means two things at once.

use bevy::prelude::*;

/// Composition category filter for `[`/`]` cycling and the BROWSE tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterCategory {
    All,
    Chambers,
    Halls,
    Ramps,
    Shafts,
    Blueprints,
}

impl FilterCategory {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Chambers,
        Self::Halls,
        Self::Ramps,
        Self::Shafts,
        Self::Blueprints,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Everything",
            Self::Chambers => "Chambers (grounded sanctuary)",
            Self::Halls => "Halls & Junctions",
            Self::Ramps => "Ramps",
            Self::Shafts => "Grounded Ramp Towers",
            Self::Blueprints => "Room Blueprints",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuTab {
    Browse,
    Registers,
    Render,
    Actions,
}

impl MenuTab {
    pub const ALL: [Self; 4] = [Self::Browse, Self::Registers, Self::Render, Self::Actions];

    pub fn label(self) -> &'static str {
        match self {
            Self::Browse => "BROWSE",
            Self::Registers => "REGISTERS",
            Self::Render => "RENDER",
            Self::Actions => "ACTIONS",
        }
    }
}

#[derive(Resource)]
pub struct LabMenuState {
    pub is_open: bool,
    pub active_tab: usize,
    pub selected_item: usize,
    pub active_filter: FilterCategory,
}

impl Default for LabMenuState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: 0,
            selected_item: 0,
            active_filter: FilterCategory::All,
        }
    }
}

impl LabMenuState {
    pub fn tab(&self) -> MenuTab {
        MenuTab::ALL[self.active_tab % MenuTab::ALL.len()]
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % MenuTab::ALL.len();
        self.selected_item = 0;
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = (self.active_tab + MenuTab::ALL.len() - 1) % MenuTab::ALL.len();
        self.selected_item = 0;
    }
}
