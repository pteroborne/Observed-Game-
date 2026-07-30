//! Preset-first Play Hub and its deliberately separate advanced setup page.

use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Activate};

use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::GameState;
use crate::hex_wfc::{
    launch::{HexLaunchSpec, HexSeedPolicy},
    loading::HexLaunchRequestSequence,
};
use crate::play_setup::{LaunchContext, PlayPreset, PlaySetupDraft, save_play_setup};
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, summary_panel, text};

const HUB_SCOPE: FocusScopeId = FocusScopeId("play_hub");
const SOLO: WidgetId = WidgetId::named("play.preset.solo");
const CO_OP: WidgetId = WidgetId::named("play.preset.co_op");
const TEAM_RACE: WidgetId = WidgetId::named("play.preset.team_race");
const SPECTATE: WidgetId = WidgetId::named("play.preset.spectate");
const ADVANCED: WidgetId = WidgetId::named("play.advanced");
const START: WidgetId = WidgetId::named("play.start");
const LAN: WidgetId = WidgetId::named("play.lan");
const BACK: WidgetId = WidgetId::named("play.back");

const ADVANCED_SCOPE: FocusScopeId = FocusScopeId("play_advanced");
const TEAMS: WidgetId = WidgetId::named("play.advanced.teams");
const TEAM_SIZE: WidgetId = WidgetId::named("play.advanced.team_size");
const BOT_FILL: WidgetId = WidgetId::named("play.advanced.bot_fill");
const GUARDIAN: WidgetId = WidgetId::named("play.advanced.guardian");
const ADVANCED_START: WidgetId = WidgetId::named("play.advanced.start");
const ADVANCED_BACK: WidgetId = WidgetId::named("play.advanced.back");

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlayAction {
    SelectPreset(PlayPreset),
    Advanced,
    Launch,
    Lan,
    Back,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdvancedAction {
    CycleTeams,
    CycleTeamSize,
    ToggleBotFill,
    ToggleGuardian,
    Launch,
    Back,
}

#[derive(Component)]
pub(crate) struct PlaySummary;

pub(crate) fn setup_hub(mut commands: Commands, setup: Res<PlaySetupDraft>) {
    commands
        .spawn(screen_root(GameState::Play))
        .with_children(|root| {
            root.spawn(text("PLAY", 44.0, TITLE));
            root.spawn(text(
                "Choose the kind of run first. Match rules stay visible before launch.",
                16.0,
                DIM,
            ));
            root.spawn(summary_panel()).with_children(|summary| {
                summary.spawn((PlaySummary, text(play_summary(&setup), 17.0, ACCENT)));
            });
            root.spawn((
                panel(),
                widgets::focus_scope(FocusScope::screen(HUB_SCOPE, SOLO, BACK)),
            ))
            .with_children(|panel| {
                for (order, id, preset) in [
                    (0, SOLO, PlayPreset::Solo),
                    (1, CO_OP, PlayPreset::CoOp),
                    (2, TEAM_RACE, PlayPreset::TeamRace),
                    (3, SPECTATE, PlayPreset::Spectate),
                ] {
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(id, HUB_SCOPE, order, preset_label(preset, &setup)),
                        PlayAction::SelectPreset(preset),
                    );
                }
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(ADVANCED, HUB_SCOPE, 4, "Advanced setup"),
                    PlayAction::Advanced,
                );
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(START, HUB_SCOPE, 5, launch_label(&setup)),
                    PlayAction::Launch,
                );
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(LAN, HUB_SCOPE, 6, "LAN play"),
                    PlayAction::Lan,
                );
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(BACK, HUB_SCOPE, 7, "Back"),
                    PlayAction::Back,
                );
            });
        });
}

pub(crate) fn setup_advanced(mut commands: Commands, setup: Res<PlaySetupDraft>) {
    commands
        .spawn(screen_root(GameState::PlayAdvanced))
        .with_children(|root| {
            root.spawn(text("ADVANCED SETUP", 42.0, TITLE));
            root.spawn(text(
                "One deterministic roster feeds local play and LAN hosting. Maximum 16 seats.",
                15.0,
                DIM,
            ));
            root.spawn((
                panel(),
                widgets::focus_scope(FocusScope::screen(
                    ADVANCED_SCOPE,
                    TEAMS,
                    ADVANCED_BACK,
                )),
            ))
            .with_children(|panel| {
                for (order, id, action) in [
                    (0, TEAMS, AdvancedAction::CycleTeams),
                    (1, TEAM_SIZE, AdvancedAction::CycleTeamSize),
                    (2, BOT_FILL, AdvancedAction::ToggleBotFill),
                    (3, GUARDIAN, AdvancedAction::ToggleGuardian),
                    (4, ADVANCED_START, AdvancedAction::Launch),
                    (5, ADVANCED_BACK, AdvancedAction::Back),
                ] {
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(
                            id,
                            ADVANCED_SCOPE,
                            order,
                            advanced_label(action, &setup),
                        ),
                        action,
                    );
                }
            });
            root.spawn((PlaySummary, text(play_summary(&setup), 16.0, ACCENT)));
            root.spawn(text(
                "Activate a row to cycle it. Empty local seats become bots only when bot fill is on.",
                14.0,
                DIM,
            ));
        });
}

pub(crate) fn activate_hub(
    activation: On<Activate>,
    actions: Query<&PlayAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut setup: ResMut<PlaySetupDraft>,
    mut sequence: ResMut<HexLaunchRequestSequence>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match *action {
        PlayAction::SelectPreset(preset) => {
            setup.select_preset(preset);
            save_play_setup(&setup);
        }
        PlayAction::Advanced => next.set(GameState::PlayAdvanced),
        PlayAction::Launch => launch_local(&mut commands, &mut sequence, &setup, &mut next),
        PlayAction::Lan => next.set(GameState::LanBrowser),
        PlayAction::Back => next.set(GameState::MainMenu),
    }
}

pub(crate) fn activate_advanced(
    activation: On<Activate>,
    actions: Query<&AdvancedAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut setup: ResMut<PlaySetupDraft>,
    mut sequence: ResMut<HexLaunchRequestSequence>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match action {
        AdvancedAction::CycleTeams => {
            let maximum = 16 / setup.members_per_team.max(1);
            setup.teams = if setup.teams >= maximum {
                1
            } else {
                setup.teams + 1
            };
            setup.preset = PlayPreset::Custom;
        }
        AdvancedAction::CycleTeamSize => {
            let maximum = 16 / setup.teams.max(1);
            setup.members_per_team = if setup.members_per_team >= maximum {
                1
            } else {
                setup.members_per_team + 1
            };
            setup.preset = PlayPreset::Custom;
        }
        AdvancedAction::ToggleBotFill => {
            setup.fill_empty_seats = !setup.fill_empty_seats;
            setup.preset = PlayPreset::Custom;
        }
        AdvancedAction::ToggleGuardian => {
            setup.guardian = !setup.guardian;
            setup.preset = PlayPreset::Custom;
        }
        AdvancedAction::Launch => {
            launch_local(&mut commands, &mut sequence, &setup, &mut next);
            return;
        }
        AdvancedAction::Back => {
            next.set(GameState::Play);
            return;
        }
    }
    save_play_setup(&setup);
}

pub(crate) fn refresh_hub(
    setup: Res<PlaySetupDraft>,
    mut preset_buttons: Query<(&PlayAction, &mut WidgetLabel)>,
    mut summaries: Query<&mut Text, With<PlaySummary>>,
) {
    if !setup.is_changed() {
        return;
    }
    for (action, mut label) in &mut preset_buttons {
        label.0 = match action {
            PlayAction::SelectPreset(preset) => preset_label(*preset, &setup),
            PlayAction::Launch => launch_label(&setup),
            PlayAction::Advanced => "Advanced setup".to_string(),
            PlayAction::Lan => "LAN play".to_string(),
            PlayAction::Back => "Back".to_string(),
        };
    }
    for mut summary in &mut summaries {
        **summary = play_summary(&setup);
    }
}

pub(crate) fn refresh_advanced(
    setup: Res<PlaySetupDraft>,
    mut buttons: Query<(&AdvancedAction, &mut WidgetLabel)>,
    mut summaries: Query<&mut Text, With<PlaySummary>>,
) {
    if !setup.is_changed() {
        return;
    }
    for (action, mut label) in &mut buttons {
        label.0 = advanced_label(*action, &setup);
    }
    for mut summary in &mut summaries {
        **summary = play_summary(&setup);
    }
}

fn launch_local(
    commands: &mut Commands,
    sequence: &mut HexLaunchRequestSequence,
    setup: &PlaySetupDraft,
    next: &mut NextState<GameState>,
) {
    let Ok(validated) = setup.validate() else {
        return;
    };
    let seed = crate::flow::launch_seed();
    info!("MATCH_PREPARE preset={:?} seed={seed}", validated.preset);
    commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
    commands.insert_resource(sequence.issue(
        LaunchContext::Local,
        observed_core::PlayerId(0),
        validated.spectator,
        false,
        HexLaunchSpec {
            requested_seed: seed,
            config: crate::hex_wfc::sim::runtime_config_for(setup),
            seed_policy: HexSeedPolicy::Nearby,
        },
    ));
    next.set(GameState::Loading);
}

/// Selection is a shape, not a colour, so it survives a monochrome or colour-blind
/// reading — and it is spelled in ASCII because the shipped default font is a subset
/// with no geometric shapes: `◆`/`◇` rendered as blank tofu, which made the selected
/// preset invisible at the 1280×800 gate.
fn preset_label(preset: PlayPreset, setup: &PlaySetupDraft) -> String {
    format!(
        "{} {}",
        if setup.preset == preset { "[*]" } else { "[ ]" },
        preset.label()
    )
}

fn launch_label(setup: &PlaySetupDraft) -> String {
    format!("Start {}", setup.preset.label())
}

fn advanced_label(action: AdvancedAction, setup: &PlaySetupDraft) -> String {
    match action {
        AdvancedAction::CycleTeams => format!("Teams: {}", setup.teams),
        AdvancedAction::CycleTeamSize => {
            format!("Seats per team: {}", setup.members_per_team)
        }
        AdvancedAction::ToggleBotFill => format!(
            "Fill empty seats with bots: {}",
            if setup.fill_empty_seats { "ON" } else { "OFF" }
        ),
        AdvancedAction::ToggleGuardian => format!(
            "Guardian pressure: {}",
            if setup.guardian { "ON" } else { "OFF" }
        ),
        AdvancedAction::Launch => "Start custom run".to_string(),
        AdvancedAction::Back => "Back to presets".to_string(),
    }
}

fn play_summary(setup: &PlaySetupDraft) -> String {
    let mut summary = format!(
        "{}\n{}\n{}",
        setup.preset.label(),
        setup.preset.description(),
        setup.summary()
    );
    if !setup.fill_empty_seats && setup.teams.saturating_mul(setup.members_per_team) > 1 {
        summary.push_str("\nLocal: one controllable seat | LAN: remaining seats require humans");
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_cycles_never_exceed_sixteen_seats() {
        for teams in 1..=16 {
            for size in 1..=16 {
                if u16::from(teams) * u16::from(size) <= 16 {
                    let draft = PlaySetupDraft {
                        preset: PlayPreset::Custom,
                        teams,
                        members_per_team: size,
                        fill_empty_seats: true,
                        guardian: true,
                    };
                    assert!(draft.validate().is_ok());
                }
            }
        }
    }
}
