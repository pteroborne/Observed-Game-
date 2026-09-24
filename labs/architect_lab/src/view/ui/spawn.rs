//! A quiet board with details and placement controls revealed when needed.
use super::{
    ArchitectButton, DynamicText, HazardNotice, HoverNote, HoverNoteText, Inspector, InterfaceRoot,
    LabControls, Sidebar, UiAction,
};
use crate::view::scene::Previews;
use bevy::prelude::*;
use observed_style::architect::{Role, color};

pub fn spawn(commands: &mut Commands, camera: Entity, previews: &Previews) {
    commands.spawn((InterfaceRoot,Node {position_type:PositionType::Absolute,width:percent(100.0),height:percent(100.0),..default()},
        UiTargetCamera(camera),GlobalZIndex(100),Name::new("Rogue Architect interface"))).with_children(|root| {
        root.spawn((Node {position_type:PositionType::Absolute,left:px(0.0),right:px(0.0),top:px(0.0),height:px(66.0),
            align_items:AlignItems::Center,justify_content:JustifyContent::SpaceBetween,padding:UiRect::axes(px(24.0),px(10.0)),
            border:UiRect::bottom(px(1.0)),column_gap:px(20.0),..default()},
            BackgroundColor(color(Role::Panel)),BorderColor::all(color(Role::Border)))).with_children(|bar| {
            row(bar,|r| {label(r,"ARCHITECT",21.0,Role::Text);dynamic(r,DynamicText::Match,13.0,Role::Muted);});
            row(bar,|r| {button(r,UiAction::FloorPrevious,"<",false);
                dynamic(r,DynamicText::Floor,13.0,Role::Text);button(r,UiAction::FloorNext,">",false);
                dynamic(r,DynamicText::FloorTargets,12.0,Role::Selected);});
            row(bar,|r|{button(r,UiAction::TogglePause,"",false);button(r,UiAction::Details,"DETAILS",false);button(r,UiAction::LabControls,"LAB",false);});
        });
        root.spawn((HazardNotice,Node {position_type:PositionType::Absolute,left:px(24.0),top:px(84.0),..default()},Pickable::IGNORE)).with_children(|notice| {
            dynamic(notice,DynamicText::Hazard,14.0,Role::Guardian);
        });
        root.spawn((Sidebar,GlobalZIndex(200),Node {position_type:PositionType::Absolute,left:px(16.0),top:px(116.0),width:px(300.0),
            display:Display::None,padding:UiRect::all(px(20.0)),flex_direction:FlexDirection::Column,row_gap:px(12.0),
            border:UiRect::all(px(1.0)),border_radius:BorderRadius::all(px(6.0)),..default()},BackgroundColor(color(Role::Panel)),BorderColor::all(color(Role::Border)))).with_children(|rail| {
            // Controls first, so however long the party grows it never runs under them.
            row(rail,|r|{button(r,UiAction::Recenter,"FOCUS [F]",false);button(r,UiAction::Overview,"CONTEXT [V]",false);
                r.spawn(Node {flex_grow:1.0,..default()});button(r,UiAction::Details,"CLOSE [H]",false);});
            label(rail,"HUNTING PARTY",12.0,Role::Muted);
            dynamic(rail,DynamicText::Mode,12.0,Role::Muted);
            dynamic(rail,DynamicText::Traces,12.0,Role::Text);
            label(rail,"MAP KEY",12.0,Role::Muted);
            label(rail,"Eye: Observer sighting   /   Pyramid: Guardian\nAmber: selected   /   Cyan: watched\nViolet: prison   /   Red cross: unstable\nChevron: vertical port",12.0,Role::Muted);
            label(rail,"Drag: pan   /   Wheel: zoom\n1-5: card   /   Tab: next tile\nQ/E: rotate   /   Space: play\nPage Up/Down: floor   /   Esc: deselect",12.0,Role::Muted);
        });
        root.spawn((Inspector,Node {position_type:PositionType::Absolute,right:px(16.0),bottom:px(236.0),width:px(260.0),height:px(220.0),
            display:Display::None,flex_direction:FlexDirection::Column,row_gap:px(10.0),padding:UiRect::all(px(16.0)),
            border:UiRect::all(px(1.0)),border_radius:BorderRadius::all(px(6.0)),..default()},
            BackgroundColor(color(Role::Panel)),BorderColor::all(color(Role::Border)))).with_children(|panel| {
            dynamic(panel,DynamicText::Preview,18.0,Role::Text);
            dynamic(panel,DynamicText::Legality,13.0,Role::Valid);
            panel.spawn(Node {flex_grow:1.0,..default()});
            row(panel,|r|{button(r,UiAction::RotateLeft,"<  Q",false);label(r,"Rotate",12.0,Role::Muted);button(r,UiAction::RotateRight,"E  >",false);});
            button(panel,UiAction::Submit,"PLAY  [SPACE]",true);
        });
        super::cards::spawn_hand(root,previews);
        root.spawn((HoverNote,GlobalZIndex(300),Pickable::IGNORE,Node {position_type:PositionType::Absolute,max_width:px(280.0),
            display:Display::None,flex_direction:FlexDirection::Column,row_gap:px(4.0),padding:UiRect::new(px(12.0),px(12.0),px(9.0),px(10.0)),
            border:UiRect::new(px(3.0),px(1.0),px(1.0),px(1.0)),border_radius:BorderRadius::all(px(5.0)),..default()},
            BackgroundColor(color(Role::Panel).with_alpha(0.96)),BorderColor::all(color(Role::Border)),Name::new("Hover placement note"))).with_children(|note| {
            for (kind,size,role) in [(HoverNoteText::Title,11.0,Role::Muted),(HoverNoteText::Reason,13.0,Role::Text)] {
                note.spawn((kind,Text::new(""),TextFont {font_size:FontSize::Px(size),..default()},TextColor(color(role)),Pickable::IGNORE));
            }
        });
        root.spawn((LabControls,GlobalZIndex(200),Node {position_type:PositionType::Absolute,right:px(16.0),top:px(82.0),width:px(360.0),
            display:Display::None,flex_direction:FlexDirection::Column,padding:UiRect::all(px(20.0)),row_gap:px(15.0),
            border:UiRect::all(px(1.0)),border_radius:BorderRadius::all(px(6.0)),..default()},
            BackgroundColor(color(Role::Panel)),BorderColor::all(color(Role::Border)))).with_children(|lab| {
            label(lab,"LAB CONTROLS",18.0,Role::Text);
            dynamic(lab,DynamicText::Scenario,14.0,Role::Text);
            row(lab,|r|{button(r,UiAction::ModePrevious,"< [",false);button(r,UiAction::ModeNext,"] >",false);});
            row(lab,|r|{button(r,UiAction::ToggleBot,"BOT [B]",false);button(r,UiAction::StepBeat,"STEP [N]",false);});
            button(lab,UiAction::ToggleOverlay,"DIAGNOSTICS [O]",false);
            dynamic(lab,DynamicText::Target,12.0,Role::Muted);
            dynamic(lab,DynamicText::Message,12.0,Role::Muted);
            button(lab,UiAction::Reset,"RESET [R]",false);
            label(lab,"Diagnostics reveals all actor positions.",12.0,Role::Muted);
            button(lab,UiAction::LabControls,"CLOSE [L]",false);
        });
    });
}
pub(super) fn label(parent: &mut ChildSpawnerCommands, text: &str, size: f32, role: Role) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color(role)),
        Pickable::IGNORE,
    ));
}
fn dynamic(parent: &mut ChildSpawnerCommands, kind: DynamicText, size: f32, role: Role) {
    parent.spawn((
        kind,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color(role)),
        Pickable::IGNORE,
    ));
}
pub(super) fn row(
    parent: &mut ChildSpawnerCommands,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8.0),
            ..default()
        })
        .with_children(children);
}
fn button(parent: &mut ChildSpawnerCommands, action: UiAction, text: &str, primary: bool) {
    parent
        .spawn((
            ArchitectButton(action),
            Button,
            Node {
                min_height: px(if primary { 46.0 } else { 32.0 }),
                min_width: px(32.0),
                padding: UiRect::axes(px(9.0), px(7.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(4.0)),
                ..default()
            },
            BackgroundColor(color(Role::Card)),
            BorderColor::all(color(Role::Border)),
            Name::new(if primary {
                "Execute selected card".to_string()
            } else {
                format!("Architect control {text}")
            }),
        ))
        .with_children(|b| {
            if action == UiAction::TogglePause {
                dynamic(b, DynamicText::Pause, 13.0, Role::Text);
            } else {
                label(b, text, if primary { 13.0 } else { 12.0 }, Role::Text);
            }
        });
}
