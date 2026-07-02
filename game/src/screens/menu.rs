//! Splash, main menu, results, and the shared keyboard/controller menu navigation. Every
//! menu-like screen is a column of [`MenuButton`]s driven by the same cursor systems.

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use observed_progression::progression::cosmetic;

use super::*;
use crate::GameState;
use crate::flow::Career;

#[derive(Default)]
pub(crate) struct GamepadMenuAxis {
    direction: i8,
}

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

// --- main menu -------------------------------------------------------------
pub(crate) fn setup_main_menu(mut commands: Commands, mut cursor: ResMut<MenuCursor>) {
    cursor.0 = 0;
    commands
        .spawn(screen_root(GameState::MainMenu))
        .with_children(|root| {
            root.spawn(text("OBSERVED 2", 52.0, TITLE));
            root.spawn((MenuBanner, text("", 18.0, ACCENT)));
            root.spawn(panel()).with_children(|p| {
                p.spawn(menu_button(0, MenuAction::StartRun, "Play"));
                p.spawn(menu_button(
                    1,
                    MenuAction::Goto(GameState::Loadout),
                    "Loadout",
                ));
                p.spawn(menu_button(2, MenuAction::QuitApp, "Quit"));
            });
            root.spawn(text(
                "Up/Down or D-pad select | Enter/A confirm | Esc/B back",
                15.0,
                DIM,
            ));
        });
}

pub(crate) fn main_menu_banner(
    career: Res<Career>,
    mut banner: Query<&mut Text, With<MenuBanner>>,
) {
    if let Ok(mut t) = banner.single_mut() {
        **t = format!(
            "Level {} | {} XP | {} matches played",
            career.profile.level(),
            career.profile.xp,
            career.profile.matches_played,
        );
    }
}

// --- results ---------------------------------------------------------------
pub(crate) fn setup_results(
    mut commands: Commands,
    mut career: ResMut<Career>,
    mut cursor: ResMut<MenuCursor>,
) {
    cursor.0 = 0;
    career.award();

    let result = career.last_result.clone();
    let unlocked: Vec<String> = career
        .last_unlocks
        .iter()
        .filter_map(|id| cosmetic(*id))
        .map(|c| c.name.to_string())
        .collect();
    let headline = match result.as_ref().map(|r| r.placement) {
        Some(Some(1)) => "VICTORY",
        Some(Some(_)) => "ESCAPED",
        _ => "ABSORBED",
    };

    commands
        .spawn(screen_root(GameState::Results))
        .with_children(|root| {
            root.spawn(text(headline, 48.0, TITLE));
            root.spawn(panel()).with_children(|p| {
                if let Some(r) = &result {
                    p.spawn(text(
                        format!("placement {}", placement_label(r.placement)),
                        20.0,
                        DIM,
                    ));
                    p.spawn(text(
                        format!("{} escaped | {} absorbed", r.escaped, r.absorbed),
                        18.0,
                        DIM,
                    ));
                }
                p.spawn(text(
                    format!(
                        "Level {} | {} XP | {} matches",
                        career.profile.level(),
                        career.profile.xp,
                        career.profile.matches_played
                    ),
                    18.0,
                    ACCENT,
                ));
                if unlocked.is_empty() {
                    p.spawn(text("no new unlocks", 16.0, DIM));
                } else {
                    p.spawn(text(
                        format!("unlocked: {}", unlocked.join(", ")),
                        16.0,
                        ACCENT,
                    ));
                }
            });
            root.spawn(panel()).with_children(|p| {
                p.spawn(menu_button(
                    0,
                    MenuAction::Goto(GameState::MainMenu),
                    "Continue",
                ));
            });
        });
}

fn placement_label(placement: Option<u8>) -> String {
    match placement {
        Some(1) => "1st".to_string(),
        Some(2) => "2nd".to_string(),
        Some(n) => format!("{n}th"),
        None => "—".to_string(),
    }
}

// --- shared menu systems ---------------------------------------------------
pub(crate) fn menu_navigate(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut gamepad_axis: Local<GamepadMenuAxis>,
    mut cursor: ResMut<MenuCursor>,
    buttons: Query<&MenuButton>,
) {
    let count = buttons.iter().count();
    if count == 0 {
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) || keyboard.just_pressed(KeyCode::KeyS) {
        cursor.0 = (cursor.0 + 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) || keyboard.just_pressed(KeyCode::KeyW) {
        cursor.0 = (cursor.0 + count - 1) % count;
    }
    let direction = gamepad_menu_axis(&gamepads);
    if direction != 0 && direction != gamepad_axis.direction {
        if direction < 0 {
            cursor.0 = (cursor.0 + 1) % count;
        } else {
            cursor.0 = (cursor.0 + count - 1) % count;
        }
    }
    gamepad_axis.direction = direction;
}

pub(crate) fn menu_highlight(
    cursor: Res<MenuCursor>,
    mut buttons: Query<(&MenuButton, &mut TextColor)>,
) {
    for (button, mut color) in &mut buttons {
        color.0 = if button.index == cursor.0 {
            ACCENT
        } else {
            DIM
        };
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn menu_activate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    cursor: Res<MenuCursor>,
    buttons: Query<&MenuButton>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    if !keyboard.just_pressed(KeyCode::Enter)
        && !keyboard.just_pressed(KeyCode::Space)
        && !gamepad_confirm_pressed(&gamepads)
    {
        return;
    }
    let Some(button) = buttons.iter().find(|b| b.index == cursor.0) else {
        return;
    };
    match button.action {
        MenuAction::Goto(state) => next.set(state),
        MenuAction::StartRun => next.set(GameState::Lobby),
        MenuAction::Launch => {
            let random_seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            commands.insert_resource(crate::flow::ActiveMatchSeed(random_seed));
            next.set(GameState::Match);
        }
        MenuAction::Equip(id) => {
            career.profile.equip(id);
        }
        MenuAction::QuitApp => {
            exit.write(bevy::app::AppExit::Success);
        }
    }
}

pub(crate) fn menu_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) && !gamepad_back_pressed(&gamepads) {
        return;
    }
    match state.get() {
        GameState::MainMenu => {
            exit.write(bevy::app::AppExit::Success);
        }
        GameState::Loadout | GameState::Lobby | GameState::Results => next.set(GameState::MainMenu),
        _ => {}
    }
}
