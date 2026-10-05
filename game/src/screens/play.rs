//! Preset-first Play Hub and its deliberately separate advanced setup page.

use bevy::{
    input_focus::tab_navigation::TabIndex, prelude::*, ui::InteractionDisabled,
    ui_widgets::Activate,
};

use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::GameState;
use crate::hex_wfc::{
    launch::{HexLaunchSpec, HexSeedPolicy},
    loading::HexLaunchRequestSequence,
};
use crate::play_setup::{
    LaunchContext, PlayPreset, PlayRules, PlaySeat, PlaySetupDraft, save_play_setup,
};
use crate::view::theme::{ACCENT, DIM, TITLE, menu_panel, panel, screen_root, text};

const HUB_SCOPE: FocusScopeId = FocusScopeId("play_hub");
const SOLO: WidgetId = WidgetId::named("play.preset.solo");
const CO_OP: WidgetId = WidgetId::named("play.preset.co_op");
const TEAM_RACE: WidgetId = WidgetId::named("play.preset.team_race");
const SPECTATE: WidgetId = WidgetId::named("play.preset.spectate");
const ASCENT: WidgetId = WidgetId::named("play.rules.ascent");
const RACE: WidgetId = WidgetId::named("play.rules.race");
const OBSERVER: WidgetId = WidgetId::named("play.role.observer");
const ARCHITECT: WidgetId = WidgetId::named("play.role.architect");
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
    SelectRules(PlayRules),
    SelectSeat(PlaySeat),
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
    commands.spawn((
        screen_root(GameState::Play),
        widgets::focus_scope(FocusScope::grid(HUB_SCOPE, START, BACK, 2)),
    )).with_children(|root| {
        root.spawn(text("ENTER THE FACILITY", 38.0, TITLE));
        root.spawn(text("Choose your role, then the company you keep.", 16.0, DIM));
        root.spawn(Node { column_gap: px(24), align_items: AlignItems::Stretch, ..default() })
            .with_children(|columns| {
                columns.spawn(menu_panel(480.0)).with_children(|choices| {
                    choices.spawn(text("RULES", 14.0, ACCENT));
                    choices.spawn(Node { column_gap: px(8), ..default() }).with_children(|row| {
                        for (order, id, rules) in [(0, ASCENT, PlayRules::Ascent), (1, RACE, PlayRules::Race)] {
                            widgets::spawn_button(row, WidgetSpec::enabled(id, HUB_SCOPE, order, selection_label(setup.rules == rules, rules_choice_label(rules))).with_size(208.0, 46.0), PlayAction::SelectRules(rules));
                        }
                    });
                    choices.spawn(text("YOUR ROLE IN ASCENT", 14.0, ACCENT));
                    choices.spawn(Node { column_gap: px(8), ..default() }).with_children(|row| {
                        for (order, id, seat) in [(2, OBSERVER, PlaySeat::Observer), (3, ARCHITECT, PlaySeat::Architect)] {
                            let mut spec = WidgetSpec::enabled(id, HUB_SCOPE, order, seat_label(seat, &setup)).with_size(208.0, 76.0);
                            if !roles_available(&setup) { spec.disabled = true; }
                            widgets::spawn_button(row, spec, PlayAction::SelectSeat(seat));
                        }
                    });
                    choices.spawn((RoleHint, text(role_hint(&setup), 14.0, DIM)));
                    choices.spawn(text("PLAY WITH", 14.0, ACCENT));
                    for presets in [
                        [(4, SOLO, PlayPreset::Solo), (5, CO_OP, PlayPreset::CoOp)],
                        [(6, TEAM_RACE, PlayPreset::TeamRace), (7, SPECTATE, PlayPreset::Spectate)],
                    ] {
                        choices.spawn(Node { column_gap: px(8), ..default() }).with_children(|row| {
                            for (order, id, preset) in presets {
                                widgets::spawn_button(row, WidgetSpec::enabled(id, HUB_SCOPE, order, preset_label(preset, &setup)).with_size(208.0, 66.0), PlayAction::SelectPreset(preset));
                            }
                        });
                    }
                    choices.spawn(Node { column_gap: px(8), ..default() }).with_children(|row| {
                        widgets::spawn_button(row, WidgetSpec::enabled(ADVANCED, HUB_SCOPE, 8, "Advanced setup").with_size(208.0, 54.0), PlayAction::Advanced);
                        widgets::spawn_button(row, WidgetSpec::enabled(LAN, HUB_SCOPE, 9, "LAN play").with_size(208.0, 54.0), PlayAction::Lan);
                    });
                });
                columns.spawn(menu_panel(440.0)).with_children(|run| {
                    run.spawn(text("YOUR RUN", 14.0, ACCENT));
                    run.spawn((PlaySummary, text(play_summary(&setup), 17.0, TITLE), Node { max_width: px(384), min_height: px(265), ..default() }));
                    run.spawn(text("LAN: joining uses the host's rules. Claim your team and Architect desk in the lobby.", 14.0, DIM));
                });
            });
        root.spawn(Node { column_gap: px(24), ..default() }).with_children(|row| {
            widgets::spawn_button(row, WidgetSpec::enabled(BACK, HUB_SCOPE, 10, "Back").with_size(480.0, 54.0), PlayAction::Back);
            widgets::spawn_button(row, WidgetSpec::enabled(START, HUB_SCOPE, 11, launch_label(&setup)).with_size(440.0, 54.0), PlayAction::Launch);
        });
        root.spawn(text("Arrow keys / D-pad / stick / pointer | Enter / A select | Esc / B back", 14.0, DIM));
    });
}

#[derive(Component)]
pub(crate) struct RoleHint;

fn roles_available(setup: &PlaySetupDraft) -> bool {
    setup.rules == PlayRules::Ascent && setup.preset != PlayPreset::Spectate
}

fn role_hint(setup: &PlaySetupDraft) -> &'static str {
    if setup.preset == PlayPreset::Spectate {
        "Bot view. Your playable role is remembered."
    } else if setup.rules == PlayRules::Race {
        "Facility race uses an Observer body."
    } else {
        "Observer: traverse. Architect: build from the map."
    }
}

pub(crate) fn setup_advanced(mut commands: Commands, setup: Res<PlaySetupDraft>) {
    commands
        .spawn(screen_root(GameState::PlayAdvanced))
        .with_children(|root| {
            root.spawn(text("ADVANCED SETUP", 42.0, TITLE));
            root.spawn(text(
                "Choose the roster for local play or LAN hosting. Maximum 16 Observer bodies.",
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
            root.spawn((PlaySummary, text(advanced_summary(&setup), 16.0, ACCENT), Node { width: px(920), ..default() }));
            root.spawn(text(
                "Activate a row to cycle it. Local no-fill uses one Observer body; an Architect's body seats are bot-driven.",
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
        PlayAction::SelectRules(rules) => {
            setup.select_rules(rules);
            save_play_setup(&setup);
        }
        PlayAction::SelectSeat(seat) => {
            if roles_available(&setup) {
                setup.seat = seat;
                save_play_setup(&setup);
            }
        }
        PlayAction::Advanced => next.set(GameState::PlayAdvanced),
        PlayAction::Launch => launch_local(&mut commands, &mut sequence, &setup, &mut next),
        PlayAction::Lan => {
            let role = if setup.rules == PlayRules::Ascent && setup.seat == PlaySeat::Architect {
                observed_core::lan::LanRole::Architect
            } else {
                observed_core::lan::LanRole::Observer
            };
            commands.queue(move |world: &mut World| {
                world
                    .resource_mut::<crate::lan::LanRuntime>()
                    .requested_role = role;
            });
            next.set(GameState::LanBrowser);
        }
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
            let maximum = setup.maximum_team_size();
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
    mut commands: Commands,
    mut preset_buttons: Query<(Entity, &PlayAction, &mut WidgetLabel)>,
    mut summaries: Query<&mut Text, (With<PlaySummary>, Without<RoleHint>)>,
    mut hints: Query<&mut Text, (With<RoleHint>, Without<PlaySummary>)>,
) {
    if !setup.is_changed() {
        return;
    }
    for (entity, action, mut label) in &mut preset_buttons {
        if let PlayAction::SelectSeat(seat) = action {
            let order = if *seat == PlaySeat::Observer { 2 } else { 3 };
            if roles_available(&setup) {
                commands
                    .entity(entity)
                    .remove::<InteractionDisabled>()
                    .insert(TabIndex(order));
            } else {
                commands
                    .entity(entity)
                    .insert(InteractionDisabled)
                    .remove::<TabIndex>();
            }
        }
        label.0 = match action {
            PlayAction::SelectPreset(preset) => preset_label(*preset, &setup),
            PlayAction::SelectRules(rules) => {
                selection_label(setup.rules == *rules, rules_choice_label(*rules))
            }
            PlayAction::SelectSeat(seat) => seat_label(*seat, &setup),
            PlayAction::Launch => launch_label(&setup),
            PlayAction::Advanced => "Advanced setup".to_string(),
            PlayAction::Lan => "LAN play".to_string(),
            PlayAction::Back => "Back".to_string(),
        };
    }
    for mut summary in &mut summaries {
        **summary = play_summary(&setup);
    }
    for mut hint in &mut hints {
        **hint = role_hint(&setup).to_string();
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
        **summary = advanced_summary(&setup);
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
        (setup.rules, setup.seat),
    ));
    next.set(GameState::Loading);
}

/// Selection is a shape, not a colour, so it survives a monochrome or colour-blind
/// reading — and it is spelled in ASCII because the shipped default font is a subset
/// with no geometric shapes: `◆`/`◇` rendered as blank tofu, which made the selected
/// preset invisible at the 1280×800 gate.
fn rules_choice_label(rules: PlayRules) -> &'static str {
    match rules {
        PlayRules::Ascent => "Ascent",
        PlayRules::Race => "Facility race",
    }
}

fn advanced_summary(setup: &PlaySetupDraft) -> String {
    let Ok(validated) = setup.validate() else {
        return setup.summary();
    };
    let config = validated.local_match_config(observed_facility::hex_wfc::HexWfcConfig::default());
    format!(
        "{} | {}\nLocal: {} team(s) x {} Observer bodies\nLAN host: {} team(s) x {} bodies | empty seats {}",
        setup.rules.label(),
        launch_label(setup),
        config.teams,
        config.members_per_team,
        setup.teams,
        setup.members_per_team,
        if setup.fill_empty_seats {
            "bot-filled"
        } else {
            "require humans"
        }
    )
}

fn selection_label(selected: bool, label: &str) -> String {
    format!("{} {label}", if selected { "[*]" } else { "[ ]" })
}

fn preset_name(preset: PlayPreset, rules: PlayRules) -> &'static str {
    if preset == PlayPreset::TeamRace && rules == PlayRules::Ascent {
        "Team competition"
    } else {
        preset.label()
    }
}

fn preset_label(preset: PlayPreset, setup: &PlaySetupDraft) -> String {
    selection_label(setup.preset == preset, preset_name(preset, setup.rules))
}

fn seat_label(seat: PlaySeat, setup: &PlaySetupDraft) -> String {
    let label = match seat {
        PlaySeat::Observer => "Observer\nTraverse",
        PlaySeat::Architect => "Architect\nBuild from map",
    };
    selection_label(roles_available(setup) && setup.seat == seat, label)
}

fn launch_label(setup: &PlaySetupDraft) -> String {
    if setup.preset == PlayPreset::Spectate {
        "Watch bot match".into()
    } else if setup.rules == PlayRules::Race {
        "Start facility race".into()
    } else {
        match setup.seat {
            PlaySeat::Observer => "Start as Observer",
            PlaySeat::Architect => "Start as Architect",
        }
        .into()
    }
}

fn advanced_label(action: AdvancedAction, setup: &PlaySetupDraft) -> String {
    match action {
        AdvancedAction::CycleTeams => format!("Teams: {}", setup.teams),
        AdvancedAction::CycleTeamSize => {
            format!("Observer bodies per team: {}", setup.members_per_team)
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
    let Ok(validated) = setup.validate() else {
        return setup.summary();
    };
    let config = validated.local_match_config(observed_facility::hex_wfc::HexWfcConfig::default());
    let roster = format!(
        "{} team{} x {} Observer bod{}",
        config.teams,
        if config.teams == 1 { "" } else { "s" },
        config.members_per_team,
        if config.members_per_team == 1 {
            "y"
        } else {
            "ies"
        }
    );
    let role = if validated.spectator {
        "You: bot view"
    } else if setup.rules == PlayRules::Ascent && setup.seat == PlaySeat::Architect {
        "You: Architect desk"
    } else {
        "You: Observer body"
    };
    let goal = match setup.rules {
        PlayRules::Race => "Collect keystones, sync the station, regroup at the exit.",
        PlayRules::Ascent => {
            "Bring all your Observers to the summit.\nCaught: prison. Falls: corruption."
        }
    };
    let architect = if setup.rules == PlayRules::Ascent {
        if setup.seat == PlaySeat::Architect && !validated.spectator {
            if config.teams == 1 {
                "Architect desk: you".to_string()
            } else {
                format!(
                    "Architects: you + {} bot desk{}",
                    config.teams - 1,
                    if config.teams == 2 { "" } else { "s" }
                )
            }
        } else {
            format!(
                "Architects: {} separate bot desk{}",
                config.teams,
                if config.teams == 1 { "" } else { "s" }
            )
        }
    } else {
        "Unwatched connections can change.".to_string()
    };
    let bots = config.teams * config.members_per_team
        - u8::from(
            !validated.spectator
                && !(setup.rules == PlayRules::Ascent && setup.seat == PlaySeat::Architect),
        );
    let bot_roster = format!(
        "{bots} bot Observer {}",
        if bots == 1 { "body" } else { "bodies" }
    );
    format!(
        "{} | {}\n\n{roster}\n{role}\n{bot_roster}\n{architect}\n\n{goal}\nGuardian {}",
        setup.rules.label(),
        preset_name(setup.preset, setup.rules),
        if setup.guardian { "on" } else { "off" }
    )
}

#[cfg(test)]
#[path = "play_tests.rs"]
mod tests;
