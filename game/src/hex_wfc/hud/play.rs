//! Restrained first-person chrome. All world eligibility comes from the match brain.
//!
//! It speaks the menus' language (monospace type, dark panels, cyan accents) and keeps
//! out of the way of what the player is looking at:
//!
//! - **Objective**, top left: floor and team above the one thing to do next, with the
//!   team's keystones as pips. Its accent bar is the team's colour.
//! - **Equipment**, top right: what is in hand and the keys that matter. It used to sit
//!   bottom left, over the held plate.
//! - **Prompt**, low centre between and above the hands: a keycap for the key and one for the
//!   controller button, the action, its detail, and for a held action a bar that fills.
//! - **Notice**, top centre: one line that fades in and out; amber when something went
//!   against you.
//!
//! Every panel sizes to its content under a maximum width, so gameplay text scaling
//! (90-125%) wraps within the panel rather than stretching it.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_match::hex_wfc::HexBodyPlace;

use super::super::{HexOnboardingGate, overlay::MatchOverlayState, sim::HexWfcRuntime};
use super::words::{
    MAX_PIPS, NOTICE_SECONDS, ObjectiveFacts, Tone, notice_alpha, notice_for, objective_view,
    prompt_view,
};
use crate::{
    GameState,
    settings::{Settings, key_name},
    view::theme::{ACCENT, DIM, PANEL, TITLE, WARNING, team_color},
};

#[derive(Component)]
pub(in crate::hex_wfc) struct PlayHud;

/// A text field of the HUD, updated each frame.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hex_wfc) enum Field {
    ObjectiveHeading,
    ObjectiveGoal,
    EquipmentCounts,
    EquipmentKeys,
    PromptKey,
    PromptPad,
    PromptTitle,
    PromptDetail,
    Notice,
}

/// A panel, shown or hidden as a whole.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hex_wfc) enum Panel {
    Objective,
    Equipment,
    Prompt,
    Notice,
}

/// The objective panel's accent bar, in the team's colour.
#[derive(Component)]
pub(in crate::hex_wfc) struct TeamAccent;

/// One keystone pip, by index.
#[derive(Component)]
pub(in crate::hex_wfc) struct Pip(u8);

/// The fill of a held action's progress bar.
#[derive(Component)]
pub(in crate::hex_wfc) struct HoldFill;

/// The track the fill runs in, hidden when nothing is being held.
#[derive(Component)]
pub(in crate::hex_wfc) struct HoldTrack;

#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct HudNotice {
    tick: u64,
    until: f64,
    text: &'static str,
    tone: Tone,
}

fn text(field: Field, size: f32, color: Color) -> impl Bundle {
    (
        field,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn panel(kind: Panel) -> impl Bundle {
    (kind, BackgroundColor(PANEL), Visibility::Hidden)
}

/// A keycap: the key's name in a bordered cell.
fn keycap(field: Field, size: f32, color: Color) -> impl Bundle {
    (
        Node {
            min_width: px(size * 1.6),
            padding: UiRect::axes(px(8), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor::all(color),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        children![text(field, size, color)],
    )
}

pub(in crate::hex_wfc) fn setup(mut commands: Commands) {
    commands.insert_resource(HudNotice::default());
    commands
        .spawn((
            PlayHud,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(31),
            Name::new("Contextual gameplay HUD"),
        ))
        .with_children(|root| {
            // Objective, top left, with the team's accent bar down its side.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(20),
                    left: px(20),
                    max_width: percent(34),
                    border: UiRect::left(px(3)),
                    border_radius: BorderRadius::all(px(3)),
                    padding: UiRect::new(px(14), px(16), px(10), px(12)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    ..default()
                },
                panel(Panel::Objective),
                TeamAccent,
                BorderColor::all(ACCENT),
            ))
            .with_children(|panel| {
                panel.spawn(text(Field::ObjectiveHeading, 13.0, DIM));
                panel.spawn(text(Field::ObjectiveGoal, 18.0, TITLE));
                panel
                    .spawn(Node {
                        column_gap: px(6),
                        margin: UiRect::top(px(4)),
                        ..default()
                    })
                    .with_children(|pips| {
                        for index in 0..MAX_PIPS {
                            pips.spawn((
                                Pip(index),
                                Node {
                                    width: px(12),
                                    height: px(12),
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BorderColor::all(ACCENT),
                                BackgroundColor(Color::NONE),
                                Visibility::Hidden,
                            ));
                        }
                    });
            });
            // Equipment, top right: clear of both hands.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(20),
                    right: px(20),
                    max_width: percent(30),
                    padding: UiRect::axes(px(14), px(10)),
                    border_radius: BorderRadius::all(px(3)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: px(4),
                    ..default()
                },
                panel(Panel::Equipment),
            ))
            .with_children(|panel| {
                panel.spawn(text(Field::EquipmentCounts, 15.0, TITLE));
                panel.spawn(text(Field::EquipmentKeys, 13.0, DIM));
            });
            // Notice, top centre.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: px(22),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        max_width: percent(34),
                        padding: UiRect::axes(px(16), px(8)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    panel(Panel::Notice),
                ))
                .with_children(|panel| {
                    panel.spawn(text(Field::Notice, 19.0, ACCENT));
                });
            });
            // Prompt, low centre: above the held items, between the two hands.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: percent(30),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        max_width: percent(34),
                        padding: UiRect::axes(px(14), px(10)),
                        border_radius: BorderRadius::all(px(3)),
                        column_gap: px(12),
                        align_items: AlignItems::FlexStart,
                        ..default()
                    },
                    panel(Panel::Prompt),
                ))
                .with_children(|panel| {
                    panel
                        .spawn(Node {
                            column_gap: px(4),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|keys| {
                            keys.spawn(keycap(Field::PromptKey, 18.0, TITLE));
                            keys.spawn(keycap(Field::PromptPad, 13.0, DIM));
                        });
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            flex_shrink: 1.0,
                            ..default()
                        })
                        .with_children(|words| {
                            words.spawn(text(Field::PromptTitle, 19.0, ACCENT));
                            words.spawn(text(Field::PromptDetail, 14.0, DIM));
                            words
                                .spawn((
                                    HoldTrack,
                                    Node {
                                        width: percent(100),
                                        height: px(4),
                                        margin: UiRect::top(px(5)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.4, 0.92, 1.0, 0.15)),
                                    Visibility::Hidden,
                                ))
                                .with_children(|track| {
                                    track.spawn((
                                        HoldFill,
                                        Node {
                                            width: percent(0),
                                            height: percent(100),
                                            ..default()
                                        },
                                        BackgroundColor(ACCENT),
                                    ));
                                });
                        });
                });
            });
        });
}

pub(in crate::hex_wfc) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HudNotice>();
}

type Fields<'w, 's> = Query<
    'w,
    's,
    (
        &'static Field,
        &'static mut Text,
        &'static mut TextFont,
        &'static mut TextColor,
    ),
>;
type Panels<'w, 's> = Query<
    'w,
    's,
    (
        &'static Panel,
        &'static mut Visibility,
        &'static mut BackgroundColor,
    ),
>;
type Pips<'w, 's> = Query<
    'w,
    's,
    (
        &'static Pip,
        &'static mut Visibility,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    Without<Panel>,
>;

type HoldTracks<'w, 's> =
    Query<'w, 's, &'static mut Visibility, (With<HoldTrack>, Without<Panel>, Without<Pip>)>;

#[derive(SystemParam)]
pub(in crate::hex_wfc) struct HudContext<'w, 's> {
    runtime: Res<'w, HexWfcRuntime>,
    settings: Res<'w, Settings>,
    time: Res<'w, Time>,
    overlay: Res<'w, MatchOverlayState>,
    onboarding: Res<'w, HexOnboardingGate>,
    spectator: Option<Res<'w, crate::sim::state::SpectatorBot>>,
    notice: ResMut<'w, HudNotice>,
    fields: Fields<'w, 's>,
    panels: Panels<'w, 's>,
    pips: Pips<'w, 's>,
    accent: Query<'w, 's, &'static mut BorderColor, (With<TeamAccent>, Without<Pip>)>,
    track: HoldTracks<'w, 's>,
    fill: Query<'w, 's, &'static mut Node, With<HoldFill>>,
}

#[allow(clippy::too_many_lines)]
pub(in crate::hex_wfc) fn sync(context: HudContext) {
    let HudContext {
        runtime,
        settings,
        time,
        overlay,
        onboarding,
        spectator,
        mut notice,
        mut fields,
        mut panels,
        mut pips,
        mut accent,
        mut track,
        mut fill,
    } = context;
    let game = &runtime.match_state;
    let player = runtime.local();
    let team = &game.teams[&player.team];
    let hidden = *overlay != MatchOverlayState::Playing || onboarding.active || spectator.is_some();
    let now = time.elapsed_secs_f64();
    if notice.tick != game.tick {
        notice.tick = game.tick;
        if let Some((text, tone)) = game
            .recent_events
            .iter()
            .rev()
            .filter(|event| event.player == Some(runtime.local_player))
            .find_map(|event| notice_for(event.kind))
        {
            notice.text = text;
            notice.tone = tone;
            notice.until = now + NOTICE_SECONDS;
        }
    }

    let objective = objective_view(ObjectiveFacts {
        floor: player.cell.level,
        team: player.team.0,
        escaped: player.escaped,
        enabled: game.objectives.enabled,
        keystones: team.objectives.keystones,
        required: game.objectives.keystones_required,
        station_done: team.objectives.dual_station_complete,
        solo: team.members.len() == 1,
        ascent: runtime.ascent.is_some(),
        jailed: player.place == HexBodyPlace::Prison,
        teammates_jailed: u8::try_from(
            team.members
                .iter()
                .filter(|&&id| id != player.id && game.players[&id].place == HexBodyPlace::Prison)
                .count(),
        )
        .unwrap_or(u8::MAX),
    });
    let prompt = game
        .interaction(runtime.local_player)
        .map(|prompt| prompt_view(&prompt, &settings, team.objectives.dual_station_ticks));
    let notice_left = notice.until - now;
    let alpha = if notice_left > 0.0 {
        notice_alpha(notice_left)
    } else {
        0.0
    };
    let tone_color = match notice.tone {
        Tone::Good => ACCENT,
        Tone::Against => WARNING,
    };

    let scale = settings.gameplay_text_scale;
    for (field, mut text, mut font, mut color) in &mut fields {
        let (line, size, tint) = match field {
            Field::ObjectiveHeading => (objective.heading.clone(), 13.0, DIM),
            Field::ObjectiveGoal => (objective.goal.clone(), 18.0, TITLE),
            Field::EquipmentCounts => (
                format!(
                    "LANTERNS {}   PLATES {}",
                    game.lanterns.inventory(runtime.local_player),
                    game.pads.inventory(runtime.local_player)
                ),
                15.0,
                TITLE,
            ),
            Field::EquipmentKeys => (
                format!(
                    "[{}] Plate   [{}] Map   [{}] Pause",
                    key_name(settings.bindings.pad),
                    key_name(settings.bindings.tac_map),
                    key_name(settings.bindings.pause)
                ),
                13.0,
                DIM,
            ),
            Field::PromptKey => (
                prompt.as_ref().map_or_else(String::new, |p| p.key.clone()),
                18.0,
                TITLE,
            ),
            Field::PromptPad => (
                prompt
                    .as_ref()
                    .map_or_else(String::new, |p| p.pad.to_owned()),
                13.0,
                DIM,
            ),
            Field::PromptTitle => (
                prompt
                    .as_ref()
                    .map_or_else(String::new, |p| p.title.clone()),
                19.0,
                ACCENT,
            ),
            Field::PromptDetail => (
                prompt
                    .as_ref()
                    .map_or_else(String::new, |p| p.detail.to_owned()),
                14.0,
                DIM,
            ),
            Field::Notice => (notice.text.to_owned(), 19.0, tone_color.with_alpha(alpha)),
        };
        let next = FontSize::Px(size * scale);
        if font.font_size != next {
            font.font_size = next;
        }
        if color.0 != tint {
            color.0 = tint;
        }
        if text.0 != line {
            text.0 = line;
        }
    }

    for (panel, mut visibility, mut background) in &mut panels {
        let shown = !hidden
            && match panel {
                Panel::Objective | Panel::Equipment => true,
                Panel::Prompt => prompt.is_some(),
                Panel::Notice => alpha > 0.0,
            };
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let panel_alpha = if *panel == Panel::Notice {
            PANEL.alpha() * alpha
        } else {
            PANEL.alpha()
        };
        let tint = PANEL.with_alpha(panel_alpha);
        if background.0 != tint {
            background.0 = tint;
        }
    }

    let team_accent = team_color(usize::from(player.team.0));
    for mut border in &mut accent {
        *border = BorderColor::all(team_accent);
    }
    for (pip, mut visibility, mut background, mut border) in &mut pips {
        let (shown, filled) = objective.pips.map_or((false, false), |(held, required)| {
            (pip.0 < required, pip.0 < held)
        });
        *visibility = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        *background = BackgroundColor(if filled { ACCENT } else { Color::NONE });
        *border = BorderColor::all(ACCENT);
    }
    let progress = prompt.as_ref().and_then(|p| p.progress);
    for mut visibility in &mut track {
        *visibility = if progress.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut node in &mut fill {
        node.width = percent(progress.unwrap_or(0.0) * 100.0);
    }
}
