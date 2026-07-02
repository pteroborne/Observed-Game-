use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub(super) fn set_cursor_grab(
    cursors: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    grab: bool,
) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = if grab {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        cursor.visible = !grab;
    }
}

pub(crate) fn grab_match_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    set_cursor_grab(&mut cursors, true);
}

pub(crate) fn release_match_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    set_cursor_grab(&mut cursors, false);
}
