//! Live copy, card faces, selection lift, and responsive chrome layout.

use bevy::prelude::*;
use observed_style::{SchematicRole, TacticsRole};
use observed_ui::theme::{ChromeRole, chrome};

use super::{
    ArchitectButton, CARD_ART_SIZE, CardAccent, CardArtArm, CardArtCore, CardArtFrame,
    CardArtMotif, CardArtMotifKind, CardButton, CardText, CardTextField, ChargePip, DynamicText,
    HandDock, MapHeader, Sidebar, UiAction,
};
use crate::LabSession;
use crate::sim::{
    ARCHITECT_COOLDOWN_TICKS, Card, CardKind, District, MatchOutcome, ObserverState, TileShape,
};
use crate::view::{MapCameraState, WorkspaceLayout};

type SidebarFilter = (With<Sidebar>, Without<HandDock>, Without<MapHeader>);
type HandFilter = (With<HandDock>, Without<Sidebar>, Without<MapHeader>);
type HeaderFilter = (With<MapHeader>, Without<Sidebar>, Without<HandDock>);
type SidebarQuery<'w, 's> = Query<'w, 's, &'static mut Node, SidebarFilter>;
type HandQuery<'w, 's> = Query<'w, 's, &'static mut Node, HandFilter>;
type HeaderQuery<'w, 's> = Query<'w, 's, &'static mut Node, HeaderFilter>;
type CardButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CardButton,
        &'static Interaction,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        &'static mut UiTransform,
    ),
    With<Button>,
>;
type ActionButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ArchitectButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (With<Button>, Without<CardButton>),
>;
type CardArmFilter = (
    Without<CardArtCore>,
    Without<CardArtFrame>,
    Without<CardArtMotif>,
);
type CardFrameFilter = (
    Without<CardArtArm>,
    Without<CardArtCore>,
    Without<CardArtMotif>,
);
type CardMotifFilter = (
    Without<CardArtArm>,
    Without<CardArtCore>,
    Without<CardArtFrame>,
);
type CardCoreFilter = (
    Without<CardArtArm>,
    Without<CardArtFrame>,
    Without<CardArtMotif>,
);
type CardArmQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CardArtArm,
        &'static mut Visibility,
        &'static mut BackgroundColor,
    ),
    CardArmFilter,
>;
type CardFrameQuery<'w, 's> =
    Query<'w, 's, (&'static CardArtFrame, &'static mut BackgroundColor), CardFrameFilter>;
type CardMotifQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CardArtMotif,
        &'static mut Visibility,
        &'static mut BackgroundColor,
    ),
    CardMotifFilter,
>;
type CardCoreQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CardArtCore,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    CardCoreFilter,
>;

pub fn sync_action_buttons(session: Res<LabSession>, mut action_buttons: ActionButtonQuery) {
    for (button, interaction, mut background, mut border) in &mut action_buttons {
        let disabled = button.0 == UiAction::Submit
            && (session.sim.bot_architect
                || session.sim.cooldown > 0
                || session.sim.outcome != MatchOutcome::Running);
        let role = if disabled {
            ChromeRole::ControlDisabled
        } else if *interaction == Interaction::Pressed {
            ChromeRole::ControlPressed
        } else if *interaction == Interaction::Hovered {
            ChromeRole::ControlHover
        } else {
            ChromeRole::Control
        };
        background.0 = chrome(role);
        border.set_all(if button.0 == UiAction::Submit && !disabled {
            observed_style::schematic(SchematicRole::Selected).base_color
        } else {
            chrome(ChromeRole::Border)
        });
    }
}

pub fn sync_layout(
    windows: Query<&Window>,
    mut sidebars: SidebarQuery,
    mut hands: HandQuery,
    mut headers: HeaderQuery,
) {
    let (Ok(window), Ok(mut sidebar), Ok(mut hand), Ok(mut header)) = (
        windows.single(),
        sidebars.single_mut(),
        hands.single_mut(),
        headers.single_mut(),
    ) else {
        return;
    };
    let layout = WorkspaceLayout::for_window(Vec2::new(window.width(), window.height()));
    sidebar.width = px(layout.sidebar_width);
    hand.left = px(layout.sidebar_width);
    hand.height = px(layout.hand_height);
    header.left = px(layout.sidebar_width);
}

pub fn sync_dynamic_text(
    session: Res<LabSession>,
    camera: Res<MapCameraState>,
    mut dynamic: Query<(&DynamicText, &mut Text, &mut TextColor)>,
) {
    let target = session.target();
    let card = session.sim.deck.hand.get(session.selected_card).copied();
    let refusal = target
        .and_then(|target| {
            session
                .sim
                .selected_command(session.selected_card, target, session.rotation)
        })
        .and_then(|command| session.sim.refusal(command));
    for (slot, mut text, mut color) in dynamic.iter_mut() {
        let value = match slot {
            DynamicText::Mode => {
                if session.sim.bot_architect {
                    "BOT AUTOPILOT / SHARED COMMAND PATH".to_string()
                } else {
                    "HUMAN CONTROL / CONSOLE ARMED".to_string()
                }
            }
            DynamicText::Match => {
                let active = session
                    .sim
                    .observers
                    .values()
                    .filter(|observer| observer.state == ObserverState::Active)
                    .count();
                let status = match session.sim.outcome {
                    MatchOutcome::Running => "LIVE",
                    MatchOutcome::RogueVictory => "ROGUE VICTORY",
                    MatchOutcome::LoyalVictory => "LOYAL VICTORY",
                };
                let seconds = session.sim.cooldown.div_ceil(60);
                format!(
                    "{status}  /  T+{:03}\nLOYAL {active}  /  CONTRADICTIONS {}\nCOOLDOWN {seconds}s",
                    session.sim.tick / 60,
                    session.sim.contradictions.len()
                )
            }
            DynamicText::Target => target.map_or_else(
                || "NO TARGET".to_string(),
                |cell| format!("FLOOR {:02} / CELL {}, {}", cell.level + 1, cell.q, cell.r),
            ),
            DynamicText::Legality => match refusal {
                Some(reason) => format!("HELD / {}", reason.label().to_uppercase()),
                None if target.is_some() && card.is_some() => "READY TO MUTATE".to_string(),
                None => "NO COMMAND".to_string(),
            },
            DynamicText::Preview => card.map_or_else(
                || "EMPTY SLOT".to_string(),
                |card| format!("{}  /  FACE {}", card_title(card), session.rotation + 1),
            ),
            DynamicText::Message => session.last_message.to_uppercase(),
            DynamicText::Traces => session
                .sim
                .traces
                .iter()
                .map(|(role, trace)| {
                    format!(
                        "{:<11} {}",
                        role.to_uppercase(),
                        trace.selected.unwrap_or("awaiting signal").to_uppercase()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            DynamicText::HandStatus => {
                format!("{} CARDS / DRAW AFTER PLAY", session.sim.deck.hand.len())
            }
            DynamicText::Zoom => format!("{:.0}%", 100.0 / camera.zoom),
            DynamicText::Scenario => format!(
                "{}\n{}",
                session.sim.mode.short_label(),
                session.sim.mode.description()
            ),
        };
        **text = value;
        color.0 = match slot {
            DynamicText::Legality if refusal.is_some() => {
                observed_style::tactics(TacticsRole::Blocked).base_color
            }
            DynamicText::Legality => observed_style::schematic(SchematicRole::Pinned).base_color,
            DynamicText::Mode if session.sim.bot_architect => chrome(ChromeRole::TextDim),
            DynamicText::Mode => observed_style::schematic(SchematicRole::Pinned).base_color,
            _ => color.0,
        };
    }
}

pub fn sync_card_text(
    session: Res<LabSession>,
    mut card_text: Query<(&CardText, &mut Text, &mut TextColor)>,
) {
    for (label, mut text, mut color) in card_text.iter_mut() {
        let Some(card) = session.sim.deck.hand.get(label.index).copied() else {
            **text = "EMPTY".to_string();
            continue;
        };
        **text = match label.field {
            CardTextField::District => card_district(card),
            CardTextField::Title => card_title(card),
            CardTextField::Meta => card_meta(card),
        };
        color.0 = if matches!(label.field, CardTextField::District) {
            card_accent(card)
        } else if matches!(label.field, CardTextField::Title) {
            chrome(ChromeRole::TextMain)
        } else {
            chrome(ChromeRole::TextDim)
        };
    }
}

pub fn sync_card_buttons(session: Res<LabSession>, mut card_buttons: CardButtonQuery) {
    for (slot, interaction, mut node, mut background, mut border, mut transform) in
        &mut card_buttons
    {
        let selected = slot.0 == session.selected_card;
        let role = if *interaction == Interaction::Pressed {
            ChromeRole::ControlPressed
        } else if selected || *interaction == Interaction::Hovered {
            ChromeRole::ControlHover
        } else {
            ChromeRole::Control
        };
        background.0 = chrome(role);
        border.set_all(if selected {
            observed_style::schematic(SchematicRole::Selected).base_color
        } else {
            chrome(ChromeRole::Border)
        });
        let fan = slot.0 as f32 - 2.0;
        node.margin.top = px(if selected {
            0.0
        } else {
            11.0 + fan.abs() * 3.0
        });
        *transform = UiTransform {
            translation: Val2::px(0.0, if selected { -8.0 } else { 0.0 }),
            scale: if selected {
                Vec2::splat(1.045)
            } else {
                Vec2::ONE
            },
            rotation: if selected {
                Rot2::IDENTITY
            } else {
                Rot2::radians(fan * 0.026)
            },
        };
    }
}

pub fn sync_card_accents(
    session: Res<LabSession>,
    mut accents: Query<(&CardAccent, &mut BackgroundColor, Option<&mut BorderColor>)>,
) {
    for (accent, mut background, border) in &mut accents {
        let Some(card) = session.sim.deck.hand.get(accent.0).copied() else {
            continue;
        };
        let color = card_accent(card);
        background.0 = color;
        if let Some(mut border) = border {
            border.set_all(color);
        }
    }
}

pub fn sync_charge_pips(
    session: Res<LabSession>,
    mut pips: Query<(&ChargePip, &mut BackgroundColor)>,
) {
    let elapsed = ARCHITECT_COOLDOWN_TICKS.saturating_sub(session.sim.cooldown);
    let charged = if session.sim.cooldown == 0 {
        5
    } else {
        (elapsed as usize * 5) / ARCHITECT_COOLDOWN_TICKS as usize
    };
    for (pip, mut background) in &mut pips {
        background.0 = if pip.0 < charged {
            observed_style::schematic(SchematicRole::Pinned).base_color
        } else {
            observed_style::tactics(TacticsRole::DevGrid).base_color
        };
    }
}

pub fn sync_card_art(
    session: Res<LabSession>,
    mut arms: CardArmQuery,
    mut frames: CardFrameQuery,
    mut motifs: CardMotifQuery,
    mut cores: CardCoreQuery,
) {
    for (arm, mut visibility, mut background) in arms.iter_mut() {
        let Some(card) = session.sim.deck.hand.get(arm.index).copied() else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let rotation = if arm.index == session.selected_card {
            session.rotation
        } else {
            0
        };
        let mask = match card.kind {
            CardKind::Tile(shape) => shape.doors(rotation),
            CardKind::Door => (1 << rotation) | (1 << ((rotation + 3) % 6)),
        };
        *visibility = if mask & (1 << arm.face) != 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        background.0 = card_accent(card);
    }
    for (frame, mut background) in &mut frames {
        let Some(card) = session.sim.deck.hand.get(frame.0).copied() else {
            continue;
        };
        background.0 = card_accent(card).with_alpha(0.58);
    }
    for (motif, mut visibility, mut background) in &mut motifs {
        let Some(card) = session.sim.deck.hand.get(motif.index).copied() else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let visible = match motif.kind {
            CardArtMotifKind::Institutional => {
                card.kind != CardKind::Door && card.district == Some(District::Institutional)
            }
            CardArtMotifKind::LiminalGrid => {
                card.kind != CardKind::Door && card.district == Some(District::LiminalGrid)
            }
            CardArtMotifKind::Door => card.kind == CardKind::Door,
        };
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        background.0 = card_accent(card).with_alpha(0.28);
    }
    for (core, mut node, mut background, mut border) in cores.iter_mut() {
        let Some(card) = session.sim.deck.hand.get(core.0).copied() else {
            continue;
        };
        let door = card.kind == CardKind::Door;
        let size = if door {
            Vec2::new(14.0, 22.0)
        } else {
            Vec2::splat(20.0)
        };
        node.left = px(CARD_ART_SIZE.x * 0.5 - size.x * 0.5);
        node.top = px(CARD_ART_SIZE.y * 0.5 - size.y * 0.5);
        node.width = px(size.x);
        node.height = px(size.y);
        node.border_radius = if door {
            BorderRadius::all(px(3.0))
        } else {
            BorderRadius::MAX
        };
        background.0 = chrome(ChromeRole::Control);
        border.set_all(card_accent(card));
    }
}

fn card_title(card: Card) -> String {
    match card.kind {
        CardKind::Tile(TileShape::DeadEnd) => "DEAD END".to_string(),
        CardKind::Tile(TileShape::Corridor) => "CORRIDOR".to_string(),
        CardKind::Tile(TileShape::Bend) => "BEND".to_string(),
        CardKind::Tile(TileShape::Junction) => "JUNCTION".to_string(),
        CardKind::Tile(TileShape::Hall) => "HALL".to_string(),
        CardKind::Door => "DEPLOYABLE DOOR".to_string(),
    }
}

fn card_district(card: Card) -> String {
    card.district.map_or_else(
        || "TACTICAL / ANY DISTRICT".to_string(),
        |district| district.label().to_uppercase(),
    )
}

fn card_meta(card: Card) -> String {
    match card.kind {
        CardKind::Tile(shape) => {
            let branches = shape.base_doors().count_ones();
            format!(
                "{branches} BRANCH{} / TILE",
                if branches == 1 { "" } else { "ES" }
            )
        }
        CardKind::Door => "CLOSED ON DEPLOY / THRESHOLD".to_string(),
    }
}

fn card_accent(card: Card) -> Color {
    card.district.map_or_else(
        || observed_style::schematic(SchematicRole::Pinned).base_color,
        |district| observed_style::architecture_tactical(district.register()).base_color,
    )
}
