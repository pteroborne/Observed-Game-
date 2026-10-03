//! Match modals render after every world/desk camera. A root on the default world
//! camera can have a high UI z-index and still be erased by the Architect's later
//! opaque board pass. Explicit targets keep help and pause visible in every role.

use bevy::{camera::visibility::RenderLayers, prelude::*, ui::UiTargetCamera};

use super::overlay::OverlayRoot;
use crate::GameState;
use crate::screens::onboarding::OnboardingPanel;

#[derive(Component)]
pub(super) struct ModalUiCamera;

pub(super) fn setup(mut commands: Commands) {
    commands.spawn((
        ModalUiCamera,
        DespawnOnExit(GameState::HexWfc),
        Camera2d,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::none(),
        Name::new("Hex match modal UI camera"),
    ));
}

type ModalRoots<'w, 's> = Query<
    'w,
    's,
    (Entity, Option<&'static UiTargetCamera>),
    Or<(With<OnboardingPanel>, With<OverlayRoot>)>,
>;

pub(super) fn target_roots(
    mut commands: Commands,
    mut camera: Query<(Entity, &mut Camera), With<ModalUiCamera>>,
    roots: ModalRoots,
) {
    let Ok((camera, mut pass)) = camera.single_mut() else {
        return;
    };
    let active = !roots.is_empty();
    if pass.is_active != active {
        pass.is_active = active;
    }
    for (root, target) in &roots {
        if target.is_none_or(|target| target.0 != camera) {
            commands.entity(root).insert(UiTargetCamera(camera));
        }
    }
}
