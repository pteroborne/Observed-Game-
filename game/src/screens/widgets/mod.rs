//! Semantic, accessible frontend widgets shared by every production screen.
//!
//! This layer owns focus and presentation only. Screen modules attach their own small
//! action components, so adding a button never expands a global business dispatcher.

mod focus;
mod presentation;

use bevy::{
    input_focus::{
        InputFocus, InputFocusVisible,
        tab_navigation::{TabGroup, TabIndex},
    },
    prelude::*,
    ui::{FocusPolicy, InteractionDisabled},
    ui_widgets::{Activate, Button as HeadlessButton, UiWidgetsPlugins},
};

pub(crate) use focus::{ActiveFocusScopes, FocusMemory};

#[derive(Component, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WidgetId {
    namespace: &'static str,
    key: u64,
}

impl WidgetId {
    pub(crate) const fn named(namespace: &'static str) -> Self {
        Self { namespace, key: 0 }
    }

    pub(crate) const fn keyed(namespace: &'static str, key: u64) -> Self {
        Self { namespace, key }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct FocusScopeId(pub(crate) &'static str);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FocusScope {
    pub(crate) id: FocusScopeId,
    pub(crate) default: WidgetId,
    pub(crate) back: Option<WidgetId>,
    pub(crate) priority: i16,
    /// Number of visual columns used by directional navigation. Tab remains linear.
    pub(crate) columns: u16,
    /// Screens may remember the last focused control; modal/destructive scopes must
    /// reopen on their safe default.
    pub(crate) restore_focus: bool,
}

impl FocusScope {
    /// Top-level scope with no semantic Back action. This keeps Escape/controller
    /// East from turning an explicit destructive choice (such as Quit) into an
    /// accidental shortcut.
    pub(crate) const fn root(id: FocusScopeId, default: WidgetId) -> Self {
        Self {
            id,
            default,
            back: None,
            priority: 0,
            columns: 1,
            restore_focus: true,
        }
    }

    pub(crate) const fn screen(id: FocusScopeId, default: WidgetId, back: WidgetId) -> Self {
        Self {
            id,
            default,
            back: Some(back),
            priority: 0,
            columns: 1,
            restore_focus: true,
        }
    }

    pub(crate) const fn grid(
        id: FocusScopeId,
        default: WidgetId,
        back: WidgetId,
        columns: u16,
    ) -> Self {
        Self {
            id,
            default,
            back: Some(back),
            priority: 0,
            columns,
            restore_focus: true,
        }
    }

    pub(crate) const fn overlay(
        id: FocusScopeId,
        default: WidgetId,
        back: WidgetId,
        priority: i16,
    ) -> Self {
        Self {
            id,
            default,
            back: Some(back),
            priority,
            columns: 1,
            restore_focus: false,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct FocusTarget {
    pub(crate) scope: FocusScopeId,
    pub(crate) order: u16,
}

#[derive(Component, Clone, Debug)]
pub(crate) struct WidgetLabel(pub(crate) String);

#[derive(Component)]
pub(crate) struct WidgetText;

#[derive(Component)]
pub(crate) struct FocusMarker;

#[derive(Resource, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiInputCapture {
    owner: Option<&'static str>,
}

impl UiInputCapture {
    pub(crate) fn capture(&mut self, owner: &'static str) {
        self.owner = Some(owner);
    }

    pub(crate) fn release(&mut self, owner: &'static str) {
        if self.owner == Some(owner) {
            self.owner = None;
        }
    }

    pub(crate) const fn is_active(&self) -> bool {
        self.owner.is_some()
    }
}

#[derive(Message, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFeedback {
    FocusMoved,
    Activated,
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
            width: 420.0,
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
            min_height: px(spec.height),
            padding: UiRect::axes(px(18), px(10)),
            margin: UiRect::vertical(px(3)),
            border: UiRect::all(px(2)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
        Outline {
            color: Color::NONE,
            width: px(0),
            offset: px(2),
        },
        Name::new(format!("Widget {}:{}", spec.id.namespace, spec.id.key)),
    ));
    if spec.disabled {
        entity.insert(InteractionDisabled);
    } else {
        entity.insert(TabIndex(i32::from(spec.order)));
    }
    entity.with_children(|button| {
        button.spawn((
            FocusMarker,
            Text::new(">"),
            TextFont {
                font_size: 22.0,
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
                font_size: 18.0,
                ..default()
            },
            TextColor(crate::view::theme::TITLE),
            TextLayout::new_with_justify(Justify::Center),
        ));
    });
    entity.id()
}

pub(crate) fn focus_scope(scope: FocusScope) -> impl Bundle {
    (scope, TabGroup::modal())
}

pub(crate) fn activation_enabled(
    activation: &On<Activate>,
    disabled: &Query<(), With<InteractionDisabled>>,
) -> bool {
    disabled.get(activation.entity).is_err()
}

pub(crate) struct FrontendWidgetsPlugin;

impl Plugin for FrontendWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiWidgetsPlugins)
            .init_resource::<InputFocus>()
            .insert_resource(InputFocusVisible(true))
            .init_resource::<FocusMemory>()
            .init_resource::<focus::GamepadNavigationLatch>()
            .init_resource::<ActiveFocusScopes>()
            .init_resource::<UiInputCapture>()
            .add_message::<UiFeedback>()
            .add_observer(focus::emit_activation_feedback)
            .add_systems(
                Update,
                (
                    (focus::sync_active_scopes, focus::initialize_focus).chain(),
                    (
                        focus::focus_hovered_widget,
                        focus::handle_focus_input,
                        focus::capture_focus_memory,
                    )
                        .chain()
                        .after(focus::initialize_focus),
                    presentation::update_button_visuals,
                    presentation::sync_widget_text,
                    presentation::sync_accessibility,
                ),
            );
    }
}
