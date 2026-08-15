//! Widget state, as colour.
//!
//! Moved out of `game/src/view/theme.rs` when the composition studio became a
//! second consumer. Only the *state mapping* moved: the game's own palette
//! (title, accent, panel, warning) and its screen-root helpers stayed behind,
//! because those are the frontend's identity rather than shared machinery.
//!
//! This is chrome — menus, panels, tool controls. Gameplay colour (markers,
//! surfaces, districts) comes from `observed_style` and never from here.

use bevy::prelude::*;

/// Focus-ring accent, border, and dim. Defined here rather than imported so
/// the crate stays Bevy-only; `game/src/view/theme.rs` keeps its own copies of
/// the same values as part of the frontend palette.
const ACCENT: Color = Color::srgb(0.40, 0.92, 1.0);
const BORDER: Color = Color::srgba(0.40, 0.92, 1.0, 0.5);
const DIM: Color = Color::srgb(0.58, 0.64, 0.74);

/// Label colour on chrome. Travels with the widget rather than the host
/// palette: a button whose text colour came from elsewhere could be rendered
/// unreadable against the treatment this module chose for it.
pub const LABEL: Color = Color::srgb(0.95, 0.97, 1.0);

/// The chrome palette as a named vocabulary.
///
/// [`widget_treatment`] answers "what does *this widget* look like in *this
/// state*". These roles answer the flatter question a theming layer asks: "what
/// colour is a surface, a border, dim text". A third-party widget set is themed
/// by mapping its tokens onto these, which is why they are named for their job
/// rather than for a widget.
///
/// Same boundary as the rest of this module: chrome only. Gameplay colour comes
/// from `observed_style`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeRole {
    /// The panel or window behind everything else.
    Surface,
    /// A control sitting on `Surface`: button face, field, track.
    Control,
    /// `Control` under the pointer.
    ControlHover,
    /// `Control` while held.
    ControlPressed,
    /// `Control` that cannot be operated.
    ControlDisabled,
    /// The filled portion of a slider or progress track.
    ControlFill,
    /// Ordinary separators and control outlines.
    Border,
    /// The focus ring. Paired with a shape cue by [`widget_treatment`], because
    /// focus must never rest on hue alone.
    Accent,
    /// Primary reading text.
    TextMain,
    /// Secondary text: units, hints, inactive tabs.
    TextDim,
    /// Text belonging to a disabled control.
    TextDisabled,
}

impl ChromeRole {
    pub const ALL: [ChromeRole; 11] = [
        ChromeRole::Surface,
        ChromeRole::Control,
        ChromeRole::ControlHover,
        ChromeRole::ControlPressed,
        ChromeRole::ControlDisabled,
        ChromeRole::ControlFill,
        ChromeRole::Border,
        ChromeRole::Accent,
        ChromeRole::TextMain,
        ChromeRole::TextDim,
        ChromeRole::TextDisabled,
    ];
}

/// The one place chrome colour is decided.
///
/// Values match [`widget_treatment`]'s non-high-contrast branch, so a themed
/// third-party widget and a hand-built one agree on screen.
#[must_use]
pub fn chrome(role: ChromeRole) -> Color {
    match role {
        ChromeRole::Surface => Color::srgb(0.035, 0.045, 0.06),
        ChromeRole::Control => Color::srgb(0.07, 0.10, 0.16),
        ChromeRole::ControlHover => Color::srgb(0.09, 0.22, 0.31),
        ChromeRole::ControlPressed => Color::srgb(0.12, 0.50, 0.62),
        ChromeRole::ControlDisabled => Color::srgb(0.035, 0.045, 0.06),
        ChromeRole::ControlFill => Color::srgb(0.12, 0.50, 0.62),
        ChromeRole::Border => BORDER,
        ChromeRole::Accent => ACCENT,
        ChromeRole::TextMain => LABEL,
        ChromeRole::TextDim => DIM,
        ChromeRole::TextDisabled => DIM.with_alpha(0.35),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WidgetVisualState {
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub disabled: bool,
    pub high_contrast: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetTreatment {
    pub background: Color,
    pub border: Color,
    pub outline: Color,
    pub outline_width: f32,
}

/// Semantic state mapping for chrome.
///
/// The visible chevron is supplied by the widget itself; this treatment adds a
/// shape and outline cue so focus never relies on hue alone.
#[must_use]
pub fn widget_treatment(state: WidgetVisualState) -> WidgetTreatment {
    let normal = if state.high_contrast {
        Color::BLACK
    } else {
        Color::srgb(0.07, 0.10, 0.16)
    };
    let hovered = if state.high_contrast {
        Color::srgb(0.08, 0.20, 0.72)
    } else {
        Color::srgb(0.09, 0.22, 0.31)
    };
    let pressed = if state.high_contrast {
        Color::srgb(0.0, 0.72, 1.0)
    } else {
        Color::srgb(0.12, 0.50, 0.62)
    };
    WidgetTreatment {
        background: if state.disabled {
            Color::srgb(0.035, 0.045, 0.06)
        } else if state.pressed {
            pressed
        } else if state.hovered {
            hovered
        } else {
            normal
        },
        border: if state.focused {
            Color::WHITE
        } else if state.disabled {
            DIM.with_alpha(0.35)
        } else {
            BORDER
        },
        outline: if state.focused { ACCENT } else { Color::NONE },
        outline_width: if state.focused { 3.0 } else { 0.0 },
    }
}
