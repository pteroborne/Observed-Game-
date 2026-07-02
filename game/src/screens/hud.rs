//! The in-match 2D overlay: the text HUD + pause panel ([`match_draw`]) and the Tab
//! tac-map ([`draw_tac_map`]), a top-down schematic rebuilt each frame from the live
//! match and keystone inventory (see [`crate::tacmap`]).

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_style::{self as style, MarkerRole};

use super::*;
use crate::flow::LOCAL_TEAM;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::{GameState, tacmap};

// Tac-map overlay layout (pixels). The 3×3 grid of rooms sits below a title strip.
const TAC_TITLE_H: f32 = 26.0;
const TAC_INSET: f32 = 22.0;
const TAC_STEP: f32 = 92.0; // distance between adjacent room centres
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
fn tac_center(room: RoomId) -> Vec2 {
    let g = tacmap::grid_pos(room);
    Vec2::new(
        TAC_INSET + g.x * TAC_STEP + TAC_ROOM * 0.5,
        TAC_TITLE_H + TAC_INSET + g.y * TAC_STEP + TAC_ROOM * 0.5,
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
pub(crate) fn draw_tac_map(
    state: Res<TacMapState>,
    runtime: Res<MatchRuntime>,
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
    let model = tacmap::build_map(&runtime.live.host_match().competitive, &keys, tp.place);

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

    let spine = tacmap::spine();
    let rival_count = model.rivals.len() as f32;

    commands.entity(panel).with_children(|p| {
        // Route bars first, under the rooms (every spine step is grid-axis-aligned).
        for pair in spine.windows(2) {
            let (c1, c2) = (tac_center(pair[0]), tac_center(pair[1]));
            let mid = (c1 + c2) * 0.5;
            let (w, h) = if (c1.y - c2.y).abs() < 1.0 {
                ((c1.x - c2.x).abs(), 5.0)
            } else {
                (5.0, (c1.y - c2.y).abs())
            };
            p.spawn((
                tac_box(mid, w, h, route_col.with_alpha(0.5)),
                crate::diagnostics::DiagnosticTacMapVisual::route(pair[0], pair[1]),
            ));
        }
        // Room squares: collapse-swallowed rooms read red, spine rooms warm, rest dim.
        for id in 0..9u32 {
            let room = RoomId(id);
            let fill = if model.collapse.contains(&room) {
                collapse_col.with_alpha(0.55)
            } else if spine.contains(&room) {
                spine_fill
            } else {
                plain_fill
            };
            p.spawn((
                tac_box(tac_center(room), TAC_ROOM, TAC_ROOM, fill),
                crate::diagnostics::DiagnosticTacMapVisual::room(room),
            ));
        }
        // The exit room: a green (open) or red (locked) outline.
        let exit_center = tac_center(model.exit);
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
            crate::diagnostics::DiagnosticTacMapVisual::one(
                crate::diagnostics::DiagnosticTacMapRole::Exit,
                Some(model.exit),
            ),
        ));
        // Keystone pips in the top-right of their room.
        for room in &model.keystones {
            let c = tac_center(*room) + Vec2::new(TAC_ROOM * 0.5 - 7.0, -(TAC_ROOM * 0.5) + 7.0);
            p.spawn((
                tac_box(c, 10.0, 10.0, key_col),
                crate::diagnostics::DiagnosticTacMapVisual::one(
                    crate::diagnostics::DiagnosticTacMapRole::Keystone,
                    Some(*room),
                ),
            ));
        }
        // Rival pips, fanned so several in one room stay distinct.
        for (slot, (_, room)) in model.rivals.iter().enumerate() {
            let off = Vec2::new((slot as f32 - (rival_count - 1.0) * 0.5) * 9.0, 8.0);
            p.spawn((
                tac_box(tac_center(*room) + off, 13.0, 13.0, rival_col),
                crate::diagnostics::DiagnosticTacMapVisual::one(
                    crate::diagnostics::DiagnosticTacMapRole::Rival,
                    Some(*room),
                ),
            ));
        }
        // YOU: room centre, or the midpoint of the hallway you're walking.
        let you = match model.player {
            tacmap::PlayerMark::Room(r) => tac_center(r),
            tacmap::PlayerMark::Between(a, b) => (tac_center(a) + tac_center(b)) * 0.5,
        };
        let player_room = match model.player {
            tacmap::PlayerMark::Room(room) => Some(room),
            tacmap::PlayerMark::Between(_, _) => None,
        };
        p.spawn((
            tac_box(you, 16.0, 16.0, you_col),
            crate::diagnostics::DiagnosticTacMapVisual::one(
                crate::diagnostics::DiagnosticTacMapRole::Player,
                player_room,
            ),
        ));
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_draw(
    runtime: Res<MatchRuntime>,
    paused: Res<MatchPaused>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    log: Res<crate::guardian::ActionLog>,
    tp: Res<TeleportState>,
    mut hud: Query<&mut Text, With<MatchHud>>,
    mut pause_panel: Query<&mut Visibility, With<PausePanel>>,
) {
    let live = &runtime.live;
    let game = live.host_match();
    let facility = &game.competitive;

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
    let threshold_debug = if crate::diagnostics::visual_audit_enabled() {
        let mut lines = String::new();
        for gap in &tp.geom.gaps {
            if gap.kind != crate::teleport::GapKind::OneWayEntry {
                lines.push_str(&format!(
                    "  {}\n",
                    crate::diagnostics::threshold_label(&gap.threshold)
                ));
            }
        }
        format!("\nTHRESHOLDS:\n{lines}")
    } else {
        String::new()
    };
    let freecam_debug = if crate::diagnostics::freecam_enabled() {
        "\nFREECAM: WASD move | Space/E up | Ctrl/Q down | RMB/Arrows look | R top-down\n"
            .to_string()
    } else {
        String::new()
    };

    if let Ok(mut hud) = hud.single_mut() {
        **hud = format!(
            "ROUND {}\nYou (Team 1): {}\nescaped {} | absorbed {}\ncollapse {:.0}%\n\
             keystones {} / {} | EXIT {}\n\
             tools torch {}/{} | pads {}/{}\n\
             pressure gate {} | hits {}\n\n\
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
            keys.held,
            keys.required,
            if keys.gate_open() { "OPEN" } else { "LOCKED" },
            items.carried(ItemKind::AnchorTorch),
            placed_torches,
            items.carried(ItemKind::TeleportPad),
            placed_pads,
            if game.trap_active() { "ACTIVE" } else { "idle" },
            game.trap_hits,
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
