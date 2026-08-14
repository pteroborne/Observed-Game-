use std::collections::BTreeMap;

use bevy::{
    a11y::AccessibilityNode,
    ecs::system::SystemParam,
    input::gamepad::{GamepadAxis, GamepadButton},
    input_focus::{
        FocusCause, InputFocus, InputFocusVisible,
        tab_navigation::{TabGroup, TabIndex},
    },
    prelude::*,
    ui::{FocusPolicy, InteractionDisabled, Pressed},
    ui_widgets::{Activate, Button as HeadlessButton},
};

use crate::{AppState, LabSettings};

const NORMAL_BUTTON: Color = Color::srgb(0.08, 0.13, 0.20);
const HOVERED_BUTTON: Color = Color::srgb(0.12, 0.28, 0.40);
const PRESSED_BUTTON: Color = Color::srgb(0.15, 0.62, 0.74);
const DISABLED_BUTTON: Color = Color::srgb(0.055, 0.07, 0.085);
const TEXT: Color = Color::srgb(0.88, 0.94, 1.0);
const MUTED_TEXT: Color = Color::srgb(0.48, 0.54, 0.60);
const ACCENT: Color = Color::srgb(0.25, 0.85, 1.0);
const NORMAL_BORDER: Color = Color::srgba(0.25, 0.65, 0.85, 0.5);

#[derive(Component, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum WidgetId {
    MainContinueUnavailable,
    MainStart,
    MainSettings,
    MainControls,
    MainQuit,
    SettingsHighContrast,
    SettingsDiagnostics,
    SettingsBack,
    ControlsBack,
    GameplayPause,
    GameplayReset,
    PauseResume,
    PauseResetAndResume,
    PauseReturnToMain,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum FocusScopeId {
    MainMenu,
    Settings,
    Controls,
    Gameplay,
    Pause,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct FocusScope {
    id: FocusScopeId,
    default: WidgetId,
}

impl FocusScope {
    pub(crate) const fn new(id: FocusScopeId, default: WidgetId) -> Self {
        Self { id, default }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct FocusTarget {
    scope: FocusScopeId,
    order: u16,
}

#[derive(Component, Debug)]
pub(crate) struct WidgetLabel(pub(crate) String);

#[derive(Component)]
pub(crate) struct WidgetText;

#[derive(Component)]
pub(crate) struct FocusMarker;

#[derive(Resource, Debug, Default)]
pub(crate) struct FocusMemory(BTreeMap<FocusScopeId, WidgetId>);

#[derive(Resource, Debug, Default)]
pub(crate) struct GamepadNavigationLatch {
    vertical_axis: i8,
}

#[derive(Clone, Copy)]
enum FocusStep {
    Previous,
    Next,
}

pub(crate) struct WidgetSpec {
    pub(crate) id: WidgetId,
    pub(crate) scope: FocusScopeId,
    pub(crate) order: u16,
    pub(crate) label: String,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) disabled: bool,
}

impl WidgetSpec {
    pub(crate) fn enabled(
        id: WidgetId,
        scope: FocusScopeId,
        order: u16,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            scope,
            order,
            label: label.into(),
            width: 340.0,
            height: 52.0,
            disabled: false,
        }
    }

    pub(crate) fn disabled(
        id: WidgetId,
        scope: FocusScopeId,
        order: u16,
        label: impl Into<String>,
    ) -> Self {
        Self {
            disabled: true,
            ..Self::enabled(id, scope, order, label)
        }
    }

    pub(crate) const fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

pub(crate) fn spawn_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    spec: WidgetSpec,
    behavior: B,
) -> Entity {
    let label = spec.label.clone();
    let mut entity = parent.spawn((
        HeadlessButton,
        behavior,
        spec.id,
        FocusTarget {
            scope: spec.scope,
            order: spec.order,
        },
        WidgetLabel(label.clone()),
        Interaction::None,
        FocusPolicy::Block,
        Node {
            width: px(spec.width),
            height: px(spec.height),
            margin: UiRect::vertical(px(3)),
            border: UiRect::all(px(2)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(if spec.disabled {
            DISABLED_BUTTON
        } else {
            NORMAL_BUTTON
        }),
        BorderColor::all(if spec.disabled {
            MUTED_TEXT.with_alpha(0.4)
        } else {
            NORMAL_BORDER
        }),
        Outline {
            color: Color::NONE,
            width: px(0),
            offset: px(2),
        },
        Name::new(format!("Widget {:?}", spec.id)),
    ));

    if spec.disabled {
        entity.insert(InteractionDisabled);
    } else {
        entity.insert(TabIndex(spec.order as i32));
    }

    entity.with_children(|button| {
        button.spawn((
            FocusMarker,
            Text::new(">"),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                ..default()
            },
        ));
        button.spawn((
            WidgetText,
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(if spec.disabled { MUTED_TEXT } else { TEXT }),
            TextLayout::justify(Justify::Center),
        ));
    });

    entity.id()
}

pub(crate) fn focus_scope(scope: FocusScope) -> impl Bundle {
    (scope, TabGroup::modal())
}

pub(crate) fn clear_focus(mut focus: ResMut<InputFocus>) {
    focus.clear();
}

pub(crate) fn initialize_focus(
    scopes: Query<&FocusScope, Added<FocusScope>>,
    targets: Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
    memory: Res<FocusMemory>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let Some(scope) = scopes.iter().next() else {
        return;
    };

    let remembered = memory.0.get(&scope.id).copied().unwrap_or(scope.default);
    let target = targets
        .iter()
        .find(|(_, id, target)| target.scope == scope.id && **id == remembered)
        .map(|(entity, _, _)| entity)
        .or_else(|| first_target(scope.id, &targets));

    if let Some(target) = target {
        focus.set(target, FocusCause::Navigated);
        focus_visible.0 = true;
    } else {
        focus.clear();
    }
}

pub(crate) fn focus_hovered_widget(
    interactions: HoveredWidgetQuery,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    for (entity, interaction) in &interactions {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            focus.set(entity, FocusCause::Pressed);
            focus_visible.0 = true;
        }
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

#[derive(SystemParam)]
pub(crate) struct FocusNavigationContext<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    latch: ResMut<'w, GamepadNavigationLatch>,
    scopes: Query<'w, 's, &'static FocusScope>,
    targets: Query<
        'w,
        's,
        (Entity, &'static WidgetId, &'static FocusTarget),
        Without<InteractionDisabled>,
    >,
    focus: ResMut<'w, InputFocus>,
    focus_visible: ResMut<'w, InputFocusVisible>,
    commands: Commands<'w, 's>,
}

pub(crate) fn handle_focus_input(mut context: FocusNavigationContext) {
    let Some(scope) = context.scopes.iter().next() else {
        context.latch.vertical_axis = 0;
        return;
    };

    let mut previous = context.keyboard.just_pressed(KeyCode::ArrowUp)
        || (context.keyboard.just_pressed(KeyCode::Tab)
            && (context.keyboard.pressed(KeyCode::ShiftLeft)
                || context.keyboard.pressed(KeyCode::ShiftRight)));
    let mut next = context.keyboard.just_pressed(KeyCode::ArrowDown)
        || (context.keyboard.just_pressed(KeyCode::Tab)
            && !context.keyboard.pressed(KeyCode::ShiftLeft)
            && !context.keyboard.pressed(KeyCode::ShiftRight));
    let mut activate = context.keyboard.just_pressed(KeyCode::Enter)
        || context.keyboard.just_pressed(KeyCode::Space);

    let mut stick_direction = 0;
    for gamepad in &context.gamepads {
        previous |= gamepad.just_pressed(GamepadButton::DPadUp);
        next |= gamepad.just_pressed(GamepadButton::DPadDown);
        activate |= gamepad.just_pressed(GamepadButton::South);

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
        previous |= stick_direction > 0;
        next |= stick_direction < 0;
        context.latch.vertical_axis = stick_direction;
    }

    let step = match (previous, next) {
        (true, false) => Some(FocusStep::Previous),
        (false, true) => Some(FocusStep::Next),
        _ => None,
    };

    if let Some(step) = step
        && let Some(target) = adjacent_target(scope.id, context.focus.get(), step, &context.targets)
    {
        context.focus.set(target, FocusCause::Navigated);
        context.focus_visible.0 = true;
    }

    if activate
        && let Some(entity) = context.focus.get()
        && context.targets.get(entity).is_ok()
    {
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

    let Some(entity) = focus.get() else {
        return;
    };
    let Ok((id, target)) = targets.get(entity) else {
        return;
    };
    memory.0.insert(target.scope, *id);
}

type ButtonVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Interaction,
        Has<Pressed>,
        Has<InteractionDisabled>,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        &'static mut Outline,
    ),
    With<HeadlessButton>,
>;

pub(crate) fn update_button_visuals(
    settings: Res<LabSettings>,
    focus: Res<InputFocus>,
    mut buttons: ButtonVisualQuery,
    mut markers: Query<(&ChildOf, &mut Visibility), With<FocusMarker>>,
) {
    for (entity, interaction, pressed, disabled, mut background, mut border, mut outline) in
        &mut buttons
    {
        let is_focused = focus.get() == Some(entity);
        let (normal, hovered, active) = if settings.high_contrast {
            (
                Color::BLACK,
                Color::srgb(0.08, 0.20, 0.72),
                Color::srgb(0.0, 0.72, 1.0),
            )
        } else {
            (NORMAL_BUTTON, HOVERED_BUTTON, PRESSED_BUTTON)
        };

        *background = if disabled {
            DISABLED_BUTTON.into()
        } else if pressed || *interaction == Interaction::Pressed {
            active.into()
        } else if *interaction == Interaction::Hovered {
            hovered.into()
        } else {
            normal.into()
        };

        *border = if is_focused {
            BorderColor::all(Color::WHITE)
        } else if disabled {
            BorderColor::all(MUTED_TEXT.with_alpha(0.4))
        } else {
            BorderColor::all(NORMAL_BORDER)
        };
        outline.color = if is_focused { ACCENT } else { Color::NONE };
        outline.width = if is_focused { px(3) } else { px(0) };
    }

    for (parent, mut visibility) in &mut markers {
        *visibility = if focus.get() == Some(parent.parent()) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(crate) fn sync_accessibility(
    mut widgets: Query<(
        &WidgetLabel,
        Has<InteractionDisabled>,
        &mut AccessibilityNode,
    )>,
) {
    for (label, disabled, mut node) in &mut widgets {
        if node.label() != Some(label.0.as_str()) {
            node.set_label(label.0.clone());
        }
        if disabled {
            node.set_disabled();
        } else {
            node.clear_disabled();
        }
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

fn adjacent_target(
    scope: FocusScopeId,
    current: Option<Entity>,
    step: FocusStep,
    targets: &Query<(Entity, &WidgetId, &FocusTarget), Without<InteractionDisabled>>,
) -> Option<Entity> {
    let ordered = ordered_targets(scope, targets);
    if ordered.is_empty() {
        return None;
    }

    let current_index = current.and_then(|entity| {
        ordered
            .iter()
            .position(|(_, _, candidate)| *candidate == entity)
    });
    let index = match (current_index, step) {
        (Some(index), FocusStep::Previous) => index.saturating_sub(1),
        (Some(index), FocusStep::Next) => (index + 1).min(ordered.len() - 1),
        (None, FocusStep::Previous) => ordered.len() - 1,
        (None, FocusStep::Next) => 0,
    };
    Some(ordered[index].2)
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

pub(crate) fn scope_for_state(state: AppState) -> Option<FocusScopeId> {
    match state {
        AppState::MainMenu => Some(FocusScopeId::MainMenu),
        AppState::Settings => Some(FocusScopeId::Settings),
        AppState::Controls => Some(FocusScopeId::Controls),
        AppState::Gameplay => Some(FocusScopeId::Gameplay),
        AppState::Paused => Some(FocusScopeId::Pause),
        AppState::Boot | AppState::Loading => None,
    }
}

pub(crate) fn remember_activated(memory: &mut FocusMemory, state: AppState, widget: WidgetId) {
    if let Some(scope) = scope_for_state(state) {
        memory.0.insert(scope, widget);
    }
}
