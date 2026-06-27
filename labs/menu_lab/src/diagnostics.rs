use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::{
    AppState, LabSettings,
    session::{GameOwned, GameplaySession},
    ui::ScreenRoot,
};

#[derive(Resource, Debug, Default)]
pub struct LifecycleDiagnostics {
    pub generations: u32,
    pub resets: u32,
    pub cleanup_attempts: u32,
    pub last_cleanup: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LifecycleHealth {
    pub screen_roots_ok: bool,
    pub session_entities_ok: bool,
    pub session_resource_ok: bool,
}

#[derive(Component)]
pub(crate) struct DiagnosticsText;

#[derive(Component)]
pub(crate) struct DiagnosticsOverlay;

#[derive(SystemParam)]
pub(crate) struct DiagnosticQueries<'w, 's> {
    screen_roots: Query<'w, 's, (), With<ScreenRoot>>,
    game_owned: Query<'w, 's, (), With<GameOwned>>,
    overlay: Single<'w, 's, &'static mut Visibility, With<DiagnosticsOverlay>>,
    text: Single<'w, 's, (&'static mut Text, &'static mut TextColor), With<DiagnosticsText>>,
}

pub(crate) fn setup_overlay(mut commands: Commands) {
    commands.spawn((
        DiagnosticsOverlay,
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            right: px(14),
            width: px(330),
            padding: UiRect::all(px(14)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.35, 0.75, 1.0, 0.65)),
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.92)),
        GlobalZIndex(100),
        children![(
            DiagnosticsText,
            Text::new("Lifecycle diagnostics starting…"),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.76, 0.88, 1.0)),
            TextLayout::new_with_justify(Justify::Left),
        )],
    ));
}

pub(crate) fn update_overlay(
    state: Res<State<AppState>>,
    settings: Res<LabSettings>,
    diagnostics: Res<LifecycleDiagnostics>,
    session: Option<Res<GameplaySession>>,
    mut queries: DiagnosticQueries,
) {
    **queries.overlay = if settings.diagnostics_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    let screen_count = queries.screen_roots.iter().count();
    let owned_count = queries.game_owned.iter().count();
    let expected = session.as_ref().map(|value| value.owned_entities);
    let health = evaluate_health(*state.get(), screen_count, owned_count, expected);

    let cleanup = diagnostics
        .last_cleanup
        .map(|(before, after)| format!("{before} → {after} {}", pass(after == 0)))
        .unwrap_or_else(|| "not run".to_string());

    let generation = session
        .as_ref()
        .map(|value| value.generation.to_string())
        .unwrap_or_else(|| "—".to_string());

    let (mut text, mut text_color) = queries.text.into_inner();
    **text = format!(
        "LIFECYCLE MONITOR\n\
         STATE          {:?}\n\
         SCREEN ROOTS   {screen_count} {}\n\
         SESSION        {owned_count} / {} {}\n\
         RESOURCE       {} {}\n\
         GENERATION     {generation}\n\
         RESETS         {}\n\
         CLEANUPS       {}\n\
         LAST CLEANUP   {cleanup}\n\n\
         Esc pause/back  •  R reset  •  F1 monitor",
        state.get(),
        pass(health.screen_roots_ok),
        expected.unwrap_or(0),
        pass(health.session_entities_ok),
        if session.is_some() {
            "present"
        } else {
            "absent"
        },
        pass(health.session_resource_ok),
        diagnostics.resets,
        diagnostics.cleanup_attempts,
    );

    let all_ok = health.screen_roots_ok
        && health.session_entities_ok
        && health.session_resource_ok
        && diagnostics.last_cleanup.is_none_or(|(_, after)| after == 0);
    *text_color = if all_ok {
        TextColor(Color::srgb(0.55, 1.0, 0.72))
    } else {
        TextColor(Color::srgb(1.0, 0.42, 0.38))
    };
}

pub(crate) fn evaluate_health(
    state: AppState,
    screen_roots: usize,
    owned_entities: usize,
    expected_entities: Option<usize>,
) -> LifecycleHealth {
    let session_expected = matches!(state, AppState::Gameplay | AppState::Paused);

    LifecycleHealth {
        screen_roots_ok: screen_roots == 1,
        session_entities_ok: if session_expected {
            expected_entities.is_some_and(|expected| expected == owned_entities)
        } else {
            owned_entities == 0
        },
        session_resource_ok: session_expected == expected_entities.is_some(),
    }
}

fn pass(value: bool) -> &'static str {
    if value { "[PASS]" } else { "[FAIL]" }
}
