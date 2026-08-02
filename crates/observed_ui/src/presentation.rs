use bevy::{
    a11y::AccessibilityNode,
    input_focus::InputFocus,
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::Button as HeadlessButton,
};

use super::{FocusMarker, WidgetLabel, WidgetText};

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

pub fn update_button_visuals(
    contrast: Option<Res<crate::HighContrast>>,
    focus: Res<InputFocus>,
    mut buttons: ButtonVisualQuery,
    mut markers: Query<(&ChildOf, &mut Visibility), With<FocusMarker>>,
) {
    let high_contrast = contrast.is_some_and(|contrast| contrast.0);
    for (entity, interaction, pressed, disabled, mut background, mut border, mut outline) in
        &mut buttons
    {
        let treatment = crate::theme::widget_treatment(crate::theme::WidgetVisualState {
            focused: focus.0 == Some(entity),
            hovered: *interaction == Interaction::Hovered,
            pressed: pressed || *interaction == Interaction::Pressed,
            disabled,
            high_contrast,
        });
        *background = treatment.background.into();
        *border = BorderColor::all(treatment.border);
        outline.color = treatment.outline;
        outline.width = px(treatment.outline_width);
    }
    for (parent, mut visibility) in &mut markers {
        *visibility = if focus.0 == Some(parent.parent()) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn sync_widget_text(
    widgets: Query<(&WidgetLabel, &Children), Changed<WidgetLabel>>,
    mut text: Query<&mut Text, With<WidgetText>>,
) {
    for (label, children) in &widgets {
        for child in children.iter() {
            if let Ok(mut text) = text.get_mut(child) {
                **text = label.0.clone();
            }
        }
    }
}

pub fn sync_accessibility(
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
