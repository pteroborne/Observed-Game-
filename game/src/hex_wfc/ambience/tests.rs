use super::*;

#[test]
fn physical_soundscape_bounds_voices_crossfades_and_mutes_loading_audio() {
    let runtime = crate::hex_wfc::view::tests::test_runtime();
    let at = runtime.local().position;
    let cell = runtime.local().cell;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(runtime)
        .insert_resource(Settings::default())
        .insert_resource(Soundscape::default());
    app.insert_resource(Sounds {
        air: Handle::default(),
        dry: Handle::default(),
        wind: Handle::default(),
        buzz: Handle::default(),
        machine: Handle::default(),
        water: Handle::default(),
        creak: Handle::default(),
    });
    app.add_systems(Update, sync);
    for index in 0..8 {
        app.world_mut().spawn((
            HexPractical(cell),
            PointLight::default(),
            GlobalTransform::from_translation(at + Vec3::X * index as f32),
        ));
    }
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&Emitter>()
            .iter(app.world())
            .count(),
        4
    );
    for register in [
        ArchitectureRegister::InfiniteGallery,
        ArchitectureRegister::Thinning,
        ArchitectureRegister::LiminalGrid,
    ] {
        app.world_mut()
            .resource_mut::<HexWfcRuntime>()
            .match_state
            .facility
            .architecture
            .insert(cell, register);
        app.update();
        assert!(app.world_mut().query::<&Bed>().iter(app.world()).count() <= 2);
    }
    app.world_mut().resource_mut::<Settings>().music_volume = 0.0;
    app.update();
    for playback in app
        .world_mut()
        .query_filtered::<&PlaybackSettings, Or<(With<Bed>, With<Emitter>)>>()
        .iter(app.world())
    {
        assert_eq!(playback.volume, Volume::Linear(0.0));
    }
}
