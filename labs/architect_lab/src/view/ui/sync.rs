//! Simulation-owned legality and live labels; no invented combat statistics.
use super::{
    ArchitectButton, CardButton, CardText, CardTextField, ChargePip, DynamicText, HandDock,
    Inspector, LabControls, Sidebar, UiAction, can_submit,
};
use crate::view::{MapCameraState, WorkspaceLayout};
use crate::{
    LabSession,
    sim::{ARCHITECT_COOLDOWN_TICKS, Card, CardKind, MatchOutcome, ObserverState, TileShape},
};
use bevy::prelude::*;
use observed_style::architect::{Role, color};

type ActionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ArchitectButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        &'static mut Node,
    ),
    With<Button>,
>;
pub fn sync_action_buttons(
    session: Res<LabSession>,
    state: Res<MapCameraState>,
    mut buttons: ActionQuery,
) {
    for (button, interaction, mut background, mut border, mut node) in &mut buttons {
        let relevant = match button.0 {
            UiAction::FloorPrevious | UiAction::FloorNext => session.sim.world.config.levels > 1,
            UiAction::Overview => state.floor > 0,
            _ => true,
        };
        node.display = if relevant {
            Display::Flex
        } else {
            Display::None
        };
        let ready = button.0 == UiAction::Submit && can_submit(&session, &state);
        let active = match button.0 {
            UiAction::ToggleBot => session.sim.bot_architect,
            UiAction::TogglePause => session.paused,
            UiAction::ToggleOverlay => session.debug_overlay,
            UiAction::Overview => state.overview,
            _ => false,
        };
        background.0 = color(if *interaction != Interaction::None {
            Role::Hover
        } else {
            Role::Card
        });
        border.set_all(color(if ready || active {
            Role::Selected
        } else {
            Role::Border
        }));
        if ready {
            background.0 = color(Role::Selected).with_alpha(0.3);
        }
    }
}
type LayoutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        Option<&'static Sidebar>,
        Option<&'static Inspector>,
        Option<&'static HandDock>,
        Option<&'static LabControls>,
    ),
    Or<(
        With<Sidebar>,
        With<Inspector>,
        With<HandDock>,
        With<LabControls>,
    )>,
>;
pub fn sync_layout(
    windows: Query<&Window>,
    session: Res<LabSession>,
    state: Res<MapCameraState>,
    mut nodes: LayoutQuery,
) {
    let size = windows.single().map_or(Vec2::new(1600.0, 1000.0), |w| {
        Vec2::new(w.width(), w.height())
    });
    let layout = WorkspaceLayout::for_window(size);
    for (mut node, rail, inspector, hand, lab) in &mut nodes {
        if rail.is_some() {
            node.display = if state.details {
                Display::Flex
            } else {
                Display::None
            };
        }
        if inspector.is_some() {
            node.bottom = px(layout.hand_height + 16.0);
            node.display = if state.placement_visible(&session) {
                Display::Flex
            } else {
                Display::None
            };
        }
        if hand.is_some() {
            node.height = px(layout.hand_height);
        }
        if lab.is_some() {
            node.display = if state.lab_controls {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
pub fn sync_dynamic_text(
    session: Res<LabSession>,
    state: Res<MapCameraState>,
    mut dynamic: Query<(&DynamicText, &mut Text, &mut TextColor)>,
) {
    let target = session.target().filter(|c| c.level == state.floor);
    let card = session.sim.deck.hand.get(session.selected_card).copied();
    let refusal = target
        .and_then(|t| {
            session
                .sim
                .selected_command(session.selected_card, t, session.rotation)
        })
        .and_then(|c| session.sim.refusal(c));
    for (slot, mut text, mut tint) in &mut dynamic {
        **text = match slot {
            DynamicText::Mode => {
                if session.sim.bot_architect {
                    "Autopilot".to_string()
                } else {
                    "Rogue Architect".to_string()
                }
            }
            DynamicText::Match => match session.sim.outcome {
                MatchOutcome::RogueVictory => "ROGUE VICTORY",
                MatchOutcome::LoyalVictory => "LOYAL VICTORY",
                MatchOutcome::Running => {
                    if session.paused {
                        "Planning"
                    } else {
                        "Live"
                    }
                }
            }
            .to_string(),
            DynamicText::Pause => if session.paused {
                "RESUME [P]"
            } else {
                "PAUSE [P]"
            }
            .to_string(),
            DynamicText::Target => target.map_or_else(
                || "Choose a room on this floor".to_string(),
                |c| {
                    format!(
                        "Floor {:02} / cell {}, {}\nOrientation {} / 6",
                        c.level + 1,
                        c.q,
                        c.r,
                        session.rotation + 1
                    )
                },
            ),
            DynamicText::Legality => {
                if session.sim.bot_architect {
                    "Autopilot owns the hand".to_string()
                } else if let Some(reason) = refusal {
                    reason.label().to_string()
                } else if target.is_some() && card.is_some() {
                    "Ready to play".to_string()
                } else {
                    "Select a target".to_string()
                }
            }
            DynamicText::Preview => card.map_or_else(|| "Empty hand".to_string(), card_title),
            DynamicText::Message => {
                let mut message = session.last_message.clone();
                if session.debug_overlay
                    && let Some(cell) = target
                {
                    let landing = crate::falls::find_lower_surviving_structure(&session.sim, cell);
                    message += &landing.map_or_else(
                        || "\nFall: true void below".to_string(),
                        |c| format!("\nFall: lands on floor {}", c.level + 1),
                    );
                }
                message
            }
            DynamicText::Guidance => {
                if state.lab_controls || state.details {
                    "".to_string()
                } else if session.sim.bot_architect {
                    "Autopilot".to_string()
                } else if target.is_some() {
                    "Preview on tile".to_string()
                } else {
                    "Choose a tile".to_string()
                }
            }
            DynamicText::HandStatus => {
                if session.sim.cooldown == 0 {
                    "Charged".to_string()
                } else {
                    format!(
                        "Recharge {}s{}",
                        session.sim.cooldown.div_ceil(60),
                        if session.paused { " / paused" } else { "" }
                    )
                }
            }
            DynamicText::Scenario => format!(
                "{}\n{}",
                session.sim.mode.short_label(),
                session.sim.mode.description()
            ),
            DynamicText::Floor => format!(
                "Floor {:02} / {:02}  /  {}",
                state.floor + 1,
                session.sim.world.config.levels,
                crate::sim::floor_register(state.floor).slug()
            ),
            DynamicText::Hazard => {
                let mut s = if session.sim.contradictions.is_empty() {
                    String::new()
                } else {
                    format!("{} unstable cells", session.sim.contradictions.len())
                };
                if let Some((c, tick)) = session.sim.condemned {
                    s += &format!(
                        "\nMOVE! Floor {} ({},{})\nRetracts in {}s",
                        c.level + 1,
                        c.q,
                        c.r,
                        tick.saturating_sub(session.sim.tick).div_ceil(60)
                    );
                } else if let Some(t) = session.sim.next_retraction_tick {
                    s += &format!(
                        "\nRetraction in {}s",
                        t.saturating_sub(session.sim.tick).div_ceil(60)
                    );
                }
                if session.debug_overlay {
                    s += &format!(
                        "\nDiagnostics ON\nFloor power: {}",
                        if session.sim.economy.is_powered(state.floor) {
                            "ON"
                        } else {
                            "OFF"
                        }
                    );
                }
                s
            }
            DynamicText::Traces => {
                let known = session.sim.rogue_knowledge();
                let mut rows = Vec::new();
                for (id, o) in &session.sim.observers {
                    let location = if session.debug_overlay {
                        Some(o.cell)
                    } else {
                        known.known_observers.get(id).copied()
                    };
                    let status = match o.state {
                        ObserverState::Jailed => "Jailed".to_string(),
                        ObserverState::Corrupted => "Corrupted".to_string(),
                        ObserverState::Active => location.map_or_else(
                            || "Undetected".to_string(),
                            |c| format!("Last: floor {}", c.level + 1),
                        ),
                    };
                    rows.push(format!("EYE {:02} / {status}", id.0));
                }
                for (id, g) in &session.sim.guardians {
                    rows.push(format!("HUNTER {:02} / Floor {}", id.0, g.cell.level + 1));
                }
                if rows.len() > 4 {
                    let extra = rows.len() - 4;
                    rows.truncate(4);
                    rows.push(format!("+ {extra} other hunters"));
                }
                rows.join("\n")
            }
        };
        if matches!(slot, DynamicText::Legality) {
            tint.0 = color(if can_submit(&session, &state) {
                Role::Valid
            } else {
                Role::Muted
            });
        }
    }
}
pub fn sync_card_text(session: Res<LabSession>, mut labels: Query<(&CardText, &mut Text)>) {
    for (label, mut text) in &mut labels {
        **text = session.sim.deck.hand.get(label.index).copied().map_or_else(
            || "EMPTY".to_string(),
            |c| match label.field {
                CardTextField::Title => card_title(c),
                CardTextField::District => {
                    c.district.map_or("Any district", |d| d.label()).to_string()
                }
            },
        );
    }
}
type CardQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CardButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        &'static mut UiTransform,
    ),
    With<Button>,
>;
pub fn sync_card_buttons(session: Res<LabSession>, mut cards: CardQuery) {
    for (card, interaction, mut bg, mut border, mut transform) in &mut cards {
        let selected = card.0 == session.selected_card;
        bg.0 = color(if *interaction != Interaction::None {
            Role::Hover
        } else {
            Role::Card
        });
        border.set_all(color(if selected {
            Role::Selected
        } else {
            Role::Border
        }));
        *transform =
            UiTransform::from_translation(Val2::px(0.0, if selected { -4.0 } else { 0.0 }));
    }
}
pub fn sync_charge_pips(
    session: Res<LabSession>,
    mut pips: Query<(&ChargePip, &mut BackgroundColor)>,
) {
    let elapsed = ARCHITECT_COOLDOWN_TICKS.saturating_sub(session.sim.cooldown);
    let charged = (elapsed as usize * 5) / ARCHITECT_COOLDOWN_TICKS as usize;
    for (pip, mut background) in &mut pips {
        background.0 = color(if pip.0 < charged {
            Role::Valid
        } else {
            Role::Border
        });
    }
}
fn card_title(card: Card) -> String {
    match card.kind {
        CardKind::Tile(TileShape::DeadEnd) => "DEAD END",
        CardKind::Tile(TileShape::Corridor) => "CORRIDOR",
        CardKind::Tile(TileShape::Bend) => "BEND",
        CardKind::Tile(TileShape::Junction) => "JUNCTION",
        CardKind::Tile(TileShape::Hall) => "HALL",
        CardKind::Door => "DOOR",
    }
    .to_string()
}
