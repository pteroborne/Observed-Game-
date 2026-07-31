use std::collections::BTreeMap;

use bevy::{
    ecs::system::SystemParam,
    input::gamepad::{GamepadAxis, GamepadButton},
    input_focus::{InputFocus, InputFocusVisible},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::{Activate, Button as HeadlessButton},
};

use super::{FocusScope, FocusScopeId, FocusTarget, UiFeedback, UiInputCapture, WidgetId};

#[derive(Resource, Debug, Default)]
pub(crate) struct FocusMemory(pub(crate) BTreeMap<FocusScopeId, WidgetId>);

#[derive(Resource, Debug, Default)]
pub(crate) struct GamepadNavigationLatch {
    vertical_axis: i8,
    horizontal_axis: i8,
}

#[derive(Resource, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActiveFocusScopes {
    ordered: Vec<(i16, FocusScopeId)>,
}

impl ActiveFocusScopes {
    pub(crate) fn active(&self) -> Option<FocusScopeId> {
        self.ordered.last().map(|(_, id)| *id)
    }
}

#[derive(Clone, Copy)]
enum FocusStep {
    Previous,
    Next,
    Up,
    Down,
    Left,
    Right,
}

pub(crate) fn sync_active_scopes(
    scopes: Query<&FocusScope>,
    mut active: ResMut<ActiveFocusScopes>,
) {
    let mut ordered = scopes
        .iter()
        .map(|scope| (scope.priority, scope.id))
        .collect::<Vec<_>>();
    ordered.sort_unstable();
    ordered.dedup();
    if active.ordered != ordered {
        active.ordered = ordered;
    }
}

pub(crate) fn initialize_focus(
    active: Res<ActiveFocusScopes>,
    scopes: Query<&FocusScope>,
    targets: Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
    memory: Res<FocusMemory>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    if !active.is_changed() {
        return;
    }
    let Some(active_id) = active.active() else {
        focus.clear();
        return;
    };
    let Some(scope) = scopes.iter().find(|scope| scope.id == active_id) else {
        focus.clear();
        return;
    };
    let remembered = if scope.restore_focus {
        memory.0.get(&active_id).copied().unwrap_or(scope.default)
    } else {
        scope.default
    };
    let target = targets
        .iter()
        .find(|(_, id, target)| target.scope == active_id && **id == remembered)
        .map(|(entity, _, _)| entity)
        .or_else(|| first_target(active_id, &targets));
    if let Some(entity) = target {
        focus.set(entity);
        focus_visible.0 = true;
    } else {
        focus.clear();
    }
}

type HoveredWidgetQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Interaction),
    (
        Changed<Interaction>,
        With<HeadlessButton>,
        Without<InteractionDisabled>,
    ),
>;

pub(crate) fn focus_hovered_widget(
    interactions: HoveredWidgetQuery,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut feedback: MessageWriter<UiFeedback>,
) {
    for (entity, interaction) in &interactions {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && focus.0 != Some(entity)
        {
            focus.set(entity);
            focus_visible.0 = true;
            feedback.write(UiFeedback::FocusMoved);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct FocusNavigationContext<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    latch: ResMut<'w, GamepadNavigationLatch>,
    active: Res<'w, ActiveFocusScopes>,
    scopes: Query<'w, 's, &'static FocusScope>,
    targets: Query<
        'w,
        's,
        (Entity, &'static WidgetId, &'static FocusTarget),
        Without<InteractionDisabled>,
    >,
    capture: Res<'w, UiInputCapture>,
    focus: ResMut<'w, InputFocus>,
    focus_visible: ResMut<'w, InputFocusVisible>,
    feedback: MessageWriter<'w, UiFeedback>,
    commands: Commands<'w, 's>,
}

pub(crate) fn handle_focus_input(mut context: FocusNavigationContext) {
    if context.capture.is_active() {
        context.latch.vertical_axis = 0;
        context.latch.horizontal_axis = 0;
        return;
    }
    let Some(scope_id) = context.active.active() else {
        context.latch.vertical_axis = 0;
        context.latch.horizontal_axis = 0;
        return;
    };
    let Some(scope) = context.scopes.iter().find(|scope| scope.id == scope_id) else {
        return;
    };

    let shift = context.keyboard.pressed(KeyCode::ShiftLeft)
        || context.keyboard.pressed(KeyCode::ShiftRight);
    let mut previous = context.keyboard.just_pressed(KeyCode::Tab) && shift;
    let mut next = context.keyboard.just_pressed(KeyCode::Tab) && !shift;
    let mut up = context.keyboard.just_pressed(KeyCode::ArrowUp);
    let mut down = context.keyboard.just_pressed(KeyCode::ArrowDown);
    let mut left = context.keyboard.just_pressed(KeyCode::ArrowLeft);
    let mut right = context.keyboard.just_pressed(KeyCode::ArrowRight);
    let mut activate = context.keyboard.just_pressed(KeyCode::Enter)
        || context.keyboard.just_pressed(KeyCode::Space);
    let mut back = context.keyboard.just_pressed(KeyCode::Escape);

    let mut stick_direction = 0;
    for gamepad in &context.gamepads {
        up |= gamepad.just_pressed(GamepadButton::DPadUp);
        down |= gamepad.just_pressed(GamepadButton::DPadDown);
        left |= gamepad.just_pressed(GamepadButton::DPadLeft);
        right |= gamepad.just_pressed(GamepadButton::DPadRight);
        activate |= gamepad.just_pressed(GamepadButton::South);
        back |= gamepad.just_pressed(GamepadButton::East);
        let vertical = gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
        if vertical > 0.55 {
            stick_direction = 1;
        } else if vertical < -0.55 {
            stick_direction = -1;
        }
    }
    if stick_direction == 0 {
        context.latch.vertical_axis = 0;
    } else if stick_direction != context.latch.vertical_axis {
        up |= stick_direction > 0;
        down |= stick_direction < 0;
        context.latch.vertical_axis = stick_direction;
    }

    let mut horizontal_stick = 0;
    for gamepad in &context.gamepads {
        let horizontal = gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        if horizontal > 0.55 {
            horizontal_stick = 1;
        } else if horizontal < -0.55 {
            horizontal_stick = -1;
        }
    }
    if horizontal_stick == 0 {
        context.latch.horizontal_axis = 0;
    } else if horizontal_stick != context.latch.horizontal_axis {
        right |= horizontal_stick > 0;
        left |= horizontal_stick < 0;
        context.latch.horizontal_axis = horizontal_stick;
    }

    if scope.columns <= 1 {
        previous |= up;
        next |= down;
        up = false;
        down = false;
        left = false;
        right = false;
    }

    let step = match (previous, next, up, down, left, right) {
        (true, false, false, false, false, false) => Some(FocusStep::Previous),
        (false, true, false, false, false, false) => Some(FocusStep::Next),
        (false, false, true, false, false, false) => Some(FocusStep::Up),
        (false, false, false, true, false, false) => Some(FocusStep::Down),
        (false, false, false, false, true, false) => Some(FocusStep::Left),
        (false, false, false, false, false, true) => Some(FocusStep::Right),
        _ => None,
    };
    if let Some(step) = step
        && let Some(target) = adjacent_target(
            scope_id,
            context.focus.0,
            step,
            scope.columns,
            &context.targets,
        )
        && context.focus.0 != Some(target)
    {
        context.focus.set(target);
        context.focus_visible.0 = true;
        context.feedback.write(UiFeedback::FocusMoved);
    }

    let requested = if back {
        scope
            .back
            .and_then(|id| target_for_id(scope_id, id, &context.targets))
    } else if activate {
        context
            .focus
            .0
            .filter(|entity| context.targets.get(*entity).is_ok())
    } else {
        None
    };
    if let Some(entity) = requested {
        context.commands.trigger(Activate { entity });
    }
}

pub(crate) fn capture_focus_memory(
    focus: Res<InputFocus>,
    targets: Query<(&WidgetId, &FocusTarget), Without<InteractionDisabled>>,
    mut memory: ResMut<FocusMemory>,
) {
    if !focus.is_changed() {
        return;
    }
    let Some(entity) = focus.0 else {
        return;
    };
    let Ok((id, target)) = targets.get(entity) else {
        return;
    };
    memory.0.insert(target.scope, *id);
}

pub(crate) fn emit_activation_feedback(
    activation: On<Activate>,
    disabled: Query<(), With<InteractionDisabled>>,
    capture: Res<UiInputCapture>,
    mut feedback: MessageWriter<UiFeedback>,
) {
    if !capture.is_active() && disabled.get(activation.entity).is_err() {
        feedback.write(UiFeedback::Activated);
    }
}

fn first_target(
    scope: FocusScopeId,
    targets: &Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
) -> Option<Entity> {
    ordered_targets(scope, targets)
        .first()
        .map(|(_, _, entity)| *entity)
}

fn target_for_id(
    scope: FocusScopeId,
    id: WidgetId,
    targets: &Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
) -> Option<Entity> {
    targets
        .iter()
        .find(|(_, candidate, target)| target.scope == scope && **candidate == id)
        .map(|(entity, _, _)| entity)
}

fn adjacent_target(
    scope: FocusScopeId,
    current: Option<Entity>,
    step: FocusStep,
    columns: u16,
    targets: &Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
) -> Option<Entity> {
    let ordered = ordered_targets(scope, targets);
    if ordered.is_empty() {
        return None;
    }
    let current = current.and_then(|entity| {
        ordered
            .iter()
            .position(|(_, _, candidate)| *candidate == entity)
    });
    if matches!(
        step,
        FocusStep::Up | FocusStep::Down | FocusStep::Left | FocusStep::Right
    ) {
        let Some(current) = current else {
            return Some(ordered[0].2);
        };
        return spatial_target(&ordered, current, step, columns.max(1));
    }
    let index = match (current, step) {
        (Some(0), FocusStep::Previous) | (None, FocusStep::Previous) => ordered.len() - 1,
        (Some(index), FocusStep::Previous) => index - 1,
        (Some(index), FocusStep::Next) => (index + 1) % ordered.len(),
        (None, FocusStep::Next) => 0,
        _ => unreachable!("spatial focus steps return above"),
    };
    Some(ordered[index].2)
}

fn spatial_target(
    ordered: &[(u16, WidgetId, Entity)],
    current_index: usize,
    step: FocusStep,
    columns: u16,
) -> Option<Entity> {
    let current_order = ordered[current_index].0;
    let row = current_order / columns;
    let column = current_order % columns;
    let candidate = match step {
        FocusStep::Up => ordered
            .iter()
            .filter(|(order, _, _)| *order / columns < row && *order % columns == column)
            .max_by_key(|(order, _, _)| *order),
        FocusStep::Down => ordered
            .iter()
            .filter(|(order, _, _)| *order / columns > row && *order % columns == column)
            .min_by_key(|(order, _, _)| *order)
            .or_else(|| ordered.iter().find(|(order, _, _)| *order > current_order)),
        FocusStep::Left => ordered
            .iter()
            .filter(|(order, _, _)| *order / columns == row && *order % columns < column)
            .max_by_key(|(order, _, _)| *order),
        FocusStep::Right => ordered
            .iter()
            .filter(|(order, _, _)| *order / columns == row && *order % columns > column)
            .min_by_key(|(order, _, _)| *order),
        FocusStep::Previous | FocusStep::Next => None,
    };
    candidate.map(|(_, _, entity)| *entity)
}

fn ordered_targets(
    scope: FocusScopeId,
    targets: &Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
) -> Vec<(u16, WidgetId, Entity)> {
    let mut ordered = targets
        .iter()
        .filter(|(_, _, target)| target.scope == scope)
        .map(|(entity, id, target)| (target.order, *id, entity))
        .collect::<Vec<_>>();
    ordered.sort_unstable();
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_scope_is_the_highest_priority_scope() {
        let active = ActiveFocusScopes {
            ordered: vec![
                (0, FocusScopeId("screen")),
                (100, FocusScopeId("pause")),
                (200, FocusScopeId("confirmation")),
            ],
        };
        assert_eq!(active.active(), Some(FocusScopeId("confirmation")));
    }

    #[test]
    fn two_column_directional_navigation_matches_the_visual_grid() {
        let mut world = World::new();
        let entities = (0..5).map(|_| world.spawn_empty().id()).collect::<Vec<_>>();
        let ordered = entities
            .iter()
            .enumerate()
            .map(|(order, entity)| (order as u16, WidgetId::keyed("grid", order as u64), *entity))
            .collect::<Vec<_>>();

        assert_eq!(
            spatial_target(&ordered, 0, FocusStep::Right, 2),
            Some(entities[1])
        );
        assert_eq!(
            spatial_target(&ordered, 0, FocusStep::Down, 2),
            Some(entities[2])
        );
        assert_eq!(
            spatial_target(&ordered, 3, FocusStep::Down, 2),
            Some(entities[4]),
            "an incomplete final row remains reachable"
        );
    }

    #[test]
    fn modal_scopes_reopen_on_their_safe_default() {
        let modal = FocusScope::overlay(
            FocusScopeId("confirm"),
            WidgetId::named("cancel"),
            WidgetId::named("cancel"),
            200,
        );
        assert!(!modal.restore_focus);
    }
}
