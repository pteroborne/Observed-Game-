mod lab;

// The progression model was promoted into `crates/observed_progression` (refactor R9).
// Re-export it under the familiar `model` path; this lab is the projection. The
// orthogonality test (which crosses into the match brain) stays in this lab's tests.
pub mod model {
    pub use observed_progression::progression::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{ProgRuntime, SaveSlot};
pub use model::Profile;

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Profile>()
            .init_resource::<ProgRuntime>()
            .init_resource::<SaveSlot>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::present_cells,
                    lab::present_xp_bar,
                    lab::align_xp_bar,
                    lab::draw_selection,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Progression Camera")));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.016, 0.02)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Progression & Cosmetics".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ProgressionPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut profile: ResMut<Profile>,
    mut save: ResMut<SaveSlot>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // A progressed profile: a few wins (levels + unlocks), some cosmetics
        // equipped, and a save written — so the grid and the save string both show.
        for _ in 0..6 {
            let placement = lab::play_local_match();
            profile.award_match(placement);
        }
        profile.equip(1); // Ember
        profile.equip(6); // Comet
        profile.equip(9); // Champion
        save.0 = Some(profile.serialize());
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.4 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use lab::{CosmeticCell, ProgUiRoot};
    use model::catalog;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(ProgressionPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    /// Run the proven competitive match to a deterministic finish and return each
    /// team's placement. Takes no profile — the sim cannot see progression. (Stays in
    /// this lab because it crosses into the match brain.)
    fn play_match_placements() -> Vec<Option<u8>> {
        use competitive_facility::model::CompetitiveFacility;
        let mut m = CompetitiveFacility::authored();
        for _ in 0..10_000 {
            if m.finished {
                break;
            }
            m.advance_round(&[]);
        }
        assert!(m.finished);
        m.teams.iter().map(|t| t.placement).collect()
    }

    #[test]
    fn cosmetics_and_progression_never_touch_the_simulation() {
        // The match result before any progression…
        let baseline = play_match_placements();
        // …after a heavily-progressed, fully-cosmeticized profile…
        let mut p = Profile::new();
        for _ in 0..12 {
            p.award_match(Some(1));
        }
        for c in catalog() {
            p.equip(c.id); // equip everything unlocked
        }
        let after = play_match_placements();
        assert_eq!(
            baseline, after,
            "progression/cosmetics must not change the deterministic match"
        );
    }

    #[test]
    fn boots_with_a_cell_per_cosmetic_and_ui() {
        let mut app = test_app();
        assert_eq!(count::<CosmeticCell>(&mut app), catalog().len());
        assert_eq!(count::<ProgUiRoot>(&mut app), 1);
        assert_eq!(app.world().resource::<Profile>().level(), 0);
    }

    #[test]
    fn the_profile_round_trips_through_the_save_slot() {
        let mut app = test_app();
        {
            let mut profile = app.world_mut().resource_mut::<Profile>();
            for _ in 0..5 {
                profile.award_match(Some(1));
            }
            profile.equip(1);
        }
        // Save, then load into a fresh profile via the slot.
        let serialized = app.world().resource::<Profile>().serialize();
        app.world_mut().resource_mut::<SaveSlot>().0 = Some(serialized.clone());
        let loaded = Profile::parse(&serialized).expect("save parses");
        assert!(app.world().resource::<Profile>().same_state(&loaded));
        app.update();
        assert_eq!(count::<CosmeticCell>(&mut app), catalog().len());
    }

    #[test]
    fn awarding_matches_progresses_and_stays_leak_free() {
        let mut app = test_app();
        let cells = count::<CosmeticCell>(&mut app);
        for expected_played in 1..=8 {
            app.world_mut()
                .resource_mut::<Profile>()
                .award_match(Some(1));
            app.update();
            assert_eq!(count::<CosmeticCell>(&mut app), cells);
            assert_eq!(count::<ProgUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<Profile>().matches_played,
                expected_played
            );
        }
        // After eight wins the profile has levelled and unlocked beyond the defaults.
        assert!(app.world().resource::<Profile>().level() >= 3);
        assert!(app.world().resource::<Profile>().unlocked.len() > 3);
    }
}
