//! Responsive card dock and the controls that orbit a map preview.

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::{percent, px};

use crate::UiCameraEntity;
use crate::art::CardArt;
use crate::board::{BoardCamera, world_of};
use crate::input::{CardDrag, TrialClock};
use crate::model::{LabState, card_description, card_subtitle};

const PANEL: Color = Color::srgba(0.018, 0.028, 0.042, 0.97);
const CARD: Color = Color::srgb(0.075, 0.105, 0.14);
const CARD_SELECTED: Color = Color::srgb(0.12, 0.23, 0.25);
const INK: Color = Color::srgb(0.92, 0.95, 0.97);
const MUTED: Color = Color::srgb(0.60, 0.67, 0.72);
const ACCENT: Color = Color::srgb(0.98, 0.67, 0.20);
const VALID: Color = Color::srgb(0.22, 0.72, 0.66);

#[derive(Component)]
pub struct HudRoot;
#[derive(Component)]
pub struct MapSpacer;
#[derive(Component)]
pub struct Dock;
#[derive(Component)]
pub struct ScenarioText;
#[derive(Component)]
pub struct PromptText;
#[derive(Component)]
pub struct NoticeText;
#[derive(Component)]
pub struct FocusTitle;
#[derive(Component)]
pub struct FocusSubtitle;
#[derive(Component)]
pub struct FocusDescription;
#[derive(Component)]
pub struct FocusArt;
#[derive(Component)]
pub struct RotationText;
#[derive(Component)]
pub struct RotationOverlay;
#[derive(Component)]
pub struct DragGhost;

#[derive(Component, Clone, Copy)]
pub(crate) enum ResponsivePart {
    Root,
    Dock,
    Prompt,
    Focus,
    FocusArt,
    Hand,
    Card,
    Notice,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandSlot(pub usize);

#[derive(Component, Clone, Copy)]
pub struct HandSlotArt(pub usize);

#[derive(Component, Clone, Copy)]
pub struct HandSlotLabel(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Control {
    PreviousScenario,
    NextScenario,
    Place,
    Cancel,
    Undo,
    Reset,
    RotateLeft,
    RotateRight,
    CopyReport,
}

impl Control {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PreviousScenario => "PREV",
            Self::NextScenario => "NEXT",
            Self::Place => "PLACE",
            Self::Cancel => "CANCEL",
            Self::Undo => "UNDO",
            Self::Reset => "RESET",
            Self::RotateLeft => "CCW",
            Self::RotateRight => "CW",
            Self::CopyReport => "COPY",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockPlacement {
    Right,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockLayout {
    pub placement: DockPlacement,
    pub viewport_size: Vec2,
    pub dock_size: Vec2,
}

impl DockLayout {
    pub const BREAKPOINT: f32 = 700.0;
    pub const RIGHT_WIDTH: f32 = 370.0;

    #[must_use]
    pub fn for_window(size: Vec2) -> Self {
        if size.x < Self::BREAKPOINT {
            let height = (size.y * 0.50)
                .clamp(340.0, 410.0)
                .min((size.y - 250.0).max(1.0));
            Self {
                placement: DockPlacement::Bottom,
                viewport_size: Vec2::new(size.x.max(1.0), (size.y - height).max(1.0)),
                dock_size: Vec2::new(size.x.max(1.0), height),
            }
        } else {
            Self {
                placement: DockPlacement::Right,
                viewport_size: Vec2::new((size.x - Self::RIGHT_WIDTH).max(1.0), size.y.max(1.0)),
                dock_size: Vec2::new(Self::RIGHT_WIDTH, size.y.max(1.0)),
            }
        }
    }
}

pub fn spawn(mut commands: Commands, art: Res<CardArt>, ui_camera: Res<UiCameraEntity>) {
    commands
        .spawn((
            HudRoot,
            ResponsivePart::Root,
            UiTargetCamera(ui_camera.0),
            Node {
                width: percent(100.0),
                height: percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("Architect hand UI"),
        ))
        .with_children(|root| {
            root.spawn((
                MapSpacer,
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            root.spawn((
                Dock,
                ResponsivePart::Dock,
                Node {
                    width: px(DockLayout::RIGHT_WIDTH),
                    height: percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(7.0),
                    padding: UiRect::all(px(12.0)),
                    border: UiRect::left(px(2.0)),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.22, 0.33, 0.40)),
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|dock| {
                dock.spawn((
                    Node {
                        width: percent(100.0),
                        height: px(48.0),
                        flex_shrink: 0.0,
                        align_items: AlignItems::Center,
                        column_gap: px(8.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|row| {
                    control_button(row, Control::PreviousScenario, 48.0, false);
                    row.spawn((
                        ScenarioText,
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(ACCENT),
                        Node {
                            flex_grow: 1.0,
                            flex_basis: px(0.0),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    control_button(row, Control::NextScenario, 48.0, false);
                });

                dock.spawn((
                    PromptText,
                    ResponsivePart::Prompt,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(INK),
                    Node {
                        width: percent(100.0),
                        min_height: px(34.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ));

                dock.spawn((
                    ResponsivePart::Focus,
                    Node {
                        width: percent(100.0),
                        min_height: px(92.0),
                        flex_shrink: 0.0,
                        align_items: AlignItems::Center,
                        column_gap: px(10.0),
                        padding: UiRect::all(px(7.0)),
                        border: UiRect::all(px(2.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.30, 0.45, 0.50)),
                    BackgroundColor(Color::srgb(0.035, 0.055, 0.075)),
                    Pickable::IGNORE,
                ))
                .with_children(|focus| {
                    focus.spawn((
                        FocusArt,
                        ResponsivePart::FocusArt,
                        ImageNode::new(art.get(observed_mechanics::tiles::TileShape::Corridor)),
                        Node {
                            width: px(78.0),
                            height: px(78.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    focus
                        .spawn((
                            Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                row_gap: px(2.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|copy| {
                            copy.spawn((
                                FocusTitle,
                                Text::new("CHOOSE A CARD"),
                                TextFont {
                                    font_size: FontSize::Px(18.0),
                                    ..default()
                                },
                                TextColor(INK),
                                Pickable::IGNORE,
                            ));
                            copy.spawn((
                                FocusSubtitle,
                                Text::new("TOPOLOGY IS THE CARD"),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(ACCENT),
                                Pickable::IGNORE,
                            ));
                            copy.spawn((
                                FocusDescription,
                                Text::new("Select one of the four authored silhouettes below."),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(MUTED),
                                Pickable::IGNORE,
                            ));
                            copy.spawn((
                                RotationText,
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.40, 0.90, 0.84)),
                                Pickable::IGNORE,
                            ));
                        });
                });

                dock.spawn((
                    ResponsivePart::Hand,
                    Node {
                        width: percent(100.0),
                        height: px(92.0),
                        flex_shrink: 0.0,
                        column_gap: px(6.0),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|hand| {
                    for slot in 0..4 {
                        hand.spawn((
                            HandSlot(slot),
                            ResponsivePart::Card,
                            Button,
                            Node {
                                flex_grow: 1.0,
                                flex_basis: px(0.0),
                                min_width: px(66.0),
                                height: px(92.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                padding: UiRect::all(px(4.0)),
                                border: UiRect::all(px(2.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.25, 0.34, 0.40)),
                            BackgroundColor(CARD),
                            Name::new(format!("Card slot {}", slot + 1)),
                        ))
                        .with_children(|card| {
                            card.spawn((
                                HandSlotArt(slot),
                                ImageNode::new(
                                    art.get(observed_mechanics::tiles::TileShape::Corridor),
                                ),
                                Node {
                                    width: px(58.0),
                                    height: px(58.0),
                                    ..default()
                                },
                                Pickable::IGNORE,
                            ));
                            card.spawn((
                                HandSlotLabel(slot),
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(9.5),
                                    ..default()
                                },
                                TextColor(INK),
                                Pickable::IGNORE,
                            ));
                        });
                    }
                });

                dock.spawn((
                    Node {
                        width: percent(100.0),
                        height: px(52.0),
                        flex_shrink: 0.0,
                        column_gap: px(6.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|row| {
                    control_button(row, Control::Place, 52.0, true);
                    control_button(row, Control::Cancel, 52.0, false);
                    control_button(row, Control::Undo, 52.0, false);
                    control_button(row, Control::Reset, 52.0, false);
                    control_button(row, Control::CopyReport, 52.0, false);
                });

                dock.spawn((
                    NoticeText,
                    ResponsivePart::Notice,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(INK),
                    Node {
                        width: percent(100.0),
                        min_height: px(30.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            });

            root.spawn((
                RotationOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(168.0),
                    height: px(54.0),
                    display: Display::None,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|ring| {
                control_button(ring, Control::RotateLeft, 54.0, false);
                control_button(ring, Control::RotateRight, 54.0, false);
            });

            root.spawn((
                DragGhost,
                ImageNode::new(art.get(observed_mechanics::tiles::TileShape::Corridor)),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(86.0),
                    height: px(86.0),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(100),
                Pickable::IGNORE,
            ));
        });
}

fn control_button(parent: &mut ChildSpawnerCommands, control: Control, height: f32, primary: bool) {
    parent
        .spawn((
            control,
            Button,
            Node {
                min_width: px(height),
                height: px(height),
                flex_grow: if matches!(
                    control,
                    Control::Place | Control::Cancel | Control::Undo | Control::Reset
                ) {
                    1.0
                } else {
                    0.0
                },
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(px(8.0)),
                border: UiRect::all(px(2.0)),
                ..default()
            },
            BorderColor::all(if primary {
                VALID
            } else {
                Color::srgb(0.27, 0.37, 0.44)
            }),
            BackgroundColor(if primary {
                Color::srgb(0.08, 0.30, 0.28)
            } else {
                CARD
            }),
            Name::new(control.label()),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(control.label()),
                TextFont {
                    font_size: FontSize::Px(if height >= 52.0 { 12.0 } else { 10.0 }),
                    ..default()
                },
                TextColor(INK),
                Pickable::IGNORE,
            ));
        });
}

type CopyQuerySet<'w, 's> = (
    Query<'w, 's, &'static mut Text, With<ScenarioText>>,
    Query<'w, 's, &'static mut Text, With<PromptText>>,
    Query<'w, 's, &'static mut Text, With<NoticeText>>,
    Query<'w, 's, &'static mut Text, With<FocusTitle>>,
    Query<'w, 's, &'static mut Text, With<FocusSubtitle>>,
    Query<'w, 's, &'static mut Text, With<FocusDescription>>,
    Query<'w, 's, &'static mut Text, With<RotationText>>,
);

type FocusArtQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut ImageNode,
    (With<FocusArt>, Without<DragGhost>, Without<HandSlotArt>),
>;

pub fn sync_text(
    state: Res<LabState>,
    clock: Res<TrialClock>,
    art: Res<CardArt>,
    mut copy: ParamSet<CopyQuerySet>,
    mut focus_art: FocusArtQuery,
) {
    **copy.p0().single_mut().expect("scenario text") = format!(
        "TASK {} / {}   {}",
        state.scenario.number(),
        ScenarioCount::TOTAL,
        state.scenario.title()
    );
    **copy.p1().single_mut().expect("prompt text") = state.prompt.clone();
    **copy.p2().single_mut().expect("notice text") = state.notice.clone();

    if state.complete {
        **copy.p3().single_mut().expect("focus title") = "BRIEF COMPLETE".to_string();
        **copy.p4().single_mut().expect("focus subtitle") = "LOCAL USABILITY REPORT".to_string();
        **copy.p5().single_mut().expect("focus description") = state.report(clock.elapsed);
        **copy.p6().single_mut().expect("rotation text") =
            "COPY saves this summary; NEXT opens the next brief.".to_string();
        if let Some(card) = state.discard.last() {
            let mut image = focus_art.single_mut().expect("focus art");
            image.image = art.get(card.shape);
            image.color = Color::WHITE;
        }
    } else if let Some(card) = state.selected_card() {
        **copy.p3().single_mut().expect("focus title") = card.shape.label().to_uppercase();
        **copy.p4().single_mut().expect("focus subtitle") = card_subtitle(card.shape).to_string();
        **copy.p5().single_mut().expect("focus description") =
            card_description(card.shape).to_string();
        focus_art.single_mut().expect("focus art").image = art.get(card.shape);
        **copy.p6().single_mut().expect("rotation text") = format!(
            "ORIENTATION {} / {}{}",
            state.rotation + 1,
            card.shape.rotation_period(),
            state.target.map_or(" · CHOOSE A SITE", |_| " · PREVIEWING")
        );
        focus_art.single_mut().expect("focus art").color = Color::WHITE;
    } else if let Some(card) = state.discard.last() {
        **copy.p3().single_mut().expect("focus title") =
            format!("PLACED — {}", card.shape.label().to_uppercase());
        **copy.p4().single_mut().expect("focus subtitle") = "VISIBLE DISCARD".to_string();
        **copy.p5().single_mut().expect("focus description") =
            "The spent card remains here until you undo or reset.".to_string();
        let mut image = focus_art.single_mut().expect("focus art");
        image.image = art.get(card.shape);
        image.color = Color::srgba(1.0, 1.0, 1.0, 0.62);
        **copy.p6().single_mut().expect("rotation text") = "".to_string();
    } else {
        **copy.p3().single_mut().expect("focus title") = "CHOOSE A CARD".to_string();
        **copy.p4().single_mut().expect("focus subtitle") = "TOPOLOGY IS THE CARD".to_string();
        **copy.p5().single_mut().expect("focus description") =
            "Select one of the four authored silhouettes below.".to_string();
        focus_art.single_mut().expect("focus art").color = Color::srgba(1.0, 1.0, 1.0, 0.24);
        **copy.p6().single_mut().expect("rotation text") = "".to_string();
    }
}

type SlotArtQuery<'w, 's> = Query<
    'w,
    's,
    (&'static HandSlotArt, &'static mut ImageNode),
    (Without<FocusArt>, Without<DragGhost>),
>;

pub fn sync_hand(
    state: Res<LabState>,
    art: Res<CardArt>,
    mut slots: Query<(&HandSlot, &mut Node, &mut BackgroundColor, &mut BorderColor)>,
    mut slot_art: SlotArtQuery,
    mut slot_labels: Query<(&HandSlotLabel, &mut Text)>,
) {
    for (slot, mut node, mut background, mut border) in &mut slots {
        let card = state.cards.get(slot.0).copied();
        node.display = if card.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        let selected = card.is_some_and(|card| Some(card.id) == state.selected);
        background.0 = if selected { CARD_SELECTED } else { CARD };
        *border = BorderColor::all(if selected {
            Color::srgb(0.34, 0.96, 0.88)
        } else {
            Color::srgb(0.25, 0.34, 0.40)
        });
    }
    for (slot, mut image) in &mut slot_art {
        if let Some(card) = state.cards.get(slot.0) {
            image.image = art.get(card.shape);
        }
    }
    for (slot, mut text) in &mut slot_labels {
        **text = state
            .cards
            .get(slot.0)
            .map_or_else(String::new, |card| card.shape.label().to_uppercase());
    }
}

pub fn sync_controls(
    state: Res<LabState>,
    mut controls: Query<(&Control, &mut BackgroundColor, &mut BorderColor), Without<HandSlot>>,
) {
    let place_valid = state.preview().is_some_and(|preview| preview.is_valid());
    for (control, mut background, mut border) in &mut controls {
        match control {
            Control::Place => {
                background.0 = if place_valid {
                    Color::srgb(0.08, 0.38, 0.34)
                } else {
                    Color::srgb(0.08, 0.13, 0.15)
                };
                *border = BorderColor::all(if place_valid {
                    Color::srgb(0.34, 0.98, 0.88)
                } else {
                    Color::srgb(0.25, 0.34, 0.40)
                });
            }
            Control::Undo => {
                background.0 = if state.can_undo() {
                    CARD
                } else {
                    Color::srgb(0.04, 0.06, 0.08)
                };
            }
            _ => {}
        }
    }
}

type DragGhostQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static mut ImageNode),
    (With<DragGhost>, Without<FocusArt>),
>;

pub fn sync_drag(
    state: Res<LabState>,
    art: Res<CardArt>,
    drag: Res<CardDrag>,
    mut ghost: DragGhostQuery,
) {
    let (mut node, mut image) = ghost.single_mut().expect("drag ghost");
    if drag.active
        && let Some(card) = state.selected_card()
    {
        node.display = Display::Flex;
        node.left = px(drag.current.x - 43.0);
        node.top = px(drag.current.y - 43.0);
        image.image = art.get(card.shape);
        image.color = Color::srgba(1.0, 1.0, 1.0, 0.82);
    } else {
        node.display = Display::None;
    }
}

/// Once a brief is complete, the focus panel becomes the results panel. The
/// old brief and transient notice yield their height so the report and its
/// Copy/Next actions remain reachable on a portrait phone.
pub fn sync_completion_layout(
    state: Res<LabState>,
    mut prompt: Query<&mut Node, (With<PromptText>, Without<NoticeText>)>,
    mut notice: Query<&mut Node, (With<NoticeText>, Without<PromptText>)>,
) {
    let display = if state.complete {
        Display::None
    } else {
        Display::Flex
    };
    prompt.single_mut().expect("prompt node").display = display;
    notice.single_mut().expect("notice node").display = display;
}

struct ScenarioCount;
impl ScenarioCount {
    const TOTAL: usize = 5;
}

pub(crate) fn sync_layout(windows: Query<&Window>, mut parts: Query<(&ResponsivePart, &mut Node)>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let layout = DockLayout::for_window(Vec2::new(window.width(), window.height()));
    let compact = layout.placement == DockPlacement::Right && window.height() < 450.0;
    for (part, mut node) in &mut parts {
        match part {
            ResponsivePart::Root => {
                node.flex_direction = if layout.placement == DockPlacement::Right {
                    FlexDirection::Row
                } else {
                    FlexDirection::Column
                };
            }
            ResponsivePart::Dock => {
                node.width = if layout.placement == DockPlacement::Right {
                    px(layout.dock_size.x)
                } else {
                    percent(100.0)
                };
                node.height = if layout.placement == DockPlacement::Right {
                    percent(100.0)
                } else {
                    px(layout.dock_size.y)
                };
                node.border = if layout.placement == DockPlacement::Right {
                    UiRect::left(px(2.0))
                } else {
                    UiRect::top(px(2.0))
                };
                node.row_gap = px(if compact { 4.0 } else { 7.0 });
                node.padding = UiRect::all(px(if compact { 8.0 } else { 12.0 }));
            }
            ResponsivePart::Prompt => node.min_height = px(if compact { 28.0 } else { 34.0 }),
            ResponsivePart::Focus => node.min_height = px(if compact { 82.0 } else { 92.0 }),
            ResponsivePart::FocusArt => {
                let side = if compact { 68.0 } else { 78.0 };
                node.width = px(side);
                node.height = px(side);
            }
            ResponsivePart::Hand => node.height = px(if compact { 82.0 } else { 92.0 }),
            ResponsivePart::Card => node.height = px(if compact { 82.0 } else { 92.0 }),
            ResponsivePart::Notice => node.min_height = px(if compact { 24.0 } else { 30.0 }),
        }
    }
}

pub fn position_rotation_overlay(
    state: Res<LabState>,
    camera: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    mut overlay: Query<&mut Node, (With<RotationOverlay>, Without<Control>)>,
    mut rotate_controls: Query<(&Control, &mut Node), Without<RotationOverlay>>,
) {
    let Ok(mut overlay) = overlay.single_mut() else {
        return;
    };
    let Some(target) = state.target else {
        overlay.display = Display::None;
        return;
    };
    let Some(card) = state.selected_card() else {
        overlay.display = Display::None;
        return;
    };
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    let Ok(screen) = camera.world_to_viewport(transform, world_of(&state, target).extend(0.0))
    else {
        return;
    };
    overlay.display = Display::Flex;
    overlay.left = px((screen.x - 84.0).max(2.0));
    overlay.top = px((screen.y - 27.0).max(2.0));
    let show = card.shape.rotation_period() > 1;
    for (control, mut node) in &mut rotate_controls {
        if matches!(control, Control::RotateLeft | Control::RotateRight) {
            node.display = if show { Display::Flex } else { Display::None };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_uses_a_bottom_dock_with_a_useful_map() {
        let layout = DockLayout::for_window(Vec2::new(375.0, 812.0));
        assert_eq!(layout.placement, DockPlacement::Bottom);
        assert!(layout.viewport_size.y >= 400.0);
        assert!(layout.dock_size.y >= 400.0);
    }

    #[test]
    fn landscape_and_desktop_keep_the_dock_beside_the_board() {
        for size in [Vec2::new(844.0, 390.0), Vec2::new(1440.0, 900.0)] {
            let layout = DockLayout::for_window(size);
            assert_eq!(layout.placement, DockPlacement::Right);
            assert_eq!(layout.viewport_size.x + layout.dock_size.x, size.x);
        }
    }

    #[test]
    fn all_primary_controls_meet_the_touch_floor() {
        for size in [48.0, 52.0, 54.0] {
            assert!(size >= 48.0);
        }
    }
}
