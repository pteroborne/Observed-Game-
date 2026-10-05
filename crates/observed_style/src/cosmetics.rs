//! Cosmetic decoration shared by previews and world actors; never gameplay signals.
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

impl Colorway {
    pub fn from_id(id: u16) -> Self {
        match id {
            1 => Self::Ember,
            2 => Self::Cobalt,
            3 => Self::Void,
            _ => Self::Ash,
        }
    }
}
/// Decorative gimbal/badge finish. Role iris and haze retain their semantic finish.
pub fn trim(id: u16) -> crate::guardian::Finish {
    let mut finish = crate::observer::finish(crate::observer::Part::Trim);
    if id != 0 {
        finish.base_color = accent(Colorway::from_id(id));
    }
    finish
}
