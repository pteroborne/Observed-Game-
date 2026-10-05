//! The raised Sky paths must work for autonomous teammates, not only waypoints.
use super::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, authored_last_promenade};
use observed_hex::{HexCoord, hex_origin};

fn turn(mut point: Vec3, rotation: usize) -> Vec3 {
    for _ in 0..rotation {
        point = Vec3::new(
            0.5 * point.x - 0.875 * point.z,
            point.y,
            6.0 / 7.0 * point.x + 0.5 * point.z,
        );
    }
    point
}

#[test]
fn promenade_bot_follows_raised_loop_and_entry_at_every_heading() {
    let player = PlayerId(0);
    let anchor = HexCoord {
        q: 4,
        r: 4,
        level: 7,
    };
    for rotation in 0..6 {
        let mut game = HexWfcMatch::new_with_content(
            44,
            HexMatchConfig {
                guardian: false,
                teams: 1,
                members_per_team: 1,
                wfc: showcase_config(8),
            },
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .expect("Sky fixture");
        game.hand_mutation_to_architects();
        // Isolate the authored crossing: no surrounding floor can bypass a gap.
        game.facility.blueprints.clear();
        for placement in game.facility.placements.values_mut() {
            placement.space = HexSpace::Void;
            placement.archetype = HexArchetype::Void;
            placement.doors = 0;
            placement.up = PortClass::Sealed;
            placement.down = PortClass::Sealed;
        }
        let sectors = authored_last_promenade(game.facility.config, anchor, rotation).unwrap();
        for placement in sectors {
            game.facility.placements.insert(placement.coord, placement);
            game.facility
                .architecture
                .insert(placement.coord, ArchitectureRegister::Thinning);
        }
        game.geometry = HexWfcGeometrySnapshot::project_with_rooms(
            &game.facility,
            game.content.cells(),
            game.content.rooms(),
        )
        .expect("authored Sky geometry and guides");
        game.physics = RapierTraversalScene::from_arena_spec(&game.geometry.arena);
        let half_height = game.content.traversal_config().half_height;
        let centers = sectors.map(|sector| {
            Vec3::from_array(hex_origin(sector.coord)) + Vec3::Y * (2.5 + half_height)
        });
        let entry = Vec3::from_array(hex_origin(anchor))
            + turn(
                Vec3::new(-3.2, 0.5 + half_height, -5.4),
                usize::from(rotation),
            );
        let body = game.players.get_mut(&player).unwrap();
        body.cell = anchor;
        body.position = entry;
        game.sync_teleports_to_bodies();
        let mut driver = HexBotDriver::new();
        // Enter via the ramp, make both circuits, then leave via the same ramp.
        for (index, target) in [1, 2, 0, 2, 1, 0, 0].into_iter().enumerate() {
            let destination = if index == 6 { entry } else { centers[target] };
            let mut reached = false;
            for _ in 0..900 {
                if game.players[&player].position.distance(destination) < 0.8 {
                    reached = true;
                    break;
                }
                let command = driver.command_to(&game, player, sectors[target].coord, destination);
                game.step(&HexInputFrame {
                    version: HEX_INPUT_VERSION,
                    tick: game.tick + 1,
                    commands: BTreeMap::from([(player, command)]),
                });
                assert!(
                    game.players[&player].in_facility(),
                    "rotation {rotation}: bot fell"
                );
            }
            assert!(
                reached,
                "rotation {rotation}, leg {index}: bot at {:?}, wanted {destination:?}",
                game.players[&player].position
            );
        }
    }
}
