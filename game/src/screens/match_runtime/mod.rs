pub(crate) mod ambience;
pub(crate) mod crossing;
pub(crate) mod input;
pub(crate) mod pause_settings;
pub(crate) mod session;
pub(crate) mod spectator;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

use super::input::{gamepad_pause_pressed, gamepad_quit_pressed};
use crate::layout::WALL_HEIGHT;
use crate::sim::director::MatchDirector;
use crate::sim::nav::nav_for_place;
use crate::sim::state::{ItemIntent, LastTeleportPad, MatchPaused, SpectatorBot, TeleportState};
use crate::view::assets::MatchAssets;
use crate::view::components::{KeystoneItem, MatchAudioCue, TeleportAnimation};

use crate::GameState;
use crate::flow::{Career, MATCH_SEED};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::teleport::Place;

// Re-exports
pub(crate) use ambience::{
    collapse_state_for_place, countdown_klaxon_active, district_for_place, palette_for_game,
    palette_for_match,
};
pub(crate) use crossing::{
    compute_gap_dests, debug_cross_gap_for_capture, debug_place_into, place_body_at, teleport_sim,
};
pub(crate) use session::{cleanup_match_resources, setup_match};
pub(crate) use spectator::drive_spectator_bot;

#[derive(SystemParam)]
pub(crate) struct MatchPumpInput<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
}

const ITEM_INTERACT_RADIUS: f32 = 1.8;
const PAD_ACTIVATE_RADIUS: f32 = 1.25;

pub(super) fn body_xz(tp: &TeleportState) -> Vec2 {
    Vec2::new(tp.body.position.x, tp.body.position.z)
}

fn pickup_or_drop_item(
    items: &mut ItemsState,
    kind: ItemKind,
    place: Place,
    pos: Vec2,
    version: u32,
) -> bool {
    if items.pickup(kind, place, pos, ITEM_INTERACT_RADIUS) {
        true
    } else {
        items.drop(kind, place, pos, version)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn item_actions(
    director: Res<MatchDirector>,
    keys: Res<KeystoneState>,
    mut tp: ResMut<TeleportState>,
    mut items: ResMut<ItemsState>,
    mut item_intent: ResMut<ItemIntent>,
    paused: Res<MatchPaused>,
    mut anim: ResMut<TeleportAnimation>,
    mut last_pad: ResMut<LastTeleportPad>,
    mut log: Option<ResMut<crate::guardian::ActionLog>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    mut juice: ResMut<crate::view::components::CameraJuice>,
    assets: Res<MatchAssets>,
    settings: Res<crate::settings::Settings>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
    mut commands: Commands,
) {
    let intent = std::mem::take(&mut *item_intent);
    if paused.0 || director.done {
        return;
    }

    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);

    let pos = body_xz(&tp);
    let place = tp.place;
    let version = director.live.host_match().reroute_commits;
    let mut changed = false;

    if intent.torch_action {
        let acted = if items.pickup(ItemKind::AnchorTorch, place, pos, ITEM_INTERACT_RADIUS) {
            true
        } else {
            let mut connections = match place {
                Place::Room(_) => tp.geom.gaps.iter().map(|gap| gap.target).collect(),
                Place::Hallway { .. } => Vec::new(),
            };
            connections.sort_by_key(|room| room.0);
            connections.dedup();
            items.drop_anchor_torch(place, pos, version, &connections)
        };
        if acted {
            audio_director.request(
                &mut commands,
                &assets.tool_interact,
                MatchAudioCue::ToolInteract,
                "Tool interaction",
                None,
                &settings,
            );
        }
        changed |= acted;
    }
    if intent.pad_action {
        let acted = pickup_or_drop_item(&mut items, ItemKind::TeleportPad, place, pos, version);
        if acted {
            audio_director.request(
                &mut commands,
                &assets.tool_interact,
                MatchAudioCue::ToolInteract,
                "Tool interaction",
                None,
                &settings,
            );
        }
        changed |= acted;
    }

    let on_pad_link = items.pad_link_target(place, pos, PAD_ACTIVATE_RADIUS);
    let is_latched = last_pad
        .last_used_pos
        .is_some_and(|(last_place, last_pos)| {
            crate::items::same_place(place, last_place)
                && pos.distance(last_pos) <= PAD_ACTIVATE_RADIUS + 0.3
        });

    if !is_latched {
        if let Some((last_place, last_pos)) = last_pad.last_used_pos
            && (!crate::items::same_place(place, last_place)
                || pos.distance(last_pos) > PAD_ACTIVATE_RADIUS + 0.3)
        {
            last_pad.last_used_pos = None;
        }

        if let Some((target_place, target_pos)) = on_pad_link {
            let nav = nav_for_place(
                seed_val,
                director.live.host_match(),
                &keys,
                &items,
                target_place,
            );
            place_body_at(&mut tp, target_place, target_pos, &nav);
            let collision_catalog = tp.collision_catalog.clone();
            let simulation_content_hash = tp.simulation_content_hash;
            let dests = compute_gap_dests(
                seed_val,
                tp.place,
                &tp.geom,
                director.live.host_match(),
                &keys,
                &items,
                &collision_catalog,
                simulation_content_hash,
            );
            tp.gap_dests = dests;
            changed = true;
            last_pad.last_used_pos = Some((target_place, target_pos));
            anim.trigger(2.0, Color::srgba(0.0, 0.8, 1.0, 1.0));
            juice.teleport_ease_timer = 0.45;
            audio_director.request(
                &mut commands,
                &assets.tool_interact,
                MatchAudioCue::ToolInteract,
                "Teleport pad link",
                None,
                &settings,
            );
            if let (Place::Room(room), Some(log)) = (target_place, log.as_mut()) {
                log.add(format!("Teleported via pad to Room {}!", room.0));
            }
        }
    }

    if changed {
        let nav = nav_for_place(
            seed_val,
            director.live.host_match(),
            &keys,
            &items,
            tp.place,
        );
        let mut geom = crate::teleport::geom_for(tp.place, &nav);
        if matches!(tp.place, Place::Room(_)) {
            crate::teleport::open_entry(&mut geom, tp.arrived_from);
        }
        let current_place = tp.place;
        let current_layout = tp.layout.clone();
        tp.set_arena_for_place(
            crate::teleport::place_arena(&geom, 0.0, WALL_HEIGHT),
            current_place,
            &geom,
            0.0,
            current_layout.as_ref(),
        );
        if geom.poly.is_some() {
            let clamped = crate::teleport::contain(&geom, body_xz(&tp), tp.config.radius);
            tp.body.position.x = clamped.x;
            tp.body.position.z = clamped.y;
        }
        tp.geom = geom;
        let collision_catalog = tp.collision_catalog.clone();
        let simulation_content_hash = tp.simulation_content_hash;
        tp.gap_dests = compute_gap_dests(
            seed_val,
            tp.place,
            &tp.geom,
            director.live.host_match(),
            &keys,
            &items,
            &collision_catalog,
            simulation_content_hash,
        );
        tp.rendered = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_pump(
    time: Res<Time>,
    input: MatchPumpInput,
    settings: Res<crate::settings::Settings>,
    rebind: Res<pause_settings::PauseSettingsRebind>,
    mut director: ResMut<MatchDirector>,
    spectator_bot: Option<Res<SpectatorBot>>,
    mut paused: ResMut<MatchPaused>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // While a pause-settings rebind capture is in flight, the keyboard belongs to the
    // capture: Escape is its cancel (not an unpause) and any other key — including the
    // pause binding or `Q` — is a candidate binding, not a menu action. This system is
    // ordered before the pause-settings chain so it observes the capture *before* the
    // same press resolves it.
    let rebind_capturing = rebind.0.is_active();
    if !rebind_capturing
        && (input.keyboard.just_pressed(settings.bindings.pause)
            || gamepad_pause_pressed(&input.gamepads))
    {
        paused.0 = !paused.0;
        input::set_cursor_grab(&mut cursors, !paused.0);
    }
    if paused.0 {
        if !rebind_capturing
            && (input.keyboard.just_pressed(KeyCode::KeyQ) || gamepad_quit_pressed(&input.gamepads))
        {
            next.set(GameState::MainMenu);
        }
        return;
    }
    let spectator_visible_finished = spectator_bot
        .as_ref()
        .is_some_and(|spectator| spectator.finished);
    if let Some(result) = director.tick(
        time.delta(),
        spectator_bot.is_some(),
        spectator_visible_finished,
    ) {
        info!(
            "MATCH_END seed={} live_finished={} live_winner={:?} series_finished={} series_winner={:?} result_winner={:?} placement={:?} series_event={}",
            director.live.seed,
            director.live.finished(),
            director.live.host_match().competitive.winner,
            director.series.finished(),
            director.series.winner,
            result.winner,
            result.placement,
            director.series.last_event
        );
        career.record(result);
        next.set(GameState::Results);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn keystone_pickup(
    tp: Res<TeleportState>,
    mut keys: ResMut<KeystoneState>,
    mut director: ResMut<MatchDirector>,
    items: Query<(Entity, &KeystoneItem, &GlobalTransform)>,
    assets: Res<MatchAssets>,
    settings: Res<crate::settings::Settings>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
    mut commands: Commands,
) {
    const PICKUP_RADIUS: f32 = 2.2;
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    for (entity, item, transform) in &items {
        let here = Vec2::new(transform.translation().x, transform.translation().z);
        let was_open = keys.gate_open();
        if body.distance(here) <= PICKUP_RADIUS && keys.collect(item.0) {
            director.record_local_keystone(item.0);
            audio_director.request(
                &mut commands,
                &assets.keystone,
                MatchAudioCue::Keystone,
                "Keystone pickup",
                None,
                &settings,
            );
            if !was_open && keys.gate_open() {
                audio_director.request(
                    &mut commands,
                    &assets.exit_unlock,
                    MatchAudioCue::ExitUnlock,
                    "Exit unlock",
                    None,
                    &settings,
                );
            }
            commands.entity(entity).despawn();
        }
    }
}
