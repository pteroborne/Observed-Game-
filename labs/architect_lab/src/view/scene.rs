//! Cached architectural models projected into the board and five offscreen cards.
use super::{
    MapCameraState, board_position, camera_rotation,
    models::{ModelAssets, Models, Part},
};
use crate::{
    LabSession,
    sim::{CardKind, DoorState, ObserverState},
};
use bevy::camera::{RenderTarget, visibility::RenderLayers};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexSpace;
use observed_hex::{HexFace, PortClass};
use observed_style::architect::{Role, color};

#[derive(Component)]
pub(crate) struct BoardVisual;
#[derive(Component)]
pub(crate) struct PreviewVisual;
#[derive(Component)]
pub(crate) struct PreviewCamera(usize);
fn preview_origin(index: usize) -> Vec3 {
    Vec3::new(1000.0 + index as f32 * 40.0, 0.0, 0.0)
}
#[derive(Resource)]
pub(crate) struct Previews(pub [Handle<Image>; 5]);

pub(super) fn setup_previews(commands: &mut Commands, images: &mut Assets<Image>) -> Previews {
    Previews(std::array::from_fn(|index| {
        let image = images.add(Image::new_target_texture(
            384,
            240,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        commands.spawn((
            PreviewCamera(index),
            Camera3d::default(),
            Camera {
                order: -(index as isize) - 1,
                clear_color: color(Role::Card).into(),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            RenderLayers::layer(index + 2),
            Projection::Orthographic(OrthographicProjection {
                scale: 0.055,
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_translation(
                preview_origin(index) + Vec3::Y * 1.5 + camera_rotation() * Vec3::Z * 40.0,
            )
            .with_rotation(camera_rotation()),
            Name::new(format!("Card {} model camera", index + 1)),
        ));
        image
    }))
}

pub fn sync_previews(
    mut commands: Commands,
    session: Res<LabSession>,
    mut previous: Local<Vec<(CardKind, Option<crate::sim::District>, u8)>>,
    mut cameras: Query<(&PreviewCamera, &mut Projection)>,
    existing: Query<Entity, With<PreviewVisual>>,
    assets: ModelAssets,
) {
    let ModelAssets {
        mut models,
        mut meshes,
        mut materials,
    } = assets;
    let keys: Vec<_> = session
        .sim
        .deck
        .hand
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (
                c.kind,
                c.district,
                if i == session.selected_card {
                    session.rotation
                } else {
                    0
                },
            )
        })
        .collect();
    if *previous == keys {
        return;
    }
    *previous = keys.clone();
    for e in &existing {
        commands.entity(e).despawn();
    }
    for (camera, mut projection) in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = if keys.get(camera.0).is_some_and(|k| k.0 == CardKind::Door) {
                0.026
            } else {
                0.055
            };
        }
    }
    for (i, (kind, district, rotation)) in keys.into_iter().enumerate() {
        let parts = match kind {
            CardKind::Tile(shape) => models.room(
                district.map_or(ArchitectureRegister::Institutional, |d| d.register()),
                shape.doors(rotation),
                &mut meshes,
                &mut materials,
            ),
            CardKind::Door => door_parts(&models, models.ghost.clone(), rotation),
        };
        for mut part in parts {
            part.transform.translation += preview_origin(i);
            commands.spawn((
                PreviewVisual,
                Mesh3d(part.mesh),
                MeshMaterial3d(part.material),
                part.transform,
                RenderLayers::layer(i + 2),
            ));
        }
    }
}

pub fn rebuild_board(
    mut commands: Commands,
    mut session: ResMut<LabSession>,
    state: Res<MapCameraState>,
    mut last_view: Local<Option<(u8, bool)>>,
    existing: Query<Entity, With<BoardVisual>>,
    assets: ModelAssets,
) {
    let ModelAssets {
        mut models,
        mut meshes,
        mut materials,
    } = assets;
    let view = (state.floor, state.overview);
    if !session.dirty && *last_view == Some(view) {
        return;
    }
    *last_view = Some(view);
    session.dirty = false;
    for e in &existing {
        commands.entity(e).despawn();
    }
    let config = session.sim.world.config;
    let targets = session.sim.mutable_targets();
    let selected = session.target().filter(|c| c.level == state.floor);
    for (&cell, p) in &session.sim.world.placements {
        let active = cell.level == state.floor;
        let depth = i16::from(state.floor) - i16::from(cell.level);
        if !active && (!state.overview || !(1..=2).contains(&depth)) {
            continue;
        }
        let mut at = board_position(config, cell);
        if !active {
            at += Vec3::new(-4.0, -12.0, -4.0) * f32::from(depth);
        }
        if p.space == HexSpace::Void {
            if active && targets.contains(&cell) {
                ring(
                    &mut commands,
                    &mut models,
                    &mut materials,
                    at,
                    Role::Border,
                    0.12,
                    "Missing tile / rebuild",
                );
            }
            continue;
        }
        if active {
            let register = session
                .sim
                .world
                .architecture
                .get(&cell)
                .copied()
                .unwrap_or_else(|| crate::sim::floor_register(cell.level));
            let parts = models.room(register, p.doors, &mut meshes, &mut materials);
            for part in parts {
                spawn_part(&mut commands, part, at, "Authored deck");
            }
            if session.sim.prison_core.contains(&cell) {
                ring(
                    &mut commands,
                    &mut models,
                    &mut materials,
                    at,
                    Role::Prison,
                    0.18,
                    "Protected prison",
                );
            }
            if session.sim.observed.contains(&cell) {
                ring(
                    &mut commands,
                    &mut models,
                    &mut materials,
                    at,
                    Role::Observer,
                    0.13,
                    "Watched / protected",
                );
            }
            if session.sim.contradictions.contains(&cell) {
                let material = models.signal(Role::Guardian, &mut materials);
                for angle in [-0.7, 0.7] {
                    spawn_part(
                        &mut commands,
                        Part {
                            mesh: models.cube.clone(),
                            material: material.clone(),
                            transform: Transform::from_xyz(0.0, 0.8, 0.0)
                                .with_rotation(Quat::from_rotation_y(angle))
                                .with_scale(Vec3::new(2.7, 0.12, 0.2)),
                        },
                        at,
                        "Unmatched ports / unstable",
                    );
                }
            }
            if p.up != PortClass::Sealed || p.down != PortClass::Sealed {
                // Explicit vertical port glyph, not a fabricated walkable staircase.
                let mat = models.signal(Role::Fixture, &mut materials);
                let direction = if p.up != PortClass::Sealed { 1.0 } else { -1.0 };
                for (x, z) in [(-0.45, 0.0), (0.45, 0.0)] {
                    spawn_part(
                        &mut commands,
                        Part {
                            mesh: models.cube.clone(),
                            material: mat.clone(),
                            transform: Transform::from_xyz(x, 0.85, z)
                                .with_rotation(Quat::from_rotation_y(direction * x.signum() * 0.7))
                                .with_scale(Vec3::new(0.14, 0.15, 1.5)),
                        },
                        at,
                        "Vertical connection",
                    );
                }
            }
        } else {
            let material = models.signal(Role::Context, &mut materials);
            spawn_part(
                &mut commands,
                Part {
                    mesh: models.slab.clone(),
                    material,
                    transform: Transform::IDENTITY,
                },
                at,
                "Context deck",
            );
        }
    }
    if let Some(cell) = session.hovered_target.filter(|c| c.level == state.floor) {
        ring(
            &mut commands,
            &mut models,
            &mut materials,
            board_position(config, cell),
            Role::Text,
            0.15,
            "Pointer target",
        );
    }
    if let Some(cell) = selected {
        let at = board_position(config, cell);
        ring(
            &mut commands,
            &mut models,
            &mut materials,
            at,
            Role::Selected,
            0.28,
            "Selected target",
        );
        if let Some(card) = session.sim.deck.hand.get(session.selected_card) {
            let parts = match card.kind {
                CardKind::Tile(shape) => models.room(
                    card.district
                        .map_or(ArchitectureRegister::Institutional, |d| d.register()),
                    shape.doors(session.rotation),
                    &mut meshes,
                    &mut materials,
                ),
                CardKind::Door => door_parts(&models, models.ghost.clone(), session.rotation),
            };
            for mut part in parts {
                part.material = models.ghost.clone();
                spawn_part(
                    &mut commands,
                    part,
                    at + Vec3::Y * 0.4,
                    "Proposed card / ghost",
                );
            }
        }
    }
    if let Some((cell, _)) = session
        .sim
        .condemned
        .filter(|(c, _)| c.level == state.floor)
    {
        ring(
            &mut commands,
            &mut models,
            &mut materials,
            board_position(config, cell) + Vec3::Y * 0.25,
            Role::Guardian,
            0.48,
            "Condemned tile",
        );
    }
    for (&key, &door) in &session.sim.doors {
        if key.cell.level != state.floor {
            continue;
        }
        let [a, b] = observed_hex::face_edge(key.face);
        let at = board_position(config, key.cell)
            + Vec3::new((a.0 + b.0) as f32 * 0.5, 0.5, (a.1 + b.1) as f32 * 0.5);
        let material = models.signal(
            if door == DoorState::Open {
                Role::Valid
            } else {
                Role::Guardian
            },
            &mut materials,
        );
        for part in door_parts(&models, material, key.face.index() as u8) {
            spawn_part(&mut commands, part, at, "Deployed door");
        }
    }
    // Native debug view can reveal all actors, but ordinary play uses Rogue knowledge.
    let known = session.sim.rogue_knowledge();
    for observer in session.sim.observers.values() {
        let cell = if session.debug_overlay {
            Some(observer.cell)
        } else {
            known.known_observers.get(&observer.id).copied()
        };
        let Some(cell) = cell.filter(|c| c.level == state.floor) else {
            continue;
        };
        if observer.state == ObserverState::Corrupted {
            continue;
        }
        let at = board_position(config, cell)
            + Vec3::new(f32::from(observer.id.0) * 1.5 - 0.75, 1.8, 0.0);
        let mat = models.signal(Role::Observer, &mut materials);
        spawn_part(
            &mut commands,
            Part {
                mesh: models.sphere.clone(),
                material: mat,
                transform: Transform::IDENTITY,
            },
            at,
            "Observer eye / last sighting",
        );
        let mat = models.signal(Role::Background, &mut materials);
        spawn_part(
            &mut commands,
            Part {
                mesh: models.sphere.clone(),
                material: mat,
                transform: Transform::from_translation(camera_rotation() * Vec3::Z * 0.75)
                    .with_scale(Vec3::splat(0.4)),
            },
            at,
            "Observer pupil",
        );
    }
    for guardian in session
        .sim
        .guardians
        .values()
        .filter(|g| g.cell.level == state.floor)
    {
        let mat = models.signal(Role::Guardian, &mut materials);
        spawn_part(
            &mut commands,
            Part {
                mesh: models.pyramid.clone(),
                material: mat,
                transform: Transform::IDENTITY,
            },
            board_position(config, guardian.cell) + Vec3::Y * 0.6,
            "Guardian pyramid",
        );
    }
}
fn spawn_part(commands: &mut Commands, mut part: Part, at: Vec3, name: &str) {
    part.transform.translation += at;
    commands.spawn((
        BoardVisual,
        Mesh3d(part.mesh),
        MeshMaterial3d(part.material),
        part.transform,
        RenderLayers::layer(0),
        Name::new(name.to_owned()),
    ));
}
fn ring(
    commands: &mut Commands,
    models: &mut Models,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    role: Role,
    width: f32,
    name: &str,
) {
    let material = models.signal(role, materials);
    for face in HexFace::LATERAL {
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.65, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.65, b.1 as f32);
        let delta = b - a;
        spawn_part(
            commands,
            Part {
                mesh: models.cube.clone(),
                material: material.clone(),
                transform: Transform::from_translation((a + b) * 0.5)
                    .with_rotation(Quat::from_rotation_y(-delta.z.atan2(delta.x)))
                    .with_scale(Vec3::new(delta.length() - 0.25, 0.08, width)),
            },
            at,
            name,
        );
    }
}
fn door_parts(models: &Models, material: Handle<StandardMaterial>, rotation: u8) -> Vec<Part> {
    let turn = Quat::from_rotation_y(-f32::from(rotation) * std::f32::consts::TAU / 6.0);
    [
        (Vec3::new(0.0, 1.7, -2.0), Vec3::new(0.55, 3.4, 0.55)),
        (Vec3::new(0.0, 1.7, 2.0), Vec3::new(0.55, 3.4, 0.55)),
        (Vec3::new(0.0, 3.4, 0.0), Vec3::new(0.55, 0.45, 4.5)),
    ]
    .into_iter()
    .map(|(at, size)| Part {
        mesh: models.cube.clone(),
        material: material.clone(),
        transform: Transform::from_translation(turn * at)
            .with_rotation(turn)
            .with_scale(size),
    })
    .collect()
}
