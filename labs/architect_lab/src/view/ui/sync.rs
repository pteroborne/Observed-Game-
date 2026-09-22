//! Simulation-owned legality and live labels; no invented combat statistics.
use super::{
    ArchitectButton, CardButton, CardText, CardTextField, ChargePip, DynamicText, HandDock,
    HoverNote, HoverNoteText, Inspector, LabControls, Sidebar, UiAction, can_submit,
};
use crate::view::{MapCameraState, PlacementSurvey, WorkspaceLayout};
use crate::{
    LabSession,
    placement::{refusal_tally, refusals_on_level},
    sim::{
        ARCHITECT_COOLDOWN_TICKS, Card, CardKind, CommandRefusal, MatchOutcome, ObserverState,
        TileShape,
    },
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
    survey: Res<PlacementSurvey>,
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
                } else if card.is_some() {
                    floor_guidance(&survey, state.floor)
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
            DynamicText::FloorTargets => {
                if card.is_none() || session.sim.bot_architect {
                    String::new()
                } else {
                    floor_targets(&survey, state.floor)
                }
            }
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
/// "12 targets" here, plus any other floor the card can reach, so an empty deck is
/// never a mystery: "0 here  /  02: 7".
fn floor_targets(survey: &PlacementSurvey, floor: u8) -> String {
    let here = survey
        .by_level
        .get(usize::from(floor))
        .copied()
        .unwrap_or(0);
    let mut text = format!("{here} target{}", if here == 1 { "" } else { "s" });
    let elsewhere: Vec<_> = survey
        .by_level
        .iter()
        .enumerate()
        .filter(|&(level, &count)| level != usize::from(floor) && count > 0)
        .map(|(level, count)| format!("{:02}: {count}", level + 1))
        .collect();
    if !elsewhere.is_empty() {
        text += &format!("   /   {}", elsewhere.join("  "));
    }
    text
}
/// The hand header before a tile is chosen: why the board is as dark as it is.
fn floor_guidance(survey: &PlacementSurvey, floor: u8) -> String {
    let here = survey
        .by_level
        .get(usize::from(floor))
        .copied()
        .unwrap_or(0);
    let lead = if here == 0 {
        "Nothing fits on this floor".to_string()
    } else {
        "Choose a tile".to_string()
    };
    let reasons: Vec<_> = refusals_on_level(&survey.verdicts, floor)
        .into_iter()
        .take(3)
        .map(|(refusal, count)| format!("{count} {}", refusal_tally(refusal)))
        .collect();
    if reasons.is_empty() {
        lead
    } else {
        format!("{lead}   /   locked: {}", reasons.join(", "))
    }
}
type HoverNoteQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut BorderColor,
        &'static ComputedNode,
    ),
    With<HoverNote>,
>;
pub fn sync_hover_note(
    windows: Query<&Window>,
    session: Res<LabSession>,
    state: Res<MapCameraState>,
    survey: Res<PlacementSurvey>,
    mut notes: HoverNoteQuery,
    mut texts: Query<(&HoverNoteText, &mut Text, &mut TextColor)>,
) {
    let Ok((mut node, mut border, computed)) = notes.single_mut() else {
        return;
    };
    let window = windows.single().ok();
    let card = session.sim.deck.hand.get(session.selected_card).copied();
    let shown = window
        .and_then(Window::cursor_position)
        .zip(card)
        .and_then(|(cursor, card)| {
            let cell = session.hovered_target.filter(|c| c.level == state.floor)?;
            let verdict = survey.verdict(cell)?;
            (!state.details && !state.lab_controls && !session.sim.bot_architect)
                .then_some((cursor, card, verdict))
        });
    let Some((cursor, card, verdict)) = shown else {
        node.display = Display::None;
        return;
    };
    let (title, reason, role) = match verdict.refusal {
        None => (
            format!("{} FITS HERE", card_title(card)),
            format!(
                "{} of 6 rotations connect. Click to preview.",
                verdict.rotations.len()
            ),
            Role::Selected,
        ),
        Some(refusal) => (
            format!("{} IS LOCKED OUT", card_title(card)),
            capitalised(refusal.label()),
            refusal_role(refusal),
        ),
    };
    for (kind, mut text, mut tint) in &mut texts {
        match kind {
            HoverNoteText::Title => {
                **text = title.clone();
                tint.0 = color(role);
            }
            HoverNoteText::Reason => **text = reason.clone(),
        }
    }
    border.left = color(role);
    // Beside the pointer, flipped left or up near the window edge. The measured size
    // lags a frame behind new text, which is invisible at pointer speed.
    let size = computed.size() * computed.inverse_scale_factor();
    let bounds = window.map_or(Vec2::splat(f32::MAX), |w| Vec2::new(w.width(), w.height()));
    let mut at = cursor + Vec2::new(18.0, 16.0);
    if at.x + size.x > bounds.x - 8.0 {
        at.x = cursor.x - size.x - 14.0;
    }
    if at.y + size.y > bounds.y - 8.0 {
        at.y = cursor.y - size.y - 12.0;
    }
    node.left = px(at.x.max(8.0));
    node.top = px(at.y.max(8.0));
    node.display = Display::Flex;
}
fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect::<String>() + "."
    })
}
/// The hue of whatever is holding the tile: cyan for sight, violet for the prison.
fn refusal_role(refusal: CommandRefusal) -> Role {
    match refusal {
        CommandRefusal::Observed => Role::Observer,
        CommandRefusal::PrisonCore | CommandRefusal::Anchored => Role::Prison,
        CommandRefusal::Occupied => Role::Guardian,
        _ => Role::Muted,
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
