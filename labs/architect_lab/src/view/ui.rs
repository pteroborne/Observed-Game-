//! Screen-space command rail and the Architect's literal hand of cards.

mod cards;
mod spawn;
mod sync;

use bevy::prelude::*;

use super::MapCameraState;
use crate::{ArchitectAction, LabSession};

pub use spawn::spawn;
pub use sync::{
    sync_action_buttons, sync_card_accents, sync_card_art, sync_card_buttons, sync_card_text,
    sync_charge_pips, sync_dynamic_text, sync_layout,
};

pub(super) const CARD_ART_SIZE: Vec2 = Vec2::new(104.0, 72.0);

#[derive(Component)]
pub(crate) struct InterfaceRoot;

#[derive(Component)]
pub(crate) struct Sidebar;

#[derive(Component)]
pub(crate) struct HandDock;

#[derive(Component)]
pub(crate) struct MapHeader;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChargePip(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CardButton(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CardAccent(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArchitectButton(pub(super) UiAction);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAction {
    RotateLeft,
    RotateRight,
    Submit,
    ToggleBot,
    TogglePause,
    StepBeat,
    Reset,
    ZoomIn,
    ZoomOut,
    Recenter,
    ModePrevious,
    ModeNext,
    ToggleOverlay,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DynamicText {
    Mode,
    Match,
    Target,
    Legality,
    Preview,
    Message,
    Traces,
    HandStatus,
    Zoom,
    Scenario,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CardText {
    pub index: usize,
    pub field: CardTextField,
}

#[derive(Clone, Copy)]
pub(crate) enum CardTextField {
    District,
    Title,
    Meta,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CardArtArm {
    pub index: usize,
    pub face: u8,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CardArtFrame(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CardArtMotifKind {
    Institutional,
    LiminalGrid,
    Door,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CardArtMotif {
    pub index: usize,
    pub kind: CardArtMotifKind,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CardArtCore(pub usize);

type PressedUiQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        Option<&'static ArchitectButton>,
        Option<&'static CardButton>,
    ),
    (Changed<Interaction>, With<Button>),
>;

pub fn handle_ui_actions(
    interactions: PressedUiQuery,
    mut session: ResMut<LabSession>,
    mut camera: ResMut<MapCameraState>,
) {
    for (interaction, action, card) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(card) = card {
            session.apply_action(ArchitectAction::SelectCard(card.0));
            continue;
        }
        let Some(action) = action else {
            continue;
        };
        match action.0 {
            UiAction::RotateLeft => session.apply_action(ArchitectAction::Rotate(-1)),
            UiAction::RotateRight => session.apply_action(ArchitectAction::Rotate(1)),
            UiAction::Submit => session.apply_action(ArchitectAction::Submit),
            UiAction::ToggleBot => session.apply_action(ArchitectAction::ToggleBot),
            UiAction::TogglePause => session.apply_action(ArchitectAction::TogglePause),
            UiAction::StepBeat => session.apply_action(ArchitectAction::StepBeat),
            UiAction::Reset => {
                session.apply_action(ArchitectAction::Reset);
                camera.reset_for_mode(session.sim.mode);
            }
            UiAction::ZoomIn => camera.zoom_centered(0.86),
            UiAction::ZoomOut => camera.zoom_centered(1.16),
            UiAction::Recenter => camera.reset_for_mode(session.sim.mode),
            UiAction::ModePrevious => {
                session.apply_action(ArchitectAction::CycleMode(-1));
                camera.reset_for_mode(session.sim.mode);
            }
            UiAction::ModeNext => {
                session.apply_action(ArchitectAction::CycleMode(1));
                camera.reset_for_mode(session.sim.mode);
            }
            UiAction::ToggleOverlay => session.apply_action(ArchitectAction::ToggleOverlay),
        }
    }
}
