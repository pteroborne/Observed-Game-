//! Chargeworks machinery accents, decorative electric fields and fixed work lights.
use bevy::color::Color;

pub const FIXTURE_INTENSITY: f32 = 7_500_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 170_000.0;
/// Decorative field motion, metres per second; this never transports a payload.
pub const FIELD_SPEED: f32 = 0.8;
pub const FIELD_EMERGENCY: f32 = 0.12;

#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(0.79, 0.92, 1.0)
}
#[must_use]
pub fn charge_color() -> Color {
    Color::srgba(0.08, 0.78, 0.89, 0.42)
}
#[must_use]
pub fn field_color() -> Color {
    Color::srgb(0.12, 0.62, 0.93)
}
#[must_use]
pub fn field_edge_color() -> Color {
    Color::srgb(0.46, 0.88, 1.0)
}
#[must_use]
pub fn warning_color() -> Color {
    Color::srgb(0.96, 0.59, 0.16)
}
#[must_use]
pub fn belt_color() -> Color {
    Color::srgb(0.08, 0.12, 0.17)
}
