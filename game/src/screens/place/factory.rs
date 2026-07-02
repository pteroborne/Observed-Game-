//! The place rebuild entry point: `rebuild_place` tears down and respawns a place's
//! entire presentation (shell, thresholds, monitors, items, lighting, guardian) whenever
//! the teleport signature changes, plus the room-tinted floor material/light colour
//! helpers it uses to give each room a distinct hue.

use bevy::prelude::*;
use observed_core::RoomId;
use observed_style as style;

use crate::GameState;
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::screens::match_runtime;
use crate::sim::director::MatchDirector;
use crate::sim::state::TeleportState;
use crate::teleport::{self, GapKind, Place};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;

use super::monitors::{
    GuardianConsole, spawn_guardian_observation_monitors, spawn_tether_camera_monitors,
};
use super::{item_visuals, lighting, preview, shell};

pub(crate) fn room_color(room_id: RoomId) -> Color {
    let r = (((room_id.0 * 17 + 5) % 255) as f32) / 255.0;
    let g = (((room_id.0 * 31 + 13) % 255) as f32) / 255.0;
    let b = (((room_id.0 * 59 + 29) % 255) as f32) / 255.0;
    Color::srgb(0.3 + r * 0.7, 0.3 + g * 0.7, 0.3 + b * 0.7)
}

pub(crate) fn room_floor_material(
    room_id: RoomId,
    base_handle: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let mut mat = (*materials.get(base_handle).unwrap()).clone();
    let col = room_color(room_id);
    if mat.base_color_texture.is_some() {
        mat.base_color = Color::WHITE;
        mat.emissive = LinearRgba::from(col) * 0.45;
    } else {
        mat.base_color = col;
        mat.emissive = LinearRgba::from(col) * 3.0;
    }
    materials.add(mat)
}

pub(crate) fn room_light_color(room_id: RoomId) -> Color {
    let col = room_color(room_id);
    let r = 0.3 + (col.to_linear().red) * 2.0;
    let g = 0.3 + (col.to_linear().green) * 2.0;
    let b = 0.3 + (col.to_linear().blue) * 2.0;
    Color::srgb(r.min(1.0), g.min(1.0), b.min(1.0))
}

#[derive(Resource, Default)]
pub(crate) struct LastRenderedSignature(pub(crate) Option<(Place, u64, usize)>);

/// Rebuild the place presentation geometry (floors, walls, ceiling, previews, lights, items).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_place(
    assets: Res<MatchAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Res<crate::guardian::Guardian>,
    runtime: Res<MatchDirector>,
    existing: Query<Entity, With<PlaceGeometry>>,
    last_sig: Option<ResMut<LastRenderedSignature>>,
    mut commands: Commands,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let tp = tp.into_inner();
    let game = runtime.live.host_match();
    let seed_val = seed.map(|s| s.0).unwrap_or(crate::flow::MATCH_SEED);
    let nav = match_runtime::nav_for_place(seed_val, game, &keys, &items, tp.place);

    // Signature to detect if the place needs rebuilding (e.g. dropped items or tethers changed)
    let signature = {
        let mut tethers_hash = 0u64;
        for &conn in &nav.connections {
            if nav.is_tethered(game.local_room(), conn) {
                tethers_hash += 1;
            }
        }
        let item_count = items.placed_in(tp.place).len();
        (tp.place, tethers_hash, item_count)
    };

    let mut rebuild = true;
    if let Some(mut sig) = last_sig {
        if tp.rendered == Some(tp.place) && sig.0 == Some(signature) {
            rebuild = false;
        } else {
            sig.0 = Some(signature);
        }
    } else {
        commands.insert_resource(LastRenderedSignature(Some(signature)));
    }

    if tp.rendered == Some(tp.place) && !rebuild {
        return;
    }
    tp.rendered = Some(tp.place);

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let geom = tp.geom.clone();
    let y_offset = teleport::place_y_offset(tp.place);

    // The place's structural shell (floor/ceiling/walls), by place shape.
    match tp.place {
        Place::Room(room) => {
            let floor_material = room_floor_material(room, &assets.floor_material, &mut materials);
            shell::spawn_room_shell(
                &mut commands,
                &assets,
                &mut meshes,
                &geom,
                floor_material,
                y_offset,
            );
        }
        Place::Hallway { .. } => shell::spawn_hallway_shell(
            &mut commands,
            &assets,
            &geom,
            assets.floor_material.clone(),
            &tp.arena.solids,
            y_offset,
        ),
    }

    // The threshold gateways cut into that shell.
    for gap in &geom.gaps {
        if gap.kind == teleport::GapKind::OneWayEntry {
            continue;
        }
        let (ea, eb) = match tp.place {
            Place::Room(room) => (room, gap.target),
            Place::Hallway { from, to, .. } => (from, to),
        };
        let tethered = nav.is_tethered(ea, eb);

        if gap.kind.is_passage() {
            let dest = tp
                .gap_dests
                .iter()
                .find(|d| d.threshold == gap.threshold)
                .or_else(|| {
                    tp.gap_dests
                        .iter()
                        .find(|d| (d.gap_center - gap.center).length() < 0.05)
                })
                .cloned()
                .unwrap_or_else(|| preview::fallback_dest(tp.place, gap, &nav, game));

            // A maze hallway can expose multiple apertures to the same room-side
            // threshold. Keep one full preview for the canonical aperture and use short
            // stubs for secondary apertures instead of drawing overlapping room copies.
            let multi_aperture_room_preview = matches!(tp.place, Place::Hallway { .. })
                && matches!(dest.place, Place::Room(_))
                && gap.threshold.hall.slot.0 != 0
                && geom
                    .gaps
                    .iter()
                    .filter(|other| {
                        other.kind.is_passage()
                            && other.target == gap.target
                            && other.normal.dot(gap.normal) > 0.99
                    })
                    .count()
                    > 1;
            if multi_aperture_room_preview {
                preview::spawn_passage_stub(&mut commands, &assets, gap, y_offset);
            } else {
                preview::spawn_passage_preview(
                    &mut commands,
                    &assets,
                    &mut meshes,
                    &mut materials,
                    gap,
                    tp.place,
                    &dest,
                    &nav,
                    game,
                );
            }

            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::passage(tethered),
                y_offset,
            );
        } else if gap.kind == GapKind::LockedExit {
            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::locked_exit(&assets, tethered),
                y_offset,
            );
        } else {
            shell::spawn_threshold_gateway(
                &mut commands,
                &assets,
                gap,
                shell::ThresholdStyle::side_door(&assets),
                y_offset,
            );
        }
    }

    // Spawn specialized room visual elements (monitors, interactive console)
    if let Place::Room(room) = tp.place {
        if room.0 == 5 {
            spawn_tether_camera_monitors(
                &mut commands,
                &assets,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
            );
        } else if room.0 == 6 {
            spawn_guardian_observation_monitors(
                &mut commands,
                &assets,
                &mut materials,
                &geom,
                y_offset,
                seed_val,
                room,
            );
        } else if room.0 == 3 {
            // Guardian Control Room: central interactive console
            let console_inactive = materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.2, 0.2),
                emissive: LinearRgba::new(0.2, 0.2, 0.2, 1.0),
                ..default()
            });
            commands.spawn((
                PlaceGeometry,
                DespawnOnExit(GameState::Match),
                GuardianConsole,
                Mesh3d(assets.placeholder_mesh.clone()),
                MeshMaterial3d(console_inactive),
                Transform::from_xyz(0.0, 1.0 + y_offset, 0.0).with_scale(Vec3::new(1.0, 2.0, 1.0)),
                Name::new("Guardian Control Console"),
            ));
        }
    }

    // Spawn keystone item
    if let Place::Room(room) = tp.place
        && keys.has_uncollected(room)
    {
        item_visuals::spawn_keystone_item(&mut commands, &assets, room, y_offset);
    }

    // Spawn dropped items
    for item in items.placed_in(tp.place) {
        item_visuals::spawn_dropped_item(&mut commands, &assets, item, y_offset);
    }

    // Lighting and surface details
    let district = match_runtime::district_for_place(seed_val, tp.place);
    let light_color = match tp.place {
        Place::Room(room) => room_light_color(room),
        Place::Hallway { .. } => style::district(district).light_color,
    };

    let place_transform = Transform::from_xyz(0.0, y_offset, 0.0);
    lighting::spawn_place_lighting(
        &mut commands,
        &assets,
        &geom,
        light_color,
        place_transform,
        false,
    );

    let accent = assets.district_accent_materials[district.index()].clone();
    lighting::spawn_surface_detail(
        &mut commands,
        &assets,
        &geom,
        accent,
        place_transform,
        false,
    );

    // Spawn guardian
    if let Place::Room(room) = tp.place
        && room == guardian.room
    {
        spawn_guardian_model(&mut commands, &assets, guardian.pos);
    }
}

pub(crate) fn spawn_guardian_model(commands: &mut Commands, assets: &MatchAssets, pos: Vec3) {
    commands
        .spawn((
            crate::guardian::GuardianModel,
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.rival_body_mesh.clone()),
            MeshMaterial3d(assets.trap_active_material.clone()),
            Transform::from_translation(pos),
            Name::new("Guardian"),
        ))
        .with_children(|g| {
            g.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.05, 0.05),
                    intensity: 3000.0,
                    range: 9.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
        });
}
