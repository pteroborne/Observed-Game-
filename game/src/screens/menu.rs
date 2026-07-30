//! Timed splash composition. Every interactive screen uses [`super::widgets`].

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

use super::SplashTimer;
use super::input::gamepad_confirm_pressed;
use crate::GameState;
use crate::view::theme::{ACCENT, DIM, TITLE, screen_root, text};

// --- splash ----------------------------------------------------------------
pub(crate) fn setup_splash(mut commands: Commands) {
    commands.insert_resource(SplashTimer(Timer::from_seconds(1.6, TimerMode::Once)));
    commands
        .spawn(screen_root(GameState::Splash))
        .with_children(|root| {
            root.spawn(text("OBSERVED 2", 64.0, TITLE));
            root.spawn(text(
                "a competitive traversal game of a building that changes when unobserved",
                18.0,
                DIM,
            ));
            root.spawn(text("press Enter / A", 18.0, ACCENT));
        });
}

pub(crate) fn splash_advance(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut timer: ResMut<SplashTimer>,
    mut next: ResMut<NextState<GameState>>,
) {
    if timer.0.tick(time.delta()).just_finished()
        || keyboard.just_pressed(KeyCode::Enter)
        || gamepad_confirm_pressed(&gamepads)
    {
        next.set(GameState::MainMenu);
    }
}

pub(crate) fn cleanup_splash(mut commands: Commands) {
    commands.remove_resource::<SplashTimer>();
}
