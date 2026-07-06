//! First-run onboarding (Phase 48): a short, diegetic, skippable beat that teaches the
//! core loop by pointing at the systems that already read (the legend, the tac-map,
//! the HUD control line) instead of a modal wall of text. Gated by
//! `Settings::first_run`, so it shows on exactly the player's first match and never
//! again (the flag flips and persists to disk once dismissed).

use bevy::prelude::*;

use crate::GameState;
use crate::settings::{Settings, key_name, save_settings};
use crate::sim::state::MatchIntent;
use crate::view::theme::{ACCENT, BORDER, PANEL, TITLE};

/// Each beat names one system the player will already see doing its job — this is
/// "teach by being read," not new UI: the tac-map, the legend, and the pause reference
/// all exist regardless of onboarding; this just calls attention to them in order.
pub(crate) fn onboarding_beats(settings: &Settings) -> Vec<String> {
    let bindings = &settings.bindings;
    vec![
        "You are unobserved between glances: rooms reroute only while no one is looking. \
         Move to explore — the facility freezes exactly what you see."
            .to_string(),
        format!(
            "Drop an anchor torch ({}) to freeze a room's doorways so a route holds still \
             while you use it.",
            key_name(bindings.torch)
        ),
        format!(
            "{} opens the tac-map: your position, sighted rivals, and the exit's lock state.",
            key_name(bindings.tac_map)
        ),
        "Gantry hallways let you jump a gap — a fall reroutes you, it never ends the run."
            .to_string(),
        "Collapse is the countdown pressure: watch the HUD's collapse % and the klaxon."
            .to_string(),
    ]
}

/// How long a beat shows before auto-advancing if the player doesn't move.
const BEAT_SECONDS: f32 = 6.0;

#[derive(Component)]
pub(crate) struct OnboardingPanel;

#[derive(Component)]
pub(crate) struct OnboardingText;

#[derive(Resource)]
pub(crate) struct OnboardingState {
    pub(crate) beat: usize,
    pub(crate) timer: f32,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            beat: 0,
            timer: BEAT_SECONDS,
        }
    }
}

/// Spawn the onboarding panel only if this is the player's first match
/// (`Settings::first_run`); otherwise inert (no panel, no systems do anything).
pub(crate) fn spawn_onboarding(mut commands: Commands, settings: Res<Settings>) {
    if !settings.first_run {
        return;
    }
    commands.insert_resource(OnboardingState::default());
    commands
        .spawn((
            OnboardingPanel,
            DespawnOnExit(GameState::Match),
            Node {
                position_type: PositionType::Absolute,
                top: px(16),
                left: percent(50),
                width: px(560),
                margin: UiRect::left(px(-280)),
                padding: UiRect::all(px(14)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(BORDER),
        ))
        .with_children(|p| {
            let beats = onboarding_beats(&settings);
            p.spawn((
                OnboardingText,
                Text::new(beats[0].clone()),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TITLE),
            ));
            p.spawn((
                Text::new("any move advances | Esc dismisses"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(ACCENT),
            ));
        });
}

/// Advance/dismiss the onboarding beats: any movement/look/jump input or the timer
/// advances to the next beat; the last beat's dismissal (or an explicit Escape) flips
/// `first_run` false and persists it, then despawns the panel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_onboarding(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    intent: Option<Res<MatchIntent>>,
    mut settings: ResMut<Settings>,
    mut state: Option<ResMut<OnboardingState>>,
    mut panel: Query<Entity, With<OnboardingPanel>>,
    mut label: Query<&mut Text, With<OnboardingText>>,
    mut commands: Commands,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let moved = intent.as_ref().is_some_and(|i| !i.0.is_neutral());
    let skip = keyboard.just_pressed(KeyCode::Escape);
    state.timer -= time.delta_secs();
    if !skip && !moved && state.timer > 0.0 {
        return;
    }

    let beats = onboarding_beats(&settings);
    if skip || state.beat + 1 >= beats.len() {
        settings.first_run = false;
        save_settings(&settings);
        commands.remove_resource::<OnboardingState>();
        for entity in &mut panel {
            commands.entity(entity).despawn();
        }
        return;
    }

    state.beat += 1;
    state.timer = BEAT_SECONDS;
    if let Ok(mut text) = label.single_mut() {
        **text = beats[state.beat].clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_beat_is_non_empty_and_the_sequence_is_short_enough_to_read() {
        let settings = Settings::default();
        let beats = onboarding_beats(&settings);
        assert!(
            beats.len() <= 6,
            "onboarding stays a short sequence, not a wall of text"
        );
        for beat in beats {
            assert!(!beat.is_empty());
            assert!(
                beat.len() < 220,
                "each beat stays a short contextual hint, not a paragraph"
            );
        }
    }

    #[test]
    fn onboarding_beats_reflect_rebindings() {
        let mut settings = Settings::default();

        // Assert defaults first
        let beats_default = onboarding_beats(&settings);
        assert!(
            beats_default[1].contains("(F)"),
            "default anchor torch key is F"
        );
        assert!(
            beats_default[2].contains("Tab opens"),
            "default tac-map key is Tab"
        );

        // Rebind keys
        crate::settings::BindingSlot::Torch.set(&mut settings.bindings, KeyCode::KeyG);
        crate::settings::BindingSlot::TacMap.set(&mut settings.bindings, KeyCode::KeyM);

        let beats_rebound = onboarding_beats(&settings);
        assert!(
            beats_rebound[1].contains("(G)"),
            "rebound anchor torch key should show G"
        );
        assert!(
            beats_rebound[2].contains("M opens"),
            "rebound tac-map key should show M"
        );
    }
}
