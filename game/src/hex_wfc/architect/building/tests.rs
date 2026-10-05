use super::*;
use observed_match::ascent::sim::{CardKind, District};
#[test]
fn wonder_thumbnails_find_the_whole_room_even_when_facing_off_the_first_edge() {
    let game = observed_match::hex_wfc::HexWfcMatch::new(
        7,
        observed_match::hex_wfc::HexMatchConfig {
            wfc: observed_facility::hex_wfc::HexWfcConfig {
                levels: 8,
                ..default()
            },
            ..default()
        },
        &crate::hex_wfc::sim::load_prototypes(),
    )
    .expect("preview fixture");
    for (kind, district) in [
        (CardKind::ArchiveWell, District::LIBRARY),
        (CardKind::RainCourt, District::ZEN),
        (CardKind::SwitchingConcourse, District::LUMEN),
        (CardKind::JadeNave, District::MONUMENT),
        (CardKind::LastPromenade, District::SKY),
    ] {
        let register = district.register();
        let first = *game
            .facility
            .architecture
            .iter()
            .find(|(_, r)| **r == register)
            .unwrap()
            .0;
        assert!(
            (0..6).any(|r| built_by(&game, kind, first, r).is_none()),
            "fixture exercises the border failure"
        );
        for rotation in 0..6 {
            let (cell, pieces) =
                preview_by(&game, kind, register, rotation).expect("complete thumbnail");
            let expected = match kind {
                CardKind::LastPromenade => observed_facility::hex_wfc::authored_last_promenade(
                    game.facility.config,
                    cell,
                    rotation,
                ),
                CardKind::JadeNave => observed_facility::hex_wfc::authored_jade_nave(
                    game.facility.config,
                    cell,
                    rotation,
                ),
                CardKind::SwitchingConcourse => {
                    observed_facility::hex_wfc::authored_switching_concourse(
                        game.facility.config,
                        cell,
                        rotation,
                    )
                }
                CardKind::RainCourt => observed_facility::hex_wfc::authored_rain_court(
                    game.facility.config,
                    cell,
                    rotation,
                ),
                _ => observed_facility::hex_wfc::authored_archive_well(
                    game.facility.config,
                    cell,
                    rotation,
                ),
            }
            .unwrap();
            let actual: BTreeSet<_> = pieces.iter().map(|p| p.source_cell).collect();
            assert_eq!(actual, expected.map(|p| p.coord).into_iter().collect());
            assert!(cutaway_mesh(&pieces, true, bearing()).is_some());
            assert!(cutaway_mesh(&pieces, false, bearing()).is_some());
            if kind == CardKind::LastPromenade {
                let base = f32::from(cell.level) * TILE_LEVEL_HEIGHT;
                let bridge_pieces: Vec<_> = pieces
                    .iter()
                    .filter(|p| {
                        let points = world_points(p);
                        let min = points
                            .iter()
                            .map(|p| p.y - base)
                            .fold(f32::INFINITY, f32::min);
                        let max = points
                            .iter()
                            .map(|p| p.y - base)
                            .fold(f32::NEG_INFINITY, f32::max);
                        min >= 1.9 && max <= 2.51
                    })
                    .cloned()
                    .collect();
                for piece in &bridge_pieces {
                    assert!(
                        cutaway_mesh(std::slice::from_ref(piece), false, bearing()).is_some(),
                        "every raised walking hull survives individually"
                    );
                }
                let mesh = cutaway_mesh(&bridge_pieces, false, bearing())
                    .expect("thin elevated bridges retained");
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("bridge positions")
                };
                assert!(
                    positions.iter().any(|p| (p[1] - base - 2.5).abs() < 0.01),
                    "preview preserves bridge height"
                );
            }
            if kind == CardKind::JadeNave {
                let base = f32::from(cell.level) * TILE_LEVEL_HEIGHT;
                let upper_and_roof: Vec<_> = pieces
                    .iter()
                    .filter(|piece| {
                        let points = world_points(piece);
                        let min = points
                            .iter()
                            .map(|p| p.y - base)
                            .fold(f32::INFINITY, f32::min);
                        let max = points
                            .iter()
                            .map(|p| p.y - base)
                            .fold(f32::NEG_INFINITY, f32::max);
                        (min >= 2.7 && max <= 3.1) || min >= 7.7
                    })
                    .cloned()
                    .collect();
                let mesh = cutaway_mesh(&upper_and_roof, false, bearing())
                    .expect("raised crossing survives the cutaway");
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("preview positions");
                };
                assert!(
                    positions.iter().any(|p| (p[1] - base - 3.0).abs() < 0.01),
                    "actual bridge elevation is retained"
                );
                assert!(
                    positions.iter().all(|p| p[1] - base <= 3.1),
                    "roof is removed from the preview"
                );
            }
        }
    }
}
