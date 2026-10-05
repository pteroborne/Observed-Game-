//! Compatibility view for the deprecated isolated-place regression fixture.
use super::{MAP_H, MAP_INSET, MAP_ROOM, MAP_W, ReplayMapElement, ReplayMapPanel, ReplayPlayback};
use crate::sim::replay::{ReplayActorId, ReplayActorPose, ReplayRoom, ReplaySample, ReplayTape};
use crate::view::theme::{ACCENT, BORDER, PANEL, TEAM_COLORS, TITLE};
use bevy::prelude::*;
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
    if !tape.scene_frames.is_empty() {
        return;
    }
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
                replay_box(
                    center,
                    MAP_ROOM,
                    MAP_ROOM,
                    if tape.ascent_result.is_some() {
                        PANEL
                    } else {
                        room_color(room)
                    },
                    true,
                ),
                Text::new(format!("R{}", room.id.0)),
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
                    false,
                ),
            ));
        }
    });
}

fn replay_box(center: Vec2, w: f32, h: f32, color: Color, outlined: bool) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - w * 0.5),
            top: px(center.y - h * 0.5),
            width: px(w),
            height: px(h),
            border: UiRect::all(px(if outlined { 1.0 } else { 0.0 })),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(color),
        BorderColor::all(BORDER),
    )
}

pub(super) fn focused_pose(
    sample: &ReplaySample,
    focus: ReplayActorId,
) -> Option<&ReplayActorPose> {
    sample.actors.iter().find(|pose| pose.actor == focus)
}

pub(super) fn focus_line(pose: &ReplayActorPose, tape: &ReplayTape) -> String {
    let where_at = pose
        .place
        .map(|place| format!("{place:?}"))
        .or_else(|| pose.room.map(|room| format!("Room {}", room.0)))
        .unwrap_or_else(|| {
            match pose.status.as_str() {
                "jailed" => "prison",
                "Rogue" => "no Observer body",
                _ => "unknown",
            }
            .to_string()
        });
    format!(
        "{} | {} | {} | {}",
        tape.actors
            .iter()
            .find(|actor| actor.id == pose.actor)
            .map(|actor| actor.label.clone())
            .unwrap_or_else(|| pose.actor.label()),
        where_at,
        pose.status,
        pose.task
    )
}

pub(super) fn recent_markers(
    tape: &ReplayTape,
    sample: usize,
) -> Vec<&crate::sim::replay::ReplayMarker> {
    tape.markers
        .iter()
        .filter(|marker| marker.sample <= sample)
        .rev()
        .take(if tape.ascent_result.is_some() { 2 } else { 5 })
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
