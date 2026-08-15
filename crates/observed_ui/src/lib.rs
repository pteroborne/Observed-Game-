//! Semantic, accessible frontend widgets shared by every production screen and
//! by the authoring tools.
//!
//! This layer owns focus and presentation only. Consumers attach their own small
//! action components, so adding a button never expands a global business dispatcher.
//!
//! Extracted from `game/src/screens/widgets/` in Arc R once the composition
//! studio became a second consumer. It moved as-is: the modules already had no
//! dependency on game state, which is what made the extraction a file move
//! rather than a rewrite, and is the property to preserve.

pub mod focus;
pub mod presentation;
pub mod theme;

/// Whether chrome draws in its high-contrast variant.
///
/// The one thing this crate needs to know about its host's preferences, so it
/// is a resource the consumer sets rather than a settings type the crate
/// imports. `game` mirrors its own accessibility setting into this; a tool can
/// leave it absent and get the default treatment.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HighContrast(pub bool);

use bevy::{
    input_focus::{
        InputFocus, InputFocusVisible,
        tab_navigation::{TabGroup, TabIndex},
    },
    prelude::*,
    ui::{FocusPolicy, InteractionDisabled},
    ui_widgets::{Activate, Button as HeadlessButton, UiWidgetsPlugins, popover::PopoverPlugin},
};

pub use focus::{ActiveFocusScopes, FocusMemory};
pub use theme::{WidgetTreatment, WidgetVisualState, widget_treatment};

#[derive(Component, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WidgetId {
    namespace: &'static str,
    key: u64,
}

impl WidgetId {
    pub const fn named(namespace: &'static str) -> Self {
        Self { namespace, key: 0 }
    }

    pub const fn keyed(namespace: &'static str, key: u64) -> Self {
        Self { namespace, key }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FocusScopeId(pub &'static str);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusScope {
    pub id: FocusScopeId,
    pub default: WidgetId,
    pub back: Option<WidgetId>,
    pub priority: i16,
    /// Number of visual columns used by directional navigation. Tab remains linear.
    pub columns: u16,
    /// Screens may remember the last focused control; modal/destructive scopes must
    /// reopen on their safe default.
    pub restore_focus: bool,
}

impl FocusScope {
    /// Top-level scope with no semantic Back action. This keeps Escape/controller
    /// East from turning an explicit destructive choice (such as Quit) into an
    /// accidental shortcut.
    pub const fn root(id: FocusScopeId, default: WidgetId) -> Self {
        Self {
            id,
            default,
            back: None,
            priority: 0,
            columns: 1,
            restore_focus: true,
        }
    }

    pub const fn screen(id: FocusScopeId, default: WidgetId, back: WidgetId) -> Self {
        Self {
            id,
            default,
            back: Some(back),
            priority: 0,
            columns: 1,
            restore_focus: true,
        }
    }

    pub const fn grid(id: FocusScopeId, default: WidgetId, back: WidgetId, columns: u16) -> Self {
        Self {
            id,
            default,
            back: Some(back),
            priority: 0,
            columns,
            restore_focus: true,
        }
    }

    pub const fn overlay(
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
pub struct FocusTarget {
    pub scope: FocusScopeId,
    pub order: u16,
}

#[derive(Component, Clone, Debug)]
pub struct WidgetLabel(pub String);

#[derive(Component)]
pub struct WidgetText;

#[derive(Component)]
pub struct FocusMarker;

#[derive(Resource, Debug, Default, Eq, PartialEq)]
pub struct UiInputCapture {
    owner: Option<&'static str>,
}

impl UiInputCapture {
    pub fn capture(&mut self, owner: &'static str) {
        self.owner = Some(owner);
    }

    pub fn release(&mut self, owner: &'static str) {
        if self.owner == Some(owner) {
            self.owner = None;
        }
    }

    pub const fn is_active(&self) -> bool {
        self.owner.is_some()
    }
}

#[derive(Message, Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiFeedback {
    FocusMoved,
    Activated,
}

pub struct WidgetSpec {
    pub id: WidgetId,
    pub scope: FocusScopeId,
    pub order: u16,
    pub label: String,
    pub width: f32,
    pub height: f32,
    pub disabled: bool,
}

impl WidgetSpec {
    pub fn enabled(
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

    pub fn disabled(
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

    pub const fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

pub fn spawn_button<B: Bundle>(
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
                font_size: FontSize::Px(22.0),
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
            TextColor(crate::theme::LABEL),
            TextLayout::justify(Justify::Center),
        ));
    });
    entity.id()
}

pub fn focus_scope(scope: FocusScope) -> impl Bundle {
    (scope, TabGroup::modal())
}

pub fn activation_enabled(
    activation: &On<Activate>,
    disabled: &Query<(), With<InteractionDisabled>>,
) -> bool {
    disabled.get(activation.entity).is_err()
}

pub struct FrontendWidgetsPlugin;

impl Plugin for FrontendWidgetsPlugin {
    fn build(&self, app: &mut App) {
        // Bevy 0.19's `DefaultPlugins` already includes this group. Headless
        // harnesses may still omit it, so install it only when its first member
        // is absent instead of registering every widget observer twice.
        if !app.is_plugin_added::<PopoverPlugin>() {
            app.add_plugins(UiWidgetsPlugins);
        }

        app.init_resource::<InputFocus>()
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

#[cfg(test)]
mod tests {
    use super::{FrontendWidgetsPlugin, UiWidgetsPlugins};
    use bevy::prelude::*;

    #[test]
    fn frontend_widgets_coexist_with_bevys_default_widget_group() {
        let mut app = App::new();
        app.add_plugins(UiWidgetsPlugins);

        // Bevy panics immediately when a plugin is registered twice. Production
        // reaches this shape through `DefaultPlugins` followed by our frontend.
        app.add_plugins(FrontendWidgetsPlugin);
    }
}
