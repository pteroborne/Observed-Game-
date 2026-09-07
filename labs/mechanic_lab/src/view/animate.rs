//! Motion, which is the one channel colour cannot take away.
//!
//! A breathing tile reads the same under every colour-vision deficiency, at any
//! palette, on any screen. That makes it the strongest signal available for the
//! thing a player most needs to notice — what the facility is about to do —
//! and it is why the telegraph moves rather than merely being tinted.
//!
//! The *rhythm* carries the outcome as well: a boundary about to wall up
//! flashes hard and fast, one about to open breathes slowly. So even with the
//! ghost colour entirely stripped out, closing and opening remain
//! distinguishable.

use bevy::prelude::*;

/// Attached to anything that should draw attention to itself.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pulse {
    /// Seconds per cycle. Short reads as urgent, long as ambient.
    pub period: f32,
    /// Scale multiplier at the bottom and top of the cycle.
    pub scale: (f32, f32),
    /// Alpha at the bottom and top of the cycle.
    pub alpha: (f32, f32),
    /// The scale this entity was spawned at, so pulsing composes with it.
    pub base_scale: f32,
    /// A hard flash rather than a sine breath.
    pub sharp: bool,
}

impl Pulse {
    /// About to wall up: the faster of the two, because it takes a route away.
    ///
    /// Both rhythms are deliberately unhurried. A board where several marks
    /// blink quickly is a board that is hard to look at while thinking, and
    /// thinking is what the turn is for — the pulse has to be noticeable
    /// without being a distraction you have to work around.
    #[must_use]
    pub const fn closing(base_scale: f32) -> Self {
        Self {
            period: 1.5,
            scale: (0.88, 1.35),
            alpha: (0.40, 1.0),
            base_scale,
            sharp: true,
        }
    }

    /// About to open: slow and soft, because it is an offer rather than a loss.
    #[must_use]
    pub const fn opening(base_scale: f32) -> Self {
        Self {
            period: 3.2,
            scale: (0.85, 1.2),
            alpha: (0.28, 0.85),
            base_scale,
            sharp: false,
        }
    }

    /// Something is coming but not what: a neutral middle rhythm.
    #[must_use]
    pub const fn pending(base_scale: f32) -> Self {
        Self {
            period: 2.3,
            scale: (0.88, 1.25),
            alpha: (0.32, 0.9),
            base_scale,
            sharp: false,
        }
    }

    fn wave(self, seconds: f32) -> f32 {
        let phase = (seconds / self.period).fract();
        if self.sharp {
            // Squared off, but gently: enough to read as a beat rather than a
            // drift, not so much that it strobes.
            let sine = (phase * std::f32::consts::TAU).sin();
            (sine * 1.8).clamp(-1.0, 1.0) * 0.5 + 0.5
        } else {
            (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5
        }
    }
}

fn lerp(range: (f32, f32), t: f32) -> f32 {
    range.0 + (range.1 - range.0) * t
}

/// Drive every pulsing mark. Runs every frame, unlike the board redraw, which
/// only runs when the match changes — the two are deliberately separate so an
/// animation never costs a full respawn.
pub fn pulse(
    time: Res<Time>,
    mut sprites: Query<(&Pulse, &mut Transform, Option<&mut Sprite>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    handles: Query<(&Pulse, &MeshMaterial2d<ColorMaterial>)>,
) {
    let seconds = time.elapsed_secs();
    for (pulse, mut transform, sprite) in &mut sprites {
        let t = pulse.wave(seconds);
        transform.scale = Vec3::splat(pulse.base_scale * lerp(pulse.scale, t));
        if let Some(mut sprite) = sprite {
            let alpha = lerp(pulse.alpha, t);
            sprite.color.set_alpha(alpha);
        }
    }
    for (pulse, handle) in &handles {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let mut color = material.color;
            color.set_alpha(lerp(pulse.alpha, pulse.wave(seconds)));
            material.color = color;
        }
    }
}

/// A mark that slides from where it was to where it is.
///
/// Simultaneous resolution means everything moves at once, and everything
/// moving at once is exactly what a teleport hides: three pawns, two guardians
/// and a rewired wall all changing between one frame and the next is not a turn
/// you can read. The glide is short on purpose — long enough to see who went
/// where, short enough that it never becomes a wait.
#[derive(Component, Clone, Copy, Debug)]
pub struct Glide {
    pub from: Vec2,
    pub to: Vec2,
    /// Seconds on the clock when the slide began.
    pub start: f32,
    pub duration: f32,
}

impl Glide {
    #[must_use]
    pub fn new(from: Vec2, to: Vec2, now: f32) -> Self {
        Self {
            from,
            to,
            start: now,
            duration: 0.34,
        }
    }
}

/// Slide everything that moved this turn into place.
pub fn glide(time: Res<Time>, mut moving: Query<(&Glide, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (glide, mut transform) in &mut moving {
        let raw = ((now - glide.start) / glide.duration).clamp(0.0, 1.0);
        // Ease out: quick off the mark, settling rather than stopping dead.
        let t = 1.0 - (1.0 - raw).powi(3);
        let at = glide.from.lerp(glide.to, t);
        transform.translation.x = at.x;
        transform.translation.y = at.y;
    }
}
