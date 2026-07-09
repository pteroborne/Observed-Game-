//! Splash, main menu, results, and the shared keyboard/controller menu navigation. Every
//! menu-like screen is a column of [`MenuButton`]s driven by the same cursor systems.

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use observed_progression::progression::cosmetic;

use super::input::{gamepad_back_pressed, gamepad_confirm_pressed, gamepad_menu_axis};
use super::{MenuAction, MenuBanner, MenuButton, MenuCursor, SplashTimer, menu_button};
use crate::GameState;
use crate::flow::Career;
use crate::sim::replay::ReplayTape;
use crate::sim::state::SpectatorBot;
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, text};

use crate::view::components::UiAssets;

#[derive(Default)]
pub(crate) struct GamepadMenuAxis {
    direction: i8,
}

#[derive(Component)]
pub(crate) struct ResultsSummaryText;

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
                p.spawn(menu_button(1, MenuAction::SpectateAi, "Spectate AI"));
                p.spawn(menu_button(
                    2,
                    MenuAction::Goto(GameState::Loadout),
                    "Loadout",
                ));
                p.spawn(menu_button(
                    3,
                    MenuAction::Goto(GameState::Settings),
                    "Settings",
                ));
                p.spawn(menu_button(4, MenuAction::QuitApp, "Quit"));
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
    tape: Option<Res<ReplayTape>>,
) {
    cursor.0 = 0;
    if career.award() {
        crate::flow::save_profile(&career.profile);
    }

    let result = career.last_result.clone();
    let tape = tape.as_deref();
    let unlocked: Vec<String> = career
        .last_unlocks
        .iter()
        .filter_map(|id| cosmetic(*id))
        .map(|c| c.name.to_string())
        .collect();
    let headline = result_headline(result.as_ref());

    commands
        .spawn(screen_root(GameState::Results))
        .with_children(|root| {
            root.spawn(text(headline, 48.0, TITLE));
            root.spawn(panel()).with_children(|p| {
                if let Some(r) = &result {
                    p.spawn((
                        ResultsSummaryText,
                        text(
                            format!(
                                "placement {} | {}",
                                placement_label(r.placement),
                                if r.local_won {
                                    "series won"
                                } else {
                                    "series lost"
                                }
                            ),
                            20.0,
                            TITLE,
                        ),
                    ));
                    p.spawn((
                        ResultsSummaryText,
                        text(
                            format!(
                                "winner {} | {} escaped | {} absorbed",
                                r.winner
                                    .map(|team| team.label())
                                    .unwrap_or_else(|| "none".to_string()),
                                r.escaped,
                                r.absorbed
                            ),
                            18.0,
                            DIM,
                        ),
                    ));
                }
                p.spawn((
                    ResultsSummaryText,
                    text(replay_summary_line(tape), 18.0, DIM),
                ));
                for line in replay_story_lines(tape) {
                    p.spawn((ResultsSummaryText, text(line, 15.0, DIM)));
                }
                p.spawn((
                    ResultsSummaryText,
                    text(
                        format!(
                            "Level {} | {} XP | {} matches",
                            career.profile.level(),
                            career.profile.xp,
                            career.profile.matches_played
                        ),
                        18.0,
                        ACCENT,
                    ),
                ));
                if unlocked.is_empty() {
                    p.spawn((ResultsSummaryText, text("no new unlocks", 16.0, DIM)));
                } else {
                    p.spawn((
                        ResultsSummaryText,
                        text(format!("unlocked: {}", unlocked.join(", ")), 16.0, ACCENT),
                    ));
                }
            });
            root.spawn(panel()).with_children(|p| {
                p.spawn(menu_button(0, MenuAction::StartRun, "Play again"));
                p.spawn(menu_button(
                    1,
                    MenuAction::Goto(GameState::Replay),
                    "Watch replay",
                ));
                p.spawn(menu_button(
                    2,
                    MenuAction::Goto(GameState::MainMenu),
                    "Main menu",
                ));
            });
        });
}

fn result_headline(result: Option<&crate::flow::MatchResult>) -> &'static str {
    match result {
        Some(result) if result.local_won || result.placement == Some(1) => "VICTORY",
        Some(result) if is_non_winning_placement(result.placement) => "PLACED",
        Some(_) => "ABSORBED",
        None => "RESULTS",
    }
}

fn is_non_winning_placement(placement: Option<u8>) -> bool {
    placement.is_some_and(|place| usize::from(place) < observed_match::facility::TEAM_COUNT)
}

fn placement_label(placement: Option<u8>) -> String {
    match placement {
        Some(1) => "1st".to_string(),
        Some(2) => "2nd".to_string(),
        Some(n) => format!("{n}th"),
        None => "—".to_string(),
    }
}

fn replay_summary_line(tape: Option<&ReplayTape>) -> String {
    tape.map(|tape| {
        format!(
            "seed {} | {} | {} replay samples",
            tape.seed,
            tape.map_name,
            tape.samples.len()
        )
    })
    .unwrap_or_else(|| "no replay tape available".to_string())
}

fn replay_story_lines(tape: Option<&ReplayTape>) -> Vec<String> {
    let Some(tape) = tape else {
        return vec!["story: no replay events recorded".to_string()];
    };
    let mut lines: Vec<String> = tape
        .markers
        .iter()
        .rev()
        .take(4)
        .map(|marker| format!("r{} {}", marker.live_round, marker.label))
        .collect();
    lines.reverse();
    if lines.is_empty() {
        vec!["story: no replay events recorded".to_string()]
    } else {
        let mut out = vec!["story:".to_string()];
        out.extend(lines);
        out
    }
}

// --- shared menu systems ---------------------------------------------------
#[allow(clippy::too_many_arguments)]
pub(crate) fn menu_navigate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut gamepad_axis: Local<GamepadMenuAxis>,
    mut cursor: ResMut<MenuCursor>,
    buttons: Query<&MenuButton>,
    ui_assets: Res<UiAssets>,
    settings: Res<crate::settings::Settings>,
) {
    let count = buttons.iter().count();
    if count == 0 {
        return;
    }
    let old_val = cursor.0;
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

    if cursor.0 != old_val {
        crate::screens::audio::play_ui_sound(
            &mut commands,
            &ui_assets.hover,
            settings.effective_sfx_volume(),
            crate::view::components::MatchAudioCue::UiHover,
        );
    }
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
    ui_assets: Res<UiAssets>,
    settings: Res<crate::settings::Settings>,
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
    crate::screens::audio::play_ui_sound(
        &mut commands,
        &ui_assets.click,
        settings.effective_sfx_volume(),
        crate::view::components::MatchAudioCue::UiClick,
    );
    match button.action {
        MenuAction::Goto(state) => next.set(state),
        MenuAction::StartRun => {
            commands.remove_resource::<SpectatorBot>();
            next.set(GameState::Lobby);
        }
        MenuAction::SpectateAi => {
            let seed = crate::flow::launch_seed();
            info!("MATCH_START mode=spectate seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.insert_resource(SpectatorBot::for_seed(seed));
            next.set(GameState::Match);
        }
        MenuAction::Launch => {
            let seed = crate::flow::launch_seed();
            info!("MATCH_START mode=play seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.remove_resource::<SpectatorBot>();
            next.set(GameState::Match);
        }
        MenuAction::Equip(id) => {
            if career.profile.equip(id) {
                crate::flow::save_profile(&career.profile);
            }
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
        GameState::Replay => next.set(GameState::Results),
        _ => {}
    }
}
