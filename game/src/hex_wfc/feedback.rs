//! Screen-space semantic event feedback for the hex match.

use bevy::prelude::*;

use super::cues::cue_for;
use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct EventBanner;

#[derive(Resource, Default)]
pub(super) struct FeedbackProjection {
    last_tick: u64,
    expires_at: u64,
}

pub(super) fn setup(mut commands: Commands) {
    commands.insert_resource(FeedbackProjection::default());
    commands.spawn((
        EventBanner,
        DespawnOnExit(GameState::HexWfc),
        Text::new(""),
        TextFont {
            font_size: 24.0,
            ..Default::default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(38),
            left: percent(35),
            width: percent(30),
            padding: UiRect::axes(px(14), px(9)),
            border: UiRect::all(px(1)),
            justify_content: JustifyContent::Center,
            ..Default::default()
        },
        BackgroundColor(Color::srgba(0.004, 0.01, 0.024, 0.88)),
        BorderColor::all(Color::NONE),
        GlobalZIndex(50),
        Name::new("Hex WFC gameplay cue"),
    ));
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<FeedbackProjection>();
}

pub(super) fn sync(
    runtime: Res<HexWfcRuntime>,
    mut state: ResMut<FeedbackProjection>,
    mut banner: Query<(&mut Text, &mut TextColor, &mut BorderColor), With<EventBanner>>,
) {
    let Ok((mut text, mut text_color, mut border)) = banner.single_mut() else {
        return;
    };
    if state.last_tick != runtime.match_state.tick {
        state.last_tick = runtime.match_state.tick;
        if let Some(event) = runtime.match_state.recent_events.last() {
            let definition = cue_for(event.kind);
            let color = observed_style::marker(definition.marker).base_color;
            **text = format!("{}  {}", definition.glyph, definition.label);
            text_color.0 = color;
            *border = BorderColor::all(color);
            state.expires_at = runtime.match_state.tick + 120;
        }
    }
    if runtime.match_state.tick > state.expires_at {
        **text = String::new();
        *border = BorderColor::all(Color::NONE);
    }
}
