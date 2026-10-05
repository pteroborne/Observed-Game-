//! Recorded actor forms and event/equipment signals, using shared semantic art.
use super::{LAYER, ReplayFigure, ReplayScene};
use crate::{
    GameState,
    sim::replay::{
        ReplayTape,
        scene::{ReplayBody, ReplaySceneFrame, body_position},
    },
};
use bevy::{camera::visibility::RenderLayers, prelude::*};
use observed_match::hex_wfc::{HexBodyPlace, HexGuardianStatus};
pub(super) struct FigureView<'a> {
    pub tape: &'a ReplayTape,
    pub frame: &'a ReplaySceneFrame,
    pub following: Option<&'a ReplaySceneFrame>,
    pub fraction: f32,
    pub clock: f32,
    pub focus: &'a ReplayBody,
    pub level: u8,
    pub prison: bool,
    pub eye_view: bool,
}
pub(super) fn draw(
    commands: &mut Commands,
    cache: &mut ReplayScene,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    view: FigureView,
) {
    let FigureView {
        tape,
        frame,
        following,
        fraction,
        clock,
        focus: body,
        level,
        prison,
        eye_view,
    } = view;
    // Figure mesh/material handles are cached; only transforms and visibility change.
    for actor in &frame.bodies {
        if actor.place == HexBodyPlace::Void
            || (actor.place == HexBodyPlace::Prison) != prison
            || (prison && actor.team != body.team)
        {
            continue;
        }
        if !eye_view && actor.cell.level != level {
            continue;
        }
        if eye_view && actor.player == body.player {
            continue;
        }
        let at = body_position(
            actor,
            following.and_then(|f| f.bodies.iter().find(|b| b.player == actor.player)),
            fraction,
        );
        let look = tape
            .cosmetics
            .get(&actor.player)
            .copied()
            .unwrap_or_default();
        let gaze = observed_observer::form::Gaze {
            yaw: actor.yaw,
            pitch: actor.pitch,
        };
        let poses = observed_observer::form::pose(gaze, clock, u32::from(actor.player.0));
        for (index, (part, mut transform)) in observed_observer::form::parts()
            .into_iter()
            .zip(poses)
            .enumerate()
        {
            let mesh = cache
                .figure_meshes
                .entry((0, index))
                .or_insert_with(|| meshes.add(observed_observer::mesh::mesh(part.shape)))
                .clone();
            use observed_observer::form::Look;
            use observed_style::observer::Part;
            let role = if actor.player == body.player {
                observed_style::MarkerRole::You
            } else if actor.team == body.team {
                observed_style::MarkerRole::Teammate
            } else {
                observed_style::MarkerRole::Rival
            };
            let finish = if part.look == Look::Trim {
                observed_style::cosmetics::trim(look.color)
            } else {
                observed_style::observer::finish(match part.look {
                    Look::Globe => Part::Globe,
                    Look::Trim => Part::Trim,
                    Look::Iris | Look::Haze => Part::Iris(role),
                    Look::Pupil => Part::Pupil,
                })
            };
            let material = figure_material(
                cache,
                materials,
                if part.look == Look::Trim {
                    100 + look.color as u8
                } else {
                    40 + role as u8 * 5 + part.look as u8
                },
                finish.base_color,
                finish.emissive,
            );
            // Overview scales the eye to remain readable at floor scale; its location is exact.
            if !eye_view {
                transform.translation *= 3.0;
                transform.scale *= 3.0;
            }
            transform.translation += at;
            spawn_figure(commands, mesh, material, transform);
        }
        draw_cosmetics(
            commands,
            cache,
            meshes,
            materials,
            (tape, frame, actor),
            (at, look, eye_view),
        );
    }
    if !prison {
        for guardian in &frame.guardians {
            if !eye_view && guardian.cell.level != level {
                continue;
            }
            let next = following.and_then(|f| f.guardians.iter().find(|g| g.id == guardian.id));
            let at = next
                .filter(|n| n.position.distance_squared(guardian.position) < 225.0)
                .map_or(guardian.position, |n| {
                    guardian.position.lerp(n.position, fraction)
                });
            let form = if guardian.minor {
                observed_guardian::form::Form::Roller
            } else {
                observed_guardian::form::Form::Tumbler { tiers: 4 }
            };
            let state = match guardian.status {
                HexGuardianStatus::Active => observed_guardian::form::State::Hunting,
                HexGuardianStatus::FrozenByPlayer => observed_guardian::form::State::FrozenBySight,
                HexGuardianStatus::FrozenByAnchor => observed_guardian::form::State::FrozenByAnchor,
            };
            let toward = guardian
                .target
                .and_then(|id| frame.bodies.iter().find(|b| b.player == id))
                .map_or(at + Vec3::Z, |b| b.position);
            let poses = observed_guardian::form::pose(
                form,
                state,
                clock,
                clock,
                observed_guardian::form::Stage {
                    at: Vec3::ZERO,
                    toward: toward - at,
                },
            );
            for (index, (part, pose)) in observed_guardian::form::parts(form)
                .into_iter()
                .zip(poses.parts)
                .enumerate()
            {
                let Some(mut transform) = pose else {
                    continue;
                };
                let mesh = cache
                    .figure_meshes
                    .entry((if guardian.minor { 2 } else { 1 }, index))
                    .or_insert_with(|| meshes.add(observed_guardian::mesh::mesh(part.shape)))
                    .clone();
                use observed_guardian::form::Look;
                use observed_style::guardian::Part;
                let finish = observed_style::guardian::finish(match part.look {
                    Look::Shell => Part::Shell,
                    Look::Trim | Look::Clamp => Part::Trim,
                    Look::Pupil => Part::Pupil,
                    _ => Part::Eye,
                });
                let emission = if part.look == Look::Seam {
                    observed_style::guardian::seam(state.frozen())
                } else {
                    finish.emissive
                };
                let material = figure_material(
                    cache,
                    materials,
                    120 + part.look as u8 + u8::from(state.frozen()) * 10,
                    finish.base_color,
                    emission,
                );
                transform.translation += at;
                spawn_figure(commands, mesh, material, transform);
            }
        }
    }
    let structure = if prison {
        frame.prisons.get(&body.team).unwrap_or(&frame.facility)
    } else {
        &frame.facility
    };
    if structure.exit.level == level {
        beacon(
            commands,
            cache,
            meshes,
            materials,
            Vec3::from_array(observed_hex::hex_origin(structure.exit)) + Vec3::Y * 3.0,
            observed_style::MarkerRole::Exit,
            Vec3::new(0.8, 6.0, 0.8),
        );
    }
    if !prison {
        for &cell in &frame.highlights {
            if cell.level == level {
                beacon(
                    commands,
                    cache,
                    meshes,
                    materials,
                    Vec3::from_array(observed_hex::hex_origin(cell)) + Vec3::Y * 0.4,
                    observed_style::MarkerRole::Collapse,
                    Vec3::new(9.0, 0.15, 9.0),
                );
            }
        }
        for &cell in &frame.stations {
            if cell.level == level {
                beacon(
                    commands,
                    cache,
                    meshes,
                    materials,
                    Vec3::from_array(observed_hex::hex_origin(cell)) + Vec3::Y * 1.0,
                    observed_style::MarkerRole::Control,
                    Vec3::new(1.4, 2.0, 1.4),
                );
            }
        }
        if let Some(event) = tape
            .markers
            .iter()
            .rev()
            .find(|m| m.sample <= frame.sample && m.cell.is_some())
            && frame.tick.saturating_sub(u64::from(event.live_round)) <= 120
            && let Some(cell) = event.cell.filter(|c| c.level == level)
        {
            beacon(
                commands,
                cache,
                meshes,
                materials,
                Vec3::from_array(observed_hex::hex_origin(cell)) + Vec3::Y * 3.0,
                observed_style::MarkerRole::NextRoom,
                Vec3::new(0.4, 6.0, 0.4),
            );
        }
        for &(key, state) in &frame.doors {
            if !eye_view && key.cell.level != level {
                continue;
            }
            let (at, along) = observed_match::hex_wfc::door_pose(key.cell, key.face);
            let height = if state == observed_match::ascent::sim::DoorState::Closed {
                observed_match::hex_wfc::DOOR_HEIGHT
            } else {
                0.18
            };
            let role = if state == observed_match::ascent::sim::DoorState::Closed {
                observed_style::MarkerRole::Collapse
            } else {
                observed_style::MarkerRole::Exit
            };
            let entity = beacon(
                commands,
                cache,
                meshes,
                materials,
                at + Vec3::Y * (observed_match::hex_wfc::DOOR_HEIGHT - height * 0.5),
                role,
                Vec3::new(observed_match::hex_wfc::DOOR_HALF_WIDTH * 2.0, height, 0.2),
            );
            commands.entity(entity).insert(
                Transform::from_translation(
                    at + Vec3::Y * (observed_match::hex_wfc::DOOR_HEIGHT - height * 0.5),
                )
                .with_rotation(Quat::from_rotation_arc(Vec3::X, along))
                .with_scale(Vec3::new(
                    observed_match::hex_wfc::DOOR_HALF_WIDTH * 2.0,
                    height,
                    0.2,
                )),
            );
        }
    }
}
fn figure_material(
    cache: &mut ReplayScene,
    materials: &mut Assets<StandardMaterial>,
    key: u8,
    base_color: Color,
    emissive: LinearRgba,
) -> Handle<StandardMaterial> {
    cache
        .materials
        .entry((key, true))
        .or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color,
                emissive,
                perceptual_roughness: 0.8,
                ..default()
            })
        })
        .clone()
}
fn spawn_figure(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
) {
    commands.spawn((
        ReplayFigure,
        DespawnOnExit(GameState::Replay),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        RenderLayers::layer(LAYER),
        transform,
    ));
}

fn beacon(
    commands: &mut Commands,
    cache: &mut ReplayScene,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    role: observed_style::MarkerRole,
    size: Vec3,
) -> Entity {
    let mesh = cache
        .figure_meshes
        .entry((3, 0))
        .or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0)))
        .clone();
    let look = observed_style::marker(role);
    let material = figure_material(
        cache,
        materials,
        160 + role as u8,
        look.base_color,
        look.emissive,
    );
    commands
        .spawn((
            ReplayFigure,
            DespawnOnExit(GameState::Replay),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(LAYER),
            Transform::from_translation(at).with_scale(size),
        ))
        .id()
}

fn draw_cosmetics(
    commands: &mut Commands,
    cache: &mut ReplayScene,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    recording: (&ReplayTape, &ReplaySceneFrame, &ReplayBody),
    placement: (Vec3, observed_core::cosmetics::CosmeticLook, bool),
) {
    use crate::hex_wfc::cosmetics::{badge_parts, remember, trail_count, trail_scale};
    let (tape, frame, actor) = recording;
    let (at, look, eye_view) = placement;
    let finish = observed_style::cosmetics::trim(look.color);
    let material = figure_material(
        cache,
        materials,
        140 + look.color as u8,
        finish.base_color,
        finish.emissive,
    );
    let cube = cache
        .figure_meshes
        .entry((4, 0))
        .or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0)))
        .clone();
    for mut transform in badge_parts(look) {
        if !eye_view {
            transform.translation *= 3.0;
            transform.scale *= 3.0;
        }
        transform.translation += at;
        spawn_figure(commands, cube.clone(), material.clone(), transform);
    }
    if trail_count(look) == 0 {
        return;
    }
    let sphere = cache
        .figure_meshes
        .entry((4, 1))
        .or_insert_with(|| meshes.add(Sphere::new(1.0)))
        .clone();
    let recent: Vec<_> = tape
        .scene_frames
        .iter()
        .rev()
        .filter(|f| f.tick <= frame.tick)
        .take(8)
        .collect();
    let mut history = std::collections::VecDeque::new();
    for sample in recent.iter().rev() {
        if let Some(body) = sample
            .bodies
            .iter()
            .find(|b| b.player == actor.player && b.place == actor.place)
        {
            remember(&mut history, body.position);
        } else {
            history.clear();
        }
    }
    remember(&mut history, at);
    for (index, point) in history.iter().skip(1).take(trail_count(look)).enumerate() {
        let mut transform = Transform::from_translation(
            *point + Vec3::Y * (observed_observer::form::EYE_RISE - 0.35),
        );
        transform.scale = trail_scale(look, index) * if eye_view { 1.0 } else { 3.0 };
        spawn_figure(commands, sphere.clone(), material.clone(), transform);
    }
}
