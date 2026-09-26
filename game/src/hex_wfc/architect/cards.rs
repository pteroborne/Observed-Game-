//! The hand's art: each card is a miniature of the tile it will build.
//!
//! As `architect_lab` draws its hand: every card has its own camera, rendering into an
//! image the card shows, at the board's isometric pitch and under the board's key light.
//! A tile card shows the real authored tile - the same hulls, cut away the same way, that
//! the board's ghost and the built room will have - in its district's concrete, with its
//! doorways where the card puts them. The card picked up turns with the desk's rotation,
//! so the tile in hand is the tile that will be played. A door card shows a door frame.
//!
//! Each card's scene is built far from the facility and from the board, on a render
//! layer of its own, and rebuilt only when the card or its rotation changes.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::authored_hall;
use observed_hex::{HexFace, hex_origin};
use observed_match::ascent::sim::{Card, CardKind};
use observed_match::hex_wfc::project_hypothetical_cell;
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::building;
use super::pick;
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

/// Cards in a hand.
pub(super) const HAND: usize = 5;
/// The render layer of the first card; the others follow it.
pub(super) const FIRST_CARD_LAYER: usize = 5;
/// The card image, in pixels: twice what it is shown at, for crisp edges.
const IMAGE: UVec2 = UVec2::new(260, 176);
/// Metres per image pixel for a tile, and for the smaller door frame.
const TILE_SCALE: f32 = 0.092;
const DOOR_SCALE: f32 = 0.04;

/// Every card layer, for the light that lights them.
pub(super) fn layers() -> impl Iterator<Item = usize> {
    FIRST_CARD_LAYER..FIRST_CARD_LAYER + HAND
}

/// Where card `slot`'s miniature is built.
fn origin(slot: usize) -> Vec3 {
    #[allow(clippy::cast_precision_loss)]
    Vec3::new(-20_000.0 - slot as f32 * 100.0, 0.0, 0.0)
}

#[derive(Component)]
pub(super) struct CardCamera(usize);

#[derive(Component)]
pub(super) struct CardModel(usize);

/// The five card images, and what each last showed.
#[derive(Resource)]
pub(super) struct CardArt {
    pub images: [Handle<Image>; HAND],
    shown: [Option<(Card, u8, bool)>; HAND],
}

pub(super) fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    art: Option<Res<CardArt>>,
) {
    if art.is_some() {
        return;
    }
    let images = std::array::from_fn(|slot| {
        let image = images.add(Image::new_target_texture(
            IMAGE.x,
            IMAGE.y,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        let rotation = pick::rotation();
        #[allow(clippy::cast_possible_wrap)]
        let order = -(slot as isize) - 10;
        commands.spawn((
            CardCamera(slot),
            DespawnOnExit(GameState::HexWfc),
            Camera3d::default(),
            Camera {
                order,
                clear_color: color(Role::Card).into(),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scale: TILE_SCALE,
                ..OrthographicProjection::default_3d()
            }),
            AmbientLight {
                color: color(Role::Text),
                brightness: 360.0,
                ..default()
            },
            RenderLayers::layer(FIRST_CARD_LAYER + slot),
            Transform::from_translation(origin(slot) + Vec3::Y * 1.5 + rotation * Vec3::Z * 200.0)
                .with_rotation(rotation),
            Name::new(format!("Architect card {} camera", slot + 1)),
        ));
        image
    });
    commands.insert_resource(CardArt {
        images,
        shown: [None; HAND],
    });
}

/// Rebuild the miniature of every card that has changed, or turned.
pub(super) fn sync(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut art: ResMut<CardArt>,
    assets: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut cameras: Query<(&CardCamera, &mut Projection)>,
    models: Query<(Entity, &CardModel)>,
) {
    let (mut meshes, mut materials) = assets;
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let Some(hand) = ascent.session().hands.get(&desk.team) else {
        return;
    };
    for slot in 0..HAND {
        // A card in hand shows its tile as dealt; the one picked up, as it would be played.
        let lifted = desk.selected == Some(slot);
        let wanted = hand.deck.hand.get(slot).map(|&card| {
            let rotation = if lifted { desk.rotation } else { 0 };
            (card, rotation, lifted)
        });
        if art.shown[slot] == wanted {
            continue;
        }
        art.shown[slot] = wanted;
        for (entity, model) in &models {
            if model.0 == slot {
                commands.entity(entity).despawn();
            }
        }
        let Some((card, rotation, _)) = wanted else {
            continue;
        };
        for (camera, mut projection) in &mut cameras {
            if camera.0 == slot
                && let Projection::Orthographic(ortho) = &mut *projection
            {
                ortho.scale = match card.kind {
                    CardKind::Tile(_) => TILE_SCALE,
                    CardKind::Door => DOOR_SCALE,
                };
            }
        }
        let layer = RenderLayers::layer(FIRST_CARD_LAYER + slot);
        let at = origin(slot);
        let mut spawn = |mesh: Mesh, material: Handle<StandardMaterial>, transform: Transform| {
            commands.spawn((
                CardModel(slot),
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                transform,
                layer.clone(),
            ));
        };
        match card.kind {
            CardKind::Tile(shape) => {
                let register = card
                    .district
                    .map_or(ArchitectureRegister::ALL[0], |district| district.register());
                let physical = &runtime.match_state;
                // The tile as the corpus builds it in this district: projected at a cell
                // of the district, and moved from there to the card.
                let Some(cell) = physical
                    .facility
                    .architecture
                    .iter()
                    .find(|(_, found)| **found == register)
                    .map(|(&cell, _)| cell)
                else {
                    continue;
                };
                let Some(placement) = authored_hall(cell, shape.doors(rotation)) else {
                    continue;
                };
                let Ok(pieces) = project_hypothetical_cell(
                    &physical.facility,
                    cell,
                    placement,
                    physical.content().cells(),
                ) else {
                    continue;
                };
                let moved = Transform::from_translation(at - Vec3::from_array(hex_origin(cell)));
                for floor in [true, false] {
                    if let Some(mesh) = building::cutaway_mesh(&pieces, floor, building::bearing())
                    {
                        let material = materials.add(building::tile_material(register, floor));
                        spawn(mesh, material, moved);
                    }
                }
                // At this size the doorways read by their thresholds, not by gaps in cut
                // walls: a bar on each, amber on the card in hand.
                let threshold = materials.add(StandardMaterial {
                    base_color: color(if lifted {
                        Role::Selected
                    } else {
                        Role::Observer
                    }),
                    unlit: true,
                    ..default()
                });
                for face in HexFace::LATERAL
                    .into_iter()
                    .filter(|face| shape.doors(rotation) & (1 << face.index()) != 0)
                {
                    let mut bar = threshold_bar(face);
                    bar.translation += at;
                    spawn(Cuboid::new(1.0, 1.0, 1.0).into(), threshold.clone(), bar);
                }
            }
            CardKind::Door => {
                let material = materials.add(StandardMaterial {
                    base_color: color(Role::Fixture),
                    unlit: true,
                    ..default()
                });
                for part in door_frame(HexFace::LATERAL[usize::from(rotation % 6)]) {
                    spawn(
                        Cuboid::new(1.0, 1.0, 1.0).into(),
                        material.clone(),
                        Transform {
                            translation: at + part.translation,
                            ..part
                        },
                    );
                }
            }
        }
    }
}

/// A bar lying on the floor across the doorway on `face`, at the edge of the tile.
fn threshold_bar(face: HexFace) -> Transform {
    let angle = pick::face_angle(face);
    let outward = Vec3::new(angle.cos(), 0.0, angle.sin());
    Transform::from_translation(outward * 6.4 + Vec3::Y * (pick::deck(0) + 0.1))
        .with_rotation(Quat::from_rotation_y(-angle))
        .with_scale(Vec3::new(1.2, 0.25, 4.2))
}

/// A door frame standing across the doorway on `face`: two posts and a lintel.
fn door_frame(face: HexFace) -> [Transform; 3] {
    let angle = pick::face_angle(face);
    let across = Quat::from_rotation_y(-angle);
    let side = across * Vec3::Z * 1.4;
    let post = |at: Vec3| {
        Transform::from_translation(at + Vec3::Y * 1.3)
            .with_rotation(across)
            .with_scale(Vec3::new(0.35, 2.6, 0.35))
    };
    [
        post(side),
        post(-side),
        Transform::from_translation(Vec3::Y * 2.75)
            .with_rotation(across)
            .with_scale(Vec3::new(0.35, 0.35, 3.15)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_card_has_a_layer_of_its_own_clear_of_the_board_and_the_climb() {
        let layers: Vec<usize> = layers().collect();
        assert_eq!(layers.len(), HAND);
        assert!(!layers.contains(&super::super::board::BOARD_LAYER));
        assert!(!layers.contains(&super::super::stack::STACK_LAYER));
        // The world, the portal preview and the survivor map.
        assert!(layers.iter().all(|&layer| layer > 2));
    }

    #[test]
    fn the_door_frame_stands_across_its_doorway() {
        for face in HexFace::LATERAL {
            let [left, right, _] = door_frame(face);
            let angle = pick::face_angle(face);
            let outward = Vec3::new(angle.cos(), 0.0, angle.sin());
            let span = left.translation - right.translation;
            assert!(span.dot(outward).abs() < 1e-4, "{face:?}");
            assert!(span.length() > 2.0, "{face:?}");
        }
    }
}
