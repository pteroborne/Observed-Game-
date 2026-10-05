//! Representative cosmetic designs. These are decoration, never gameplay signals.
use bevy::color::Color;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Colorway {
    #[default]
    Ash,
    Ember,
    Cobalt,
    Void,
}

pub fn accent(colorway: Colorway) -> Color {
    match colorway {
        Colorway::Ash => Color::srgb(0.72, 0.77, 0.83),
        Colorway::Ember => Color::srgb(1.0, 0.45, 0.20),
        Colorway::Cobalt => Color::srgb(0.28, 0.55, 1.0),
        Colorway::Void => Color::srgb(0.75, 0.45, 0.95),
    }
}
