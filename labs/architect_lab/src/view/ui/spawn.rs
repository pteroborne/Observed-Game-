//! Static composition of the Architect command surface.

use bevy::prelude::*;
use observed_style::{SchematicRole, TacticsRole};
use observed_ui::theme::{ChromeRole, chrome};

use super::cards::spawn_hand;
use super::{ArchitectButton, ChargePip, DynamicText, InterfaceRoot, MapHeader, Sidebar, UiAction};

pub fn spawn(commands: &mut Commands, camera: Entity) {
    commands
        .spawn((
            InterfaceRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(100),
            Name::new("Rogue Architect interface"),
        ))
        .with_children(|root| {
            spawn_sidebar(root);
            spawn_map_header(root);
            spawn_hand(root);
        });
}

fn spawn_sidebar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Sidebar,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: px(304.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(px(18.0), px(14.0)),
            border: UiRect::right(px(2.0)),
            row_gap: px(9.0),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Surface)),
        BorderColor::all(chrome(ChromeRole::Border)),
        Name::new("Architect command rail"),
    ))
    .with_children(|rail| {
        rail.spawn((
            Text::new("ROGUE SYSTEM / HUNT CONTROL"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextDim)),
        ));
        rail.spawn((
            Text::new("ARCHITECT"),
            TextFont {
                font_size: FontSize::Px(30.0),
                ..default()
            },
            TextColor(observed_style::schematic(SchematicRole::Selected).base_color),
        ));
        rail.spawn((
            DynamicText::Mode,
            Text::new("HUMAN CONTROL"),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(observed_style::schematic(SchematicRole::Pinned).base_color),
        ));
        spawn_rule(rail);
        spawn_label(rail, "HUNT STATUS");
        spawn_panel(rail, |panel| {
            panel.spawn((
                DynamicText::Match,
                Text::new("LIVE"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
            ));
        });
        spawn_charge_meter(rail);
        spawn_label(rail, "TARGET TILE");
        spawn_panel(rail, |panel| {
            panel.spawn((
                DynamicText::Target,
                Text::new("FLOOR 01 / CELL 0, 0"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
            ));
            panel.spawn((
                DynamicText::Legality,
                Text::new("READY TO MUTATE"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(observed_style::schematic(SchematicRole::Pinned).base_color),
            ));
            panel.spawn((
                DynamicText::Preview,
                Text::new("JUNCTION / FACE 1"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextDim)),
            ));
        });
        rail.spawn((
            Node {
                width: percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(8.0),
                ..default()
            },
            Name::new("Rotation controls"),
        ))
        .with_children(|row| {
            spawn_button(row, UiAction::RotateLeft, "<  TURN");
            spawn_button(row, UiAction::RotateRight, "TURN  >");
        });
        spawn_primary_button(rail);
        rail.spawn((
            DynamicText::Message,
            Text::new("Choose a card and target."),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextDim)),
            Node {
                min_height: px(38.0),
                ..default()
            },
        ));
        spawn_rule(rail);
        spawn_label(rail, "THE HUNTING PARTY");
        rail.spawn((
            DynamicText::Traces,
            Text::new("Trees have not evaluated yet."),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextMain)),
            Node {
                min_height: px(66.0),
                ..default()
            },
        ));
        rail.spawn((
            Node {
                width: percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6.0),
                row_gap: px(6.0),
                ..default()
            },
            Name::new("Simulation controls"),
        ))
        .with_children(|row| {
            spawn_small_button(row, UiAction::ToggleBot, "BOT [B]");
            spawn_small_button(row, UiAction::TogglePause, "PAUSE [P]");
            spawn_small_button(row, UiAction::StepBeat, "STEP [N]");
            spawn_small_button(row, UiAction::ToggleOverlay, "OVERLAY [O]");
            spawn_small_button(row, UiAction::Reset, "RESET [R]");
        });
        spawn_rule(rail);
        spawn_label(rail, "LEGIBILITY CONTRACT / KEY");
        rail.spawn((
            Text::new(
                "FLOOR PALETTES\n\
                • F01 Institutional   • F02 Liminal Grid\n\
                • F03 Wellshaft       • F04 Facet Monument\n\
                • F05 Megastructure\n\n\
                ACTORS & CELLS\n\
                ◉ Observer [Prey]   ▲ Guardian [Hunter]\n\
                • Blue: Watched     • Amber: Prison Core\n\
                • Red: Contradiction / Closed Door\n\n\
                DEBUG OVERLAY [O]\n\
                • v F#: Safe drop   • X VOID: Fatal drop\n\
                • RETRACT: Timer    • [PWR]: Generator",
            ),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextDim)),
        ));
    });
}

fn spawn_charge_meter(rail: &mut ChildSpawnerCommands) {
    spawn_label(rail, "MUTATION CHARGE");
    rail.spawn((
        Node {
            width: percent(100.0),
            height: px(12.0),
            flex_direction: FlexDirection::Row,
            column_gap: px(4.0),
            ..default()
        },
        Name::new("Five segment mutation charge"),
    ))
    .with_children(|meter| {
        for index in 0..5 {
            meter.spawn((
                ChargePip(index),
                Node {
                    height: percent(100.0),
                    flex_grow: 1.0,
                    border_radius: BorderRadius::all(px(6.0)),
                    ..default()
                },
                BackgroundColor(observed_style::tactics(TacticsRole::DevGrid).base_color),
                Pickable::IGNORE,
            ));
        }
    });
}

fn spawn_map_header(root: &mut ChildSpawnerCommands) {
    root.spawn((
        MapHeader,
        Node {
            position_type: PositionType::Absolute,
            left: px(304.0),
            right: px(0.0),
            top: px(0.0),
            height: px(64.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(px(18.0), px(8.0)),
            border: UiRect::bottom(px(1.0)),
            column_gap: px(10.0),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Surface).with_alpha(0.9)),
        BorderColor::all(chrome(ChromeRole::Border)),
        Name::new("Facility map toolbar"),
    ))
    .with_children(|bar| {
        bar.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                min_width: px(190.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|copy| {
            copy.spawn((
                Text::new("THE HUNTING BOARD"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
            ));
            copy.spawn((
                Text::new("CLICK TARGET / DRAG PAN / WHEEL ZOOM"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextDim)),
            ));
        });
        spawn_mode_picker(bar);
        bar.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                ..default()
            },
            Name::new("Map zoom controls"),
        ))
        .with_children(|controls| {
            spawn_small_button(controls, UiAction::ToggleOverlay, "OVERLAY [O]");
            spawn_small_button(controls, UiAction::ZoomOut, "- OUT");
            controls.spawn((
                DynamicText::Zoom,
                Text::new("100%"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
                Node {
                    min_width: px(48.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ));
            spawn_small_button(controls, UiAction::ZoomIn, "IN +");
            spawn_small_button(controls, UiAction::Recenter, "CENTER [F]");
        });
    });
}

fn spawn_mode_picker(bar: &mut ChildSpawnerCommands) {
    bar.spawn((
        Node {
            height: px(48.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(7.0),
            padding: UiRect::axes(px(7.0), px(4.0)),
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(12.0)),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Control)),
        BorderColor::all(observed_style::schematic(SchematicRole::Selected).base_color),
        Name::new("Scenario carousel"),
    ))
    .with_children(|picker| {
        spawn_small_button(picker, UiAction::ModePrevious, "< [");
        picker.spawn((
            DynamicText::Scenario,
            Text::new("POCKET\nONE FLOOR / 30 CELLS"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(observed_style::schematic(SchematicRole::Selected).base_color),
            TextLayout::justify(Justify::Center),
            Node {
                min_width: px(174.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
        spawn_small_button(picker, UiAction::ModeNext, "] >");
    });
}

fn spawn_label(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        Text::new(label.to_string()),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(chrome(ChromeRole::TextDim)),
        Pickable::IGNORE,
    ));
}

fn spawn_rule(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: percent(100.0),
            height: px(1.0),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Border)),
        Pickable::IGNORE,
    ));
}

fn spawn_panel(
    parent: &mut ChildSpawnerCommands,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(8.0)),
                row_gap: px(4.0),
                ..default()
            },
            BackgroundColor(chrome(ChromeRole::Control)),
            BorderColor::all(chrome(ChromeRole::Border)),
        ))
        .with_children(children);
}

fn spawn_button(parent: &mut ChildSpawnerCommands, action: UiAction, label: &str) {
    spawn_sized_button(parent, action, label, 43.0, 12.0, 1.0);
}

fn spawn_small_button(parent: &mut ChildSpawnerCommands, action: UiAction, label: &str) {
    spawn_sized_button(parent, action, label, 32.0, 10.0, 0.0);
}

fn spawn_sized_button(
    parent: &mut ChildSpawnerCommands,
    action: UiAction,
    label: &str,
    height: f32,
    font_size: f32,
    grow: f32,
) {
    parent
        .spawn((
            ArchitectButton(action),
            Button,
            Node {
                height: px(height),
                min_width: px(height),
                flex_grow: grow,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(9.0), px(5.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(height * 0.4)),
                ..default()
            },
            BackgroundColor(chrome(ChromeRole::Control)),
            BorderColor::all(chrome(ChromeRole::Border)),
            Name::new(format!("Architect control {label}")),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(font_size),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_primary_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            ArchitectButton(UiAction::Submit),
            Button,
            Node {
                width: percent(100.0),
                height: px(52.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::all(px(18.0)),
                ..default()
            },
            BackgroundColor(chrome(ChromeRole::ControlHover)),
            BorderColor::all(observed_style::schematic(SchematicRole::Selected).base_color),
            Name::new("Execute selected card"),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("PLAY THIS CARD  [SPACE]"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextMain)),
                Pickable::IGNORE,
            ));
        });
}
