//! The in-match 2D overlay: the text HUD + pause panel ([`match_draw`]) and the Tab
//! tac-map ([`draw_tac_map`]), a top-down schematic rebuilt each frame from the live
//! match and keystone inventory (see [`crate::tacmap`]).

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_facility::map_spec::RoomRole;
use observed_style::{self as style, MarkerRole, SurfaceRole};

use super::input::gamepad_map_pressed;
use crate::flow::LOCAL_TEAM;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{MatchPaused, SpectatorBot, TeleportState};
use crate::view::components::{
    MatchHud, PausePanel, TacMapElement, TacMapPanel, TacMapState, TeleportAnimation,
    TeleportOverlay,
};
use crate::view::theme::{BORDER, PANEL, TAC_MAP_SIZE, TITLE, screen_root, text};
use crate::{GameState, tacmap};

// Tac-map overlay layout (pixels). The 3×3 grid of rooms sits below a title strip.
const TAC_TITLE_H: f32 = 26.0;
const TAC_INSET: f32 = 22.0;
const TAC_ROOM: f32 = 46.0; // room square size

/// Toggle the tac-map overlay with Tab (shows/hides the panel root; `draw_tac_map`
/// fills it while shown).
pub(crate) fn toggle_tac_map(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut state: ResMut<TacMapState>,
    mut panel: Query<&mut Visibility, With<TacMapPanel>>,
) {
    if keyboard.just_pressed(KeyCode::Tab) || gamepad_map_pressed(&gamepads) {
        state.0 = !state.0;
        if let Ok(mut visibility) = panel.single_mut() {
            *visibility = if state.0 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

/// A room centre in the tac-map panel's local pixel frame.
fn tac_bounds(rooms: &[(RoomId, Vec2, RoomRole)]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for (_, pos, _) in rooms {
        min = min.min(*pos);
        max = max.max(*pos);
    }
    if rooms.is_empty() {
        (Vec2::ZERO, Vec2::ONE)
    } else {
        (min, max)
    }
}

fn tac_center_for_pos(pos: Vec2, bounds: (Vec2, Vec2)) -> Vec2 {
    let (min, max) = bounds;
    let span = (max - min).max(Vec2::ONE);
    let g = (pos - min) / span;
    let usable_w = TAC_MAP_SIZE - TAC_INSET * 2.0 - TAC_ROOM;
    let usable_h = TAC_MAP_SIZE - TAC_TITLE_H - TAC_INSET * 2.0 - TAC_ROOM;
    Vec2::new(
        TAC_INSET + g.x * usable_w + TAC_ROOM * 0.5,
        TAC_TITLE_H + TAC_INSET + g.y * usable_h + TAC_ROOM * 0.5,
    )
}

/// An absolutely-positioned filled box centred at `center` — the tac-map's building block.
fn tac_box(center: Vec2, w: f32, h: f32, color: Color) -> impl Bundle {
    (
        TacMapElement,
        DespawnOnExit(GameState::Match),
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - w * 0.5),
            top: px(center.y - h * 0.5),
            width: px(w),
            height: px(h),
            ..default()
        },
        BackgroundColor(color),
    )
}

/// Rebuild the tac-map's room/route/marker nodes from the live match each frame while it
/// is shown. Presentation-only — reads the brain + keystone inventory (see `tacmap`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tac_map(
    state: Res<TacMapState>,
    director: Res<MatchDirector>,
    spectator_bot: Option<Res<SpectatorBot>>,
    tp: Res<TeleportState>,
    keys: Res<KeystoneState>,
    panel: Query<Entity, With<TacMapPanel>>,
    existing: Query<Entity, With<TacMapElement>>,
    mut commands: Commands,
) {
    // Tear down the previous frame's dynamic nodes (the static title child is untagged).
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !state.0 {
        return;
    }
    let Ok(panel) = panel.single() else {
        return;
    };
    let model = tacmap::build_map(&director.live.host_match().competitive, &keys, tp.place);
    let bounds = tac_bounds(&model.rooms);
    let room_centers: Vec<(RoomId, Vec2)> = model
        .rooms
        .iter()
        .map(|(room, pos, _)| (*room, tac_center_for_pos(*pos, bounds)))
        .collect();
    let center_for = |room: RoomId| {
        room_centers
            .iter()
            .find_map(|(id, center)| (*id == room).then_some(*center))
            .unwrap_or_else(|| tac_center_for_pos(tacmap::grid_pos(room), bounds))
    };

    // Palette: route/markers from the shared style markers so the map matches the
    // in-world legend; quiet fills for the room boxes themselves.
    let spine_fill = Color::srgb(0.30, 0.26, 0.10);
    let plain_fill = Color::srgb(0.10, 0.13, 0.18);
    let route_col = style::marker(MarkerRole::NextRoom).base_color;
    let you_col = style::marker(MarkerRole::You).base_color;
    let rival_col = style::marker(MarkerRole::Rival).base_color;
    let exit_open_col = style::marker(MarkerRole::Exit).base_color;
    let collapse_col = style::marker(MarkerRole::Collapse).base_color;
    let locked_col = Color::srgb(1.0, 0.32, 0.22);
    let key_col = Color::srgb(1.0, 0.82, 0.3);

    let rival_count = model.rivals.len() as f32;

    commands.entity(panel).with_children(|p| {
        // Route bars first, under the rooms.
        for &(a, b) in &model.routes {
            let (c1, c2) = (center_for(a), center_for(b));
            let mid = (c1 + c2) * 0.5;
            let (w, h) = if (c1.y - c2.y).abs() < 1.0 {
                ((c1.x - c2.x).abs(), 5.0)
            } else if (c1.x - c2.x).abs() < 1.0 {
                (5.0, (c1.y - c2.y).abs())
            } else {
                ((c1.x - c2.x).abs().max(5.0), (c1.y - c2.y).abs().max(5.0))
            };
            p.spawn((
                tac_box(mid, w, h, route_col.with_alpha(0.5)),
                crate::evidence::DiagnosticTacMapVisual::route(a, b),
            ));
        }
        // Room squares: collapse-swallowed rooms read red; objective/tool rooms read warm.
        for &(room, _, role) in &model.rooms {
            let fill = if model.collapse.contains(&room) {
                collapse_col.with_alpha(0.55)
            } else if matches!(
                role,
                RoomRole::Start
                    | RoomRole::Exit
                    | RoomRole::Keystone
                    | RoomRole::AnchorCheckpoint
                    | RoomRole::TeleportRelay
                    | RoomRole::DualStation
            ) {
                spine_fill
            } else {
                plain_fill
            };
            p.spawn((
                tac_box(center_for(room), TAC_ROOM, TAC_ROOM, fill),
                crate::evidence::DiagnosticTacMapVisual::room(room),
            ));
        }
        // The exit room: a green (open) or red (locked) outline.
        let exit_center = center_for(model.exit);
        p.spawn((
            TacMapElement,
            DespawnOnExit(GameState::Match),
            Node {
                position_type: PositionType::Absolute,
                left: px(exit_center.x - TAC_ROOM * 0.5),
                top: px(exit_center.y - TAC_ROOM * 0.5),
                width: px(TAC_ROOM),
                height: px(TAC_ROOM),
                border: UiRect::all(px(3)),
                ..default()
            },
            BorderColor::all(if model.exit_open {
                exit_open_col
            } else {
                locked_col
            }),
            crate::evidence::DiagnosticTacMapVisual::one(
                crate::evidence::DiagnosticTacMapRole::Exit,
                Some(model.exit),
            ),
        ));
        // Keystone pips in the top-right of their room.
        for room in &model.keystones {
            let c = center_for(*room) + Vec2::new(TAC_ROOM * 0.5 - 7.0, -(TAC_ROOM * 0.5) + 7.0);
            p.spawn((
                tac_box(c, 10.0, 10.0, key_col),
                crate::evidence::DiagnosticTacMapVisual::one(
                    crate::evidence::DiagnosticTacMapRole::Keystone,
                    Some(*room),
                ),
            ));
        }
        // Rival pips, fanned so several in one room stay distinct.
        for (slot, (_, room)) in model.rivals.iter().enumerate() {
            let off = Vec2::new((slot as f32 - (rival_count - 1.0) * 0.5) * 9.0, 8.0);
            p.spawn((
                tac_box(center_for(*room) + off, 13.0, 13.0, rival_col),
                crate::evidence::DiagnosticTacMapVisual::one(
                    crate::evidence::DiagnosticTacMapRole::Rival,
                    Some(*room),
                ),
            ));
        }
        if let Some(bot) = spectator_bot.as_ref()
            && let Some(team) = bot.teamplay.team(bot.focused_team)
        {
            let member_count = team.members.len() as f32;
            for (index, member) in team.members.iter().enumerate() {
                let off = Vec2::new((index as f32 - (member_count - 1.0) * 0.5) * 12.0, -9.0);
                p.spawn((
                    tac_box(
                        center_for(member.room) + off,
                        9.0,
                        9.0,
                        you_col.with_alpha(0.85),
                    ),
                    crate::evidence::DiagnosticTacMapVisual::one(
                        crate::evidence::DiagnosticTacMapRole::Player,
                        Some(member.room),
                    ),
                ));
            }
        }
        // YOU: room centre, or the midpoint of the hallway you're walking.
        let you = match model.player {
            tacmap::PlayerMark::Room(r) => center_for(r),
            tacmap::PlayerMark::Between(a, b) => (center_for(a) + center_for(b)) * 0.5,
        };
        let player_room = match model.player {
            tacmap::PlayerMark::Room(room) => Some(room),
            tacmap::PlayerMark::Between(_, _) => None,
        };
        p.spawn((
            tac_box(you, 16.0, 16.0, you_col),
            crate::evidence::DiagnosticTacMapVisual::one(
                crate::evidence::DiagnosticTacMapRole::Player,
                player_room,
            ),
        ));
        p.spawn((
            TacMapElement,
            DespawnOnExit(GameState::Match),
            Node {
                position_type: PositionType::Absolute,
                left: px(10),
                bottom: px(8),
                ..default()
            },
            Text::new(format!(
                "SERIES R{} | alive {} | countdown {}",
                director.series.current.index,
                director.series.active_team_count(),
                director
                    .series
                    .current
                    .remaining_countdown()
                    .map_or("--".to_string(), |rounds| rounds.to_string())
            )),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(TITLE),
        ));
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_draw(
    director: Res<MatchDirector>,
    spectator_bot: Option<Res<SpectatorBot>>,
    paused: Res<MatchPaused>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    log: Res<crate::guardian::ActionLog>,
    tp: Res<TeleportState>,
    mut hud: Query<&mut Text, With<MatchHud>>,
    mut pause_panel: Query<&mut Visibility, With<PausePanel>>,
) {
    let live = &director.live;
    let game = live.host_match();
    let facility = &game.competitive;
    let local_series = director.series.team_objective(LOCAL_TEAM);
    let local_series_status = local_series
        .map(|team| team.status_label(director.series.current.required_keystones()))
        .unwrap_or_else(|| "between rounds".to_string());
    let countdown = director
        .series
        .current
        .remaining_countdown()
        .map_or("--".to_string(), |rounds| rounds.to_string());
    let control_line = spectator_bot.as_ref().map_or("manual control", |bot| {
        if bot.finished {
            "spectating AI: visible run stopped"
        } else if director.series.finished() {
            "spectating AI: series ready, finishing visible run"
        } else {
            "spectating AI: bot driving"
        }
    });
    let teamplay_line = spectator_bot.as_ref().map_or_else(String::new, |bot| {
        let plan = &bot.teamplay.plan;
        let focused = bot
            .teamplay
            .team(bot.focused_team)
            .map(|team| team.status_line(plan.keystone_rooms.len()))
            .unwrap_or_else(|| "focused team eliminated".to_string());
        format!(
            "BOT CO-OP seed {} R{} tick {} | gate R{} anchor R{} pads R{}->R{} guardian R{}\n\
             focused: {} | co-op {} anchors {} pads {} pad jumps {} guardian catches {}\n\
             bot event: {}",
            bot.seed,
            bot.teamplay.round_index,
            bot.teamplay.tick,
            plan.dual_station_room.0,
            plan.anchor_room.0,
            plan.relay_rooms.0.0,
            plan.relay_rooms.1.0,
            plan.guardian_room.0,
            focused,
            bot.teamplay.metrics.co_op_completions,
            bot.teamplay.metrics.anchors_dropped,
            bot.teamplay.metrics.pads_dropped,
            bot.teamplay.metrics.pad_uses,
            bot.teamplay.metrics.guardian_catches,
            bot.last_teamplay_event,
        )
    });

    // Objective / threat / rival markers are diegetic — the keystone items and the rival
    // avatars walking your room (`sync_rival_avatars`) — so this only drives the HUD +
    // pause overlay. No linear progress meter: the goal is to race non-linearly through
    // the maze to the exit, so the running status is just placement, not a "% to exit".
    let local = facility.team(LOCAL_TEAM);
    let local_status = match local.map(|t| (t.placement, t.active_runner())) {
        Some((Some(1), _)) => "ESCAPED 1st".to_string(),
        Some((Some(n), _)) => format!("escaped {n}"),
        Some((None, true)) => "in the maze".to_string(),
        _ => "absorbed".to_string(),
    };
    let placed_torches = items
        .placed
        .iter()
        .filter(|item| item.kind == ItemKind::AnchorTorch)
        .count();
    let placed_pads = items
        .placed
        .iter()
        .filter(|item| item.kind == ItemKind::TeleportPad)
        .count();

    let mut log_lines = String::new();
    for entry in &log.entries {
        log_lines.push_str(&format!("  - {}\n", entry));
    }
    let threshold_debug = if crate::evidence::visual_audit_enabled() {
        let mut lines = String::new();
        for gap in &tp.geom.gaps {
            if gap.kind != crate::teleport::GapKind::OneWayEntry {
                lines.push_str(&format!(
                    "  {}\n",
                    crate::evidence::threshold_label(&gap.threshold)
                ));
            }
        }
        format!("\nTHRESHOLDS:\n{lines}")
    } else {
        String::new()
    };
    let freecam_debug = if crate::evidence::freecam_enabled() {
        "\nFREECAM: WASD move | Space/E up | Ctrl/Q down | RMB/Arrows look | R top-down\n"
            .to_string()
    } else {
        String::new()
    };

    if let Ok(mut hud) = hud.single_mut() {
        **hud = format!(
            "ROUND {}\nYou (Team 1): {}\nescaped {} | absorbed {}\ncollapse {:.0}%\n\
             SERIES R{} | alive {} | adversary {} | countdown {}\n\
             {}\n\
             {}\n\
             series objective: {} | event: {}\n\
             keystones {} / {} | EXIT {}\n\
             tools torch {}/{} | pads {}/{}\n\
             route logic: decohere + anchor + relay\n\n\
             {}\
             {}\
             ACTION LOG:\n{}\n\
             NET lockstep {} | replica {}/{} {} | drop {} dup {} reorder {}\n\n\
             WASD+mouse or Deck controls | E/X seize/link | F/L1 torch | C/Y pad\n\
             Tab/R1 map | Esc/Start pause",
            facility.round,
            local_status,
            facility.escaped_count(),
            facility.absorbed_count(),
            facility.purge_line.max(0.0) * 100.0,
            director.series.current.index,
            director.series.active_team_count(),
            director.series.adversary_strength(),
            countdown,
            control_line,
            teamplay_line,
            local_series_status,
            director.series.last_event,
            keys.held,
            keys.required,
            if keys.gate_open() { "OPEN" } else { "LOCKED" },
            items.carried(ItemKind::AnchorTorch),
            placed_torches,
            items.carried(ItemKind::TeleportPad),
            placed_pads,
            threshold_debug,
            freecam_debug,
            log_lines,
            live.network.profile.label(),
            live.remote.committed_round,
            live.resolved,
            if live.in_sync() { "in sync" } else { "syncing" },
            live.network.dropped,
            live.network.duplicated,
            live.network.reordered,
        );
    }
    if let Ok(mut visibility) = pause_panel.single_mut() {
        *visibility = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(crate) fn update_teleport_animation(
    time: Res<Time>,
    mut anim: ResMut<TeleportAnimation>,
    mut overlay: Query<(&mut Visibility, &mut BackgroundColor), With<TeleportOverlay>>,
) {
    if anim.timer > 0.0 {
        anim.timer = (anim.timer - time.delta_secs()).max(0.0);
        if let Ok((mut visibility, mut bg_color)) = overlay.single_mut() {
            *visibility = Visibility::Visible;
            let ratio = anim.timer / anim.max_time;
            let alpha = ratio * ratio; // smooth exponential fade
            *bg_color = BackgroundColor(anim.color.with_alpha(alpha));
        }
    } else {
        if let Ok((mut visibility, _)) = overlay.single_mut() {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Spawn the Match's screen-rooted HUD chrome: the status panel, the teleport
/// overlay, the pause panel, the tac-map panel, and the legend. State-scoped to the
/// Match, so it despawns with the screen.
pub(crate) fn spawn_match_hud(commands: &mut Commands) {
    commands
        .spawn(screen_root(GameState::Match))
        .with_children(|root| {
            root.spawn((
                MatchHud,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(16),
                    left: px(16),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                Text::new("Match starting…"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TITLE),
            ));
            root.spawn((
                TeleportOverlay,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                PausePanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                children![(
                    Text::new("PAUSED\n\nEsc / Start  Resume\nQ / Y        Quit to menu"),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(TITLE),
                )],
            ));
            root.spawn((
                TacMapPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(16),
                    right: px(16),
                    width: px(TAC_MAP_SIZE),
                    height: px(TAC_MAP_SIZE),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                children![(
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(6),
                        left: px(10),
                        ..default()
                    },
                    Text::new("TAC-MAP"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(TITLE),
                )],
            ));
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(16),
                    left: px(16),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                children![
                    text("LEGEND", 15.0, TITLE),
                    text("exit", 13.0, style::marker(MarkerRole::Exit).base_color),
                    text("keystone — pick up", 13.0, Color::srgb(1.0, 0.82, 0.3)),
                    text(
                        "anchor torch - F drop/pick",
                        13.0,
                        style::marker(MarkerRole::Control).base_color
                    ),
                    text(
                        "teleport pad - C drop/pick, E link",
                        13.0,
                        style::marker(MarkerRole::You).base_color
                    ),
                    text("locked exit (red door)", 13.0, Color::srgb(1.0, 0.32, 0.22)),
                    text(
                        "collapse — threat",
                        13.0,
                        style::marker(MarkerRole::Collapse).base_color
                    ),
                    text(
                        "rubble threshold",
                        13.0,
                        style::surface(SurfaceRole::Rubble).base_color
                    ),
                    text("klaxon countdown", 13.0, style::klaxon().base_color),
                    text(
                        "rival teams",
                        13.0,
                        style::marker(MarkerRole::Rival).base_color
                    ),
                    text(
                        "rival-held door (their colour)",
                        13.0,
                        style::marker(MarkerRole::Rival).base_color
                    ),
                    text(
                        "rival anchor — their torch holds the door",
                        13.0,
                        style::team(1).base_color
                    ),
                    text("mystery corridors", 13.0, Color::srgb(1.0, 0.32, 0.22)),
                    text(
                        "gantry edge — jump line",
                        13.0,
                        style::surface(SurfaceRole::GantryEdge).base_color
                    ),
                ],
            ));
        });
}
