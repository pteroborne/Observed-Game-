use super::*;
use observed_match::hex_wfc::HexPiecePart;
use observed_traversal::StableColliderId;

#[test]
fn explicit_low_ceiling_does_not_reclassify_an_untagged_deck() {
    let hull = vec![
        Vec3::new(-2.0, 3.5, -2.0),
        Vec3::new(2.0, 3.5, 2.0),
        Vec3::new(-2.0, 3.7, 2.0),
        Vec3::new(2.0, 3.7, -2.0),
    ];
    let mut panel = piece(hull);
    assert_eq!(horizontal_surface(&panel), HorizontalSurface::Floor);
    panel.surface = Some(observed_authoring::HullSurface::Ceiling);
    assert_eq!(horizontal_surface(&panel), HorizontalSurface::Ceiling);
    assert_eq!(MeshGroupKey::for_piece(&panel), MeshGroupKey::Ceiling);
    panel.surface = Some(observed_authoring::HullSurface::Trim);
    assert_eq!(MeshGroupKey::for_piece(&panel), MeshGroupKey::Trim);
}

fn piece(points: Vec<Vec3>) -> HexStructurePiece {
    HexStructurePiece {
        surface: None,
        id: StableColliderId(1),
        anchor: default(),
        source_cell: default(),
        role: HexStructureRole::Hall,
        part: HexPiecePart::Authored,
        tile: None,
        center: Vec3::ZERO,
        rotation: [0.0, 0.0, 0.0, 1.0],
        shape: ColliderShape::ConvexHull { points },
    }
}

#[test]
fn horizontal_hulls_select_floor_wall_and_ceiling_material_classes() {
    assert_eq!(
        horizontal_surface(&piece(vec![Vec3::ZERO, Vec3::Y * 0.5])),
        HorizontalSurface::Floor
    );
    assert_eq!(
        horizontal_surface(&piece(vec![Vec3::ZERO, Vec3::Y * 4.0])),
        HorizontalSurface::Wall
    );
    assert_eq!(
        horizontal_surface(&piece(vec![
            Vec3::Y * 7.5,
            Vec3::Y * observed_hex::TILE_LEVEL_HEIGHT,
        ])),
        HorizontalSurface::Ceiling
    );
}

/// A balcony is a floor. It had been a wall, because it is three metres up
/// and the classifier only asked how high the hull was.
#[test]
fn a_thin_slab_off_the_ground_is_a_deck_rather_than_a_wall() {
    assert_eq!(
        horizontal_surface(&piece(vec![
            Vec3::new(-2.0, 3.0, -1.5),
            Vec3::new(2.0, 3.4, 1.5),
        ])),
        HorizontalSurface::Floor
    );
}

/// And a pier is still a wall, at any height. Thickness alone would call a
/// short post a deck, so the test is the ratio rather than the thickness.
#[test]
fn a_stub_at_the_same_height_is_still_a_wall() {
    assert_eq!(
        horizontal_surface(&piece(vec![
            Vec3::new(-0.3, 1.0, -0.3),
            Vec3::new(0.3, 4.0, 0.3),
        ])),
        HorizontalSurface::Wall
    );
}

#[test]
fn same_tile_with_changed_local_hulls_cannot_reuse_a_stale_mesh() {
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut meshes = Assets::<Mesh>::default();
    let hull = |width: f32| {
        [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0].into_iter().flat_map(move |y| {
                    [-1.0, 1.0]
                        .into_iter()
                        .map(move |z| Vec3::new(x * width, y, z))
                })
            })
            .collect::<Vec<_>>()
    };
    let before = hull(1.0);
    let after = hull(2.0);
    let old = assets
        .merged_mesh_for(
            &mut meshes,
            Some("tile@cell"),
            MeshGroupKey::Interior,
            &[&before],
        )
        .unwrap();
    let new = assets
        .merged_mesh_for(
            &mut meshes,
            Some("tile@cell"),
            MeshGroupKey::Interior,
            &[&after],
        )
        .unwrap();
    let restored = assets
        .merged_mesh_for(
            &mut meshes,
            Some("tile@cell"),
            MeshGroupKey::Interior,
            &[&before],
        )
        .unwrap();
    assert_ne!(old, new);
    assert_eq!(old, restored);
    assert_eq!(meshes.len(), 2);
}

#[test]
fn identical_open_edge_geometry_shares_meshes_across_cell_origins() {
    let hull = [-1.0, 1.0]
        .into_iter()
        .flat_map(|x| {
            [-1.0, 1.0]
                .into_iter()
                .flat_map(move |y| [-1.0, 1.0].into_iter().map(move |z| Vec3::new(x, y, z)))
        })
        .collect::<Vec<_>>();
    let mut first = piece(hull.clone());
    first.part = HexPiecePart::Lip;
    first.tile = Some(observed_authoring::TileKey {
        register: "megastructure".into(),
        archetype: "hall_turn_60".into(),
        variant: 3,
    });
    first.source_cell = observed_hex::HexCoord {
        q: 1,
        r: 1,
        level: 0,
    };
    first.center = Vec3::from_array(observed_hex::hex_origin(first.source_cell));
    let mut second = first.clone();
    second.source_cell.q = 5;
    second.center = Vec3::from_array(observed_hex::hex_origin(second.source_cell));
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut meshes = Assets::<Mesh>::default();
    let key1 = super::super::shell::cell_mesh_key(&[&first]).unwrap();
    let key2 = super::super::shell::cell_mesh_key(&[&second]).unwrap();
    let a = assets
        .merged_mesh_for(&mut meshes, Some(&key1), MeshGroupKey::Lip, &[&hull])
        .unwrap();
    let b = assets
        .merged_mesh_for(&mut meshes, Some(&key2), MeshGroupKey::Lip, &[&hull])
        .unwrap();
    assert_eq!(
        a, b,
        "the same local structure must not be rebuilt for another cell"
    );
    assert_eq!(meshes.len(), 1);
}

#[test]
fn changed_slab_ownership_cannot_reuse_a_stale_wall_mesh() {
    let cube = |offset: Vec3| {
        [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0].into_iter().flat_map(move |y| {
                    [-1.0, 1.0]
                        .into_iter()
                        .map(move |z| offset + Vec3::new(x, y, z))
                })
            })
            .collect::<Vec<_>>()
    };
    let target = cube(Vec3::ZERO);
    let before = cube(Vec3::X);
    let after = cube(Vec3::X * 1.5);
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut meshes = Assets::<Mesh>::default();
    let first = assets
        .merged_mesh_for_owned(
            &mut meshes,
            Some("wall"),
            MeshGroupKey::Interior,
            &[&target],
            &[&before],
        )
        .unwrap();
    let changed = assets
        .merged_mesh_for_owned(
            &mut meshes,
            Some("wall"),
            MeshGroupKey::Interior,
            &[&target],
            &[&after],
        )
        .unwrap();
    let restored = assets
        .merged_mesh_for_owned(
            &mut meshes,
            Some("wall"),
            MeshGroupKey::Interior,
            &[&target],
            &[&before],
        )
        .unwrap();
    assert_ne!(first, changed);
    assert_eq!(first, restored);
}

#[test]
fn signed_zero_coordinates_share_the_same_geometry_recipe() {
    let original = vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z];
    let reconstructed = original
        .iter()
        .map(|p| {
            Vec3::from_array(
                p.to_array()
                    .map(|value| if value == 0.0 { -0.0 } else { value }),
            )
        })
        .collect::<Vec<_>>();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut meshes = Assets::<Mesh>::default();
    let a = assets
        .merged_mesh_for(
            &mut meshes,
            Some("tetra"),
            MeshGroupKey::Interior,
            &[&original],
        )
        .unwrap();
    let b = assets
        .merged_mesh_for(
            &mut meshes,
            Some("tetra"),
            MeshGroupKey::Interior,
            &[&reconstructed],
        )
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(meshes.len(), 1);
}
