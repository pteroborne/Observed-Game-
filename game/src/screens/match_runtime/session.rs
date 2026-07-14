//! The Match session's resource lifecycle: [`setup_match`] builds and inserts every
//! resource the Match owns (the director, the teleport/body state, keystones, items,
//! the guardian, the drop-in assets) and [`cleanup_match_resources`] removes them on
//! exit. The full set is enumerated exactly once in [`for_each_match_resource`], and a
//! test asserts nothing survives `OnExit(Match)` — resources cannot silently leak
//! across matches again.

use bevy::prelude::*;
use observed_facility::map_spec::RoomRole;
use observed_traversal::FpsBody;

use super::crossing::compute_gap_dests;
use crate::flow::{Career, MATCH_SEED};
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::layout::WALL_HEIGHT;
use crate::sim::director::MatchDirector;
use crate::sim::nav::nav_from_brain;
use crate::sim::replay::ReplayTape;
use crate::sim::state::{
    ItemIntent, LastTeleportPad, MapKnowledge, MatchIntent, MatchPaused, RivalSightings,
    SpectatorBot, TeleportState,
};
use crate::teleport::Place;
use crate::view::assets::{MatchAssets, all_planned_assets_present};
use crate::view::components::{
    CameraJuice, DebugHud, DecohereFx, MatchAudioState, RivalBleedState, TacMapState,
    TeleportAnimation,
};

// --- match (first-person 3D, networked) ------------------------------------
#[allow(clippy::too_many_arguments)]
pub(crate) fn setup_match(
    mut commands: Commands,
    mut career: ResMut<Career>,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    spectator_bot: Option<ResMut<SpectatorBot>>,
    settings: Res<crate::settings::Settings>,
    debug_hud: Res<DebugHud>,
    content: Res<crate::content::GameContent>,
) {
    career.begin_match();
    if !all_planned_assets_present() {
        warn!("one or more planned match assets are absent; procedural fallbacks will be used");
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let has_spectator = spectator_bot.is_some();
    if let Some(mut spectator) = spectator_bot
        && spectator.seed != seed_val
    {
        *spectator = SpectatorBot::for_seed(seed_val);
    }
    let map_spec = crate::map_catalog::active_map_spec(seed_val);
    let career_config = crate::sim::director::BotPopulations {
        rival_teams: career.bot_rival_teams,
        ai_teammates: career.bot_ai_teammates,
        guardian: career.bot_guardian,
    };
    let config = if let Some(env_config) = crate::sim::director::BotPopulations::from_env() {
        env_config
    } else {
        career_config
    };
    info!(
        "MATCH_START seed={} map={} spectator={} rivals={} teammates={} guardian={}",
        seed_val,
        map_spec.name,
        has_spectator,
        config.rival_teams,
        config.ai_teammates,
        config.guardian
    );
    let director = MatchDirector::new(seed_val, map_spec.clone(), config);
    let game = director.live.host_match();
    let initial_escaped = game.competitive.escaped_count();
    let initial_commits = game.reroute_commits;
    let keys = KeystoneState::for_map(seed_val, &map_spec);
    let items = ItemsState::single_player();
    let tp_config = content.traversal_config();
    let start_place = Place::Room(game.local_room());
    let start_geom =
        crate::teleport::geom_for(start_place, &nav_from_brain(seed_val, game, &keys, &items));
    let start_gap_dests = compute_gap_dests(
        seed_val,
        start_place,
        &start_geom,
        game,
        &keys,
        &items,
        &content.collision_catalog,
        content.simulation_hash.0,
    );
    let spawn = Vec3::new(0.0, tp_config.half_height, 0.0);
    commands.insert_resource(director);
    commands.insert_resource(ReplayTape::new_with_content(
        seed_val,
        &map_spec,
        content.simulation_hash.0,
        content.presentation_hash.0,
    ));
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
        collapse_sting_place: None,
    });
    commands.insert_resource(crate::screens::audio::AudioDirector::default());
    commands.insert_resource(RivalBleedState::default());
    let rapier = crate::teleport::place_rapier_scene(&start_geom, 0.0, WALL_HEIGHT);
    commands.insert_resource(TeleportState {
        place: start_place,
        body: FpsBody::spawned(spawn, 0.0),
        config: tp_config,
        rapier,
        collision_catalog: content.collision_catalog.clone(),
        simulation_content_hash: content.simulation_hash.0,
        layout: None,
        geom: start_geom,
        prev_xz: Vec2::ZERO,
        crossed_exit: false,
        pending_exit: None,
        arrived_from: None,
        gap_dests: start_gap_dests,
        rendered: None,
        prev_grounded: false,
    });
    commands.insert_resource(keys);
    commands.insert_resource(items);
    commands.insert_resource(RivalSightings::default());
    commands.insert_resource(MapKnowledge::default());

    if config.guardian {
        let guardian_room = map_spec
            .role_room(RoomRole::GuardianControl)
            .unwrap_or_else(|| {
                panic!(
                    "active map spec `{}` is missing a required GuardianControl room; \
                 every catalog map must satisfy MapSpec::validate()",
                    map_spec.name
                )
            });
        commands.insert_resource(crate::guardian::Guardian {
            room: guardian_room,
            ..default()
        });
        commands.insert_resource(crate::guardian::ActionLog::default());
    }
    commands.insert_resource(TeleportAnimation::default());
    commands.insert_resource(LastTeleportPad::default());
    commands.insert_resource(CameraJuice::default());

    commands.insert_resource(MatchAssets::load(
        &asset_server,
        &content.manifest,
        &mut texture_atlases,
        &mut meshes,
        &mut materials,
    ));

    super::super::hud::spawn_match_hud(&mut commands, settings.high_contrast, debug_hud.0);
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
        $apply!(crate::screens::audio::AudioDirector);
        $apply!(crate::view::components::RivalBleedState);
        $apply!(crate::view::components::DecohereFx);
        $apply!(crate::view::components::TeleportAnimation);
        $apply!(crate::view::components::CameraJuice);
        $apply!(crate::keystones::KeystoneState);
        $apply!(crate::sim::state::RivalSightings);
        $apply!(crate::sim::state::MapKnowledge);
        $apply!(crate::items::ItemsState);
        $apply!(crate::guardian::Guardian);
        $apply!(crate::guardian::ActionLog);
        $apply!(crate::screens::onboarding::OnboardingState);
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
