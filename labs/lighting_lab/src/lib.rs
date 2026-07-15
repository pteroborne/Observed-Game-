//! **lighting_lab** — the nine production architecture-register dioramas.
//!
//! Each scene isolates one liminal register from the user-chosen reference set
//! (see `docs/light_and_line_arc_plan.md`): keys **1–9** switch dioramas, **V**
//! toggles volumetric fog (scenes that stage it), **B** toggles bloom, **R**
//! respawns the scene, **F1** hides the overlay. The signal kit (objective,
//! anchor, rival, exit frame — real `observed_style` treatments) stands in
//! every scene: a register that hides a signal fails the Legibility Contract.
//!
//! `OBSERVED2_CAPTURE=<dir>` walks all nine scenes plus the volumetrics×bloom
//! matrix on the Facet Monument scene, screenshots each, grades every capture
//! against the luminance corridor ([`luminance`]), records per-scene frame
//! times, writes `manifest.json`, and exits.

pub mod luminance;
pub mod scenes;

use bevy::{
    app::AppExit,
    input::InputSystems,
    light::VolumetricFog,
    post_process::bloom::Bloom,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use scenes::{Scene, SceneCam, SceneCtx, SceneSpawned};
use serde::Serialize;

/// The active diorama; setting `dirty` rebuilds it on the next frame.
#[derive(Resource)]
pub struct SceneState {
    pub scene: Scene,
    pub dirty: bool,
}

/// The V / B toggles. Volumetrics apply only where the scene stages a shaft.
#[derive(Resource)]
pub struct Toggles {
    pub volumetrics: bool,
    pub bloom: bool,
}

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct OverlayText;

pub struct LightingLabPlugin;

impl Plugin for LightingLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SceneState {
            scene: Scene::ShadowScreen,
            dirty: true,
        })
        .insert_resource(Toggles {
            volumetrics: true,
            bloom: true,
        })
        .add_systems(Startup, spawn_overlay)
        .add_systems(
            Update,
            (
                scene_input.after(InputSystems),
                rebuild_scene,
                sync_camera_effects,
                update_overlay,
            )
                .chain(),
        );
    }
}

fn scene_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SceneState>,
    mut toggles: ResMut<Toggles>,
    mut overlay: Query<&mut Visibility, With<OverlayRoot>>,
) {
    const KEYS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in KEYS.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            state.scene = Scene::ALL[i];
            state.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        toggles.volumetrics = !toggles.volumetrics;
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        toggles.bloom = !toggles.bloom;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        for mut vis in &mut overlay {
            *vis = match *vis {
                Visibility::Hidden => Visibility::Visible,
                _ => Visibility::Hidden,
            };
        }
    }
}

/// Tear down the previous diorama completely and build the requested one. The
/// reset discipline: everything a scene makes is [`SceneSpawned`], so a switch
/// leaks nothing.
fn rebuild_scene(
    mut state: ResMut<SceneState>,
    spawned: Query<Entity, With<SceneSpawned>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    let mut ctx = SceneCtx {
        commands: &mut commands,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    state.scene.spawn(&mut ctx);
}

/// Keep the camera's Bloom / VolumetricFog components in sync with the toggles.
/// Volumetric fog is only inserted where the scene stages a shaft to see.
fn sync_camera_effects(
    state: Res<SceneState>,
    toggles: Res<Toggles>,
    cams: Query<(Entity, Has<Bloom>, Has<VolumetricFog>), With<SceneCam>>,
    mut commands: Commands,
) {
    for (cam, has_bloom, has_vol) in &cams {
        if toggles.bloom && !has_bloom {
            commands.entity(cam).insert(Bloom::NATURAL);
        } else if !toggles.bloom && has_bloom {
            commands.entity(cam).remove::<Bloom>();
        }
        let want_vol = toggles.volumetrics && state.scene.volumetric();
        if want_vol && !has_vol {
            commands.entity(cam).insert(VolumetricFog {
                // A bounded volume must remain visible even when a backend
                // culls an off-screen spotlight before the volume pass.
                ambient_color: Color::srgb(0.32, 0.50, 0.78),
                ambient_intensity: 0.72,
                step_count: 96,
                ..default()
            });
        } else if !want_vol && has_vol {
            commands.entity(cam).remove::<VolumetricFog>();
        }
    }
}

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            OverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                top: px(12),
                padding: UiRect::all(px(12)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.9, 1.0, 0.55)),
            GlobalZIndex(20),
            Name::new("Lighting Lab Overlay"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayText,
                Text::new("…"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.94, 1.0)),
            ));
        });
}

fn update_overlay(
    state: Res<SceneState>,
    toggles: Res<Toggles>,
    spawned: Query<(), With<SceneSpawned>>,
    cams: Query<(), With<SceneCam>>,
    mut text: Query<&mut Text, With<OverlayText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let scene = state.scene;
    **text = format!(
        "LIGHTING LAB — scene {}/9  [{}]\n{}\n\n\
         entities {} · cameras {}\n\
         volumetrics {} (V){} · bloom {} (B)\n\
         1–9 scene · R respawn · F1 overlay",
        scene.index() + 1,
        scene.slug(),
        scene.title(),
        spawned.iter().count(),
        cams.iter().count(),
        if toggles.volumetrics { "on" } else { "off" },
        if scene.volumetric() {
            ""
        } else {
            " [n/a here]"
        },
        if toggles.bloom { "on" } else { "off" },
    );
}

// --- capture: walk every register, grade every frame, prove it ---------------

#[derive(Clone, Copy, Debug)]
enum CaptureStep {
    /// One scene in its default rig.
    Scene(usize),
    /// The volumetrics × bloom matrix on the Facet Monument scene:
    /// 0 = vol off / bloom on, 1 = vol on / bloom off, 2 = vol off / bloom off.
    Matrix(usize),
}

impl CaptureStep {
    const SETTLE: f32 = 1.5;
    const READBACK: f32 = 0.7;

    fn sequence() -> Vec<CaptureStep> {
        let mut steps: Vec<CaptureStep> = (0..Scene::ALL.len()).map(CaptureStep::Scene).collect();
        steps.extend([0, 1, 2].map(CaptureStep::Matrix));
        steps
    }

    fn label(self) -> String {
        match self {
            CaptureStep::Scene(i) => {
                format!("scene_{:02}_{}", i + 1, Scene::ALL[i].slug())
            }
            CaptureStep::Matrix(v) => {
                let variant = ["vol-off_bloom-on", "vol-on_bloom-off", "vol-off_bloom-off"][v];
                format!("scene_05_facet-monument_{variant}")
            }
        }
    }

    fn apply(self, state: &mut SceneState, toggles: &mut Toggles) {
        match self {
            CaptureStep::Scene(i) => {
                state.scene = Scene::ALL[i];
                toggles.volumetrics = true;
                toggles.bloom = true;
            }
            CaptureStep::Matrix(v) => {
                state.scene = Scene::FacetMonument;
                toggles.volumetrics = v == 1;
                toggles.bloom = v == 0;
            }
        }
        state.dirty = true;
    }
}

#[derive(Serialize)]
struct ManifestEntry {
    file: String,
    frame_hash: String,
    #[serde(flatten)]
    verdict: luminance::CorridorVerdict,
    avg_frame_ms: f32,
}

#[derive(Resource, Default)]
struct CaptureManifest {
    entries: Vec<ManifestEntry>,
    expected: usize,
}

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    steps: Vec<CaptureStep>,
    index: usize,
    phase: u8, // 0 = staging, 1 = settling, 2 = shot queued, 3 = all steps done
    next_at: f32,
    frame_ms_sum: f32,
    frame_count: u32,
    pending_frame_ms: f32,
}

#[allow(clippy::too_many_arguments)]
fn capture_progress(
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut state: ResMut<SceneState>,
    mut toggles: ResMut<Toggles>,
    mut manifest: ResMut<CaptureManifest>,
    mut overlay: Query<&mut Visibility, With<OverlayRoot>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match run.phase {
        0 => {
            // Clean dioramas: the overlay never appears in evidence.
            for mut vis in &mut overlay {
                *vis = Visibility::Hidden;
            }
            let step = run.steps[run.index];
            step.apply(&mut state, &mut toggles);
            run.next_at = elapsed + CaptureStep::SETTLE;
            run.frame_ms_sum = 0.0;
            run.frame_count = 0;
            run.phase = 1;
        }
        1 => {
            run.frame_ms_sum += time.delta_secs() * 1000.0;
            run.frame_count += 1;
            if elapsed >= run.next_at {
                let step = run.steps[run.index];
                let label = step.label();
                let file = format!("{}/{label}.png", run.dir);
                run.pending_frame_ms = run.frame_ms_sum / run.frame_count.max(1) as f32;
                let avg_frame_ms = run.pending_frame_ms;
                let entry_file = format!("{label}.png");
                manifest.expected += 1;
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(file))
                    .observe(
                        move |shot: On<ScreenshotCaptured>,
                              mut manifest: ResMut<CaptureManifest>| {
                            let (verdict, hash) = match shot.image.clone().try_into_dynamic() {
                                Ok(dynamic) => {
                                    let rgba = dynamic.to_rgba8();
                                    (
                                        luminance::corridor(rgba.as_raw(), 4),
                                        frame_hash(rgba.as_raw()),
                                    )
                                }
                                Err(_) => (luminance::corridor(&[], 1), frame_hash(&[])),
                            };
                            info!(
                                "CAPTURE_VERDICT {entry_file} p05={:.4} p50={:.4} p95={:.4} floor={} ceiling={}",
                                verdict.p05,
                                verdict.p50,
                                verdict.p95,
                                verdict.floor_pass,
                                verdict.ceiling_pass
                            );
                            manifest.entries.push(ManifestEntry {
                                file: entry_file.clone(),
                                frame_hash: format!("{hash:016x}"),
                                verdict,
                                avg_frame_ms,
                            });
                        },
                    );
                run.next_at = elapsed + CaptureStep::READBACK;
                run.phase = 2;
            }
        }
        2 if elapsed >= run.next_at => {
            run.index += 1;
            if run.index >= run.steps.len() {
                run.phase = 3;
                run.next_at = elapsed + 5.0; // readback grace before forced exit
            } else {
                run.phase = 0;
            }
        }
        3 if manifest.entries.len() >= manifest.expected || elapsed >= run.next_at => {
            let volumetric_pair_differs = capture_pair_differs(
                &manifest.entries,
                "scene_05_facet-monument.png",
                "scene_05_facet-monument_vol-off_bloom-on.png",
            );
            let json = serde_json::to_string_pretty(&manifest.entries)
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
            let path = format!("{}/manifest.json", run.dir);
            if let Err(e) = std::fs::write(&path, json) {
                error!("manifest write failed: {e}");
            }
            if !volumetric_pair_differs {
                error!("Facet Monument volumetric-on/off captures are identical or missing");
            }
            exit.write(if volumetric_pair_differs {
                AppExit::Success
            } else {
                AppExit::error()
            });
            run.phase = 4;
        }
        _ => {}
    }
}

fn frame_hash(rgba: &[u8]) -> u64 {
    rgba.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn capture_pair_differs(entries: &[ManifestEntry], on: &str, off: &str) -> bool {
    let hash = |file: &str| {
        entries
            .iter()
            .find(|entry| entry.file == file)
            .map(|entry| entry.frame_hash.as_str())
    };
    matches!((hash(on), hash(off)), (Some(on), Some(off)) if on != off)
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.004, 0.006, 0.012)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Lighting Lab (Arc I: Light & Line)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(LightingLabPlugin);

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(CaptureRun {
            dir,
            steps: CaptureStep::sequence(),
            index: 0,
            phase: 0,
            next_at: 0.0,
            frame_ms_sum: 0.0,
            frame_count: 0,
            pending_frame_ms: 0.0,
        })
        .init_resource::<CaptureManifest>()
        .add_systems(Update, capture_progress.after(scene_input));
    }

    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        input::InputPlugin,
        light::{FogVolume, VolumetricLight},
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_plugins(LightingLabPlugin);
        app.update();
        app
    }

    fn count<T: bevy::prelude::Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn count_named(app: &mut App, expected: &str) -> usize {
        let world = app.world_mut();
        let mut query = world.query::<&Name>();
        query
            .iter(world)
            .filter(|name| name.as_str() == expected)
            .count()
    }

    fn any_name_contains(app: &mut App, needle: &str) -> bool {
        let world = app.world_mut();
        let mut query = world.query::<&Name>();
        query.iter(world).any(|name| name.as_str().contains(needle))
    }

    fn switch_to(app: &mut App, scene: Scene) {
        let mut state = app.world_mut().resource_mut::<SceneState>();
        state.scene = scene;
        state.dirty = true;
        app.update();
        app.update();
    }

    #[test]
    fn every_scene_spawns_a_rig_with_exactly_one_camera() {
        let mut app = test_app();
        for scene in Scene::ALL {
            switch_to(&mut app, scene);
            assert_eq!(
                count::<SceneCam>(&mut app),
                1,
                "{}: exactly one camera",
                scene.slug()
            );
            assert!(
                count::<SceneSpawned>(&mut app) > 10,
                "{}: a real diorama, not an empty stage",
                scene.slug()
            );
        }
    }

    #[test]
    fn switching_scenes_leaks_nothing() {
        let mut app = test_app();
        switch_to(&mut app, Scene::ShadowScreen);
        let baseline = count::<SceneSpawned>(&mut app);
        for scene in Scene::ALL {
            switch_to(&mut app, scene);
        }
        switch_to(&mut app, Scene::ShadowScreen);
        assert_eq!(
            count::<SceneSpawned>(&mut app),
            baseline,
            "returning to a scene rebuilds exactly the same rig"
        );
    }

    #[test]
    fn the_capture_sequence_covers_every_scene_plus_the_matrix() {
        let steps = CaptureStep::sequence();
        assert_eq!(steps.len(), Scene::ALL.len() + 3);
        let labels: Vec<String> = steps.iter().map(|s| s.label()).collect();
        for scene in Scene::ALL {
            assert!(
                labels.iter().any(|l| l.contains(scene.slug())),
                "{} is captured",
                scene.slug()
            );
        }
        assert!(labels.iter().any(|l| l.contains("vol-off_bloom-off")));
    }

    #[test]
    fn scene_labels_are_stable_evidence_filenames() {
        // Evidence filenames are load-bearing (docs link them); pin them.
        assert_eq!(CaptureStep::Scene(0).label(), "scene_01_shadow-screen");
        assert_eq!(CaptureStep::Scene(4).label(), "scene_05_facet-monument");
        assert_eq!(CaptureStep::Scene(8).label(), "scene_09_thinning");
        assert_eq!(
            CaptureStep::Matrix(2).label(),
            "scene_05_facet-monument_vol-off_bloom-off"
        );
    }

    #[test]
    fn exposed_catalogue_uses_production_register_names() {
        let slugs: Vec<&str> = Scene::ALL.iter().map(|scene| scene.slug()).collect();
        assert_eq!(
            slugs,
            [
                "shadow-screen",
                "monolith",
                "overlit-grid",
                "institutional",
                "facet-monument",
                "megastructure",
                "wellshaft",
                "infinite-gallery",
                "thinning",
            ]
        );
        let exposed = Scene::ALL
            .iter()
            .flat_map(|scene| [scene.slug(), scene.title()])
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        for reference in ["shoji", "lumon", "severance", "halo", "forerunner", "silo"] {
            assert!(
                !exposed.contains(reference),
                "reference-only name leaked into the public catalogue: {reference}"
            );
        }
    }

    #[test]
    fn shadow_screen_is_one_sparse_run_with_floor_only_blades() {
        let coverage =
            scenes::shadow_screen::SCREEN_LENGTH / scenes::shadow_screen::CORRIDOR_LENGTH;
        assert!((0.20..=0.35).contains(&coverage));
        assert!((3..=8).contains(&scenes::shadow_screen::FLOOR_BLADE_COUNT));

        let mut app = test_app();
        switch_to(&mut app, Scene::ShadowScreen);
        assert_eq!(
            count_named(&mut app, "Floor light blade"),
            scenes::shadow_screen::FLOOR_BLADE_COUNT
        );
        assert!(count_named(&mut app, "Shadow screen slat") >= 12);
        assert!(!any_name_contains(&mut app, "Paper"));
    }

    #[test]
    fn institutional_set_contains_turns_decision_room_and_factory_expanse() {
        use scenes::institutional::Zone;

        assert_eq!(scenes::institutional::route_turn_count(), 4);
        let plan = scenes::institutional::plan();
        assert_eq!(
            plan.values()
                .filter(|zone| **zone == Zone::DecisionRoom)
                .count(),
            9
        );
        assert_eq!(
            plan.values()
                .filter(|zone| **zone == Zone::FactoryExpanse)
                .count(),
            24
        );

        let mut app = test_app();
        switch_to(&mut app, Scene::Institutional);
        assert_eq!(count_named(&mut app, "Decision room floor"), 9);
        assert_eq!(count_named(&mut app, "Empty factory floor"), 24);
    }

    #[test]
    fn facet_monument_uses_obtuse_junctions_and_toggleable_shaft_air() {
        for headings in scenes::facet_monument::FACET_HEADINGS_DEGREES.windows(2) {
            let turn = (headings[1] - headings[0]).abs();
            let interior = 180.0 - turn;
            assert!(
                (120.0..=150.0).contains(&interior),
                "junction must be an intentional obtuse facet: {interior}°"
            );
        }

        let mut app = test_app();
        switch_to(&mut app, Scene::FacetMonument);
        assert_eq!(count_named(&mut app, "Connected facet panel"), 8);
        assert_eq!(count::<FogVolume>(&mut app), 1);
        assert_eq!(count::<VolumetricLight>(&mut app), 2);
        assert_eq!(count::<VolumetricFog>(&mut app), 1);

        app.world_mut().resource_mut::<Toggles>().volumetrics = false;
        app.update();
        app.update();
        assert_eq!(count::<VolumetricFog>(&mut app), 0);
        app.world_mut().resource_mut::<Toggles>().volumetrics = true;
        app.update();
        app.update();
        assert_eq!(count::<VolumetricFog>(&mut app), 1);
    }

    #[test]
    fn wellshaft_practicals_form_separate_pools_and_levels_stay_outlined() {
        let mut app = test_app();
        switch_to(&mut app, Scene::Wellshaft);
        assert_eq!(
            count_named(&mut app, "Practical light"),
            scenes::wellshaft::LEVEL_COUNT
        );
        assert_eq!(
            count_named(&mut app, "Practical floor pool"),
            scenes::wellshaft::LEVEL_COUNT
        );
        assert_eq!(
            count_named(&mut app, "Stair and level silhouette lip"),
            scenes::wellshaft::LEVEL_COUNT * 4
        );
    }
}
