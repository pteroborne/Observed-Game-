//! What each form sounds like, and when. Sounds are synthesised by
//! `tools/generate_guardian_audio.py` into `assets/sounds/guardian`.
//!
//! Hunting has a voice: the Tumbler hums under its ratchet, the Plumb's orbits whoosh,
//! and the Roller is heard landing on each face. Frozen is silent, after the sound of
//! freezing: the Tumbler's latch, the Plumb's rings settling, the Roller's held tone.
//! An anchor clamps on in the lantern's dark glass voice, the same for every form.
use observed_guardian::form::{Form, ROLLER_STEP_SECONDS, State};

/// A form's sounds, as files under `sounds/guardian/`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Voice {
    /// Looped while hunting.
    pub hum: Option<&'static str>,
    /// Played at each landing while hunting.
    pub step: Option<&'static str>,
    pub freeze: &'static str,
    pub clamp: &'static str,
    pub catch: &'static str,
}

#[must_use]
pub const fn voice(form: Form) -> Voice {
    match form {
        Form::Tumbler { .. } => Voice {
            hum: Some("tumbler_hum"),
            step: None,
            freeze: "tumbler_latch",
            clamp: "tumbler_clamp",
            catch: "tumbler_catch",
        },
        Form::Plumb => Voice {
            hum: Some("plumb_hum"),
            step: None,
            freeze: "plumb_seal",
            clamp: "tumbler_clamp",
            catch: "plumb_catch",
        },
        Form::Roller => Voice {
            hum: None,
            step: Some("roller_fall"),
            freeze: "roller_balance",
            clamp: "tumbler_clamp",
            catch: "roller_catch",
        },
    }
}

/// The one-shot for entering `to` from `from`, if any.
#[must_use]
pub const fn on_entering(form: Form, from: State, to: State) -> Option<&'static str> {
    let voice = voice(form);
    match to {
        State::FrozenBySight if !from.frozen() => Some(voice.freeze),
        State::FrozenByAnchor if !matches!(from, State::FrozenByAnchor) => Some(voice.clamp),
        State::Catch => Some(voice.catch),
        _ => None,
    }
}

/// How many landings fall in the clock interval `(from, to]`.
#[must_use]
pub fn landings(from: f32, to: f32) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = |clock: f32| (clock / ROLLER_STEP_SECONDS).floor().max(0.0) as u32;
    count(to).saturating_sub(count(from))
}

#[must_use]
pub fn path(name: &str) -> String {
    format!("sounds/guardian/{name}.ogg")
}

#[cfg(test)]
mod tests {
    use observed_guardian::form::{Form, State};

    use super::{landings, on_entering, path, voice};

    /// Every sound a form names was generated.
    #[test]
    fn every_named_sound_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for form in Form::MAJORS {
            let v = voice(form);
            for name in [v.hum, v.step, Some(v.freeze), Some(v.clamp), Some(v.catch)]
                .into_iter()
                .flatten()
            {
                assert!(root.join(path(name)).exists(), "{name} missing");
            }
        }
    }

    /// Every form is heard hunting, one way or the other.
    #[test]
    fn hunting_is_never_silent() {
        for form in Form::MAJORS {
            let v = voice(form);
            assert!(v.hum.is_some() || v.step.is_some(), "{form:?}");
        }
    }

    #[test]
    fn freezing_is_heard_once() {
        let form = Form::Tumbler { tiers: 4 };
        assert_eq!(
            on_entering(form, State::Hunting, State::FrozenBySight),
            Some("tumbler_latch")
        );
        assert_eq!(
            on_entering(form, State::FrozenByAnchor, State::FrozenBySight),
            None
        );
        assert_eq!(
            on_entering(form, State::FrozenBySight, State::Hunting),
            None
        );
    }

    #[test]
    fn the_roller_lands_once_a_step() {
        assert_eq!(landings(0.0, 0.99), 0);
        assert_eq!(landings(0.99, 1.01), 1);
        assert_eq!(landings(0.0, 3.0), 3);
    }
}
