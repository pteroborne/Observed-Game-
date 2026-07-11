//! The in-match 2D overlay: the pause panel + optional debug HUD ([`match_draw`]) and
//! the Tab tac-map ([`draw_tac_map`]), a survivor's-sketch schematic rebuilt each frame
//! from the live match filtered through the player's own witnessed knowledge (see
//! [`crate::tacmap`]).
//!
//! Phase 50 immersion ruling: normal play draws **no** status HUD and no legend — the
//! world communicates diegetically (door colours, keystone glow, the klaxon) plus the
//! tac-map sketch. The top-left readouts and the legend only spawn under
//! [`DebugHud`] (`OBSERVED2_DEBUG_HUD`, or a visual-audit/freecam session).

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use observed_core::RoomId;
use observed_facility::map_spec::RoomRole;
use observed_match::facility::{CollapseState, CompetitiveFacility};
use observed_style::{self as style, MarkerRole, SurfaceRole};

use super::input::gamepad_map_pressed;
use crate::flow::LOCAL_TEAM;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{
    MapKnowledge, MatchPaused, RivalSightings, SightingKind, SpectatorBot, TeleportState,
};
use crate::teleport::Place;
use crate::view::components::{
    DebugHud, InteractionReticle, MatchHud, MatchHudReadout, PauseConfigReadout, PausePanel,
    PauseSettingsPanel, TacMapElement, TacMapPanel, TacMapState, TeleportAnimation,
    TeleportOverlay,
};
use crate::view::theme::{BORDER, DIM, PANEL, TAC_MAP_SIZE, TITLE, screen_root, text};
use crate::{GameState, settings::key_name, tacmap};

// Tac-map overlay layout (pixels). The 3×3 grid of rooms sits below a title strip.
const TAC_TITLE_H: f32 = 26.0;
const TAC_INSET: f32 = 22.0;
const TAC_ROOM_MAX: f32 = 46.0;
const TAC_ROOM_MIN: f32 = 18.0;

/// Toggle the tac-map overlay with the bound tac-map key (default Tab; shows/hides the
/// panel root, `draw_tac_map` fills it while shown).
pub(crate) fn toggle_tac_map(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    settings: Res<crate::settings::Settings>,
    mut state: ResMut<TacMapState>,
    mut panel: Query<&mut Visibility, With<TacMapPanel>>,
) {
    if keyboard.just_pressed(settings.bindings.tac_map) || gamepad_map_pressed(&gamepads) {
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

fn tac_room_size(room_count: usize) -> f32 {
    if room_count <= 12 {
        return TAC_ROOM_MAX;
    }
    (TAC_ROOM_MAX * (12.0 / room_count as f32).sqrt()).clamp(TAC_ROOM_MIN, TAC_ROOM_MAX)
}

fn tac_center_for_pos(pos: Vec2, bounds: (Vec2, Vec2), room_size: f32) -> Vec2 {
    let (min, max) = bounds;
    let span = (max - min).max(Vec2::ONE);
    let g = (pos - min) / span;
    let usable_w = TAC_MAP_SIZE - TAC_INSET * 2.0 - room_size;
    let usable_h = TAC_MAP_SIZE - TAC_TITLE_H - TAC_INSET * 2.0 - room_size;
    Vec2::new(
        TAC_INSET + g.x * usable_w + room_size * 0.5,
        TAC_TITLE_H + TAC_INSET + g.y * usable_h + room_size * 0.5,
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

fn tac_route_bar(a: Vec2, b: Vec2, thickness: f32) -> (Vec2, f32, f32) {
    let mid = (a + b) * 0.5;
    let w = (a.x - b.x).abs().max(thickness);
    let h = (a.y - b.y).abs().max(thickness);
    (mid, w, h)
}

/// Rebuild the tac-map's room/route/marker nodes from the live match each frame while
/// it is shown. Presentation-only — reads the brain + keystone inventory filtered
/// through the player's witnessed [`MapKnowledge`] (see `tacmap`): unknown rooms are
/// simply absent, glimpsed rooms are hollow outlines, and the exit only appears once
/// found.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tac_map(
    state: Res<TacMapState>,
    director: Res<MatchDirector>,
    spectator_bot: Option<Res<SpectatorBot>>,
    tp: Res<TeleportState>,
    keys: Res<KeystoneState>,
    sightings: Res<RivalSightings>,
    knowledge: Res<MapKnowledge>,
    debug_hud: Res<DebugHud>,
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
    let game = director.live.host_match();
    let model = tacmap::build_map(
        &game.competitive,
        &keys,
        &sightings,
        &knowledge,
        game.reroute_commits,
        tp.place,
    );
    // Bounds/sizing come from the FULL facility so the sketch's rooms never shift or
    // resize as more of the maze is discovered.
    let bounds = model.bounds;
    let room_size = tac_room_size(model.total_rooms);
    let route_thickness = (room_size * 0.12).clamp(3.0, 5.0);
    let room_centers: Vec<(RoomId, Vec2)> = model
        .rooms
        .iter()
        .map(|(room, pos, _)| (*room, tac_center_for_pos(*pos, bounds, room_size)))
        .chain(
            model
                .glimpsed
                .iter()
                .map(|(room, pos)| (*room, tac_center_for_pos(*pos, bounds, room_size))),
        )
        .collect();
    let center_for = |room: RoomId| {
        room_centers
            .iter()
            .find_map(|(id, center)| (*id == room).then_some(*center))
            .unwrap_or_else(|| tac_center_for_pos(tacmap::grid_pos(room), bounds, room_size))
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
    let key_size = (room_size * 0.22).clamp(6.0, 10.0);
    let rival_size = (room_size * 0.28).clamp(8.0, 13.0);
    let player_size = (room_size * 0.35).clamp(10.0, 16.0);
    let member_size = (room_size * 0.22).clamp(7.0, 9.0);

    commands.entity(panel).with_children(|p| {
        // Route bars first, under the rooms.
        for &(a, b) in &model.routes {
            let (c1, c2) = (center_for(a), center_for(b));
            if (c1.y - c2.y).abs() < 1.0 || (c1.x - c2.x).abs() < 1.0 {
                let (mid, w, h) = tac_route_bar(c1, c2, route_thickness);
                p.spawn((
                    tac_box(mid, w, h, route_col.with_alpha(0.5)),
                    crate::evidence::DiagnosticTacMapVisual::route(a, b),
                ));
            } else {
                let bend = Vec2::new(c2.x, c1.y);
                for (start, end) in [(c1, bend), (bend, c2)] {
                    let (mid, w, h) = tac_route_bar(start, end, route_thickness);
                    p.spawn((
                        tac_box(mid, w, h, route_col.with_alpha(0.5)),
                        crate::evidence::DiagnosticTacMapVisual::route(a, b),
                    ));
                }
            }
        }
        // Visited rooms: filled squares — collapse-swallowed read red; objective/tool
        // rooms read warm.
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
                tac_box(center_for(room), room_size, room_size, fill),
                crate::evidence::DiagnosticTacMapVisual::room(room),
            ));
        }
        // Glimpsed rooms: hollow outlines — the player saw *that something is there*
        // through a threshold, nothing more. A sealed glimpse still reads as threat.
        for &(room, _) in &model.glimpsed {
            let center = center_for(room);
            let outline = if model.collapse.contains(&room) {
                collapse_col.with_alpha(0.6)
            } else {
                DIM.with_alpha(0.45)
            };
            p.spawn((
                TacMapElement,
                DespawnOnExit(GameState::Match),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(center.x - room_size * 0.5),
                    top: px(center.y - room_size * 0.5),
                    width: px(room_size),
                    height: px(room_size),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BorderColor::all(outline),
                crate::evidence::DiagnosticTacMapVisual::room(room),
            ));
        }
        // The exit room, once actually found: a green (open) or red (locked) outline.
        if model.exit_known {
            let exit_center = center_for(model.exit);
            p.spawn((
                TacMapElement,
                DespawnOnExit(GameState::Match),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(exit_center.x - room_size * 0.5),
                    top: px(exit_center.y - room_size * 0.5),
                    width: px(room_size),
                    height: px(room_size),
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
        }
        // Keystone pips in the top-right of their room.
        for room in &model.keystones {
            let c = center_for(*room)
                + Vec2::new(
                    room_size * 0.5 - key_size * 0.7,
                    -(room_size * 0.5) + key_size * 0.7,
                );
            p.spawn((
                tac_box(c, key_size, key_size, key_col),
                crate::evidence::DiagnosticTacMapVisual::one(
                    crate::evidence::DiagnosticTacMapRole::Keystone,
                    Some(*room),
                ),
            ));
        }
        // Rival pips (Phase 42c: fog of war). Each pip is a *sighting*, not live truth:
        // its alpha fades with staleness (`RivalPip::alpha`) and a hollow outline marks
        // `Heard`-only evidence (a sound-bleed trace, not an eyes-on witnessing) versus a
        // filled box for `Seen`/`AnchorSpotted`. Fanned so several in one room stay
        // distinct.
        for (slot, pip) in model.rivals.iter().enumerate() {
            let off = Vec2::new(
                (slot as f32 - (rival_count - 1.0) * 0.5) * rival_size * 0.7,
                room_size * 0.18,
            );
            let center = center_for(pip.room) + off;
            let alpha = pip.alpha();
            if pip.kind == SightingKind::Heard {
                p.spawn((
                    TacMapElement,
                    DespawnOnExit(GameState::Match),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(center.x - rival_size * 0.5),
                        top: px(center.y - rival_size * 0.5),
                        width: px(rival_size),
                        height: px(rival_size),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BorderColor::all(rival_col.with_alpha(alpha)),
                    crate::evidence::DiagnosticTacMapVisual::one(
                        crate::evidence::DiagnosticTacMapRole::Rival,
                        Some(pip.room),
                    ),
                ));
            } else {
                p.spawn((
                    tac_box(center, rival_size, rival_size, rival_col.with_alpha(alpha)),
                    crate::evidence::DiagnosticTacMapVisual::one(
                        crate::evidence::DiagnosticTacMapRole::Rival,
                        Some(pip.room),
                    ),
                ));
            }
        }
        // Team labels for witnessed rivals appear ONLY in spectator mode (Phase 50
        // immersion ruling): in the live race a sighting is just an anonymous mark on a
        // survivor's sketch — "someone was here" — while the spectator overlay is an
        // observer's tool and may name teams and their personalities.
        if let Some(bot) = spectator_bot.as_ref() {
            for pip in &model.rivals {
                let center = center_for(pip.room) + Vec2::new(0.0, room_size * 0.48);
                let label = match bot.teamplay.policy(observed_core::TeamId(pip.team as u8)) {
                    Some(policy) => format!("Team {} ({})", pip.team + 1, policy.label()),
                    None => format!("Team {}", pip.team + 1),
                };
                p.spawn((
                    TacMapElement,
                    DespawnOnExit(GameState::Match),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(center.x - 30.0),
                        top: px(center.y - 6.0),
                        width: px(60.0),
                        ..default()
                    },
                    Text::new(label),
                    TextFont {
                        font_size: (room_size * 0.2).clamp(7.0, 9.0),
                        ..default()
                    },
                    TextColor(rival_col.with_alpha(pip.alpha())),
                    crate::evidence::DiagnosticTacMapVisual::one(
                        crate::evidence::DiagnosticTacMapRole::Rival,
                        Some(pip.room),
                    ),
                ));
            }
        }
        if let Some(bot) = spectator_bot.as_ref()
            && let Some(team) = bot.teamplay.team(bot.focused_team)
        {
            let member_count = team.members.len() as f32;
            for (index, member) in team.members.iter().enumerate() {
                let off = Vec2::new(
                    (index as f32 - (member_count - 1.0) * 0.5) * member_size * 1.3,
                    -room_size * 0.2,
                );
                p.spawn((
                    tac_box(
                        center_for(member.room) + off,
                        member_size,
                        member_size,
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
            tac_box(you, player_size, player_size, you_col),
            crate::evidence::DiagnosticTacMapVisual::one(
                crate::evidence::DiagnosticTacMapRole::Player,
                player_room,
            ),
        ));
        // The series meta-status line is developer telemetry, not something a runner's
        // sketch would carry — debug HUD only.
        if debug_hud.0 {
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
        }
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn match_draw(
    director: Res<MatchDirector>,
    spectator_bot: Option<Res<SpectatorBot>>,
    paused: Res<MatchPaused>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    log: Option<Res<crate::guardian::ActionLog>>,
    tp: Res<TeleportState>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    settings: Res<crate::settings::Settings>,
    mut hud: Query<(&MatchHudReadout, &mut Text)>,
    mut pause_panel: Query<&mut Visibility, With<PausePanel>>,
    mut pause_config: Query<&mut Text, (With<PauseConfigReadout>, Without<MatchHudReadout>)>,
) {
    if let Ok(mut visibility) = pause_panel.single_mut() {
        *visibility = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let (true, Ok(mut readout)) = (paused.0, pause_config.single_mut()) {
        **readout = format!(
            "Active config:  Rivals {}  |  Teammates {}  |  Guardian {}",
            if director.config.rival_teams {
                "ON"
            } else {
                "OFF"
            },
            if director.config.ai_teammates {
                "ON"
            } else {
                "OFF"
            },
            if director.config.guardian {
                "ON"
            } else {
                "OFF"
            }
        );
    }
    // Without the debug HUD (normal, immersive play) there are no readout entities to
    // fill — skip assembling the status text entirely.
    if hud.is_empty() {
        return;
    }
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
    if let Some(log) = log.as_ref() {
        for entry in &log.entries {
            log_lines.push_str(&format!("  - {}\n", entry));
        }
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

    let seed_val = seed.map(|seed| seed.0).unwrap_or(crate::flow::MATCH_SEED);
    let bindings = &settings.bindings;
    let movement_keys = format!(
        "{}{}{}{}",
        key_name(bindings.move_forward),
        key_name(bindings.move_left),
        key_name(bindings.move_back),
        key_name(bindings.move_right)
    );
    let controls_hint_line = format!(
        "{}+mouse / Deck | {}/X seize/link | {}/L1 torch | {}/Y pad | {}/R1 tac-map | {}/Start pause",
        movement_keys,
        key_name(bindings.interact),
        key_name(bindings.torch),
        key_name(bindings.pad),
        key_name(bindings.tac_map),
        key_name(bindings.pause),
    );
    let objective_line = if keys.gate_open() {
        "EXIT OPEN | reach the exit room on the tac-map".to_string()
    } else {
        format!(
            "EXIT LOCKED | {} opens tac-map for the exit",
            key_name(bindings.tac_map)
        )
    };
    let remaining_keys = keys.required.saturating_sub(keys.held);
    let keystone_line = format!(
        "KEYSTONES {} / {} | {} remaining | torch {}/{} | pads {}/{}",
        keys.held,
        keys.required,
        remaining_keys,
        items.carried(ItemKind::AnchorTorch),
        placed_torches,
        items.carried(ItemKind::TeleportPad),
        placed_pads,
    );
    let here_collapse = collapse_state_for_place(facility, tp.place);
    let collapse_rooms = facility.collapse_rooms();
    let objective_rooms = facility.objective_sequence().len().max(1);
    let frontier = if facility.collapse_frontier().is_some() {
        "active"
    } else {
        "none"
    };
    let collapse_line = format!(
        "COLLAPSE {:.0}% | sealed {}/{} | frontier {} | here {}",
        facility.purge_line.max(0.0) * 100.0,
        collapse_rooms.len(),
        objective_rooms,
        frontier,
        collapse_state_label(here_collapse),
    );
    let standings = facility.standings();
    let leader = standings
        .first()
        .map(|team| team.label())
        .unwrap_or_else(|| "none".to_string());
    let you_rank = standings
        .iter()
        .position(|team| *team == LOCAL_TEAM)
        .map(|index| ordinal(index as u8 + 1))
        .unwrap_or_else(|| "--".to_string());
    let standing_line = format!(
        "LEADER {} | you {} ({}) | SERIES R{} alive {} adv {} countdown {}",
        leader,
        you_rank,
        local_status,
        director.series.current.index,
        director.series.active_team_count(),
        director.series.adversary_strength(),
        countdown,
    );
    let debug_enabled =
        crate::evidence::visual_audit_enabled() || crate::evidence::freecam_enabled();
    let debug_line = if debug_enabled {
        format!(
            "DEBUG seed {} round {} | escaped {} absorbed {}\n\
             series objective: {} | event: {}\n\
             {}{}\n\
             ACTION LOG:\n{}\n\
             NET {} | replica {}/{} {} | drop {} dup {} reorder {}\n\
             {}",
            seed_val,
            facility.round,
            facility.escaped_count(),
            facility.absorbed_count(),
            local_series_status,
            director.series.last_event,
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
            teamplay_line,
        )
    } else {
        String::new()
    };

    for (readout, mut text) in &mut hud {
        **text = match readout {
            MatchHudReadout::Objective => objective_line.clone(),
            MatchHudReadout::Keystone => keystone_line.clone(),
            MatchHudReadout::Collapse => collapse_line.clone(),
            MatchHudReadout::Standing => standing_line.clone(),
            MatchHudReadout::Controls => format!("{control_line} | {controls_hint_line}"),
            MatchHudReadout::Debug => debug_line.clone(),
        };
    }
}

fn collapse_state_for_place(
    facility: &CompetitiveFacility,
    place: crate::teleport::Place,
) -> CollapseState {
    match place {
        crate::teleport::Place::Room(room) => facility.room_collapse(room),
        crate::teleport::Place::Hallway { from, to, .. } => {
            strongest_collapse(facility.room_collapse(from), facility.room_collapse(to))
        }
    }
}

fn strongest_collapse(a: CollapseState, b: CollapseState) -> CollapseState {
    match (a, b) {
        (CollapseState::Collapsed, _) | (_, CollapseState::Collapsed) => CollapseState::Collapsed,
        (CollapseState::Dying, _) | (_, CollapseState::Dying) => CollapseState::Dying,
        _ => CollapseState::Intact,
    }
}

fn collapse_state_label(state: CollapseState) -> &'static str {
    match state {
        CollapseState::Intact => "safe",
        CollapseState::Dying => "dying",
        CollapseState::Collapsed => "sealed",
    }
}

fn ordinal(rank: u8) -> String {
    match rank {
        1 => "1st".to_string(),
        2 => "2nd".to_string(),
        3 => "3rd".to_string(),
        n => format!("{n}th"),
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

/// Legend text scales up under the accessibility high-contrast toggle (Phase 48):
/// the standard "large text" accessibility pattern, legend-backed and Legibility-
/// Contract-safe — it boosts legibility of the *existing* semantic colours rather
/// than inventing new ones.
const LEGEND_FONT_BASE: f32 = 13.0;
const LEGEND_FONT_HIGH_CONTRAST: f32 = 17.0;
const LEGEND_BORDER_BASE: f32 = 1.0;
const LEGEND_BORDER_HIGH_CONTRAST: f32 = 2.5;

/// Spawn the Match's screen-rooted overlay chrome: the teleport overlay, the pause
/// panel, and the tac-map panel — plus, ONLY when `debug_hud` is set, the status
/// readout panel and the legend (Phase 50: normal play is HUD-free; the world and the
/// tac-map sketch carry the information). State-scoped to the Match, so it despawns
/// with the screen. `high_contrast` (from `Settings`) scales the legend's text and
/// border for readability.
pub(crate) fn spawn_match_hud(commands: &mut Commands, high_contrast: bool, debug_hud: bool) {
    let legend_font = if high_contrast {
        LEGEND_FONT_HIGH_CONTRAST
    } else {
        LEGEND_FONT_BASE
    };
    let legend_border = if high_contrast {
        LEGEND_BORDER_HIGH_CONTRAST
    } else {
        LEGEND_BORDER_BASE
    };
    let legend_rows: Vec<(&str, Color)> = vec![
        ("exit", style::marker(MarkerRole::Exit).base_color),
        ("keystone — pick up", Color::srgb(1.0, 0.82, 0.3)),
        (
            "anchor torch - F drop/pick",
            style::marker(MarkerRole::Control).base_color,
        ),
        (
            "teleport pad - C drop/pick, E link",
            style::marker(MarkerRole::You).base_color,
        ),
        ("locked exit (red door)", Color::srgb(1.0, 0.32, 0.22)),
        (
            "collapse — threat",
            style::marker(MarkerRole::Collapse).base_color,
        ),
        (
            "reticle (active close to items)",
            style::marker(MarkerRole::You).base_color,
        ),
        (
            "rubble threshold",
            style::surface(SurfaceRole::Rubble).base_color,
        ),
        ("klaxon countdown", style::klaxon().base_color),
        ("rival teams", style::marker(MarkerRole::Rival).base_color),
        (
            "rival sighting — fades as it ages",
            style::marker(MarkerRole::Rival).base_color,
        ),
        (
            "rival-held door (their colour)",
            style::marker(MarkerRole::Rival).base_color,
        ),
        (
            "rival anchor — their torch holds the door",
            style::team(1).base_color,
        ),
        ("mystery corridors", Color::srgb(1.0, 0.32, 0.22)),
        (
            "gantry edge — jump line",
            style::surface(SurfaceRole::GantryEdge).base_color,
        ),
        (
            "audio: rival bleed = nearby rival",
            style::marker(MarkerRole::Rival).base_color,
        ),
        (
            "audio: low guardian dread = guardian near",
            style::marker(MarkerRole::Collapse).base_color,
        ),
        (
            "audio: chime = keystone/tool/exit state",
            Color::srgb(1.0, 0.82, 0.3),
        ),
    ];
    commands
        .spawn(screen_root(GameState::Match))
        .with_children(|root| {
            // Spawn center interaction reticle dot (invisible initially)
            root.spawn((
                InteractionReticle,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50.0),
                    top: percent(50.0),
                    margin: UiRect {
                        left: px(-2.0),
                        top: px(-2.0),
                        ..default()
                    },
                    width: px(4.0),
                    height: px(4.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));

            if debug_hud {
                root.spawn((
                    MatchHud,
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(16),
                        left: px(16),
                        width: px(620),
                        padding: UiRect::all(px(12)),
                        border: UiRect::all(px(1)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexStart,
                        row_gap: px(4),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(BORDER),
                    Text::new("Match starting…"),
                    TextFont {
                        font_size: 1.0,
                        ..default()
                    },
                    TextColor(Color::NONE),
                ))
                .with_children(|hud| {
                    hud.spawn((
                        MatchHudReadout::Objective,
                        text("EXIT --", 17.0, style::marker(MarkerRole::Exit).base_color),
                    ));
                    hud.spawn((
                        MatchHudReadout::Keystone,
                        text("KEYSTONES -- / --", 15.0, Color::srgb(1.0, 0.82, 0.3)),
                    ));
                    hud.spawn((
                        MatchHudReadout::Collapse,
                        text(
                            "COLLAPSE --",
                            15.0,
                            style::marker(MarkerRole::Collapse).base_color,
                        ),
                    ));
                    hud.spawn((
                        MatchHudReadout::Standing,
                        text("LEADER --", 15.0, TITLE),
                    ));
                    hud.spawn((
                        MatchHudReadout::Controls,
                        text("controls loading...", 12.0, DIM),
                    ));
                    hud.spawn((MatchHudReadout::Debug, text("", 10.0, DIM)));
                });
            }
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
                children![
                    (
                        Text::new(
                            "PAUSED\n\nEsc / Start  Resume\nO              Settings\nQ / Y        Quit to menu"
                        ),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(TITLE),
                    ),
                    (
                        PauseConfigReadout,
                        Text::new(""),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(DIM),
                        Node {
                            margin: UiRect::top(px(14)),
                            ..default()
                        },
                    ),
                    (
                        PauseSettingsPanel,
                        Visibility::Hidden,
                        Node {
                            margin: UiRect::top(px(18)),
                            padding: UiRect::all(px(16)),
                            border: UiRect::all(px(1)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::FlexStart,
                            row_gap: px(4),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(BORDER),
                    )
                ],
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
            if debug_hud {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(16),
                        left: px(16),
                        padding: UiRect::all(px(12)),
                        border: UiRect::all(px(legend_border)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(BORDER),
                ))
                .with_children(|legend| {
                    legend.spawn(text("LEGEND", legend_font + 2.0, TITLE));
                    for (label, color) in legend_rows {
                        legend.spawn(text(label, legend_font, color));
                    }
                });
            }
        });
}

const ITEM_INTERACT_RADIUS: f32 = 1.8;

pub(crate) fn update_interaction_reticle(
    tp: Res<TeleportState>,
    items: Res<ItemsState>,
    keys: Res<KeystoneState>,
    runtime: Res<MatchDirector>,
    mut reticle: Query<&mut BackgroundColor, With<InteractionReticle>>,
) {
    let mut interactable_near = false;
    let pos = Vec2::new(tp.body.position.x, tp.body.position.z);
    let place = tp.place;

    // Check items in current place (anchor torches, teleport pads, battery charge, etc.)
    for item in items.placed_in(place) {
        if pos.distance(item.pos) <= ITEM_INTERACT_RADIUS {
            interactable_near = true;
            break;
        }
    }

    // Check uncollected keystone in the current room
    if !interactable_near
        && let Place::Room(room) = place
        && keys.has_uncollected(room)
        && pos.length() <= ITEM_INTERACT_RADIUS
    {
        interactable_near = true;
    }

    // Check guardian console in the Guardian Control room
    if !interactable_near
        && let Place::Room(room) = place
        && let Some(spec) = &runtime.live.host_match().competitive.map_spec
        && spec.role_room(RoomRole::GuardianControl) == Some(room)
        && pos.length() <= 2.0
    {
        interactable_near = true;
    }

    for mut bg_color in &mut reticle {
        if interactable_near {
            // Bright cyan color when close to interactables
            bg_color.0 = Color::srgb(0.0, 0.9, 1.0);
        } else {
            // Completely transparent under normal traversal
            bg_color.0 = Color::NONE;
        }
    }
}
