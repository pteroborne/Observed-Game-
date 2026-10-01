//! The plumb in the hand: arming it along the look, dialling it round the way the body
//! faces while the arm key is held, and firing it at the minor in the crosshair. Only the
//! aim leaves here, in the next tick's command; the simulation is `hex_wfc::kinetic`.

use std::f32::consts::FRAC_PI_2;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_match::ascent::economy::PLUMB_SHOT_COST;
use observed_match::hex_wfc::HexPlumbAim;

use super::super::hud::play::HudNotice;
use super::super::hud::words::Tone;
use super::super::overlay::MatchOverlayState;
use super::super::sim::HexWfcRuntime;
use super::{KineticAssets, charge};

/// The local body's plumb: which way it is armed about the way the body faces, pitch up
/// and yaw right in radians, or not yet armed.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct ArmedPlumb {
    pub(in crate::hex_wfc) armed: Option<(f32, f32)>,
}

impl ArmedPlumb {
    /// Whether the arm key (or the controller's D-pad down) is held, so the look dials the
    /// plumb rather than turning the view.
    pub(in crate::hex_wfc) fn dialing(
        keyboard: &ButtonInput<KeyCode>,
        gamepads: &Query<&Gamepad>,
        settings: &crate::settings::Settings,
    ) -> bool {
        keyboard.pressed(settings.bindings.arm_plumb)
            || gamepads
                .iter()
                .any(|pad| pad.pressed(GamepadButton::DPadDown))
    }
}

#[derive(SystemParam)]
pub(in crate::hex_wfc) struct PlumbInput<'w, 's> {
    time: Res<'w, Time>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    buttons: Option<Res<'w, ButtonInput<MouseButton>>>,
    motion: Option<Res<'w, bevy::input::mouse::AccumulatedMouseMotion>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    settings: Res<'w, crate::settings::Settings>,
    overlay: Res<'w, MatchOverlayState>,
    architect: Option<Res<'w, super::super::architect::ArchitectDesk>>,
    spectator: Option<Res<'w, crate::sim::state::SpectatorBot>>,
}

/// Arm the plumb, dial it while the arm key is held, and fire it: the aim goes to the next
/// tick's command (`HexWfcIntent::plumb`). A plumb fired unarmed, or on a pool short of it,
/// is answered here and never sent.
pub(in crate::hex_wfc) fn arm_and_fire(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    input: PlumbInput,
    assets: Option<Res<KineticAssets>>,
    armed: Option<ResMut<ArmedPlumb>>,
    mut intent: ResMut<super::super::sim::HexWfcIntent>,
    mut notice: ResMut<HudNotice>,
) {
    let PlumbInput {
        time,
        keyboard,
        buttons,
        motion,
        gamepads,
        settings,
        overlay,
        architect,
        spectator,
    } = input;
    let (Some(assets), Some(mut armed)) = (assets, armed) else {
        return;
    };
    let local = runtime.local_player;
    let Some(body) = runtime
        .match_state
        .players
        .get(&local)
        .filter(|body| body.in_facility())
    else {
        return;
    };
    if *overlay != MatchOverlayState::Playing || architect.is_some() || spectator.is_some() {
        return;
    }
    let bindings = &settings.bindings;
    if keyboard.just_pressed(bindings.arm_plumb)
        || gamepads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::DPadDown))
    {
        // Armed along the look: the way the body faces, at the pitch it looks.
        armed.armed = Some((body.pitch, 0.0));
    }
    if ArmedPlumb::dialing(&keyboard, &gamepads, &settings)
        && let Some((pitch, yaw)) = armed.armed.as_mut()
    {
        // The look the view would have turned by, dialling the plumb instead.
        let mouse = motion.as_ref().map_or(Vec2::ZERO, |motion| motion.delta)
            * (settings.mouse_sensitivity * 0.018_333);
        let stick = gamepads
            .iter()
            .map(|pad| crate::screens::input::read_gamepad_match(pad).0.look)
            .fold(Vec2::ZERO, |sum, look| sum + look);
        let turn = (mouse + stick) * runtime.match_state.look_step();
        *yaw = (*yaw + turn.x + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        *pitch = (*pitch - turn.y).clamp(-FRAC_PI_2, FRAC_PI_2);
    }
    let fired = keyboard.just_pressed(bindings.plumb)
        || buttons
            .as_ref()
            .is_some_and(|buttons| buttons.just_pressed(MouseButton::Middle))
        || gamepads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::DPadRight));
    if !fired {
        return;
    }
    let now = time.elapsed_secs_f64();
    let Some((pitch, yaw)) = armed.armed else {
        notice.show("Arm the plumb first", Tone::Against, now);
        return;
    };
    if charge(&runtime, local).is_some_and(|charge| charge < PLUMB_SHOT_COST) {
        notice.show(
            "Not enough charge for a plumb. Recharge at a powered station",
            Tone::Against,
            now,
        );
        super::super::audio::play(
            &mut commands,
            assets.empty.clone(),
            0.6 * settings.effective_sfx_volume(),
            "Kinetic empty",
            None,
        );
        return;
    }
    intent.plumb = Some(HexPlumbAim::from_radians(pitch, yaw));
}
