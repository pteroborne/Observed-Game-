use super::*;
use observed_match::hex_wfc::{HexBotDriver, HexMatchConfig, HexWfcMatch};
use std::collections::{BTreeMap, BTreeSet};
#[test]
fn panels_follow_power_with_cached_materials_and_despawn_with_their_cell() {
    let mut game = HexWfcMatch::new(
        44,
        HexMatchConfig::default(),
        &crate::hex_wfc::sim::load_prototypes(),
    )
    .unwrap();
    let local_player = *game.players.keys().next().unwrap();
    let coord = game.players[&local_player].cell;
    let mut ascent = crate::hex_wfc::ascent::rules_for(&mut game, local_player, None).unwrap();
    ascent.stage_power(coord.level, true);
    let mut app = App::new();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut images = Assets::<Image>::default();
    let mut assets = HexWfcVisualAssets::for_jade_test(&mut materials, &mut images);
    let on = assets.jade_material(4);
    let off = assets.jade_material(6);
    assert_ne!(on, off);
    assert!(materials.get(&on).unwrap().emissive.red > 0.0);
    assert_eq!(materials.get(&off).unwrap().emissive, LinearRgba::BLACK);
    let mut meshes = Assets::<Mesh>::default();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let parent = app.world_mut().spawn_empty().id();
    let count = spawn(
        &mut Commands::new(&mut queue, app.world()),
        &mut assets,
        &mut meshes,
        parent,
        coord,
        HexFace::East,
        &[],
    );
    queue.apply(app.world_mut());
    assert_eq!(count, 6);
    let panel = app
        .world_mut()
        .query_filtered::<Entity, With<JadePanel>>()
        .single(app.world())
        .unwrap();
    assert_eq!(app.world().get::<ChildOf>(panel).unwrap().parent(), parent);
    assert_eq!(
        app.world().get::<Cutaway>(panel).unwrap().cell_level,
        coord.level
    );
    app.insert_resource(assets)
        .insert_resource(HexWfcRuntime {
            match_state: game,
            bot_driver: HexBotDriver::new(),
            local_player,
            pending_visual_cells: BTreeSet::new(),
            presented_revisions: BTreeMap::new(),
            status: String::new(),
            map_open: false,
            map_level: coord.level,
            results_delay_frames: 0,
            networked: false,
            resync_attempts: 0,
            ascent: Some(ascent),
            viewed_player: None,
        })
        .add_systems(Update, sync_power);
    for powered in [false, true, false, true] {
        app.world_mut()
            .resource_mut::<HexWfcRuntime>()
            .ascent
            .as_mut()
            .unwrap()
            .stage_power(coord.level, powered);
        app.update();
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(panel)
                .unwrap()
                .0,
            if powered { on.clone() } else { off.clone() }
        );
    }
    let cached_meshes = meshes.len();
    app.world_mut().entity_mut(parent).despawn();
    assert!(app.world().get_entity(panel).is_err());
    let parent = app.world_mut().spawn_empty().id();
    let mut assets = app
        .world_mut()
        .remove_resource::<HexWfcVisualAssets>()
        .unwrap();
    spawn(
        &mut Commands::new(&mut queue, app.world()),
        &mut assets,
        &mut meshes,
        parent,
        coord,
        HexFace::East,
        &[],
    );
    queue.apply(app.world_mut());
    assert_eq!(
        meshes.len(),
        cached_meshes,
        "rewritten cells reuse identical projected meshes"
    );
}
