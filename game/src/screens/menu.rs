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
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, summary_panel, text};

use crate::view::components::UiAssets;

#[derive(Default)]
pub(crate) struct GamepadMenuAxis {
    direction: i8,
}

#[derive(Component)]
pub(crate) struct ResultsSummaryText;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResultsStory {
    pub(crate) headline: String,
    pub(crate) lines: Vec<String>,
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
                p.spawn(menu_button(
                    2,
                    MenuAction::Goto(GameState::Settings),
                    "Settings",
                ));
                p.spawn(menu_button(3, MenuAction::QuitApp, "Quit"));
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
        #[cfg(not(test))]
        crate::flow::save_profile(&career);
    }

    let result = career.last_result.clone();
    let tape = tape.as_deref();
    let unlocked: Vec<String> = career
        .last_unlocks
        .iter()
        .filter_map(|id| cosmetic(*id))
        .map(|c| c.name.to_string())
        .collect();
    let story = build_results_story(result.as_ref(), tape, !career.bot_rival_teams);

    commands
        .spawn(screen_root(GameState::Results))
        .with_children(|root| {
            root.spawn(text(story.headline, 48.0, TITLE));
            root.spawn(summary_panel()).with_children(|p| {
                p.spawn((
                    ResultsSummaryText,
                    text(story.lines.join("\n"), 17.0, TITLE),
                ));
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
                p.spawn(menu_button(0, MenuAction::Rematch, "Rematch | new seed"));
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

pub(crate) fn build_results_story(
    result: Option<&crate::flow::MatchResult>,
    tape: Option<&ReplayTape>,
    solo: bool,
) -> ResultsStory {
    let headline = result_headline(result).to_string();
    let outcome = match result {
        Some(result) if solo && result.local_won => "Solo traversal complete.".to_string(),
        Some(_) if solo => "The solo traversal ended in the facility.".to_string(),
        Some(result) if result.local_won => {
            format!(
                "You finished {}; your team won the series.",
                placement_label(result.placement)
            )
        }
        Some(result) if result.placement.is_some() => format!(
            "You finished {}; {} won the series.",
            placement_label(result.placement),
            winner_label(result.winner)
        ),
        Some(result) => format!(
            "The facility absorbed your team; {} survived.",
            winner_label(result.winner)
        ),
        None => "No completed run was recorded.".to_string(),
    };

    let escape_line = if solo {
        "One explorer entered; no rival teams joined the run.".to_string()
    } else {
        let order = tape
            .map(|tape| {
                tape.escape_order
                    .iter()
                    .map(|team| team_story_label(*team))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if order.is_empty() {
            result
                .and_then(|result| result.winner)
                .map(|winner| format!("Escape order: {} crossed first.", team_story_label(winner)))
                .unwrap_or_else(|| "Escape order: no team escaped.".to_string())
        } else {
            format!("Escape order: {}.", order.join(" -> "))
        }
    };

    let collapsed = tape
        .map(|tape| tape.collapsed_rooms.as_slice())
        .unwrap_or_default();
    let absorbed = result.map_or(0, |result| result.absorbed);
    let collapse_line = if collapsed.is_empty() {
        format!(
            "The collapse sealed no recorded rooms; {}.",
            team_count_phrase(absorbed, "team was absorbed", "teams were absorbed")
        )
    } else {
        format!(
            "The collapse sealed {}: {}; {}.",
            room_count_phrase(collapsed.len()),
            room_list(collapsed),
            team_count_phrase(absorbed, "team was absorbed", "teams were absorbed")
        )
    };

    let visited = tape
        .map(|tape| tape.visited_rooms.as_slice())
        .unwrap_or_default();
    let path_line = if visited.is_empty() {
        "Your path left no room trace.".to_string()
    } else {
        format!(
            "Your path crossed {}: {}.",
            room_count_phrase(visited.len()),
            room_list(visited)
        )
    };

    let (held, required, anchor_uses) = tape
        .map(|tape| {
            (
                tape.keystones_collected,
                tape.keystones_required,
                tape.anchor_uses,
            )
        })
        .unwrap_or((0, 0, 0));
    let key_line = if required > 0 && held >= required {
        format!("You recovered {held}/{required} keystones and opened the exit.")
    } else {
        format!("You recovered {held}/{required} keystones before the run ended.")
    };
    let anchor_line = format!(
        "You deployed the anchor {}.",
        match anchor_uses {
            0 => "zero times".to_string(),
            1 => "once".to_string(),
            uses => format!("{uses} times"),
        }
    );
    let run_line = tape
        .map(|tape| {
            format!(
                "Run {} | {} | {} replay moments.",
                tape.seed,
                tape.map_name,
                tape.samples.len()
            )
        })
        .unwrap_or_else(|| "No replay tape is available for this run.".to_string());

    ResultsStory {
        headline,
        lines: vec![
            outcome,
            escape_line,
            collapse_line,
            path_line,
            key_line,
            anchor_line,
            run_line,
        ],
    }
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
        Some(3) => "3rd".to_string(),
        Some(n) => format!("{n}th"),
        None => "--".to_string(),
    }
}

fn winner_label(winner: Option<observed_core::TeamId>) -> String {
    winner
        .map(team_story_label)
        .unwrap_or_else(|| "no team".to_string())
}

fn team_story_label(team: observed_core::TeamId) -> String {
    if team == crate::flow::LOCAL_TEAM {
        "You".to_string()
    } else {
        team.label()
    }
}

fn room_count_phrase(count: usize) -> String {
    if count == 1 {
        "1 room".to_string()
    } else {
        format!("{count} rooms")
    }
}

fn room_list(rooms: &[observed_core::RoomId]) -> String {
    const SHOWN: usize = 6;
    let mut labels: Vec<String> = rooms
        .iter()
        .take(SHOWN)
        .map(|room| format!("R{}", room.0))
        .collect();
    if rooms.len() > SHOWN {
        labels.push(format!("+{} more", rooms.len() - SHOWN));
    }
    labels.join(" -> ")
}

fn team_count_phrase(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
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
            None,
            &ui_assets.hover,
            crate::view::components::MatchAudioCue::UiHover,
            &settings,
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
    active_seed: Option<Res<crate::flow::ActiveMatchSeed>>,
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
        None,
        &ui_assets.click,
        crate::view::components::MatchAudioCue::UiClick,
        &settings,
    );
    match button.action {
        MenuAction::Goto(state) => next.set(state),
        MenuAction::StartRun => {
            let seed = crate::flow::launch_seed();
            info!("MATCH_START mode=full_wfc seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.remove_resource::<SpectatorBot>();
            next.set(GameState::FullWfc);
        }
        MenuAction::Rematch => {
            let previous = active_seed
                .as_deref()
                .map_or(crate::flow::MATCH_SEED, |seed| seed.0);
            let seed = crate::flow::rematch_seed(previous);
            info!("MATCH_START mode=rematch seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.remove_resource::<SpectatorBot>();
            next.set(GameState::FullWfc);
        }
        MenuAction::Launch => {
            let seed = crate::flow::launch_seed();
            info!("MATCH_START mode=play seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.remove_resource::<SpectatorBot>();
            next.set(GameState::Match);
        }
        MenuAction::Spectate => {
            let seed = crate::flow::launch_seed();
            info!("MATCH_START mode=spectate seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.insert_resource(SpectatorBot::for_seed(seed));
            next.set(GameState::Match);
        }
        MenuAction::Equip(id) => {
            if career.profile.equip(id) {
                crate::flow::save_profile(&career);
            }
        }
        MenuAction::ToggleRivalTeams => {
            career.bot_rival_teams = !career.bot_rival_teams;
            crate::flow::save_profile(&career);
        }
        MenuAction::ToggleAiTeammates => {
            career.bot_ai_teammates = !career.bot_ai_teammates;
            crate::flow::save_profile(&career);
        }
        MenuAction::ToggleGuardian => {
            career.bot_guardian = !career.bot_guardian;
            crate::flow::save_profile(&career);
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
