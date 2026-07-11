use bevy::input::keyboard::KeyCode;
use bevy::prelude::ButtonInput;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RebindStage {
    WaitingForActivationRelease { activation_key: KeyCode },
    Armed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveRebind<T> {
    target: T,
    stage: RebindStage,
}

/// A small keyboard-capture state machine for runtime rebinding UI.
///
/// The capture can start in a "waiting for activation release" stage, which makes
/// it impossible for the same Enter/F7/etc. press that opened the prompt to become
/// the captured binding. Once armed, pressing that activation key again is a normal
/// deliberate binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RebindCapture<T> {
    active: Option<ActiveRebind<T>>,
}

impl<T> Default for RebindCapture<T> {
    fn default() -> Self {
        Self { active: None }
    }
}

impl<T: Send + Sync + 'static> bevy::prelude::Resource for RebindCapture<T> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebindCaptureStatus<T> {
    WaitingForActivationRelease { target: T, activation_key: KeyCode },
    Armed { target: T },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebindCaptureEvent<T> {
    Armed { target: T },
    Cancelled { target: T },
    Captured { target: T, key: KeyCode },
}

impl<T: Copy> RebindCapture<T> {
    pub fn begin_waiting_for_release(&mut self, target: T, activation_key: KeyCode) {
        self.active = Some(ActiveRebind {
            target,
            stage: RebindStage::WaitingForActivationRelease { activation_key },
        });
    }

    pub fn begin_armed(&mut self, target: T) {
        self.active = Some(ActiveRebind {
            target,
            stage: RebindStage::Armed,
        });
    }

    pub fn cancel(&mut self) -> Option<T> {
        self.active.take().map(|active| active.target)
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn is_armed(&self) -> bool {
        self.active
            .is_some_and(|active| active.stage == RebindStage::Armed)
    }

    pub fn target(&self) -> Option<T> {
        self.active.map(|active| active.target)
    }

    pub fn status(&self) -> Option<RebindCaptureStatus<T>> {
        let active = self.active?;
        Some(match active.stage {
            RebindStage::WaitingForActivationRelease { activation_key } => {
                RebindCaptureStatus::WaitingForActivationRelease {
                    target: active.target,
                    activation_key,
                }
            }
            RebindStage::Armed => RebindCaptureStatus::Armed {
                target: active.target,
            },
        })
    }

    pub fn update(
        &mut self,
        keyboard: &ButtonInput<KeyCode>,
        cancel_key: KeyCode,
    ) -> Option<RebindCaptureEvent<T>> {
        let active = self.active.as_mut()?;

        if keyboard.just_pressed(cancel_key) {
            let target = active.target;
            self.active = None;
            return Some(RebindCaptureEvent::Cancelled { target });
        }

        if let RebindStage::WaitingForActivationRelease { activation_key } = active.stage {
            if keyboard.pressed(activation_key) {
                return None;
            }
            active.stage = RebindStage::Armed;
            // Fall through to the armed capture below: a key that is `just_pressed`
            // on the very frame the activation key was released is a deliberate
            // press (it cannot be the activation press — that key is, by
            // definition, not pressed on this frame), and `just_pressed` only
            // lives for one frame, so deferring would silently drop it.
            if keyboard.get_just_pressed().next().is_none() {
                return Some(RebindCaptureEvent::Armed {
                    target: active.target,
                });
            }
        }

        let key = keyboard.get_just_pressed().next().copied()?;
        let target = active.target;
        self.active = None;
        Some(RebindCaptureEvent::Captured { target, key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_press_is_not_captured_until_released_and_pressed_again() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut capture = RebindCapture::default();
        capture.begin_waiting_for_release("jump", KeyCode::Enter);

        keys.press(KeyCode::Enter);
        assert_eq!(capture.update(&keys, KeyCode::Escape), None);
        assert!(capture.is_active());
        assert!(!capture.is_armed());

        keys.reset(KeyCode::Enter);
        assert_eq!(
            capture.update(&keys, KeyCode::Escape),
            Some(RebindCaptureEvent::Armed { target: "jump" })
        );
        assert!(capture.is_armed());

        keys.press(KeyCode::Enter);
        assert_eq!(
            capture.update(&keys, KeyCode::Escape),
            Some(RebindCaptureEvent::Captured {
                target: "jump",
                key: KeyCode::Enter
            })
        );
    }

    #[test]
    fn a_key_pressed_on_the_activation_release_frame_is_still_captured() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut capture = RebindCapture::default();
        capture.begin_waiting_for_release("jump", KeyCode::Enter);

        keys.press(KeyCode::Enter);
        assert_eq!(capture.update(&keys, KeyCode::Escape), None);

        // Enter released and K pressed within the same frame: the capture arms and
        // takes K immediately (a real `ButtonInput` clears `just_pressed` next
        // frame, so waiting would lose the press).
        keys.reset(KeyCode::Enter);
        keys.press(KeyCode::KeyK);
        assert_eq!(
            capture.update(&keys, KeyCode::Escape),
            Some(RebindCaptureEvent::Captured {
                target: "jump",
                key: KeyCode::KeyK
            })
        );
        assert!(!capture.is_active());
    }

    #[test]
    fn escape_cancels_without_capturing() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut capture = RebindCapture::default();
        capture.begin_armed("jump");

        keys.press(KeyCode::Escape);
        assert_eq!(
            capture.update(&keys, KeyCode::Escape),
            Some(RebindCaptureEvent::Cancelled { target: "jump" })
        );
        assert!(!capture.is_active());
    }
}
