//! Screen-space UI: keyboard and pointer feed the same simulation command path.
mod cards;
mod spawn;
mod sync;
use super::MapCameraState;
use crate::{ArchitectAction, LabSession};
use bevy::prelude::*;
pub use spawn::spawn;
pub use sync::{
    sync_action_buttons, sync_card_buttons, sync_card_text, sync_charge_pips, sync_dynamic_text,
    sync_hover_note, sync_layout,
};
#[derive(Component)]
pub(crate) struct InterfaceRoot;
#[derive(Component)]
pub(crate) struct Sidebar;
#[derive(Component)]
pub(crate) struct Inspector;
#[derive(Component)]
pub(crate) struct HandDock;
#[derive(Component)]
pub(crate) struct LabControls;
/// The unstable-cell and retraction warning at the board's top-left corner.
#[derive(Component)]
pub(crate) struct HazardNotice;
/// The note beside the pointer: what the selected card can do at the hovered tile.
#[derive(Component)]
pub(crate) struct HoverNote;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HoverNoteText {
    Title,
    Reason,
}
#[derive(Component)]
pub(crate) struct ChargePip(pub usize);
#[derive(Component)]
pub(crate) struct CardButton(pub usize);
#[derive(Component)]
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
    Recenter,
    ModePrevious,
    ModeNext,
    ToggleOverlay,
    FloorPrevious,
    FloorNext,
    Overview,
    LabControls,
    Details,
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
    Guidance,
    Scenario,
    Floor,
    FloorTargets,
    Hazard,
    Pause,
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
}

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
        let Some(action) = action else { continue };
        match action.0 {
            UiAction::RotateLeft => session.apply_action(ArchitectAction::Rotate(-1)),
            UiAction::RotateRight => session.apply_action(ArchitectAction::Rotate(1)),
            UiAction::Submit => {
                if can_submit(&session, &camera) {
                    session.apply_action(ArchitectAction::Submit);
                }
            }
            UiAction::ToggleBot => session.apply_action(ArchitectAction::ToggleBot),
            UiAction::TogglePause => session.apply_action(ArchitectAction::TogglePause),
            UiAction::StepBeat => session.apply_action(ArchitectAction::StepBeat),
            UiAction::Reset => {
                session.apply_action(ArchitectAction::Reset);
                camera.reset_for_mode(session.sim.mode);
            }
            UiAction::Recenter => camera.center(),
            UiAction::ModePrevious | UiAction::ModeNext => {
                session.apply_action(ArchitectAction::CycleMode(
                    if action.0 == UiAction::ModePrevious {
                        -1
                    } else {
                        1
                    },
                ));
                camera.reset_for_mode(session.sim.mode);
            }
            UiAction::ToggleOverlay => session.apply_action(ArchitectAction::ToggleOverlay),
            UiAction::FloorPrevious | UiAction::FloorNext => camera.change_floor(
                if action.0 == UiAction::FloorPrevious {
                    -1
                } else {
                    1
                },
                session.sim.world.config.levels,
            ),
            UiAction::Overview => camera.overview = !camera.overview,
            UiAction::LabControls => {
                camera.lab_controls = !camera.lab_controls;
                camera.details = false;
            }
            UiAction::Details => {
                camera.details = !camera.details;
                camera.lab_controls = false;
            }
        }
    }
}
pub(crate) fn can_submit(session: &LabSession, camera: &MapCameraState) -> bool {
    !session.sim.bot_architect
        && !camera.details
        && !camera.lab_controls
        && session
            .target()
            .filter(|c| c.level == camera.floor)
            .and_then(|c| {
                session
                    .sim
                    .selected_command(session.selected_card, c, session.rotation)
            })
            .is_some_and(|cmd| session.sim.refusal(cmd).is_none())
}
