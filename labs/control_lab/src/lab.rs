use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    controls::{GamepadRegistry, LabCommand},
    model::{
        ControlSource, HumanDevice, KeyboardBindings, KeyboardSlot, LabNotice, LabRuntime,
        PLAYER_COUNT, PLAYERS, PlayerId, PlayerIntent, ProbeState, RebindCapture, RecordingBank,
        ResetRequested, ScriptPattern,
    },
};

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.20, 0.82, 1.0),
    Color::srgb(1.0, 0.38, 0.32),
    Color::srgb(0.55, 0.95, 0.30),
    Color::srgb(0.82, 0.42, 1.0),
];

#[derive(Component)]
pub(crate) struct ControlLabOwned;

#[derive(Component)]
pub(crate) struct ControlLabUiRoot;

#[derive(Component)]
pub(crate) struct ProbeVisual;

#[derive(Component)]
pub(crate) struct PlayerSummary(PlayerId);

#[derive(Component)]
pub(crate) struct PlayerCard(PlayerId);

#[derive(Component)]
pub(crate) struct FooterStatus;

#[derive(Component)]
pub(crate) struct NoticeText;

#[derive(Component)]
pub(crate) struct CommandButton(LabCommand);

type CommandButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static CommandButton),
    (Changed<Interaction>, With<Button>),
>;

type SummaryQuery<'w, 's> = Query<
    'w,
    's,
    (&'static PlayerSummary, &'static mut Text),
    (Without<FooterStatus>, Without<NoticeText>),
>;

type FooterSingle<'w, 's> = Single<
    'w,
    's,
    (&'static mut Text, &'static mut TextColor),
    (
        With<FooterStatus>,
        Without<PlayerSummary>,
        Without<NoticeText>,
    ),
>;

type NoticeSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text,
    (
        With<NoticeText>,
        Without<FooterStatus>,
        Without<PlayerSummary>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct ResetContext<'w, 's> {
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, Entity, With<ControlLabOwned>>,
    reset: ResMut<'w, ResetRequested>,
    bindings: ResMut<'w, KeyboardBindings>,
    recordings: ResMut<'w, RecordingBank>,
    runtime: ResMut<'w, LabRuntime>,
    rebind: ResMut<'w, RebindCapture>,
    registry: ResMut<'w, GamepadRegistry>,
    notice: ResMut<'w, LabNotice>,
}

#[derive(SystemParam)]
pub(crate) struct DebugUiContext<'w, 's> {
    runtime: Res<'w, LabRuntime>,
    recordings: Res<'w, RecordingBank>,
    registry: Res<'w, GamepadRegistry>,
    notice: Res<'w, LabNotice>,
    players: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static ControlSource,
            &'static PlayerIntent,
        ),
    >,
    probes: Query<'w, 's, (), With<ProbeVisual>>,
    ui_roots: Query<'w, 's, (), With<ControlLabUiRoot>>,
    summaries: SummaryQuery<'w, 's>,
    footer: FooterSingle<'w, 's>,
    notice_text: NoticeSingle<'w, 's>,
}

pub(crate) fn setup_lab(mut commands: Commands) {
    spawn_lab(&mut commands);
}

pub(crate) fn perform_reset(mut context: ResetContext) {
    if !context.reset.0 {
        return;
    }

    context.reset.0 = false;
    for entity in &context.roots {
        context.commands.entity(entity).despawn();
    }

    let focused = context.runtime.focused;
    let focus_losses = context.runtime.focus_losses;
    let reset_count = context.runtime.reset_count + 1;
    *context.bindings = KeyboardBindings::default();
    *context.recordings = RecordingBank::default();
    *context.runtime = LabRuntime {
        focused,
        focus_losses,
        reset_count,
        ..default()
    };
    *context.rebind = RebindCapture::default();
    *context.registry = GamepadRegistry::default();
    context.notice.0 = format!("Reset {reset_count} complete; baseline assignments restored.");

    spawn_lab(&mut context.commands);
}

fn spawn_lab(commands: &mut Commands) {
    spawn_world(commands);
    spawn_ui(commands);
}

fn spawn_world(commands: &mut Commands) {
    commands
        .spawn((
            ControlLabOwned,
            Name::new("Control Lab World Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Control Arena"),
                Sprite::from_color(Color::srgb(0.035, 0.065, 0.10), Vec2::new(860.0, 520.0)),
                Transform::from_xyz(-180.0, -15.0, -2.0),
            ));

            for x in [-390.0, 30.0] {
                parent.spawn((
                    Sprite::from_color(Color::srgba(0.25, 0.72, 0.90, 0.25), Vec2::new(2.0, 500.0)),
                    Transform::from_xyz(x + 210.0, -15.0, -1.0),
                ));
            }
            parent.spawn((
                Sprite::from_color(Color::srgba(0.25, 0.72, 0.90, 0.25), Vec2::new(840.0, 2.0)),
                Transform::from_xyz(-180.0, -15.0, -1.0),
            ));

            let positions = [
                Vec2::new(-390.0, 110.0),
                Vec2::new(30.0, 110.0),
                Vec2::new(-390.0, -140.0),
                Vec2::new(30.0, -140.0),
            ];
            for (index, player) in PLAYERS.into_iter().enumerate() {
                let source = match player.index() {
                    0 => ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::A)),
                    1 => ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::B)),
                    _ => ControlSource::Scripted(ScriptPattern::for_player(player)),
                };
                parent.spawn((
                    player,
                    source,
                    PlayerIntent::default(),
                    ProbeState {
                        position: positions[index],
                        spawn_position: positions[index],
                    },
                    ProbeVisual,
                    Name::new(format!("{} Control Probe", player.label())),
                    Sprite::from_color(PLAYER_COLORS[index], Vec2::new(56.0, 56.0)),
                    Transform::from_xyz(positions[index].x, positions[index].y, 1.0),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ControlLabOwned,
            ControlLabUiRoot,
            Name::new("Control Lab UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(18)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                children![
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            ..default()
                        },
                        children![
                            text_bundle("OBSERVED 2 / CONTROL LAB", 28.0, Color::WHITE),
                            text_bundle(
                                "Hardware → assignment → PlayerIntent → probe behavior",
                                15.0,
                                Color::srgb(0.45, 0.76, 0.92),
                            ),
                        ]
                    ),
                    (
                        Node {
                            width: px(350),
                            padding: UiRect::all(px(12)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.015, 0.03, 0.05, 0.94)),
                        children![(
                            NoticeText,
                            Text::new("Ready. Select a player with 1–4."),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.65, 0.86, 1.0)),
                        )]
                    )
                ],
            ));

            root.spawn(Node {
                width: percent(100),
                flex_grow: 1.0,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|content| {
                content
                    .spawn((
                        Node {
                            width: px(390),
                            padding: UiRect::all(px(10)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(8),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.015, 0.03, 0.05, 0.94)),
                    ))
                    .with_children(|cards| {
                        for player in PLAYERS {
                            spawn_player_card(cards, player);
                        }
                    });
            });

            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(118),
                    padding: UiRect::all(px(12)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.03, 0.05, 0.96)),
            ))
            .with_children(|footer| {
                footer
                    .spawn(Node {
                        width: percent(100),
                        column_gap: px(8),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|buttons| {
                        for (label, command) in [
                            ("Human / Script", LabCommand::ToggleHumanScripted),
                            ("Cycle device", LabCommand::CycleDevice),
                            ("Record", LabCommand::ToggleRecording),
                            ("Playback", LabCommand::PlayRecording),
                            ("Rebind Jump", LabCommand::BeginJumpRebind),
                            ("Reset", LabCommand::Reset),
                        ] {
                            spawn_command_button(buttons, label, command);
                        }
                    });
                footer.spawn((
                    FooterStatus,
                    Text::new("Checking invariants…"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.60, 0.85, 1.0)),
                    TextLayout::new_with_justify(Justify::Center),
                    Node {
                        width: percent(100),
                        ..default()
                    },
                ));
            });
        });
}

fn spawn_player_card(parent: &mut ChildSpawnerCommands, player: PlayerId) {
    parent
        .spawn((
            PlayerCard(player),
            CommandButton(LabCommand::Select(player)),
            Button,
            Node {
                width: percent(100),
                min_height: px(92),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(2)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.25, 0.55, 0.72, 0.45)),
            BackgroundColor(Color::srgb(0.035, 0.07, 0.11)),
        ))
        .with_children(|card| {
            card.spawn((
                PlayerSummary(player),
                Text::new(format!("{} initializing…", player.label())),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_command_button(parent: &mut ChildSpawnerCommands, label: &str, command: LabCommand) {
    parent
        .spawn((
            CommandButton(command),
            Button,
            Node {
                width: px(150),
                height: px(40),
                border: UiRect::all(px(1)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::srgba(0.25, 0.62, 0.82, 0.65)),
            BackgroundColor(Color::srgb(0.06, 0.13, 0.20)),
        ))
        .with_children(|button| {
            button.spawn(text_bundle(label, 14.0, Color::WHITE));
        });
}

fn text_bundle(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

pub(crate) fn command_buttons(
    buttons: CommandButtonQuery,
    mut commands: MessageWriter<LabCommand>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            commands.write(button.0);
        }
    }
}

pub(crate) fn button_visuals(
    runtime: Res<LabRuntime>,
    mut buttons: Query<
        (
            &Interaction,
            Option<&PlayerCard>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
) {
    for (interaction, card, mut background, mut border) in &mut buttons {
        let selected = card.is_some_and(|card| card.0 == runtime.selected_player);
        *background = match (*interaction, selected) {
            (Interaction::Pressed, _) => Color::srgb(0.12, 0.52, 0.66).into(),
            (Interaction::Hovered, _) => Color::srgb(0.09, 0.25, 0.36).into(),
            (Interaction::None, true) => Color::srgb(0.06, 0.22, 0.31).into(),
            (Interaction::None, false) => Color::srgb(0.035, 0.07, 0.11).into(),
        };
        *border = BorderColor::all(if selected {
            Color::srgb(0.30, 0.90, 1.0)
        } else {
            Color::srgba(0.25, 0.55, 0.72, 0.45)
        });
    }
}

pub(crate) fn consume_intents(
    time: Res<Time>,
    mut probes: Query<(&PlayerIntent, &mut ProbeState)>,
) {
    for (intent, mut state) in &mut probes {
        let speed = if intent.sprint_held { 185.0 } else { 110.0 };
        state.position += intent.movement * speed * time.delta_secs();

        let offset = state.position - state.spawn_position;
        state.position = state.spawn_position
            + Vec2::new(offset.x.clamp(-145.0, 145.0), offset.y.clamp(-78.0, 78.0));
    }
}

pub(crate) fn present_probes(
    mut probes: Query<
        (
            &PlayerId,
            &PlayerIntent,
            &ProbeState,
            &mut Transform,
            &mut Sprite,
        ),
        With<ProbeVisual>,
    >,
) {
    for (player, intent, state, mut transform, mut sprite) in &mut probes {
        transform.translation.x = state.position.x;
        transform.translation.y = state.position.y;
        if intent.look.length_squared() > 0.001 {
            transform.rotation = Quat::from_rotation_z(
                intent.look.y.atan2(intent.look.x) - std::f32::consts::FRAC_PI_2,
            );
        }
        let pulse = if intent.jump_pressed || intent.interact_pressed || intent.climb_pressed {
            1.22
        } else {
            1.0
        };
        transform.scale = Vec3::splat(pulse);
        sprite.color = if intent.sprint_held {
            Color::WHITE
        } else {
            PLAYER_COLORS[player.index()]
        };
    }
}

pub(crate) fn update_debug_ui(mut context: DebugUiContext) {
    for (summary, mut text) in &mut context.summaries {
        let Some((_, source, intent)) = context
            .players
            .iter()
            .find(|(player, _, _)| **player == summary.0)
        else {
            continue;
        };
        let frames = context.recordings.track(summary.0).len();
        let recording = if context.recordings.recording == Some(summary.0) {
            " RECORDING"
        } else {
            ""
        };
        **text = format!(
            "{}{}  |  {}\nmove {:+.2} {:+.2}   look {:+.2} {:+.2}\nJ:{}  S:{}  I:{}  C:{}   tape:{frames}{recording}",
            summary.0.label(),
            if context.runtime.selected_player == summary.0 {
                " [SELECTED]"
            } else {
                ""
            },
            source.label(),
            intent.movement.x,
            intent.movement.y,
            intent.look.x,
            intent.look.y,
            bit(intent.jump_pressed),
            bit(intent.sprint_held),
            bit(intent.interact_pressed),
            bit(intent.climb_pressed),
        );
    }

    let mut notice_text = context.notice_text.into_inner();
    **notice_text = context.notice.0.clone();

    let probe_count = context.probes.iter().count();
    let root_count = context.ui_roots.iter().count();
    let healthy = probe_count == PLAYER_COUNT && root_count == 1;
    let (mut footer_text, mut footer_color) = context.footer.into_inner();
    **footer_text = format!(
        "1–4 select • Tab human/script • C device • F5 record • F6 playback • F7 rebind • F8 reset\n\
         focus:{}  losses:{}  gamepads:{}  probes:{probe_count}  UI roots:{root_count}  resets:{}  {}",
        if context.runtime.focused {
            "LIVE"
        } else {
            "BLOCKED"
        },
        context.runtime.focus_losses,
        context.registry.devices.len(),
        context.runtime.reset_count,
        if healthy { "[PASS]" } else { "[FAIL]" },
    );
    *footer_color = TextColor(if healthy {
        Color::srgb(0.48, 1.0, 0.68)
    } else {
        Color::srgb(1.0, 0.35, 0.30)
    });
}

fn bit(value: bool) -> char {
    if value { '1' } else { '0' }
}
