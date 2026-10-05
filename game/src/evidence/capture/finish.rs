//! Native feature evidence: real loopback lobby, staged portrait poses, recorded replay.
use super::{Shot, shot};
use crate::{
    GameState,
    flow::Career,
    play_setup::{PlayPreset, PlayRules, PlaySeat, PlaySetupDraft},
};
use bevy::prelude::*;
use observed_core::{PlayerId, cosmetics::CosmeticLook};

#[derive(Resource)]
pub(crate) struct Peer(observed_net::lan::LanClient);
#[derive(Resource)]
pub(crate) struct Portrait {
    player: PlayerId,
    at: Vec3,
    frames: usize,
    timestep: std::time::Duration,
}

pub(super) fn sweep() -> Vec<Shot> {
    [
        ("00_cosmetics_applied", GameState::Loadout),
        ("01_ascent_coop", GameState::Play),
        ("02_ascent_advanced", GameState::PlayAdvanced),
        ("03_race_four_seats", GameState::PlayAdvanced),
        ("04_connected_ascent_lobby", GameState::Lobby),
        ("05_cobalt_comet_champion", GameState::HexWfc),
        ("06_ember_spark_veteran", GameState::HexWfc),
        ("07_void_no_trail", GameState::HexWfc),
        ("08_recorded_cosmetics", GameState::Replay),
    ]
    .into_iter()
    .enumerate()
    .map(|(case, (label, state))| Shot {
        finish: Some(case),
        extra_settle: if (5..=7).contains(&case) { 3.0 } else { 0.0 },
        ..shot(label, state)
    })
    .collect()
}
fn case_look(case: usize) -> CosmeticLook {
    match case {
        6 => CosmeticLook {
            color: 1,
            trail: 5,
            badge: 8,
        },
        7 => CosmeticLook {
            color: 3,
            trail: 4,
            badge: 9,
        },
        _ => CosmeticLook {
            color: 2,
            trail: 6,
            badge: 9,
        },
    }
}
pub(super) fn stage(world: &mut World, case: usize) {
    if let Some(portrait) = world.remove_resource::<Portrait>() {
        world
            .resource_mut::<Time<Fixed>>()
            .set_timestep(portrait.timestep);
    }
    world.remove_resource::<Peer>();
    world
        .resource_mut::<crate::settings::Settings>()
        .complete_onboarding();
    world.resource_mut::<crate::lan::LanRuntime>().leave();
    let mut career = Career::default();
    for _ in 0..15 {
        career.profile.award_match(Some(1));
    }
    let look = case_look(case);
    for id in [look.color, look.trail, look.badge] {
        assert!(career.profile.equip(id));
    }
    world.insert_resource(career);
    let mut setup = PlaySetupDraft::default();
    setup.select_preset(PlayPreset::CoOp);
    if case == 2 {
        setup.seat = PlaySeat::Architect;
    }
    if case == 3 {
        setup.select_rules(PlayRules::Race);
    }
    world.insert_resource(setup);
    if case == 4 {
        let config = observed_server::ServerConfig::from_args(
            [
                "observed-evidence",
                "--bind",
                "127.0.0.1:0",
                "--ascent",
                "--co-op",
                "3",
                "--no-discovery",
                "--seed",
                "0xF011FAC11177",
            ]
            .map(String::from),
        )
        .expect("fixture host setup");
        let handle = observed_server::ServerHandle::spawn(config).expect("loopback server");
        let address = handle.address;
        let hash = crate::hex_wfc::sim::simulation_content_hash();
        let first = observed_net::lan::LanClient::connect_with_appearance(
            address, 900, None, None, hash, look,
        )
        .unwrap();
        let second = observed_net::lan::LanClient::connect_with_appearance(
            address,
            901,
            None,
            None,
            hash,
            case_look(6),
        )
        .unwrap();
        let mut lan = world.resource_mut::<crate::lan::LanRuntime>();
        lan.listen_server = Some(handle);
        lan.client = Some(first);
        world.insert_resource(Peer(second));
    }
    if case == 8 {
        super::replay::stage(world);
        // The recording fixture runs real rules/physics; its immutable look metadata is staged.
        world
            .resource_mut::<crate::sim::replay::ReplayTape>()
            .cosmetics
            .insert(PlayerId(0), look);
    }
}
pub(super) fn pose(world: &mut World, case: usize) {
    if (5..=7).contains(&case) {
        let runtime = world.resource::<crate::hex_wfc::sim::HexWfcRuntime>();
        let player = runtime.local_player;
        let at = runtime.local().position;
        let timestep = world.resource::<Time<Fixed>>().timestep();
        world
            .resource_mut::<Time<Fixed>>()
            .set_timestep(std::time::Duration::from_secs(3600));
        world.insert_resource(Portrait {
            player,
            at,
            frames: 0,
            timestep,
        });
    }
    if case == 8 {
        super::replay::pose(world, 0);
    }
}
pub(crate) fn poll_peer(mut peer: Option<ResMut<Peer>>) {
    let Some(peer) = peer.as_deref_mut() else {
        return;
    };
    peer.0.poll();
    let claimed = peer
        .0
        .player
        .zip(peer.0.lobby.as_ref())
        .is_some_and(|(player, lobby)| {
            lobby
                .seats
                .iter()
                .any(|s| s.player == player && s.architect)
        });
    if peer.0.token.is_some() && !claimed {
        peer.0.claim_architect(true).expect("claim fixture desk");
    }
}
pub(crate) fn pose_camera(
    state: Res<State<GameState>>,
    mut pose: Option<ResMut<Portrait>>,
    mut runtime: Option<ResMut<crate::hex_wfc::sim::HexWfcRuntime>>,
    mut camera: Query<&mut Transform, With<crate::view::components::GameCam>>,
) {
    if *state.get() != GameState::HexWfc {
        return;
    }
    let (Some(pose), Some(runtime), Ok(mut camera)) = (
        pose.as_deref_mut(),
        runtime.as_deref_mut(),
        camera.single_mut(),
    ) else {
        return;
    };
    // Staged movement exercises the actual bounded trail renderer, without claiming motor validation.
    let offset = (6usize.saturating_sub(pose.frames)) as f32 * 0.2;
    let body = runtime.match_state.players.get_mut(&pose.player).unwrap();
    body.position = pose.at - Vec3::X * offset;
    body.yaw = 0.0;
    body.pitch = 0.0;
    pose.frames += 1;
    let eye = pose.at + Vec3::Y * observed_observer::form::EYE_RISE;
    *camera =
        Transform::from_translation(eye + Vec3::new(0.8, 0.45, -3.4)).looking_at(eye, Vec3::Y);
}
