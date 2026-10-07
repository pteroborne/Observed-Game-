//! Opt-in, resettable Backrooms review using the production loading, input,
//! controller and view paths. R exits the match before preparing a fresh one.
use super::{
    launch::{HexLaunchSpec, HexSeedPolicy},
    loading::HexLaunchRequestSequence,
    sim::{HexWfcRuntime, LOCAL_PLAYER},
};
use crate::{
    GameState,
    play_setup::{LaunchContext, PlayPreset, PlayRules, PlaySeat, PlaySetupDraft},
};
use bevy::prelude::*;

#[derive(Resource, Default)]
struct Reference {
    pending: bool,
    staged: bool,
    capture: Option<std::path::PathBuf>,
    frame: u16,
    resets: u8,
    before_entities: Option<usize>,
}

mod falling;

pub(crate) fn install(app: &mut App) {
    falling::install(app);
    let capture =
        std::env::var_os("OBSERVED2_BACKROOMS_REFERENCE_CAPTURE").map(std::path::PathBuf::from);
    if let Some(path) = &capture {
        std::fs::create_dir_all(path).expect("reference capture directory");
    }
    app.insert_resource(Reference {
        pending: true,
        staged: false,
        capture,
        ..default()
    })
    .add_systems(Startup, launch)
    .add_systems(OnEnter(GameState::MainMenu), launch)
    .add_systems(Update, controls.run_if(in_state(GameState::HexWfc)));
}

fn launch(
    mut commands: Commands,
    mut reference: ResMut<Reference>,
    mut sequence: ResMut<HexLaunchRequestSequence>,
    mut setup: ResMut<PlaySetupDraft>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !std::mem::take(&mut reference.pending) {
        return;
    }
    *setup = PlaySetupDraft {
        rules: PlayRules::Race,
        seat: PlaySeat::Observer,
        ..PlaySetupDraft::for_preset(PlayPreset::Solo)
    };
    let mut config = super::sim::runtime_config_for(&setup);
    config.guardian = false;
    commands.insert_resource(sequence.issue(
        LaunchContext::Local,
        LOCAL_PLAYER,
        false,
        false,
        HexLaunchSpec {
            requested_seed: crate::flow::MATCH_SEED,
            config,
            seed_policy: HexSeedPolicy::Nearby,
        },
        (setup.rules, setup.seat),
    ));
    reference.staged = false;
    reference.frame = 0;
    next.set(GameState::Loading);
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut reference: ResMut<Reference>,
    mut runtime: ResMut<HexWfcRuntime>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    entities: Query<Entity>,
    mut exit: MessageWriter<AppExit>,
) {
    reference.frame = reference.frame.saturating_add(1);
    if !reference.staged {
        runtime.match_state.hold_environment_for_reference();
        commands.spawn((
            Text::new("BACKROOMS REFERENCE | 1-6 views | R reset | WASD walk | Esc pause"),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: px(20),
                bottom: px(70),
                ..default()
            },
            DespawnOnExit(GameState::HexWfc),
        ));
    }
    let scripted_reset =
        reference.capture.is_some() && reference.resets == 0 && reference.frame == 210;
    if keys.just_pressed(KeyCode::KeyR) || scripted_reset {
        reference.pending = true;
        reference.resets = reference.resets.saturating_add(1);
        next.set(GameState::MainMenu);
        return;
    }
    let selected = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ]
    .into_iter()
    .position(|key| keys.just_pressed(key));
    if selected.is_some() || !reference.staged {
        let poses = super::vista_capture::surfaces::backrooms_poses(&runtime.match_state.facility);
        if let Some(pose) = poses.get(selected.unwrap_or(0)) {
            let ahead = Vec3::new(pose.yaw.sin(), 0.0, -pose.yaw.cos());
            let local = runtime.local_player;
            runtime.match_state.stage_body_facing(
                local,
                pose.cell,
                pose.feet,
                pose.feet + Vec3::Y * (1.6 + pose.pitch.sin() * 4.0) + ahead * 4.0,
            );
        }
        reference.staged = true;
    }
    if let Some(path) = reference.capture.clone() {
        if reference.frame == 180 {
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            let name = if reference.resets == 0 {
                "before-reset.png"
            } else {
                "after-reset.png"
            };
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.join(name)));
            let count = entities.iter().count();
            if reference.resets == 0 {
                reference.before_entities = Some(count);
            } else {
                let report = serde_json::json!({"seed":runtime.match_state.seed,"resets":reference.resets,"before_entities":reference.before_entities,"after_entities":count});
                std::fs::write(
                    path.join("reset.json"),
                    serde_json::to_string_pretty(&report).expect("reset report"),
                )
                .expect("save reset report");
            }
        }
        if reference.resets > 0 && reference.frame >= 205 {
            exit.write(AppExit::Success);
        }
    }
}
