//! Post-match results composition and its pure seven-line run story.
//!
//! The story builder has no Bevy dependency beyond its input domain types. The
//! shared widget layer owns focus, pointer/controller parity, accessibility, and
//! the unavailable-replay state; this module owns only results actions.

use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Activate};
use observed_progression::progression::cosmetic;

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled};
use crate::GameState;
use crate::flow::Career;
use crate::hex_wfc::{
    launch::{HexLaunchSpec, HexSeedPolicy},
    loading::HexLaunchRequestSequence,
};
use crate::lan::LanRuntime;
use crate::play_setup::{ActivePlayKind, ActivePlaySession, LaunchContext, PlaySetupDraft};
use crate::sim::replay::ReplayTape;
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, summary_panel, text};

const SCOPE: FocusScopeId = FocusScopeId("results");
const REMATCH: WidgetId = WidgetId::named("results.rematch");
const WATCH_REPLAY: WidgetId = WidgetId::named("results.watch_replay");
const RETURN: WidgetId = WidgetId::named("results.return");

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultsAction {
    Rematch,
    WatchReplay,
    Return,
}

#[derive(Component)]
pub(crate) struct ResultsSummaryText;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResultsStory {
    pub(crate) headline: String,
    pub(crate) lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultsPerspective {
    Solo,
    Participant,
    Spectator,
}

impl From<ActivePlayKind> for ResultsPerspective {
    fn from(kind: ActivePlayKind) -> Self {
        match kind {
            ActivePlayKind::Solo => Self::Solo,
            ActivePlayKind::CoOp | ActivePlayKind::TeamRace => Self::Participant,
            ActivePlayKind::Spectate => Self::Spectator,
        }
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    mut career: ResMut<Career>,
    tape: Option<Res<ReplayTape>>,
    lan: Res<LanRuntime>,
    active_session: Option<Res<ActivePlaySession>>,
) {
    if career.award() {
        #[cfg(not(test))]
        crate::flow::save_profile(&career);
    }

    let result = career.last_result.clone();
    let tape = tape.as_deref();
    let replay_available = tape.is_some();
    let unlocked = career
        .last_unlocks
        .iter()
        .filter_map(|id| cosmetic(*id))
        .map(|item| item.name.to_string())
        .collect::<Vec<_>>();
    // Deprecated Match tests can still enter Results without a canonical launch.
    // Every player-facing HexWfc route supplies `ActivePlaySession`, so its copy is
    // based on the finalized roster rather than old career/profile bot flags.
    let active_session = active_session.as_deref().copied();
    let perspective = active_session.map_or_else(
        || {
            if career.bot_rival_teams {
                ResultsPerspective::Participant
            } else {
                ResultsPerspective::Solo
            }
        },
        |session| session.kind.into(),
    );
    let story = build_results_story(result.as_ref(), tape, perspective);
    let returning_to_lobby = lan.client.is_some();

    commands
        .spawn(screen_root(GameState::Results))
        .with_children(|root| {
            root.spawn(text(story.headline, 48.0, TITLE));
            if let Some(session) = active_session {
                root.spawn(text(session.summary(), 16.0, DIM));
            }
            root.spawn(summary_panel()).with_children(|summary| {
                summary.spawn((
                    ResultsSummaryText,
                    text(story.lines.join("\n"), 17.0, TITLE),
                ));
                summary.spawn((
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
                    summary.spawn((ResultsSummaryText, text("no new unlocks", 16.0, DIM)));
                } else {
                    summary.spawn((
                        ResultsSummaryText,
                        text(format!("unlocked: {}", unlocked.join(", ")), 16.0, ACCENT),
                    ));
                }
            });
            root.spawn((
                panel(),
                widgets::focus_scope(FocusScope::screen(SCOPE, REMATCH, RETURN)),
            ))
            .with_children(|panel| {
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(
                        REMATCH,
                        SCOPE,
                        0,
                        if returning_to_lobby {
                            "Rematch | return to lobby"
                        } else {
                            "Rematch | new seed"
                        },
                    ),
                    ResultsAction::Rematch,
                );
                let replay_spec = if replay_available {
                    WidgetSpec::enabled(WATCH_REPLAY, SCOPE, 1, "Watch replay")
                } else {
                    WidgetSpec::disabled(WATCH_REPLAY, SCOPE, 1, "Watch replay | unavailable")
                };
                widgets::spawn_button(panel, replay_spec, ResultsAction::WatchReplay);
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(
                        RETURN,
                        SCOPE,
                        2,
                        if returning_to_lobby {
                            "Return to lobby"
                        } else {
                            "Main menu"
                        },
                    ),
                    ResultsAction::Return,
                );
            });
        });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&ResultsAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
    active_seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    tape: Option<Res<ReplayTape>>,
    lan: Res<LanRuntime>,
    setup: Res<PlaySetupDraft>,
    mut sequence: ResMut<HexLaunchRequestSequence>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match action {
        ResultsAction::Rematch if lan.client.is_some() => next.set(GameState::Lobby),
        ResultsAction::Rematch => {
            let Ok(validated) = setup.validate() else {
                return;
            };
            let previous = active_seed
                .as_deref()
                .map_or(crate::flow::MATCH_SEED, |seed| seed.0);
            let seed = crate::flow::rematch_seed(previous);
            info!("MATCH_PREPARE mode=rematch seed={seed}");
            commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
            commands.insert_resource(sequence.issue(
                LaunchContext::Rematch,
                observed_core::PlayerId(0),
                validated.spectator,
                false,
                HexLaunchSpec {
                    requested_seed: seed,
                    config: crate::hex_wfc::sim::runtime_config_for(&setup),
                    seed_policy: HexSeedPolicy::Nearby,
                },
            ));
            next.set(GameState::Loading);
        }
        ResultsAction::WatchReplay if tape.is_some() => next.set(GameState::Replay),
        ResultsAction::WatchReplay => {}
        ResultsAction::Return if lan.client.is_some() => next.set(GameState::Lobby),
        ResultsAction::Return => next.set(GameState::MainMenu),
    }
}

pub(crate) fn build_results_story(
    result: Option<&crate::flow::MatchResult>,
    tape: Option<&ReplayTape>,
    perspective: ResultsPerspective,
) -> ResultsStory {
    let headline = result_headline(result, perspective).to_string();
    let local_team = result.map_or(crate::flow::LOCAL_TEAM, |result| result.local_team);
    let viewer_team = (perspective != ResultsPerspective::Spectator).then_some(local_team);
    let outcome = match result {
        Some(result) if perspective == ResultsPerspective::Solo && result.local_won => {
            "Solo traversal complete.".to_string()
        }
        Some(_) if perspective == ResultsPerspective::Solo => {
            "The solo traversal ended in the facility.".to_string()
        }
        Some(result) if perspective == ResultsPerspective::Spectator => result
            .winner
            .map(|winner| format!("{} escaped first under observation.", winner.label()))
            .unwrap_or_else(|| "No team escaped before the facility closed in.".to_string()),
        Some(result) if result.local_won => format!(
            "You finished {}; your team won the series.",
            placement_label(result.placement)
        ),
        Some(result) if result.placement.is_some() => format!(
            "You finished {}; {} won the series.",
            placement_label(result.placement),
            winner_label(result.winner, viewer_team)
        ),
        Some(result) => format!(
            "The facility absorbed your team; {} survived.",
            winner_label(result.winner, viewer_team)
        ),
        None => "No completed run was recorded.".to_string(),
    };

    let escape_line = if perspective == ResultsPerspective::Solo {
        "One explorer entered; no rival teams joined the run.".to_string()
    } else {
        let order = tape
            .map(|tape| {
                tape.escape_order
                    .iter()
                    .map(|team| team_story_label(*team, viewer_team))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if order.is_empty() {
            result
                .and_then(|result| result.winner)
                .map(|winner| {
                    format!(
                        "Escape order: {} crossed first.",
                        team_story_label(winner, viewer_team)
                    )
                })
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
        if perspective == ResultsPerspective::Spectator {
            "The observed route left no room trace.".to_string()
        } else {
            "Your path left no room trace.".to_string()
        }
    } else if perspective == ResultsPerspective::Spectator {
        format!(
            "The observed route crossed {}: {}.",
            room_count_phrase(visited.len()),
            room_list(visited)
        )
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
    let subject = if perspective == ResultsPerspective::Spectator {
        "The observed team"
    } else {
        "You"
    };
    let key_line = if required > 0 && held >= required {
        format!("{subject} recovered {held}/{required} keystones and opened the exit.")
    } else {
        format!("{subject} recovered {held}/{required} keystones before the run ended.")
    };
    let anchor_line = format!(
        "{subject} deployed the anchor {}.",
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

fn result_headline(
    result: Option<&crate::flow::MatchResult>,
    perspective: ResultsPerspective,
) -> &'static str {
    if perspective == ResultsPerspective::Spectator && result.is_some() {
        return "OBSERVATION COMPLETE";
    }
    match result {
        Some(result) if result.local_won || result.placement == Some(1) => "VICTORY",
        Some(result) if is_non_winning_placement(result.placement) => "PLACED",
        Some(_) => "ABSORBED",
        None => "RESULTS",
    }
}

fn is_non_winning_placement(placement: Option<u8>) -> bool {
    placement.is_some_and(|place| place > 1)
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

fn winner_label(
    winner: Option<observed_core::TeamId>,
    viewer_team: Option<observed_core::TeamId>,
) -> String {
    winner
        .map(|team| team_story_label(team, viewer_team))
        .unwrap_or_else(|| "no team".to_string())
}

fn team_story_label(
    team: observed_core::TeamId,
    viewer_team: Option<observed_core::TeamId>,
) -> String {
    if viewer_team == Some(team) {
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
    let mut labels = rooms
        .iter()
        .take(SHOWN)
        .map(|room| format!("R{}", room.0))
        .collect::<Vec<_>>();
    if rooms.len() > SHOWN {
        labels.push(format!("+{} more", rooms.len() - SHOWN));
    }
    labels.join(" -> ")
}

fn team_count_phrase(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_labels_use_readable_ordinals() {
        assert_eq!(placement_label(Some(1)), "1st");
        assert_eq!(placement_label(Some(2)), "2nd");
        assert_eq!(placement_label(Some(3)), "3rd");
        assert_eq!(placement_label(Some(4)), "4th");
        assert_eq!(placement_label(None), "--");
        assert!(is_non_winning_placement(Some(16)));
        assert!(!is_non_winning_placement(None));
    }

    #[test]
    fn long_room_traces_are_bounded() {
        let rooms = (0..8).map(observed_core::RoomId).collect::<Vec<_>>();
        assert_eq!(
            room_list(&rooms),
            "R0 -> R1 -> R2 -> R3 -> R4 -> R5 -> +2 more"
        );
    }

    #[test]
    fn missing_result_still_builds_the_complete_story_shape() {
        let story = build_results_story(None, None, ResultsPerspective::Participant);
        assert_eq!(story.headline, "RESULTS");
        assert_eq!(story.lines.len(), 7);
        assert!(story.lines[6].contains("No replay tape"));
    }

    #[test]
    fn spectator_story_never_calls_a_bot_team_you() {
        let result = crate::flow::MatchResult {
            local_team: observed_core::TeamId(0),
            placement: Some(1),
            escaped: 1,
            absorbed: 1,
            winner: Some(observed_core::TeamId(0)),
            local_won: true,
        };
        let story = build_results_story(None, None, ResultsPerspective::Spectator);
        assert_eq!(story.headline, "RESULTS");

        let story = build_results_story(Some(&result), None, ResultsPerspective::Spectator);
        assert_eq!(story.headline, "OBSERVATION COMPLETE");
        assert!(story.lines[0].contains("Team 1 escaped first"));
        let joined = story.lines.join("\n");
        assert!(!joined.contains("You"));
        assert!(!joined.contains("your"));
    }
}
