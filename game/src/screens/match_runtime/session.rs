//! The Match session's resource lifecycle: [`setup_match`] builds and inserts every
//! resource the Match owns (the director, the teleport/body state, keystones, items,
//! the guardian, the drop-in assets) and [`cleanup_match_resources`] removes them on
//! exit. The full set is enumerated exactly once in [`for_each_match_resource`], and a
//! test asserts nothing survives `OnExit(Match)` — resources cannot silently leak
//! across matches again.

use bevy::prelude::*;
use observed_core::RoomId;
use observed_facility::map_spec::{RoomRole, sector_relay_v1};
use observed_traversal::{FpsBody, FpsConfig};

use super::crossing::compute_gap_dests;
use super::nav_from_brain;
use crate::flow::{Career, MATCH_SEED};
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::layout::WALL_HEIGHT;
use crate::sim::director::MatchDirector;
use crate::sim::state::{
    ItemIntent, LastTeleportPad, MatchIntent, MatchPaused, SpectatorBot, TeleportState,
};
use crate::teleport::Place;
use crate::view::assets::{MatchAssets, all_planned_assets_present};
use crate::view::components::{DecohereFx, MatchAudioState, TacMapState, TeleportAnimation};

// --- match (first-person 3D, networked) ------------------------------------
pub(crate) fn setup_match(
    mut commands: Commands,
    mut career: ResMut<Career>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    spectator_bot: Option<ResMut<SpectatorBot>>,
) {
    career.begin_match();
    if !all_planned_assets_present() {
        warn!("one or more planned match assets are absent; procedural fallbacks will be used");
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    if let Some(mut spectator) = spectator_bot
        && spectator.seed != seed_val
    {
        *spectator = SpectatorBot::for_seed(seed_val);
    }
    let map_spec = sector_relay_v1();
    let director = MatchDirector::new(seed_val, map_spec.clone());
    let game = director.live.host_match();
    let initial_escaped = game.competitive.escaped_count();
    let initial_commits = game.reroute_commits;
    let keys = KeystoneState::for_map(seed_val, &map_spec);
    let items = ItemsState::single_player();
    let tp_config = FpsConfig::default();
    let start_place = Place::Room(game.local_room());
    let start_geom =
        crate::teleport::geom_for(start_place, &nav_from_brain(seed_val, game, &keys, &items));
    let start_arena = crate::teleport::place_arena(&start_geom, 0.0, WALL_HEIGHT);
    let start_gap_dests =
        compute_gap_dests(seed_val, start_place, &start_geom, game, &keys, &items);
    let spawn = Vec3::new(0.0, tp_config.half_height, 0.0);
    commands.insert_resource(director);
    commands.insert_resource(MatchPaused(false));
    commands.insert_resource(TacMapState(false));
    commands.insert_resource(MatchIntent::default());
    commands.insert_resource(ItemIntent::default());
    commands.insert_resource(DecohereFx {
        last_commits: initial_commits,
        flash: 0.0,
    });
    commands.insert_resource(MatchAudioState {
        last_position: spawn,
        stride_distance: 0.0,
        last_place: start_place,
        escaped_count: initial_escaped,
    });
    commands.insert_resource(TeleportState {
        place: start_place,
        body: FpsBody::spawned(spawn, 0.0),
        config: tp_config,
        arena: start_arena,
        geom: start_geom,
        prev_xz: Vec2::ZERO,
        crossed_exit: false,
        pending_exit: None,
        arrived_from: None,
        gap_dests: start_gap_dests,
        rendered: None,
    });
    commands.insert_resource(keys);
    commands.insert_resource(items);
    let guardian_room = map_spec
        .role_room(RoomRole::GuardianControl)
        .unwrap_or(RoomId(8));
    commands.insert_resource(crate::guardian::Guardian {
        room: guardian_room,
        ..default()
    });
    commands.insert_resource(crate::guardian::ActionLog::default());
    commands.insert_resource(TeleportAnimation::default());
    commands.insert_resource(LastTeleportPad::default());

    commands.insert_resource(MatchAssets::load(
        &asset_server,
        &mut meshes,
        &mut materials,
    ));

    super::super::hud::spawn_match_hud(&mut commands);
}

/// Every resource the Match session owns, enumerated once. `setup_match` inserts
/// them (plus the menu-inserted `SpectatorBot`), `cleanup_match_resources` removes
/// them, and the `every_match_resource_is_removed_when_the_match_ends` test asserts
/// none survive `OnExit(Match)` — so adding a resource here is the whole checklist.
macro_rules! for_each_match_resource {
    ($apply:ident) => {
        $apply!(crate::sim::director::MatchDirector);
        $apply!(crate::sim::state::SpectatorBot);
        $apply!(crate::sim::state::MatchIntent);
        $apply!(crate::sim::state::ItemIntent);
        $apply!(crate::sim::state::MatchPaused);
        $apply!(crate::sim::state::LastTeleportPad);
        $apply!(crate::sim::state::TeleportState);
        $apply!(crate::view::assets::MatchAssets);
        $apply!(crate::view::components::TacMapState);
        $apply!(crate::view::components::MatchAudioState);
        $apply!(crate::view::components::DecohereFx);
        $apply!(crate::view::components::TeleportAnimation);
        $apply!(crate::keystones::KeystoneState);
        $apply!(crate::items::ItemsState);
        $apply!(crate::guardian::Guardian);
        $apply!(crate::guardian::ActionLog);
    };
}
#[cfg(test)]
pub(crate) use for_each_match_resource;

pub(crate) fn cleanup_match_resources(mut commands: Commands) {
    macro_rules! remove {
        ($ty:ty) => {
            commands.remove_resource::<$ty>();
        };
    }
    for_each_match_resource!(remove);
}
