//! Post-match tactical replay screen.
//!
//! The screen reads only [`ReplayTape`]. It does not reach back into live match
//! resources, so watching a replay cannot change or depend on the completed match.

use bevy::prelude::*;

use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::GameState;
use crate::sim::replay::{ReplayActorId, ReplayActorPose, ReplayRoom, ReplaySample, ReplayTape};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TEAM_COLORS, TITLE, screen_root, text};

const MAP_W: f32 = 620.0;
const MAP_H: f32 = 400.0;
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

#[derive(Resource, Clone, Debug)]
pub(crate) struct ReplayPlayback {
    pub cursor: f32,
    pub playing: bool,
    pub speed: f32,
    pub focus: ReplayActorId,
}

impl Default for ReplayPlayback {
    fn default() -> Self {
        Self {
            cursor: 0.0,
            playing: true,
            speed: 12.0,
            focus: ReplayActorId::LocalPlayer,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplayAction {
    PlayPause,
    StepBack,
    StepForward,
    JumpBack,
    JumpForward,
    NextActor,
    Back,
    Continue,
}

pub(crate) fn setup_replay(mut commands: Commands, tape: Option<Res<ReplayTape>>) {
    let focus = tape
        .as_ref()
        .map(|tape| tape.default_focus())
        .unwrap_or(ReplayActorId::LocalPlayer);
    commands.insert_resource(ReplayPlayback { focus, ..default() });

    commands
        .spawn(screen_root(GameState::Replay))
        .with_children(|root| {
            root.spawn(text("REPLAY", 42.0, TITLE));
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
                                WidgetSpec::enabled(BACK, SCOPE, 6, "Back to results")
                                    .with_size(CONTROL_W, 44.0),
                                ReplayAction::Back,
                            );
                            widgets::spawn_button(
                                controls,
                                WidgetSpec::enabled(CONTINUE, SCOPE, 7, "Continue to menu")
                                    .with_size(CONTROL_W, 44.0),
                                ReplayAction::Continue,
                            );
                        });
                });
            });
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

pub(crate) fn advance_playback(
    time: Res<Time>,
    tape: Option<Res<ReplayTape>>,
    mut playback: ResMut<ReplayPlayback>,
) {
    let Some(tape) = tape else {
        return;
    };
    let last = tape.len().saturating_sub(1) as f32;
    if playback.playing && last > 0.0 {
        playback.cursor = (playback.cursor + playback.speed * time.delta_secs()).min(last);
        if playback.cursor >= last {
            playback.playing = false;
        }
    }
}

pub(crate) fn activate(
    activation: On<bevy::ui_widgets::Activate>,
    actions: Query<&ReplayAction>,
    disabled: Query<(), With<bevy::ui::InteractionDisabled>>,
    tape: Option<Res<ReplayTape>>,
    mut playback: Option<ResMut<ReplayPlayback>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    let Some(playback) = playback.as_deref_mut() else {
        return;
    };
    let last = tape
        .as_ref()
        .map_or(0.0, |tape| tape.len().saturating_sub(1) as f32);
    match action {
        ReplayAction::PlayPause => playback.playing = !playback.playing,
        ReplayAction::StepBack => {
            playback.cursor = (playback.cursor.floor() - 1.0).max(0.0);
            playback.playing = false;
        }
        ReplayAction::StepForward => {
            playback.cursor = (playback.cursor.floor() + 1.0).min(last);
            playback.playing = false;
        }
        ReplayAction::JumpBack => {
            playback.cursor = (playback.cursor - 12.0).max(0.0);
            playback.playing = false;
        }
        ReplayAction::JumpForward => {
            playback.cursor = (playback.cursor + 12.0).min(last);
            playback.playing = false;
        }
        ReplayAction::NextActor => {
            if let Some(tape) = tape.as_deref()
                && !tape.actors.is_empty()
            {
                let index = (tape.focus_index(playback.focus) + 1) % tape.actors.len();
                playback.focus = tape.actors[index].id;
            }
        }
        ReplayAction::Back => next.set(GameState::Results),
        ReplayAction::Continue => next.set(GameState::MainMenu),
    }
}

pub(crate) fn refresh_controls(
    playback: Res<ReplayPlayback>,
    mut labels: Query<(&ReplayAction, &mut WidgetLabel), Without<bevy::ui::InteractionDisabled>>,
) {
    if !playback.is_changed() {
        return;
    }
    for (action, mut label) in &mut labels {
        label.0 = match action {
            ReplayAction::PlayPause if playback.playing => "Pause replay".to_string(),
            ReplayAction::PlayPause => "Play replay".to_string(),
            ReplayAction::StepBack => "Step back".to_string(),
            ReplayAction::StepForward => "Step forward".to_string(),
            ReplayAction::JumpBack => "Jump back".to_string(),
            ReplayAction::JumpForward => "Jump forward".to_string(),
            ReplayAction::NextActor => "Focus next actor".to_string(),
            ReplayAction::Back => "Back to results".to_string(),
            ReplayAction::Continue => "Continue to main menu".to_string(),
        };
    }
}

pub(crate) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<ReplayPlayback>();
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
    let focus = focused_pose(sample, playback.focus)
        .or_else(|| sample.actors.first())
        .map(focus_line)
        .unwrap_or_else(|| "focus unavailable".to_string());
    let markers = recent_markers(tape.as_ref(), sample.index)
        .into_iter()
        .map(|marker| format!("r{} {}", marker.live_round, marker.label))
        .collect::<Vec<_>>()
        .join("\n");
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

pub(crate) fn draw_replay_map(
    tape: Option<Res<ReplayTape>>,
    playback: Res<ReplayPlayback>,
    panel: Query<Entity, With<ReplayMapPanel>>,
    existing: Query<Entity, With<ReplayMapElement>>,
    mut commands: Commands,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(tape) = tape else {
        return;
    };
    let Some(sample) = tape.sample_at(playback.cursor.floor() as usize) else {
        return;
    };
    let Ok(panel) = panel.single() else {
        return;
    };
    let bounds = room_bounds(&tape.rooms);
    commands.entity(panel).with_children(|root| {
        for room in &tape.rooms {
            let center = room_center(room, bounds);
            root.spawn((
                ReplayMapElement,
                replay_box(center, MAP_ROOM, MAP_ROOM, room_color(room)),
                Text::new(format!("{}", room.id.0)),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(TITLE),
            ));
        }
        for (index, pose) in sample.actors.iter().enumerate() {
            let Some(room) = pose.room else {
                continue;
            };
            let Some(room) = tape.rooms.iter().find(|candidate| candidate.id == room) else {
                continue;
            };
            let center = room_center(room, bounds) + actor_offset(index);
            let is_focus = pose.actor == playback.focus;
            root.spawn((
                ReplayMapElement,
                replay_box(
                    center,
                    if is_focus { 18.0 } else { 11.0 },
                    if is_focus { 18.0 } else { 11.0 },
                    actor_color(pose.actor).with_alpha(if is_focus { 1.0 } else { 0.75 }),
                ),
            ));
        }
    });
}

fn replay_box(center: Vec2, w: f32, h: f32, color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - w * 0.5),
            top: px(center.y - h * 0.5),
            width: px(w),
            height: px(h),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(color),
    )
}

fn focused_pose(sample: &ReplaySample, focus: ReplayActorId) -> Option<&ReplayActorPose> {
    sample.actors.iter().find(|pose| pose.actor == focus)
}

fn focus_line(pose: &ReplayActorPose) -> String {
    let where_at = pose
        .place
        .map(|place| format!("{place:?}"))
        .or_else(|| pose.room.map(|room| format!("Room {}", room.0)))
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{} | {} | {} | {}",
        pose.actor.label(),
        where_at,
        pose.status,
        pose.task
    )
}

fn recent_markers(tape: &ReplayTape, sample: usize) -> Vec<&crate::sim::replay::ReplayMarker> {
    tape.markers
        .iter()
        .filter(|marker| marker.sample <= sample)
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn room_bounds(rooms: &[ReplayRoom]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for room in rooms {
        min = min.min(room.schematic);
        max = max.max(room.schematic);
    }
    if rooms.is_empty() {
        (Vec2::ZERO, Vec2::ONE)
    } else {
        (min, max)
    }
}

fn room_center(room: &ReplayRoom, bounds: (Vec2, Vec2)) -> Vec2 {
    let (min, max) = bounds;
    let span = (max - min).max(Vec2::ONE);
    let normalized = (room.schematic - min) / span;
    Vec2::new(
        MAP_INSET + normalized.x * (MAP_W - MAP_INSET * 2.0),
        MAP_INSET + normalized.y * (MAP_H - MAP_INSET * 2.0),
    )
}

fn actor_offset(index: usize) -> Vec2 {
    let x = (index % 4) as f32 - 1.5;
    let y = (index / 4 % 3) as f32 - 1.0;
    Vec2::new(x * 8.0, y * 8.0)
}

fn actor_color(actor: ReplayActorId) -> Color {
    match actor {
        ReplayActorId::LocalPlayer => ACCENT,
        ReplayActorId::Team(team) | ReplayActorId::Member { team, .. } => {
            TEAM_COLORS[team.index() % TEAM_COLORS.len()]
        }
    }
}

fn room_color(room: &ReplayRoom) -> Color {
    use observed_facility::map_spec::RoomRole;
    match room.role {
        RoomRole::Start => Color::srgb(0.18, 0.25, 0.18),
        RoomRole::Exit => Color::srgb(0.18, 0.33, 0.22),
        RoomRole::Keystone => Color::srgb(0.32, 0.27, 0.12),
        RoomRole::DualStation | RoomRole::GuardianControl => Color::srgb(0.25, 0.16, 0.24),
        RoomRole::AnchorCheckpoint | RoomRole::TeleportRelay => Color::srgb(0.12, 0.24, 0.28),
        _ => Color::srgb(0.10, 0.13, 0.18),
    }
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
            ReplayAction::Back,
            ReplayAction::Continue,
        ];
        let control_rows = actions.len().div_ceil(CONTROL_COLUMNS);

        assert!(replay_body_fits(1_280.0));
        assert_eq!(control_rows, 4);
    }
}
