//! Helpers shared by every scripted evidence driver (captures, tour, bot POV, the
//! visual audit). Brain-staging helpers live on `MatchDirector`
//! (`force_scripted_rounds`, `suppress_reroute_feedback`); this module holds the
//! presentation-side pieces.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

/// Queue a screenshot of the primary window saved to `path`.
pub(super) fn screenshot_to(commands: &mut Commands, path: String) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
