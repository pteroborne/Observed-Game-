//! Quiet tactical chrome for the Architect cutaway. Signals use both colour and
//! a shape/text cue: amber ring = selection, cyan eye = Observer, red pyramid =
//! Guardian, red cross = instability, violet cage = protected prison.
use bevy::color::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Role {
    Background,
    Panel,
    Card,
    Hover,
    Border,
    Text,
    Muted,
    Selected,
    Observer,
    Guardian,
    Valid,
    Prison,
    Context,
    Fixture,
}

#[must_use]
pub fn color(role: Role) -> Color {
    match role {
        Role::Background => Color::srgb(0.022, 0.031, 0.038),
        Role::Panel => Color::srgb(0.038, 0.050, 0.059),
        Role::Card => Color::srgb(0.052, 0.067, 0.077),
        Role::Hover => Color::srgb(0.092, 0.116, 0.126),
        Role::Border => Color::srgb(0.23, 0.29, 0.32),
        Role::Text => Color::srgb(0.94, 0.93, 0.86),
        Role::Muted => Color::srgb(0.59, 0.66, 0.68),
        Role::Selected => Color::srgb(1.0, 0.72, 0.28),
        Role::Observer => Color::srgb(0.28, 0.91, 0.96),
        Role::Guardian => Color::srgb(1.0, 0.35, 0.29),
        Role::Valid => Color::srgb(0.38, 0.88, 0.67),
        Role::Prison => Color::srgb(0.68, 0.57, 0.90),
        Role::Context => Color::srgb(0.09, 0.13, 0.15),
        Role::Fixture => Color::srgb(1.0, 0.85, 0.52),
    }
}

/// Model-scale concrete: brighter than the first-person shell because the
/// cutaway compresses walkable surfaces to a few pixels. District hue remains
/// subordinate to selection and actor signals.
#[must_use]
pub fn surface(register: observed_content::ArchitectureRegister, floor: bool) -> Color {
    let tint = crate::architecture_surface(register, crate::ArchitectureSurfaceRole::Wall)
        .base_color
        .to_srgba();
    let base = if floor {
        [0.34, 0.33, 0.29]
    } else {
        [0.43, 0.43, 0.39]
    };
    Color::srgb(
        base[0] + tint.red * 0.22,
        base[1] + tint.green * 0.22,
        base[2] + tint.blue * 0.22,
    )
}

/// Linear vertex multiplier for a sawn wall cap; walkable floors stay brighter.
pub const CUT_SURFACE_MULTIPLIER: [f32; 4] = [0.18, 0.20, 0.21, 1.0];

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color) -> f32 {
        let c = c.to_linear();
        c.red * 0.2126 + c.green * 0.7152 + c.blue * 0.0722
    }
    #[test]
    fn tactical_signals_and_copy_remain_readable_on_the_panel() {
        for role in [
            Role::Text,
            Role::Muted,
            Role::Selected,
            Role::Observer,
            Role::Guardian,
            Role::Valid,
            Role::Prison,
        ] {
            let contrast = (luminance(color(role)) + 0.05) / (luminance(color(Role::Panel)) + 0.05);
            assert!(contrast > 4.5, "{role:?} contrast: {contrast}");
        }
    }
}
