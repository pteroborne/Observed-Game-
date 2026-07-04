//! Post-match tactical replay screen.
//!
//! The screen reads only [`ReplayTape`]. It does not reach back into live match
//! resources, so watching a replay cannot change or depend on the completed match.

use bevy::prelude::*;

use super::{MenuAction, MenuCursor, menu_button};
use crate::GameState;
use crate::sim::replay::{ReplayActorId, ReplayActorPose, ReplayRoom, ReplaySample, ReplayTape};
use crate::view::theme::{
    ACCENT, BORDER, DIM, PANEL, TEAM_COLORS, TITLE, panel, screen_root, text,
};

const MAP_W: f32 = 620.0;
const MAP_H: f32 = 400.0;
const MAP_ROOM: f32 = 34.0;
const MAP_INSET: f32 = 34.0;

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

pub(crate) fn setup_replay(
    mut commands: Commands,
    mut cursor: ResMut<MenuCursor>,
    tape: Option<Res<ReplayTape>>,
) {
    cursor.0 = 0;
    let focus = tape
        .as_ref()
        .map(|tape| tape.default_focus())
        .unwrap_or(ReplayActorId::LocalPlayer);
    commands.insert_resource(ReplayPlayback { focus, ..default() });

    commands
        .spawn(screen_root(GameState::Replay))
        .with_children(|root| {
            root.spawn(text("REPLAY", 42.0, TITLE));
            root.spawn((ReplayInfo, text("Replay loading...", 16.0, DIM)));
            root.spawn((
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
            root.spawn(panel()).with_children(|p| {
                p.spawn(menu_button(
                    0,
                    MenuAction::Goto(GameState::Results),
                    "Back to results",
                ));
                p.spawn(menu_button(
                    1,
                    MenuAction::Goto(GameState::MainMenu),
                    "Continue",
                ));
            });
            root.spawn(text(
                "P play/pause | Left/Right step | [/] jump | Tab focus actor | Enter select",
                14.0,
                DIM,
            ));
        });
}

pub(crate) fn replay_controls(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    tape: Option<Res<ReplayTape>>,
    mut playback: ResMut<ReplayPlayback>,
) {
    let Some(tape) = tape else {
        return;
    };
    let last = tape.len().saturating_sub(1) as f32;
    if keyboard.just_pressed(KeyCode::KeyP) {
        playback.playing = !playback.playing;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        playback.cursor = (playback.cursor.floor() + 1.0).min(last);
        playback.playing = false;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        playback.cursor = (playback.cursor.floor() - 1.0).max(0.0);
        playback.playing = false;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        playback.cursor = (playback.cursor + 12.0).min(last);
        playback.playing = false;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        playback.cursor = (playback.cursor - 12.0).max(0.0);
        playback.playing = false;
    }
    if keyboard.just_pressed(KeyCode::Tab) && !tape.actors.is_empty() {
        let next = (tape.focus_index(playback.focus) + 1) % tape.actors.len();
        playback.focus = tape.actors[next].id;
    }
    if playback.playing && last > 0.0 {
        playback.cursor = (playback.cursor + playback.speed * time.delta_secs()).min(last);
        if playback.cursor >= last {
            playback.playing = false;
        }
    }
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
                    font_size: 10.0,
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
