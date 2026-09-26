//! A body asking its Architect for help, in Architect Ascent.
//!
//! One key (T by default, rebindable as "Ask the Architect") or the controller's D-pad
//! left asks. The player does not choose what for: the rules name it from where the body
//! is (`AscentSession::ask_for_help`) - rescue when jailed, power on a dark floor, and
//! otherwise a route on from the cell the body faces. The ask goes to the rules as the
//! body's seat's `SeatCommand::Request` on the next step, like a bot's, and is refused on a
//! cell the team has not found.
//!
//! A small panel under the objective says what the body last asked for and whether the
//! Architect has answered - a bot Architect acknowledges on its beat and builds toward a
//! route it was asked for; a human one answers from the desk - and names the key. An
//! answer is heard as well as seen.

use bevy::audio::{PlaybackMode, Volume};
use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use observed_match::ascent::session::{REQUEST_LIFETIME_TICKS, Refusal, RequestKind};
use observed_match::ascent::sim::CommandRefusal;

use super::overlay::MatchOverlayState;
use super::sim::HexWfcRuntime;
use crate::GameState;
use crate::screens::widgets::UiInputCapture;
use crate::settings::{Settings, key_name};
use crate::view::theme::{ACCENT, DIM, PANEL, TITLE, WARNING};

/// How long a refused ask is shown, in ticks.
const REFUSAL_SHOWN_TICKS: u64 = 240;

/// The local body's asks: one waiting for the next step, and the last refusal and the
/// tick it came back on.
#[derive(Resource, Debug, Default)]
pub(crate) struct AskTheArchitect {
    pub pending: bool,
    pub refused: Option<(Refusal, u64)>,
}

pub(super) struct AskPlugin;

impl Plugin for AskPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn, input, sync)
                .chain()
                .run_if(in_state(GameState::HexWfc))
                .run_if(resource_exists::<AskTheArchitect>),
        );
    }
}

#[derive(Component)]
struct AskPanel;

#[derive(Component)]
struct AskLine;

fn spawn(mut commands: Commands, existing: Query<(), With<AskPanel>>) {
    if !existing.is_empty() {
        return;
    }
    commands
        .spawn((
            AskPanel,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                top: px(148),
                left: px(20),
                max_width: px(330),
                border: UiRect::left(px(3)),
                border_radius: BorderRadius::all(px(3)),
                padding: UiRect::new(px(14), px(16), px(8), px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(ACCENT.with_alpha(0.6)),
            GlobalZIndex(31),
            Visibility::Hidden,
            Name::new("Ask the Architect"),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("YOUR ARCHITECT"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(DIM),
            ));
            panel.spawn((
                AskLine,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(TITLE),
            ));
        });
}

fn input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    settings: Res<Settings>,
    overlay: Res<MatchOverlayState>,
    capture: Res<UiInputCapture>,
    mut ask: ResMut<AskTheArchitect>,
) {
    if *overlay != MatchOverlayState::Playing || capture.is_active() {
        return;
    }
    if keyboard.just_pressed(settings.bindings.ask)
        || gamepads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::DPadLeft))
    {
        ask.pending = true;
    }
}

/// What the panel says, for the local body's live request if it has one.
#[must_use]
pub(super) fn status(
    asked: Option<(RequestKind, u64, bool)>,
    refused: Option<Refusal>,
    tick: u64,
    key: &str,
) -> (String, Color) {
    match (asked, refused) {
        (_, Some(refusal)) => (
            format!(
                "Can't ask there: {}.\n{key} / D-pad left  ask again",
                match refusal {
                    Refusal::UnknownTarget | Refusal::Architect(CommandRefusal::UnknownTarget) => {
                        "your team has not found it"
                    }
                    _ => "the rules refused it",
                }
            ),
            WARNING,
        ),
        (Some((kind, created_at, answered)), None) => {
            let left = REQUEST_LIFETIME_TICKS
                .saturating_sub(tick.saturating_sub(created_at))
                .div_ceil(60);
            if answered {
                (format!("{}\nON IT  /  {left} s", kind.label()), ACCENT)
            } else {
                (format!("{}\nasked  /  {left} s", kind.label()), TITLE)
            }
        }
        (None, None) => (format!("{key} / D-pad left  ask for help"), DIM),
    }
}

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    ask: Res<AskTheArchitect>,
    settings: Res<Settings>,
    overlay: Res<MatchOverlayState>,
    assets: Res<AssetServer>,
    mut panel: Query<&mut Visibility, With<AskPanel>>,
    mut line: Query<(&mut Text, &mut TextColor), With<AskLine>>,
    mut heard: Local<Option<u64>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let session = ascent.session();
    let asked = session.requests.get(&runtime.local_player).map(|request| {
        (
            request.kind,
            request.created_at,
            request.acknowledged_by.is_some(),
        )
    });
    // An answer is heard once.
    if let Some((_, created_at, true)) = asked
        && *heard != Some(created_at)
    {
        *heard = Some(created_at);
        let volume = settings.effective_sfx_volume() * 0.6;
        if volume > 0.0 {
            commands.spawn((
                DespawnOnExit(GameState::HexWfc),
                AudioPlayer::new(assets.load(observed_assets::UI_CLICK.path)),
                PlaybackSettings {
                    mode: PlaybackMode::Despawn,
                    volume: Volume::Linear(volume),
                    speed: 1.25,
                    ..PlaybackSettings::DESPAWN
                },
                Name::new("The Architect answers"),
            ));
        }
    }
    for mut visibility in &mut panel {
        *visibility = if *overlay == MatchOverlayState::Playing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let tick = ascent.rules().tick;
    let refused = ask
        .refused
        .filter(|(_, at)| tick.saturating_sub(*at) < REFUSAL_SHOWN_TICKS)
        .map(|(refusal, _)| refusal);
    let (said, tint) = status(asked, refused, tick, &key_name(settings.bindings.ask));
    for (mut text, mut color) in &mut line {
        if **text != said {
            **text = said.clone();
        }
        color.0 = tint;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_says_what_was_asked_and_whether_it_was_answered() {
        let (idle, _) = status(None, None, 0, "T");
        assert_eq!(idle, "T / D-pad left  ask for help");
        let (asked, tint) = status(Some((RequestKind::Route, 100, false)), None, 100, "T");
        assert_eq!(asked, "Build a route\nasked  /  15 s");
        assert_eq!(tint, TITLE);
        let (answered, tint) = status(Some((RequestKind::Rescue, 100, true)), None, 700, "T");
        assert_eq!(answered, "Need rescue\nON IT  /  5 s");
        assert_eq!(tint, ACCENT);
        let (refused, tint) = status(None, Some(Refusal::UnknownTarget), 0, "G");
        assert!(refused.contains("has not found it"), "{refused}");
        assert!(refused.contains("G / D-pad left"), "{refused}");
        assert_eq!(tint, WARNING);
        for text in [idle, asked, answered, refused] {
            assert!(text.is_ascii(), "the shipped font: {text}");
        }
    }
}
