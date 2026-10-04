use super::*;
use observed_facility::hex_wfc::HexArchetype;
use observed_hex::HexFace;
use observed_match::hex_wfc::{HexBotDriver, HexMatchConfig, HexWfcMatch};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn the_whole_cistern_stays_lit_before_entry_and_across_sector_changes() {
    let mut game = HexWfcMatch::new(
        44,
        HexMatchConfig::default(),
        &crate::hex_wfc::sim::load_prototypes(),
    )
    .expect("fixture solves");
    let local_player = *game.players.keys().next().expect("player");
    let outside = game.players[&local_player].cell;
    let cells: Vec<_> = game
        .facility
        .placements
        .keys()
        .copied()
        .filter(|c| c.level == outside.level && *c != outside)
        .take(3)
        .collect();
    assert_eq!(cells.len(), 3);
    for cell in &cells {
        game.facility.placements.get_mut(cell).unwrap().archetype = HexArchetype::Cistern {
            heading: HexFace::East,
        };
    }
    let mut ascent =
        crate::hex_wfc::ascent::rules_for(&mut game, local_player, None).expect("ascent fixture");
    ascent.stage_power(outside.level, true);
    let mut app = App::new();
    app.insert_resource(HexWfcRuntime {
        match_state: game,
        bot_driver: HexBotDriver::new(),
        local_player,
        pending_visual_cells: BTreeSet::new(),
        presented_revisions: BTreeMap::new(),
        status: String::new(),
        map_open: false,
        map_level: outside.level,
        results_delay_frames: 0,
        networked: false,
        resync_attempts: 0,
        ascent: Some(ascent),
        viewed_player: None,
    })
    .insert_resource(Time::<()>::default())
    .insert_resource(super::super::camera::OverviewFrame::default())
    .insert_resource(GlobalAmbientLight::default())
    .insert_resource(ClearColor::default())
    .insert_resource(crate::hex_wfc::power::PowerAssets::for_lighting_test())
    .add_systems(
        Update,
        (
            sync_lighting_and_atmosphere,
            sync_practical_shadow_budget,
            crate::hex_wfc::power::sync_practicals,
        ),
    );
    let point = app
        .world_mut()
        .spawn((
            HexPractical(outside),
            GlobalTransform::default(),
            PointLight {
                intensity: 800_000.0,
                ..default()
            },
        ))
        .id();
    let key = app
        .world_mut()
        .spawn((
            HexWfcKeyLight,
            SpotLight {
                intensity: 1_000_000.0,
                ..default()
            },
            Transform::default(),
        ))
        .id();
    for cell in &cells {
        let parent = app.world_mut().spawn_empty().id();
        for x in [-3.0, 0.0, 3.0] {
            spawn_practical(
                &mut app.world_mut().commands(),
                parent,
                *cell,
                Vec3::from_array(hex_origin(*cell)) + Vec3::new(x, 7.25, 0.0),
                style::hex_practical_light(
                    ArchitectureRegister::ShadowScreen,
                    HexComposition::Room,
                    3,
                ),
                true,
            );
        }
    }
    app.world_mut().flush();
    let fixed: Vec<_> = {
        let world = app.world_mut();
        world
            .query_filtered::<(Entity, &Transform), (With<SpotLight>, Without<HexWfcKeyLight>)>()
            .iter(world)
            .map(|(e, t)| (e, *t))
            .collect()
    };
    assert_eq!(fixed.len(), 9);
    let fills: Vec<_> = {
        let world = app.world_mut();
        world
            .query_filtered::<(Entity, &Transform), (
                With<PointLight>,
                With<super::practicals::FixedReservoirLight>,
            )>()
            .iter(world)
            .map(|(e, t)| (e, *t))
            .collect()
    };
    assert_eq!(fills.len(), 9);
    for current in std::iter::once(outside).chain(cells) {
        app.world_mut()
            .resource_mut::<HexWfcRuntime>()
            .match_state
            .players
            .get_mut(&local_player)
            .unwrap()
            .cell = current;
        app.update();
        for (entity, pose) in &fixed {
            let light = app.world().get::<SpotLight>(*entity).unwrap();
            assert!(light.intensity > 0.0 && light.shadow_maps_enabled);
            assert_eq!(app.world().get::<Transform>(*entity).unwrap(), pose);
        }
        for (entity, pose) in &fills {
            let light = app.world().get::<PointLight>(*entity).unwrap();
            assert!(light.intensity > 0.0 && !light.shadow_maps_enabled);
            assert_eq!(app.world().get::<Transform>(*entity).unwrap(), pose);
        }
        if current != outside {
            assert_eq!(app.world().get::<SpotLight>(key).unwrap().intensity, 0.0);
            assert!(
                !app.world()
                    .get::<SpotLight>(key)
                    .unwrap()
                    .shadow_maps_enabled
            );
        }
    }
    for powered in [false, true, false, true] {
        app.world_mut()
            .resource_mut::<HexWfcRuntime>()
            .ascent
            .as_mut()
            .unwrap()
            .stage_power(outside.level, powered);
        app.update();
        let scale = if powered { 1.0 } else { 0.12 };
        for (entity, pose) in &fixed {
            assert_eq!(
                app.world().get::<SpotLight>(*entity).unwrap().intensity,
                observed_style::cistern::FIXTURE_INTENSITY * scale
            );
            assert_eq!(app.world().get::<Transform>(*entity).unwrap(), pose);
        }
        assert_eq!(
            app.world().get::<PointLight>(point).unwrap().intensity,
            800_000.0 * scale
        );
        for (entity, _) in &fills {
            assert_eq!(
                app.world().get::<PointLight>(*entity).unwrap().intensity,
                observed_style::cistern::FIXTURE_BOUNCE_INTENSITY * scale
            );
        }
    }
}
