//! Post-match tactical replay screen.
//!
//! The screen reads only [`ReplayTape`]. It does not reach back into live match
//! resources, so watching a replay cannot change or depend on the completed match.

mod playback;
pub(crate) mod scene;
mod trace;
pub(crate) use playback::{
    ReplayAction, ReplayPlayback, ReplayTimeline, activate, advance_playback, cleanup,
    refresh_controls, refresh_timeline, scrub,
};
pub(crate) use trace::draw_replay_map;
use trace::{focus_line, focused_pose, recent_markers};

use bevy::prelude::*;
use bevy::ui_widgets::{Slider, SliderRange, SliderThumb, SliderValue, TrackClick};

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec};
use crate::GameState;
use crate::sim::replay::{ReplayActorId, ReplayTape};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, screen_root, text};

const MAP_W: f32 = 620.0;
const MAP_H: f32 = 560.0;
const MAP_ROOM: f32 = 34.0;
const MAP_INSET: f32 = 34.0;
const DETAILS_W: f32 = 500.0;
const BODY_GAP: f32 = 18.0;
const CONTROL_COLUMNS: usize = 2;
const CONTROL_GRID_WIDTH: f32 = 456.0;
const CONTROL_GAP: f32 = 8.0;
const CONTROL_W: f32 =
    (CONTROL_GRID_WIDTH - CONTROL_GAP * (CONTROL_COLUMNS as f32 - 1.0)) / CONTROL_COLUMNS as f32;

const SCOPE: FocusScopeId = FocusScopeId("replay");
const PLAY_PAUSE: WidgetId = WidgetId::named("replay.play_pause");
const STEP_BACK: WidgetId = WidgetId::named("replay.step_back");
const STEP_FORWARD: WidgetId = WidgetId::named("replay.step_forward");
const JUMP_BACK: WidgetId = WidgetId::named("replay.jump_back");
const JUMP_FORWARD: WidgetId = WidgetId::named("replay.jump_forward");
const NEXT_ACTOR: WidgetId = WidgetId::named("replay.next_actor");
const BACK: WidgetId = WidgetId::named("replay.back");
const CONTINUE: WidgetId = WidgetId::named("replay.continue");

#[derive(Component)]
pub(crate) struct ReplayInfo;

#[derive(Component)]
pub(crate) struct ReplayMapPanel;

#[derive(Component)]
pub(crate) struct ReplayMapElement;

pub(crate) fn setup_replay(
    mut commands: Commands,
    tape: Option<Res<ReplayTape>>,
    lan: Res<crate::lan::LanRuntime>,
) {
    let focus = tape
        .as_ref()
        .map(|tape| tape.default_focus())
        .unwrap_or(ReplayActorId::LocalPlayer);
    commands.insert_resource(ReplayPlayback { focus, ..default() });

    commands
        .spawn(screen_root(GameState::Replay))
        .with_children(|root| {
            root.spawn(text("REPLAY", 42.0, TITLE));
            if let Some(facts) = tape.as_ref().and_then(|tape| tape.ascent_result) {
                root.spawn(text(
                    format!(
                        "Architect Ascent | {} | {}",
                        super::results::ascent_role_label(facts.role),
                        super::results::ascent_outcome_label(facts)
                    ),
                    16.0,
                    DIM,
                ));
            }
            root.spawn(Node {
                width: px(MAP_W + DETAILS_W + BODY_GAP),
                height: px(MAP_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                column_gap: px(BODY_GAP),
                ..default()
            })
            .with_children(|body| {
                body.spawn((
                    ReplayMapPanel,
                    Node {
                        width: px(MAP_W),
                        height: px(MAP_H),
                        border: UiRect::all(px(1)),
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(BORDER),
                ));
                body.spawn((
                    replay_details_panel(),
                    widgets::focus_scope(FocusScope::grid(SCOPE, PLAY_PAUSE, BACK, 2)),
                ))
                .with_children(|details| {
                    details.spawn((ReplayInfo, text("Replay loading...", 14.0, DIM)));
                    details
                        .spawn(Node {
                            width: px(CONTROL_GRID_WIDTH),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            column_gap: px(CONTROL_GAP),
                            row_gap: px(2),
                            ..default()
                        })
                        .with_children(|controls| {
                            let has_replay = tape.as_ref().is_some_and(|tape| !tape.is_empty());
                            for (order, id, action, label) in [
                                (0, PLAY_PAUSE, ReplayAction::PlayPause, "Pause replay"),
                                (1, STEP_BACK, ReplayAction::StepBack, "Step back"),
                                (2, STEP_FORWARD, ReplayAction::StepForward, "Step forward"),
                                (3, JUMP_BACK, ReplayAction::JumpBack, "Jump back"),
                                (4, JUMP_FORWARD, ReplayAction::JumpForward, "Jump forward"),
                                (5, NEXT_ACTOR, ReplayAction::NextActor, "Focus next actor"),
                                (
                                    6,
                                    WidgetId::named("replay.previous_event"),
                                    ReplayAction::PreviousEvent,
                                    "Previous event",
                                ),
                                (
                                    7,
                                    WidgetId::named("replay.next_event"),
                                    ReplayAction::NextEvent,
                                    "Next event",
                                ),
                                (
                                    8,
                                    WidgetId::named("replay.camera"),
                                    ReplayAction::Camera,
                                    "View: Follow",
                                ),
                                (
                                    9,
                                    WidgetId::named("replay.rotate"),
                                    ReplayAction::Rotate,
                                    "Rotate view",
                                ),
                                (
                                    10,
                                    WidgetId::named("replay.floor"),
                                    ReplayAction::Floor,
                                    "Next floor",
                                ),
                                (
                                    11,
                                    WidgetId::named("replay.speed"),
                                    ReplayAction::Speed,
                                    "Speed: 1x",
                                ),
                            ] {
                                let spec = if has_replay {
                                    WidgetSpec::enabled(id, SCOPE, order, label)
                                } else {
                                    WidgetSpec::disabled(
                                        id,
                                        SCOPE,
                                        order,
                                        format!("{label} | unavailable"),
                                    )
                                };
                                widgets::spawn_button(
                                    controls,
                                    spec.with_size(CONTROL_W, 44.0),
                                    action,
                                );
                            }
                            widgets::spawn_button(
                                controls,
                                WidgetSpec::enabled(BACK, SCOPE, 12, "Back to results")
                                    .with_size(CONTROL_W, 44.0),
                                ReplayAction::Back,
                            );
                            widgets::spawn_button(
                                controls,
                                WidgetSpec::enabled(
                                    CONTINUE,
                                    SCOPE,
                                    13,
                                    if lan.client.is_some() {
                                        "Return to lobby"
                                    } else {
                                        "Main menu"
                                    },
                                )
                                .with_size(CONTROL_W, 44.0),
                                ReplayAction::Continue,
                            );
                        });
                });
            });
            root.spawn(text(
                "Recorded world | eyes = Observers | pyramids = major Guardians | cages = minors",
                14.0,
                DIM,
            ));
            root.spawn((
                ReplayTimeline,
                Slider {
                    track_click: TrackClick::Snap,
                    ..default()
                },
                SliderRange::new(
                    0.0,
                    tape.as_ref()
                        .map_or(0.0, |t| t.len().saturating_sub(1) as f32),
                ),
                SliderValue(0.0),
                Node {
                    width: px(MAP_W + DETAILS_W + BODY_GAP),
                    height: px(22),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
            ))
            .with_children(|track| {
                track.spawn((
                    SliderThumb,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        width: px(16),
                        height: px(22),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                ));
            });
            root.spawn(text("Tall beacon = event | red tile = instability | green beacon = exit | station = recharge | doors: red closed, green open", 14.0, DIM));
            root.spawn(text(
                "Tab / arrows / D-pad move focus | Enter / A / pointer activate",
                14.0,
                DIM,
            ));
        });
}

fn replay_details_panel() -> impl Bundle {
    (
        Node {
            width: px(DETAILS_W),
            height: px(MAP_H),
            padding: UiRect::all(px(18)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(8),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

pub(crate) fn update_replay_info(
    tape: Option<Res<ReplayTape>>,
    playback: Res<ReplayPlayback>,
    mut info: Query<&mut Text, With<ReplayInfo>>,
) {
    let Ok(mut text) = info.single_mut() else {
        return;
    };
    let Some(tape) = tape else {
        **text = "No replay has been recorded yet.".to_string();
        return;
    };
    if tape.is_empty() {
        **text = format!("Seed {} | {} | replay is empty", tape.seed, tape.map_name);
        return;
    }
    let Some(sample) = tape.sample_at(playback.cursor.floor() as usize) else {
        **text = format!("Seed {} | {} | replay is empty", tape.seed, tape.map_name);
        return;
    };
    if let Some((frame, following, fraction)) = scene::frames(&tape, playback.cursor) {
        let seconds = scene::replay_tick(frame, following, fraction) / 60.0;
        let duration = tape
            .scene_frames
            .last()
            .map_or(0.0, |f| f.tick as f64 / 60.0);
        let focus = focused_pose(sample, playback.focus)
            .map(|p| {
                format!(
                    "{} | {}",
                    tape.actors
                        .iter()
                        .find(|a| a.id == p.actor)
                        .map_or_else(|| p.actor.label(), |a| a.label.clone()),
                    p.status
                )
            })
            .unwrap_or_default();
        let recent = recent_markers(&tape, sample.index)
            .into_iter()
            .rev()
            .take(2)
            .map(|m| {
                format!(
                    "{:.1}s {}",
                    f64::from(m.live_round) / 60.0,
                    m.label
                        .split_once(' ')
                        .map_or(m.label.as_str(), |(_, text)| text)
                        .chars()
                        .take(85)
                        .collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let location = frame
            .bodies
            .iter()
            .find(|b| b.actor == playback.focus)
            .map_or_else(
                || "No body".into(),
                |b| match b.place {
                    observed_match::hex_wfc::HexBodyPlace::Prison => "Team prison maze".into(),
                    observed_match::hex_wfc::HexBodyPlace::Void => {
                        "Rogue operator | last facility view".into()
                    }
                    _ => format!(
                        "Floor {}",
                        playback.event_focus.map_or(
                            b.cell.level.saturating_add_signed(playback.floor_offset),
                            |c| c.level
                        ) + 1
                    ),
                },
            );
        **text = format!(
            "{seconds:.1}s / {duration:.1}s | {} | {}x\n{} | {} view | revision {}\n{}\n{}",
            if playback.playing {
                "playing"
            } else {
                "paused"
            },
            playback.speed,
            location,
            playback.view.label(),
            frame.facility.generation,
            focus,
            recent
        );
        return;
    }
    let focus = focused_pose(sample, playback.focus)
        .or_else(|| sample.actors.first())
        .map(|pose| focus_line(pose, &tape))
        .unwrap_or_else(|| "focus unavailable".to_string());
    let markers = recent_markers(tape.as_ref(), sample.index)
        .into_iter()
        .map(|marker| {
            format!(
                "{}{} {}",
                if tape.ascent_result.is_some() {
                    "t"
                } else {
                    "r"
                },
                marker.live_round,
                if tape.ascent_result.is_some() && marker.label.chars().count() > 60 {
                    format!("{}...", marker.label.chars().take(60).collect::<String>())
                } else {
                    marker.label.clone()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if tape.ascent_result.is_some() {
        **text = format!(
            "seed {} | {} | moment {} / {}\n\
             tick {} | facility revision {}\n\
             focus: {}\n\
             Room trace only: card plays, prison layouts\n\
             and facility rewrites are not reconstructed.\n\
             recent physical events:\n{}",
            tape.seed,
            if playback.playing {
                "playing"
            } else {
                "paused"
            },
            sample.index + 1,
            tape.len(),
            sample.live_round,
            sample.series_round,
            focus,
            if markers.is_empty() {
                "  none".to_string()
            } else {
                markers
            }
        );
        // Facts are also rendered in the heading; this branch avoids race vocabulary.
        return;
    }
    let result = tape
        .result
        .as_ref()
        .and_then(|result| result.winner)
        .map(|winner| format!("winner {}", winner.label()))
        .unwrap_or_else(|| "winner pending".to_string());

    **text = format!(
        "seed {} | {} | {} | sample {} / {}\n\
         live round {} | series round {} | {}\n\
         focus: {}\n\
         recent events:\n{}",
        tape.seed,
        tape.map_name,
        if playback.playing {
            "playing"
        } else {
            "paused"
        },
        sample.index,
        tape.len().saturating_sub(1),
        sample.live_round,
        sample.series_round,
        result,
        focus,
        if markers.is_empty() {
            "  none".to_string()
        } else {
            markers
        }
    );
}

#[cfg(test)]
fn replay_body_fits(viewport_width: f32) -> bool {
    MAP_W + DETAILS_W + BODY_GAP <= viewport_width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_and_controls_fit_the_reference_viewport() {
        let actions = [
            ReplayAction::PlayPause,
            ReplayAction::StepBack,
            ReplayAction::StepForward,
            ReplayAction::JumpBack,
            ReplayAction::JumpForward,
            ReplayAction::NextActor,
            ReplayAction::PreviousEvent,
            ReplayAction::NextEvent,
            ReplayAction::Camera,
            ReplayAction::Rotate,
            ReplayAction::Floor,
            ReplayAction::Speed,
            ReplayAction::Back,
            ReplayAction::Continue,
        ];
        let control_rows = actions.len().div_ceil(CONTROL_COLUMNS);

        assert!(replay_body_fits(1_280.0));
        assert_eq!(control_rows, 7);
    }
}
