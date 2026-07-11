//! The **input boundary**: the durable `PlayerId` and `PlayerIntent` that every
//! gameplay system, bot, replay tape, and network packet speaks in — instead of
//! reading hardware directly.
//!
//! This data path is **pure**: it depends only on `glam` for vector math, never on
//! Bevy ECS. The optional (default-on) `bevy` feature is the *adapter*: it derives
//! `Component` on these types so the game and labs can use them as ECS components and
//! query them. Because the field type `glam::Vec2` is the exact type Bevy re-exports
//! as `bevy::math::Vec2`, a `PlayerIntent` built either way is the same type — so a
//! consumer that does not need ECS integration depends on `player_input` with
//! `default-features = false` and inherits no Bevy. Keyboard/controller *sampling* and
//! gameplay resource wiring are themselves Bevy adapters and live in the consuming
//! labs/game, not here.

use glam::Vec2;

#[cfg(feature = "bevy")]
mod rebind;

#[cfg(feature = "bevy")]
pub use rebind::{RebindCapture, RebindCaptureEvent, RebindCaptureStatus};

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlayerId(pub u16);

impl PlayerId {
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn label(self) -> String {
        format!("P{}", self.0 + 1)
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Component))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub look: Vec2,
    pub jump_pressed: bool,
    pub sprint_held: bool,
    pub interact_pressed: bool,
    pub interact_held: bool,
    pub climb_pressed: bool,
}

impl PlayerIntent {
    pub fn sanitized(mut self) -> Self {
        self.movement = clamp_unit(self.movement);
        self.look = clamp_unit(self.look);
        self
    }

    pub fn is_neutral(self) -> bool {
        self == Self::default()
    }
}

fn clamp_unit(value: Vec2) -> Vec2 {
    if value.length_squared() > 1.0 {
        value.normalize()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Uses only pure data (glam + `Default`), so it passes both with the `bevy`
    // feature and with `--no-default-features` — proving the durable path is pure.
    #[test]
    fn intent_vectors_are_clamped_without_changing_actions() {
        let intent = PlayerIntent {
            movement: Vec2::splat(1.0),
            look: Vec2::new(0.0, 2.0),
            jump_pressed: true,
            ..Default::default()
        }
        .sanitized();

        assert!((intent.movement.length() - 1.0).abs() < 0.001);
        assert_eq!(intent.look, Vec2::Y);
        assert!(intent.jump_pressed);
    }

    #[test]
    fn a_default_intent_is_neutral_and_ids_label_one_based() {
        assert!(PlayerIntent::default().is_neutral());
        assert_eq!(PlayerId(0).label(), "P1");
        assert_eq!(PlayerId(3).index(), 3);
    }
}
