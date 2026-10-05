//! Time, focus and camera controls for the replay viewer.
use super::scene;
use crate::screens::widgets::{WidgetLabel, activation_enabled};
use crate::{
    GameState,
    sim::replay::{ReplayActorId, ReplayTape},
};
use bevy::prelude::*;
use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue, ValueChange};
#[derive(Resource, Clone, Debug)]
pub(crate) struct ReplayPlayback {
    pub cursor: f32,
    pub playing: bool,
    pub speed: f32,
    pub focus: ReplayActorId,
    pub view: scene::CameraView,
    pub detent: u8,
    pub floor_offset: i8,
    pub event_focus: Option<observed_hex::HexCoord>,
}

impl Default for ReplayPlayback {
    fn default() -> Self {
        Self {
            cursor: 0.0,
            playing: true,
            speed: 1.0,
            view: default(),
            detent: 0,
            floor_offset: 0,
            event_focus: None,
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
    PreviousEvent,
    NextEvent,
    Camera,
    Rotate,
    Floor,
    Speed,
    Back,
    Continue,
}

pub(crate) fn advance_playback(
    time: Res<Time>,
    tape: Option<Res<ReplayTape>>,
    mut playback: ResMut<ReplayPlayback>,
) {
    let Some(tape) = tape else {
        return;
    };
    if !playback.playing {
        return;
    }
    playback.cursor = advance_cursor(&tape, playback.cursor, time.delta_secs() * playback.speed);
    if playback.cursor >= tape.len().saturating_sub(1) as f32 {
        playback.playing = false;
    }
}

pub(crate) fn activate(
    activation: On<bevy::ui_widgets::Activate>,
    actions: Query<&ReplayAction>,
    disabled: Query<(), With<bevy::ui::InteractionDisabled>>,
    tape: Option<Res<ReplayTape>>,
    mut playback: Option<ResMut<ReplayPlayback>>,
    mut next: ResMut<NextState<GameState>>,
    lan: Res<crate::lan::LanRuntime>,
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
    if matches!(
        action,
        ReplayAction::PlayPause
            | ReplayAction::StepBack
            | ReplayAction::StepForward
            | ReplayAction::JumpBack
            | ReplayAction::JumpForward
    ) {
        playback.event_focus = None;
    }
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
            playback.cursor = seek_seconds(tape.as_deref(), playback.cursor, -10.0);
            playback.playing = false;
        }
        ReplayAction::JumpForward => {
            playback.cursor = seek_seconds(tape.as_deref(), playback.cursor, 10.0);
            playback.playing = false;
        }
        ReplayAction::NextActor => {
            if let Some(tape) = tape.as_deref()
                && !tape.actors.is_empty()
            {
                let index = (tape.focus_index(playback.focus) + 1) % tape.actors.len();
                playback.focus = tape.actors[index].id;
                playback.floor_offset = 0;
                playback.event_focus = None;
            }
        }
        ReplayAction::PreviousEvent | ReplayAction::NextEvent => {
            if let Some(tape) = tape.as_deref() {
                let current = playback.cursor.floor() as usize;
                let target = if *action == ReplayAction::PreviousEvent {
                    tape.markers
                        .iter()
                        .filter(|m| m.sample < current)
                        .map(|m| m.sample)
                        .max()
                } else {
                    tape.markers
                        .iter()
                        .filter(|m| m.sample > current)
                        .map(|m| m.sample)
                        .min()
                };
                if let Some(target) = target {
                    playback.cursor = target as f32;
                    let marker = tape
                        .markers
                        .iter()
                        .find(|m| m.sample == target && m.player.is_some())
                        .or_else(|| {
                            tape.markers
                                .iter()
                                .rev()
                                .find(|m| m.sample == target && m.cell.is_some())
                        });
                    playback.event_focus = marker.and_then(|m| m.cell);
                    if let Some(actor) = marker.and_then(|m| m.player).and_then(|player| {
                        tape.scene_frames
                            .iter()
                            .find(|f| f.sample == target)
                            .and_then(|f| f.bodies.iter().find(|b| b.player == player))
                    }) {
                        playback.focus = actor.actor;
                    }
                }
                playback.playing = false;
            }
        }
        ReplayAction::Camera => playback.view = playback.view.next(),
        ReplayAction::Rotate => playback.detent = (playback.detent + 1) % 6,
        ReplayAction::Floor => {
            playback.event_focus = None;
            let levels = tape
                .as_deref()
                .and_then(|t| scene::frames(t, playback.cursor))
                .map_or(1, |(f, _, _)| {
                    f.facility
                        .pieces
                        .iter()
                        .map(|p| p.source_cell.level)
                        .max()
                        .unwrap_or(0)
                        + 1
                });
            let base = tape
                .as_deref()
                .and_then(|t| scene::frames(t, playback.cursor))
                .and_then(|(f, _, _)| f.bodies.iter().find(|b| b.actor == playback.focus))
                .map_or(0, |b| b.cell.level);
            let floor = base.saturating_add_signed(playback.floor_offset);
            playback.floor_offset = ((floor + 1) % levels) as i8 - base as i8;
        }
        ReplayAction::Speed => {
            playback.speed = if playback.speed >= 4.0 {
                0.25
            } else {
                playback.speed * 2.0
            }
        }
        ReplayAction::Back => next.set(GameState::Results),
        ReplayAction::Continue => next.set(if lan.client.is_some() {
            GameState::Lobby
        } else {
            GameState::MainMenu
        }),
    }
}

pub(crate) fn refresh_controls(
    playback: Res<ReplayPlayback>,
    lan: Res<crate::lan::LanRuntime>,
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
            ReplayAction::JumpBack => "-10 seconds".to_string(),
            ReplayAction::JumpForward => "+10 seconds".to_string(),
            ReplayAction::NextActor => "Focus next actor".to_string(),
            ReplayAction::PreviousEvent => "Previous event".into(),
            ReplayAction::NextEvent => "Next event".into(),
            ReplayAction::Camera => format!("View: {}", playback.view.label()),
            ReplayAction::Rotate => "Rotate view".into(),
            ReplayAction::Floor => "Next floor".into(),
            ReplayAction::Speed => format!("Speed: {}x", playback.speed),
            ReplayAction::Back => "Back to results".to_string(),
            ReplayAction::Continue => if lan.client.is_some() {
                "Return to lobby"
            } else {
                "Main menu"
            }
            .to_string(),
        };
    }
}

pub(crate) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<ReplayPlayback>();
}

#[derive(Component)]
pub(crate) struct ReplayTimeline;

pub(crate) fn scrub(
    change: On<ValueChange<f32>>,
    timeline: Query<(), With<ReplayTimeline>>,
    mut playback: Option<ResMut<ReplayPlayback>>,
    tape: Option<Res<ReplayTape>>,
) {
    if timeline.get(change.source).is_err() {
        return;
    }
    if let (Some(playback), Some(tape)) = (playback.as_deref_mut(), tape.as_deref()) {
        playback.cursor = change.value.clamp(0.0, tape.len().saturating_sub(1) as f32);
        playback.playing = false;
        playback.event_focus = None;
    }
}
pub(crate) fn refresh_timeline(
    mut commands: Commands,
    playback: Res<ReplayPlayback>,
    timeline: Query<(Entity, &Children, &SliderRange), With<ReplayTimeline>>,
    mut thumbs: Query<&mut Node, With<SliderThumb>>,
) {
    for (entity, children, range) in &timeline {
        commands.entity(entity).insert(SliderValue(playback.cursor));
        for child in children {
            if let Ok(mut node) = thumbs.get_mut(*child) {
                node.left = px((super::MAP_W + super::DETAILS_W + super::BODY_GAP - 16.0)
                    * playback.cursor
                    / range.end().max(1.0));
            }
        }
    }
}

fn cursor_tick(tape: &ReplayTape, cursor: f32) -> f64 {
    let i = cursor.floor() as usize;
    let a = tape.samples.get(i).map_or(0, |s| s.live_round);
    let b = tape.samples.get(i + 1).map_or(a, |s| s.live_round);
    f64::from(a) + f64::from(b.saturating_sub(a)) * f64::from(cursor.fract())
}
fn seek_tick(tape: &ReplayTape, tick: f64) -> f32 {
    let i = tape
        .samples
        .partition_point(|s| f64::from(s.live_round) <= tick)
        .saturating_sub(1);
    let Some(a) = tape.samples.get(i) else {
        return 0.0;
    };
    let Some(b) = tape.samples.get(i + 1) else {
        return i as f32;
    };
    i as f32
        + ((tick - f64::from(a.live_round))
            / f64::from(b.live_round.saturating_sub(a.live_round).max(1)))
        .clamp(0.0, 1.0) as f32
}
fn advance_cursor(tape: &ReplayTape, cursor: f32, seconds: f32) -> f32 {
    if tape.scene_frames.is_empty() {
        return (cursor + seconds * 12.0).min(tape.len().saturating_sub(1) as f32);
    }
    seek_tick(tape, cursor_tick(tape, cursor) + f64::from(seconds) * 60.0)
}
fn seek_seconds(tape: Option<&ReplayTape>, cursor: f32, seconds: f32) -> f32 {
    tape.map_or(cursor, |t| {
        if t.scene_frames.is_empty() {
            (cursor + seconds * 12.0).clamp(0.0, t.len().saturating_sub(1) as f32)
        } else {
            seek_tick(
                t,
                (cursor_tick(t, cursor) + f64::from(seconds) * 60.0).max(0.0),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeline_uses_real_ticks_between_irregular_event_samples() {
        let spec = crate::map_catalog::default_map_spec(1);
        let mut tape = ReplayTape::new(1, &spec);
        for tick in [0, 1, 6, 12, 600] {
            tape.push_sample(tick, 0, Vec::new());
        }
        assert_eq!(seek_tick(&tape, 0.0), 0.0);
        assert_eq!(seek_tick(&tape, 3.5), 1.5);
        assert_eq!(cursor_tick(&tape, 1.5), 3.5);
        assert_eq!(seek_tick(&tape, 900.0), 4.0);
        assert_eq!(seek_tick(&tape, -10.0), 0.0);
    }
    #[test]
    fn scrubbing_camera_changes_and_reentry_leave_no_scene_state() {
        use crate::hex_wfc::sim::HexWfcRuntime;
        use crate::screens::replay::scene::{
            ReplayCamera, ReplayFigure, ReplayGeometry, ReplayScene,
        };
        use crate::tests::{count, go, test_app};
        let mut app = test_app();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::ZERO,
        ));
        go(&mut app, GameState::HexWfc);
        let tape = app.world().resource::<ReplayTape>().clone();
        assert!(!tape.scene_frames.is_empty());
        go(&mut app, GameState::Replay);
        assert!(!app.world().contains_resource::<HexWfcRuntime>());
        assert_eq!(count::<ReplayCamera>(&mut app), 1);
        assert!(count::<ReplayGeometry>(&mut app) > 0);
        let timeline = app
            .world_mut()
            .query_filtered::<Entity, With<ReplayTimeline>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(ValueChange {
            source: timeline,
            value: 1000.0,
            is_final: true,
        });
        app.update();
        assert_eq!(
            app.world().resource::<ReplayPlayback>().cursor,
            tape.len().saturating_sub(1) as f32
        );
        assert!(!app.world().resource::<ReplayPlayback>().playing);
        for view in [
            scene::CameraView::Team,
            scene::CameraView::Floor,
            scene::CameraView::Eyes,
            scene::CameraView::Follow,
        ] {
            app.world_mut().resource_mut::<ReplayPlayback>().view = view;
            app.update();
        }
        assert_eq!(*app.world().resource::<ReplayTape>(), tape);
        let figures = count::<ReplayFigure>(&mut app);
        go(&mut app, GameState::Results);
        assert_eq!(count::<ReplayCamera>(&mut app), 0);
        assert_eq!(count::<ReplayGeometry>(&mut app), 0);
        assert_eq!(count::<ReplayFigure>(&mut app), 0);
        assert!(!app.world().contains_resource::<ReplayScene>());
        go(&mut app, GameState::Replay);
        assert_eq!(count::<ReplayCamera>(&mut app), 1);
        assert_eq!(count::<ReplayFigure>(&mut app), figures);
    }
}
